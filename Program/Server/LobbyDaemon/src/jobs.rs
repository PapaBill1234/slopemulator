//! Typed local login states. See spec/lobbyd/04-login.md and 05-rust-milestone.md.
#![expect(
    non_upper_case_globals,
    reason = "match arms use the official command names from `crate::ids` (CamelCase constants)"
)]
use crate::{
    np::{Np, NpError},
    protocol as wire,
    store::{Account, Character, IncompleteService, Store},
};
use bloodlib::packet::Value;
use std::{
    collections::{HashMap, HashSet},
    io,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::time::Instant;

mod characters;
mod services;
mod world;
use world::{Playing, WorldToken};

pub type LinkId = u64;
/// Kick reason that goes to the client or to the GameDaemon.
/// ❓ `GetGameWorldKickReason` (`18-jobs-f.md` §5) is not verified: the gate mapping is used.
pub(crate) fn kick_reason(reason: i8) -> i8 {
    match reason {
        1..=8 => reason,
        10..=12 => reason - 1,
        _ => 0,
    }
}
#[derive(Clone, Debug)]
pub struct Output {
    pub link: LinkId,
    pub packets: Vec<Vec<u8>>,
}
/// Local session state. Each variant names its spec state (`13-jobs-a.md` §2, value is
/// `major << 16 | minor`). The spec states 1.2, 1.3, 3.1, 5.3 and 6.0 have no local value:
/// no local path stays in them (6.0 = the session is removed).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// 1.1 anonymous, NP token init done (`0x10001`).
    Init,
    /// Local sub-state of 2.0 (`0x20000`): lobby, initial user name still open.
    InitialName,
    /// 2.0 lobby, character select (`0x20000`).
    Lobby,
    /// 5.1 world-to-lobby, leave token issued (`0x50001`).
    Retained,
    /// 5.2 world-to-lobby, anonymous presence set (`0x50002`).
    RetainedInit,
    /// 3.2 lobby-to-world wait, in the queue (`0x30002`).
    WorldQueued,
    /// 3.3 lobby-to-world ok, enter token issued (`0x30003`).
    WorldReady,
    /// 4.0 in world (`0x40000`).
    World,
}
#[derive(Clone)]
struct Session {
    link: LinkId,
    gate: u64,
    token: [u8; 16],
    portal: [u8; 16],
    ipv4: u32,
    state: State,
    account: Option<Account>,
    characters: Vec<Character>,
    services: Vec<IncompleteService>,
    lobby_token: Option<u64>,
    world_token: Option<WorldToken>,
    /// The character in the game world (state 4.0): world and PC id. Set by `consume_world`.
    playing: Option<Playing>,
    admission: Option<Arc<crate::gamed::Admission>>,
    restored: HashSet<u64>,
    expires: Option<Instant>,
    _permit: Arc<tokio::sync::OwnedSemaphorePermit>,
}
#[derive(Clone, Debug)]
pub struct Settings {
    /// Local policy. The original duration is not verified.
    pub np_timeout: Duration,
    pub character_policy: Option<crate::character::CharacterPolicy>,
    /// Local deadlines. World entry needs an explicit lifetime.
    pub world_timeout: Duration,
    pub world_token_lifetime: Option<Duration>,
    /// Life time of a lobby token (`lobby-token-life-time`, msec in the original).
    /// ❓ The original default is not verified (`23-return-to-lobby.md` §3). Local value 120 s.
    /// `None`: the leave job cannot issue a token (reason 2).
    pub lobby_token_lifetime: Option<Duration>,
    /// Local opt-in policy until the original anonymous lifetime is checked.
    pub init_timeout: Option<Duration>,
    /// Local safety policy for retained tokens; original config +264 is optional.
    pub check_retained_ipv4: bool,
    pub max_sessions: usize,
    pub max_links: usize,
    /// Local fixture worlds sent in HelloOk. Empty by default (milestone R1).
    pub worlds: Vec<crate::protocol::WorldInfo>,
    /// Original config byte +400 (charge info). Default off. The on-path is not ported:
    /// it needs NP charge calls. With `charge` on, command 60 gets Fail code 2.
    pub charge: bool,
    /// Original config byte +413 (`useGrade`). Default off. The on-path is not ported:
    /// it needs NP grade calls. With `grade` on, command 106 gets Fail code 2.
    /// Account inventory is always off (no account-inventory link): commands 82 to 100
    /// always fail with code 7.
    pub grade: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            np_timeout: Duration::from_secs(5),
            character_policy: None,
            world_timeout: Duration::from_secs(5),
            world_token_lifetime: None,
            lobby_token_lifetime: Some(Duration::from_secs(120)),
            init_timeout: None,
            check_retained_ipv4: true,
            max_sessions: 1024,
            max_links: 64,
            worlds: Vec::new(),
            charge: false,
            grade: false,
        }
    }
}
pub struct Jobs<S, N> {
    pub store: S,
    pub np: N,
    gamed: Arc<dyn crate::gamed::GameDaemon>,
    settings: Arc<Settings>,
    links: HashSet<LinkId>,
    sessions: HashMap<u64, Session>,
    next_key: Arc<AtomicU64>,
    capacity: Arc<tokio::sync::Semaphore>,
    owners: Arc<std::sync::Mutex<HashMap<u64, LinkId>>>,
    character_states: HashMap<u64, crate::character::InternalState>,
    deletion_deadlines: HashMap<u64, Instant>,
    /// Last-logout writes that wait for [`Jobs::flush`] or [`Jobs::take_logouts`].
    logouts: Vec<(crate::store::AccountId, i32)>,
}
impl<S: Store, N: Np + Clone> Jobs<S, N> {
    /// The session that a packet is for. Command 18 names the gate id and command 12 the
    /// lobby token; all other commands name the client key.
    pub fn target(&self, link: LinkId, packet: &[u8]) -> u64 {
        let key = wire::key(packet);
        let token = packet
            .get(20..28)
            .and_then(|v| v.first_chunk::<8>())
            .map(|v| u64::from_le_bytes(*v));
        let found = match packet.get(2..4) {
            Some([18, 0]) => self
                .sessions
                .iter()
                .find(|(_, s)| s.link == link && s.gate == key),
            Some([12, 0]) => self
                .sessions
                .iter()
                .find(|(_, s)| s.link == link && token.is_some() && s.lobby_token == token),
            _ => None,
        };
        found.map_or(key, |(client, _)| *client)
    }
    /// A private copy of one session for a job task. The job runs [`Jobs::handle`] on the
    /// copy with no access to other sessions. [`Jobs::merge`] takes the result back.
    /// The caller must not change the session of `key` before the merge.
    pub fn checkout(&self, link: LinkId, key: u64) -> Self {
        let session = self.sessions.get(&key).cloned();
        let pcids = || {
            session
                .iter()
                .flat_map(|s| s.characters.iter().map(|c| c.pcid))
        };
        Self {
            store: self.store.clone(),
            np: self.np.clone(),
            gamed: self.gamed.clone(),
            settings: self.settings.clone(),
            links: self.links.get(&link).copied().into_iter().collect(),
            next_key: self.next_key.clone(),
            capacity: self.capacity.clone(),
            owners: self.owners.clone(),
            character_states: pcids()
                .filter_map(|id| Some((id, *self.character_states.get(&id)?)))
                .collect(),
            deletion_deadlines: pcids()
                .filter_map(|id| Some((id, *self.deletion_deadlines.get(&id)?)))
                .collect(),
            sessions: session.map(|s| (key, s)).into_iter().collect(),
            logouts: Vec::new(),
        }
    }
    /// Take back the result of a job that ran on [`Jobs::checkout`].
    pub fn merge(&mut self, key: u64, mut job: Self) {
        if let Some(old) = self.sessions.get(&key) {
            for character in &old.characters {
                self.deletion_deadlines.remove(&character.pcid);
            }
        }
        self.deletion_deadlines.extend(job.deletion_deadlines);
        self.character_states.extend(job.character_states);
        self.logouts.append(&mut job.logouts);
        match job.sessions.remove(&key) {
            Some(session) => self.sessions.insert(key, session),
            // The job ended the session (its `finish` ran in the job).
            None => self.sessions.remove(&key),
        };
    }
}
impl<S: Store, N: Np> Jobs<S, N> {
    pub fn new(store: S, np: N, settings: Settings) -> io::Result<Self> {
        if settings.np_timeout.is_zero()
            || settings.world_timeout.is_zero()
            || settings.world_token_lifetime.is_some_and(|v| v.is_zero())
            || settings.lobby_token_lifetime.is_some_and(|v| v.is_zero())
            || settings.init_timeout.is_some_and(|v| v.is_zero())
            || settings.max_sessions == 0
            || settings.max_links == 0
            || settings
                .character_policy
                .as_ref()
                .is_some_and(|p| !p.valid())
        {
            return Err(wire::invalid("limits must be positive"));
        }
        Ok(Self {
            store,
            np,
            gamed: Arc::new(crate::gamed::StubGameDaemon::default()),
            capacity: Arc::new(tokio::sync::Semaphore::new(settings.max_sessions)),
            settings: Arc::new(settings),
            links: HashSet::new(),
            sessions: HashMap::new(),
            next_key: Arc::new(AtomicU64::new(1)),
            owners: Arc::default(),
            character_states: HashMap::new(),
            deletion_deadlines: HashMap::new(),
            logouts: Vec::new(),
        })
    }
    pub(crate) fn set_resources(
        &mut self,
        source: Arc<AtomicU64>,
        capacity: Arc<tokio::sync::Semaphore>,
        owners: Arc<std::sync::Mutex<HashMap<u64, LinkId>>>,
    ) {
        self.next_key = source;
        self.capacity = capacity;
        self.owners = owners;
    }
    pub fn next_expiry(&self) -> Option<Instant> {
        self.sessions.values().filter_map(|s| s.expires).min()
    }
    pub async fn expire(&mut self) -> io::Result<Vec<Output>> {
        let outputs = self.expire_now(|_| false);
        self.flush().await;
        outputs
    }
    /// Expiry without a wait. `busy`: sessions that a job holds; their expiry runs later.
    pub fn expire_now(&mut self, busy: impl Fn(u64) -> bool) -> io::Result<Vec<Output>> {
        self.np.expire();
        let now = Instant::now();
        let keys: Vec<_> = self
            .sessions
            .iter()
            .filter(|(key, s)| s.expires.is_some_and(|deadline| deadline <= now) && !busy(**key))
            .map(|(key, _)| *key)
            .collect();
        let mut outputs = vec![];
        for key in keys {
            if matches!(
                self.state(key),
                Some(State::Retained | State::WorldReady | State::WorldQueued)
            ) {
                outputs.extend(self.kick_now(key, 8)?);
            } else if self.state(key) == Some(State::RetainedInit) {
                if let Some(session) = self.sessions.remove(&key) {
                    outputs.push(Output {
                        link: session.link,
                        packets: vec![wire::notice(20, session.gate)?],
                    });
                    self.finish(key, session);
                }
            } else {
                outputs.extend(self.close_anonymous_now(key)?);
            }
        }
        Ok(outputs)
    }
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }
    pub fn pending_services(&self, key: u64) -> Option<&[IncompleteService]> {
        self.sessions
            .get(&key)
            .map(|session| session.services.as_slice())
    }
    pub fn state(&self, key: u64) -> Option<State> {
        self.sessions.get(&key).map(|s| s.state)
    }
    /// End a session. No wait: the last-logout write goes to the logout list.
    fn finish(&mut self, key: u64, session: Session) {
        self.owners.lock().unwrap().remove(&key);
        if let Some(account) = session.account {
            self.np.logout_presence(account.id, key);
            self.logouts.push((account.id, account.bot_score));
        }
    }
    /// Write the last-logout rows of the sessions that ended.
    pub async fn flush(&mut self) {
        for (id, bot_score) in std::mem::take(&mut self.logouts) {
            let _ = self.store.update_last_logout(id, bot_score).await;
        }
    }
    /// The last-logout rows that wait. The caller writes them (`Store::update_last_logout`).
    pub fn take_logouts(&mut self) -> Vec<(crate::store::AccountId, i32)> {
        std::mem::take(&mut self.logouts)
    }
    pub async fn disconnect(&mut self, link: LinkId) {
        self.disconnect_now(link);
        self.flush().await;
    }
    pub fn disconnect_now(&mut self, link: LinkId) {
        self.links.remove(&link);
        let keys: Vec<_> = self
            .sessions
            .iter()
            .filter(|(_, s)| s.link == link)
            .map(|(k, _)| *k)
            .collect();
        for key in keys {
            if let Some(s) = self.sessions.remove(&key) {
                self.finish(key, s);
            }
        }
    }
    /// Close a pending anonymous session and notify its gate with D20.
    /// The caller supplies the trigger; no original expiry duration is assumed.
    pub async fn close_anonymous(&mut self, key: u64) -> io::Result<Vec<Output>> {
        let outputs = self.close_anonymous_now(key);
        self.flush().await;
        outputs
    }
    pub fn close_anonymous_now(&mut self, key: u64) -> io::Result<Vec<Output>> {
        let Some(session) = self.sessions.get(&key) else {
            return Ok(vec![]);
        };
        if session.state != State::Init {
            return Ok(vec![]);
        }
        let output = Output {
            link: session.link,
            packets: vec![wire::notice(20, session.gate)?],
        };
        let session = self.sessions.remove(&key).unwrap();
        self.finish(key, session);
        Ok(vec![output])
    }
    /// Local control operation. No management listener is exposed.
    pub async fn kick(&mut self, key: u64, reason: i8) -> io::Result<Vec<Output>> {
        let outputs = self.kick_now(key, reason);
        self.flush().await;
        outputs
    }
    pub fn kick_now(&mut self, key: u64, reason: i8) -> io::Result<Vec<Output>> {
        let Some(session) = self.sessions.remove(&key) else {
            return Ok(vec![]);
        };
        let output = Output {
            link: session.link,
            packets: vec![
                if matches!(session.state, State::Init | State::RetainedInit) {
                    wire::notice(20, session.gate)?
                } else {
                    wire::kick(key, kick_reason(reason))?
                },
            ],
        };
        self.finish(key, session);
        Ok(vec![output])
    }
    /// Commands whose job waits for NP, GameDaemon or the database.
    pub fn slow(cmd: i16) -> bool {
        use crate::ids::gate::*;
        matches!(
            cmd,
            StartSessionWithNPTokenReq
                | GetEnterWorldTokenReq
                | CreateCharacterReq
                | DeleteCharacterReq
                | CancelWaitCharacterDeletionReq
                | RecoverCharacterReq
                | CreateInitialUserNameReq
        )
    }
    /// Run one LobbyGate packet to its end. Tests and job tasks use this.
    /// The link task uses [`Jobs::handle_fast`] and a job task for each slow command.
    pub async fn handle(&mut self, link: LinkId, packet: &[u8]) -> io::Result<Vec<Output>> {
        let outputs = if Self::slow(wire::command(packet)?) {
            self.handle_slow(link, packet).await
        } else {
            self.handle_fast(link, packet)
        };
        self.flush().await;
        outputs
    }
    async fn handle_slow(&mut self, link: LinkId, packet: &[u8]) -> io::Result<Vec<Output>> {
        let cmd = wire::command(packet)?;
        if !self.links.contains(&link) {
            return Err(wire::invalid("Hello required"));
        }
        let key = wire::key(packet);
        self.refresh_deletion_counts(key);
        use crate::ids::gate::*;
        let packets = match cmd {
            StartSessionWithNPTokenReq => self.start(link, packet, key).await?,
            GetEnterWorldTokenReq => self.enter_world(link, packet, key).await?,
            CreateInitialUserNameReq => self.initial_user_name(link, packet, key).await?,
            _ => self.character_mutation(link, packet, key, cmd).await?,
        };
        Ok(if packets.is_empty() {
            vec![]
        } else {
            vec![Output { link, packets }]
        })
    }
    /// A command that does not wait. Last-logout writes go to the logout list.
    pub fn handle_fast(&mut self, link: LinkId, packet: &[u8]) -> io::Result<Vec<Output>> {
        use crate::ids::gate::*;
        let cmd = wire::command(packet)?;
        if cmd == HelloReq {
            let fields = wire::fields("hhhh", packet)?;
            let Value::I16(version) = fields[2] else {
                unreachable!()
            };
            if version != 58
                || self.links.contains(&link)
                || self.links.len() >= self.settings.max_links
            {
                return Ok(vec![Output {
                    link,
                    packets: vec![wire::packet("hh", &[Value::I16(3), Value::I16(2)])?],
                }]);
            }
            self.links.insert(link);
            return Ok(vec![Output {
                link,
                packets: wire::hello(&self.settings.worlds)?,
            }]);
        }
        if !self.links.contains(&link) {
            return Err(wire::invalid("Hello required"));
        }
        let key = wire::key(packet);
        self.refresh_deletion_counts(key);
        let packets = match cmd {
            InitSessionWithNPTokenReq => self.init(link, packet, key)?,
            InitSessionWithLobbyTokenReq => self.init_retained(link, packet, key)?,
            StartSessionWithLobbyTokenReq => self.start_retained(link, packet, key)?,
            GetCharacterInfoListReq => self.characters(link, packet, key)?,
            GetCharacterInfoReq => self.character_info(link, packet, key)?,
            // Build 307's D41 handler has no decode, session lookup or reply.
            CheckCharacterNameReq => vec![],
            GetGoodsCountReq
            | ListGoodsReq
            | TakeGoodsReq
            | ListLobbyServiceItemInfoReq
            | GetLobbyServiceItemInfoReq => self.goods_unavailable(link, packet, key, cmd)?,
            GetChargeInfoReq => self.charge_info(link, packet, key)?,
            ActivateGradeReq => self.activate_grade(link, packet, key)?,
            ChangeCharacterAppearanceWithAccountInventoryItemReq
            | ChangeCharacterSexWithAccountInventoryItemReq
            | ChangeCharacterRaceWithAccountInventoryItemReq
            | RenameCharacterWithAccountInventoryItemReq
            | AddSlotWithAccountInventoryItemReq
            | ChangeUserNameWithAccountInventoryItemReq
            | ExtendJumpingCharacterCreationQuotaWithAccountInventoryItemReq => {
                self.account_item_service(link, packet, key, cmd)?
            }
            // D79 GetServiceEntryListReq `hq` → D80 `hiqCqQqohq;` (02-links).
            // ❓ The 307 job is not ported. Local policy: an empty list, so the
            // client gets C72 and leaves its wait state.
            GetServiceEntryListReq => {
                if wire::fields("hq", packet).is_err() {
                    vec![]
                } else {
                    wire::service_entries(key)?
                }
            }
            LostAnonymousConnectionNotify | LostConnectionNotify | LogoutNotify => {
                if wire::fields("hq", packet).is_err() {
                    return Ok(vec![]);
                }
                // 18 names the session by gate key, 19 and 23 by client key.
                let client = if cmd == LostAnonymousConnectionNotify {
                    self.sessions
                        .iter()
                        .find(|(_, s)| s.link == link && s.gate == key)
                        .map(|(k, _)| *k)
                } else {
                    Some(key)
                };
                match client {
                    // ❓ The gate channel and `close` of each command are not read from the
                    // original. Local choice: `close` is true for all three; only 23 (the
                    // client logs out, the gate stays) has a gate channel for the notify.
                    Some(client) => self.logout(link, client, true, cmd == LogoutNotify)?,
                    None => vec![],
                }
            }
            // D28 GetWaitInfoReq `hq` → D29 `hqiiCch;` / D30 `hqE` (20-handlers §4.1).
            GetWaitInfoReq => {
                if wire::fields("hq", packet).is_err() {
                    return Ok(vec![]);
                }
                self.wait_info(link, key)?
            }
            // D31 GiveupWaitReq `hq` → D32 `hq` / D33 `hqE` (20-handlers §4.1).
            GiveupWaitReq => {
                if wire::fields("hq", packet).is_err() {
                    return Ok(vec![]);
                }
                self.giveup_wait(link, key)?
            }
            // An unknown command gets no reply. The link and its other sessions stay.
            _ => {
                eprintln!("lobbyd: unsupported LobbyGate command {cmd}, ignored");
                vec![]
            }
        };
        Ok(if packets.is_empty() {
            vec![]
        } else {
            vec![Output { link, packets }]
        })
    }
    /// LogoutJob (`13-jobs-a.md` §3.3). `close` is the job flag. `notify` says that the job
    /// has a gate channel; the notify (command 21) needs both. The reply is the notify, if any.
    /// A session of another link, or no session, ends the job with no change.
    fn logout(
        &mut self,
        link: LinkId,
        client: u64,
        close: bool,
        notify: bool,
    ) -> io::Result<Vec<Vec<u8>>> {
        let Some(last) = self
            .sessions
            .get(&client)
            .filter(|s| s.link == link)
            .map(|s| s.state)
        else {
            return Ok(vec![]);
        };
        let notice = || -> io::Result<Vec<Vec<u8>>> {
            if close && notify {
                Ok(vec![wire::notice(21, client)?])
            } else {
                Ok(vec![])
            }
        };
        let (remove, reply) = match last {
            // 1.1 and 5.2: only a `close` ends the session. No notify.
            State::Init | State::RetainedInit => (close, vec![]),
            // 2.0 (also the local initial-name sub-state): end the session, then notify.
            State::Lobby | State::InitialName => (true, notice()?),
            // 3.2: end the session and `leave` the queue (the drop of the session
            // releases the admission), then notify.
            State::WorldQueued => (true, notice()?),
            // 3.3: "absorb logout by L2W_ok state". No state change, notify as above.
            State::WorldReady => (false, notice()?),
            // 4.0 and 5.1: "invalid state" in the original. No change, no notify.
            // ❓ 5.1 is read from the pseudocode layout (13-jobs-a.md §3.3).
            State::World | State::Retained => {
                eprintln!("lobbyd: logout in invalid state {last:?} ignored");
                (false, vec![])
            }
        };
        if remove {
            if let Some(session) = self.sessions.remove(&client) {
                self.finish(client, session);
            }
        }
        Ok(reply)
    }
    /// Local world-return control. The real GameDaemon return link is deferred.
    /// The caller must supply the lifetime; no binary default is assumed.
    pub fn retain(&mut self, key: u64, lifetime: Duration) -> io::Result<u64> {
        if self.state(key) != Some(State::Lobby) {
            return Err(wire::invalid("lobby session required"));
        }
        self.issue_lobby_token(key, lifetime)
    }
    /// Issue a type-2 lobby token for the session and move it to 5.1 (`Retained`).
    /// The caller checks the state of the session. `23-return-to-lobby.md` §3 (`Issue`).
    fn issue_lobby_token(&mut self, key: u64, lifetime: Duration) -> io::Result<u64> {
        if lifetime.is_zero() {
            return Err(wire::invalid("token lifetime must be positive"));
        }
        let deadline = Instant::now()
            .checked_add(lifetime)
            .ok_or_else(|| wire::invalid("token lifetime overflow"))?;
        let session = self
            .sessions
            .get_mut(&key)
            .ok_or_else(|| wire::invalid("session required"))?;
        let token = self
            .next_key
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |v| v.checked_add(1))
            .map_err(|_| wire::invalid("token key exhausted"))?;
        session.state = State::Retained;
        session.lobby_token = Some(token);
        session.expires = Some(deadline);
        Ok(token)
    }
    fn init_retained(
        &mut self,
        link: LinkId,
        packet: &[u8],
        gate: u64,
    ) -> io::Result<Vec<Vec<u8>>> {
        let Ok(v) = wire::fields("hqqqi", packet) else {
            return Ok(vec![wire::failure(14, gate, 2)?]);
        };
        let (Value::U64(key), Value::U64(token), Value::U32(ipv4)) =
            (v[2].clone(), v[3].clone(), v[4].clone())
        else {
            unreachable!()
        };
        if gate == 0 || key == 0 || token == 0 || ipv4 == 0 {
            return Ok(vec![wire::failure(14, gate, 20)?]);
        }
        let Some((&owner, session)) = self
            .sessions
            .iter()
            .find(|(_, s)| s.link == link && s.lobby_token == Some(token))
        else {
            // No token: used before, or never issued. Reason 20 (`23-return-to-lobby.md` §3
            // InitSessionWithLobbyTokenReq). Only an expired token gives 19.
            return Ok(vec![wire::failure(14, gate, 20)?]);
        };
        if session.expires.is_some_and(|v| v <= Instant::now()) {
            let session = self.sessions.remove(&owner).unwrap();
            self.finish(owner, session);
            return Ok(vec![wire::failure(14, gate, 19)?]);
        }
        if owner != key || (self.settings.check_retained_ipv4 && session.ipv4 != ipv4) {
            return Ok(vec![wire::failure(14, gate, 20)?]);
        }
        // Verification consumes the token before job-state and binding checks.
        let session = self.sessions.get_mut(&key).unwrap();
        session.lobby_token = None;
        if session.state != State::Retained {
            return Ok(vec![wire::failure(14, gate, 5)?]);
        }
        if self
            .sessions
            .iter()
            .any(|(other, s)| *other != key && s.link == link && s.gate == gate)
        {
            let session = self.sessions.remove(&key).unwrap();
            self.finish(key, session);
            return Ok(vec![wire::failure(14, gate, 2)?]);
        }
        let session = self.sessions.get_mut(&key).unwrap();
        session.gate = gate;
        session.ipv4 = ipv4;
        session.state = State::RetainedInit;
        session.expires = self.settings.init_timeout.map(|v| Instant::now() + v);
        Ok(vec![wire::packet(
            "hqq",
            &[Value::I16(13), Value::U64(gate), Value::U64(key)],
        )?])
    }
    fn start_retained(
        &mut self,
        link: LinkId,
        packet: &[u8],
        key: u64,
    ) -> io::Result<Vec<Vec<u8>>> {
        if wire::fields("hq", packet).is_err() {
            return Ok(vec![wire::failure(17, key, 2)?]);
        }
        let Some(session) = self.sessions.get_mut(&key).filter(|s| s.link == link) else {
            return Ok(vec![wire::failure(17, key, 4)?]);
        };
        if session.state != State::RetainedInit {
            return Ok(vec![wire::failure(17, key, 5)?]);
        }
        let packets = wire::start_retained(key)?;
        session.state = State::Lobby;
        session.expires = None;
        Ok(packets)
    }
    fn init(&mut self, link: LinkId, packet: &[u8], key: u64) -> io::Result<Vec<Vec<u8>>> {
        let Ok(v) = wire::fields("hqooi", packet) else {
            return Ok(vec![wire::failure(8, key, 2)?]);
        };
        let (Value::Id(token), Value::Id(portal), Value::U32(ipv4)) = (&v[2], &v[3], &v[4]) else {
            unreachable!()
        };
        if key == 0 || *token == [0; 16] || *portal == [0; 16] {
            return Ok(vec![wire::failure(8, key, 2)?]);
        }
        if self.sessions.len() >= self.settings.max_sessions
            || self
                .sessions
                .values()
                .any(|s| s.link == link && s.gate == key)
        {
            return Ok(vec![wire::failure(8, key, 6)?]);
        }
        let Ok(permit) = self.capacity.clone().try_acquire_owned() else {
            return Ok(vec![wire::failure(8, key, 6)?]);
        };
        let client = self
            .next_key
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |key| {
                key.checked_add(1)
            })
            .map_err(|_| wire::invalid("session key exhausted"))?;
        self.sessions.insert(
            client,
            Session {
                link,
                gate: key,
                token: *token,
                portal: *portal,
                ipv4: *ipv4,
                state: State::Init,
                account: None,
                characters: vec![],
                services: vec![],
                lobby_token: None,
                world_token: None,
                playing: None,
                admission: None,
                restored: HashSet::new(),
                expires: self
                    .settings
                    .init_timeout
                    .map(|duration| Instant::now() + duration),
                _permit: Arc::new(permit),
            },
        );
        self.owners.lock().unwrap().insert(client, link);
        Ok(vec![wire::packet(
            "hqqoo",
            &[
                Value::I16(7),
                Value::U64(key),
                Value::U64(client),
                v[2].clone(),
                v[3].clone(),
            ],
        )?])
    }
    async fn start(&mut self, link: LinkId, packet: &[u8], key: u64) -> io::Result<Vec<Vec<u8>>> {
        let Ok(v) = wire::fields("hqoo", packet) else {
            return Ok(vec![wire::failure(11, key, 2)?]);
        };
        let Some(session) = self.sessions.get(&key) else {
            return Ok(vec![wire::failure(11, key, 4)?]);
        };
        if session.link != link {
            return Ok(vec![wire::failure(11, key, 4)?]);
        }
        if session.state != State::Init
            || v[2] != Value::Id(session.token)
            || v[3] != Value::Id(session.portal)
        {
            return Ok(vec![wire::failure(11, key, 5)?]);
        }
        // Keep the Init entry until the bounded NP operations complete.
        // The NP trait requires cancellation-safe operations.
        let mut session = session.clone();
        let result = self.authenticate(key, &mut session).await;
        match result {
            Ok(packets) => {
                self.sessions.insert(key, session);
                Ok(packets)
            }
            Err(reason) => {
                eprintln!("lobbyd: start failed, reason {reason}");
                self.sessions.remove(&key);
                self.finish(key, session);
                Ok(vec![wire::failure(11, key, reason)?])
            }
        }
    }
    async fn authenticate(&mut self, key: u64, session: &mut Session) -> Result<Vec<Vec<u8>>, i16> {
        let identity = tokio::time::timeout(
            self.settings.np_timeout,
            self.np.consume(session.token, session.portal),
        )
        .await
        .map_err(|_| 2i16)?
        .map_err(NpError::reason)?;
        tokio::time::timeout(
            self.settings.np_timeout,
            self.np.login_presence(identity.account, key),
        )
        .await
        .map_err(|_| 2i16)?
        .map_err(NpError::reason)?;
        // Store the identity before required DB work so all failure paths release presence.
        session.account = Some(Account {
            id: identity.account,
            name: identity.login_name,
            right: 1,
            representative_pcid: 0,
            representative_world: 0,
            bot_score: 0,
        });
        // The stored Init entry holds the presence too. If a shutdown cancels this job at a
        // later wait, `disconnect_now` on the merged copy releases the presence.
        if let Some(stored) = self.sessions.get_mut(&key) {
            stored.account = session.account.clone();
        }
        let account = match self
            .store
            .load_game_account(identity.account)
            .await
            .map_err(|_| 16i16)?
        {
            Some(account) => {
                let _ = self.store.update_last_login(account.id, session.ipv4).await;
                account
            }
            None => {
                let account = session.account.as_ref().unwrap().clone();
                self.store
                    .register_game_account(account.clone(), session.ipv4)
                    .await
                    .map_err(|_| 16i16)?;
                account
            }
        };
        session.account = Some(account.clone());
        let characters = self
            .store
            .load_character_list(account.id)
            .await
            .map_err(|_| 16i16)?;
        let services = self
            .store
            .load_incomplete_lobby_service(account.id)
            .await
            .map_err(|_| 16i16)?;
        if services.iter().any(|service| service.kind != 6) {
            return Err(2);
        }
        session.services = services;
        // Validate cached data before sending login success.
        wire::characters(key, &account, &characters).map_err(|_| 2i16)?;
        let packets = wire::start(key, &account).map_err(|_| 2i16)?;
        session.characters = characters;
        session.state = State::Lobby;
        session.expires = None;
        Ok(packets)
    }
    /// D38 reads only the local cache. Target state 0x10003 and extended lobby
    /// states are not represented by the current session model.
    fn character_info(&self, link: LinkId, packet: &[u8], key: u64) -> io::Result<Vec<Vec<u8>>> {
        let Ok(values) = wire::fields("hqo", packet) else {
            return Ok(vec![]); // The target drops malformed D38 requests.
        };
        let Value::Id(slot) = values[2] else {
            unreachable!()
        };
        let failure = |reason| Ok(vec![wire::character_info_failure(key, slot, reason)?]);
        let Some(session) = self.sessions.get(&key).filter(|s| s.link == link) else {
            return failure(4);
        };
        if session.state != State::Lobby {
            return failure(5);
        }
        let Some(character) = session
            .characters
            .iter()
            .find(|c| slot != [0; 16] && c.slot == slot)
        else {
            return failure(6);
        };
        match wire::character_info(key, character) {
            Ok(packet) => Ok(vec![packet]),
            Err(_) => failure(2),
        }
    }
    /// Goods requests with account inventory off (`use-account-inventory=no`).
    /// Build 307: each job sends its Fail packet with reason 7 ("init fail;
    /// account-inventory N/A"), 4 without a session, 5 outside the lobby.
    /// Malformed requests are dropped (decompose fail only logs).
    /// GetGoodsCount 0x1400C3550/0x14004FB70, ListGoods 0x1400C37E0/0x140053C40,
    /// TakeGoods 0x1400C3B30/0x140079130, ListLobbyServiceItemInfo
    /// 0x1400C3D40/0x1400548A0, GetLobbyServiceItemInfo 0x1400C3FC0/0x140050780.
    fn goods_unavailable(
        &self,
        link: LinkId,
        packet: &[u8],
        key: u64,
        cmd: i16,
    ) -> io::Result<Vec<Vec<u8>>> {
        use crate::ids::gate::*;
        // The reply IDs (66, 69, 72, 75, 78) are not in the 307 handler tables.
        let (format, fail) = match cmd {
            GetGoodsCountReq => ("hq", 66),
            ListGoodsReq => ("hqc", 69),
            TakeGoodsReq => ("hqq", 72),
            ListLobbyServiceItemInfoReq => ("hqQq", 75),
            _ => ("hqq", 78),
        };
        let Ok(values) = wire::fields(format, packet) else {
            return Ok(vec![]);
        };
        let reason = match self.sessions.get(&key).filter(|s| s.link == link) {
            None => 4,
            // ❓ 307 accepts client states 0x20000 and 0x10003; only Lobby is mapped.
            Some(s) if s.state != State::Lobby => 5,
            Some(_) => 7,
        };
        Ok(vec![match cmd {
            // D75 hqQE echoes the search type; D78 hqqE echoes the request key.
            ListLobbyServiceItemInfoReq => wire::packet(
                "hqQE",
                &[
                    Value::I16(fail),
                    Value::U64(key),
                    values[2].clone(),
                    Value::I16(reason),
                ],
            )?,
            GetLobbyServiceItemInfoReq => wire::packet(
                "hqqE",
                &[
                    Value::I16(fail),
                    Value::U64(key),
                    values[2].clone(),
                    Value::I16(reason),
                ],
            )?,
            _ => wire::failure(fail, key, reason)?,
        }])
    }
    fn characters(&self, link: LinkId, packet: &[u8], key: u64) -> io::Result<Vec<Vec<u8>>> {
        if wire::fields("hq", packet).is_err() {
            return Ok(vec![wire::failure(37, key, 2)?]);
        }
        let Some(s) = self.sessions.get(&key).filter(|s| s.link == link) else {
            return Ok(vec![wire::failure(37, key, 4)?]);
        };
        if s.state != State::Lobby {
            return Ok(vec![wire::failure(37, key, 5)?]);
        }
        match wire::characters(key, s.account.as_ref().unwrap(), &s.characters) {
            Ok(packets) => Ok(packets),
            Err(_) => Ok(vec![wire::failure(37, key, 2)?]),
        }
    }
}
