//! Typed local login states. See spec/lobbyd/04-login.md and 05-rust-milestone.md.
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
mod world;
use world::WorldToken;

pub type LinkId = u64;
#[derive(Clone, Debug)]
pub struct Output {
    pub link: LinkId,
    pub packets: Vec<Vec<u8>>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Init,
    InitialName,
    Lobby,
    Retained,
    RetainedInit,
    WorldQueued,
    WorldReady,
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
    /// Local opt-in policy until the original anonymous lifetime is checked.
    pub init_timeout: Option<Duration>,
    /// Local safety policy for retained tokens; original config +264 is optional.
    pub check_retained_ipv4: bool,
    pub max_sessions: usize,
    pub max_links: usize,
    /// Local fixture worlds sent in HelloOk. Empty by default (milestone R1).
    pub worlds: Vec<crate::protocol::WorldInfo>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            np_timeout: Duration::from_secs(5),
            character_policy: None,
            world_timeout: Duration::from_secs(5),
            world_token_lifetime: None,
            init_timeout: None,
            check_retained_ipv4: true,
            max_sessions: 1024,
            max_links: 64,
            worlds: Vec::new(),
        }
    }
}
pub struct Jobs<S, N> {
    pub store: S,
    pub np: N,
    gamed: Arc<dyn crate::gamed::GameDaemon>,
    settings: Settings,
    links: HashSet<LinkId>,
    sessions: HashMap<u64, Session>,
    next_key: Arc<AtomicU64>,
    capacity: Arc<tokio::sync::Semaphore>,
    owners: Arc<std::sync::Mutex<HashMap<u64, LinkId>>>,
    character_states: HashMap<u64, crate::character::InternalState>,
    deletion_deadlines: HashMap<u64, Instant>,
}
impl<S: Store, N: Np> Jobs<S, N> {
    pub fn new(store: S, np: N, settings: Settings) -> io::Result<Self> {
        if settings.np_timeout.is_zero()
            || settings.world_timeout.is_zero()
            || settings.world_token_lifetime.is_some_and(|v| v.is_zero())
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
            settings,
            links: HashSet::new(),
            sessions: HashMap::new(),
            next_key: Arc::new(AtomicU64::new(1)),
            owners: Arc::default(),
            character_states: HashMap::new(),
            deletion_deadlines: HashMap::new(),
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
        self.np.expire();
        let now = Instant::now();
        let keys: Vec<_> = self
            .sessions
            .iter()
            .filter(|(_, s)| s.expires.is_some_and(|deadline| deadline <= now))
            .map(|(key, _)| *key)
            .collect();
        let mut outputs = vec![];
        for key in keys {
            if matches!(
                self.state(key),
                Some(State::Retained | State::WorldReady | State::WorldQueued)
            ) {
                outputs.extend(self.kick(key, 8).await?);
            } else if self.state(key) == Some(State::RetainedInit) {
                if let Some(session) = self.sessions.remove(&key) {
                    outputs.push(Output {
                        link: session.link,
                        packets: vec![wire::notice(20, session.gate)?],
                    });
                    self.finish(key, session).await;
                }
            } else {
                outputs.extend(self.close_anonymous(key).await?);
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
    async fn finish(&mut self, key: u64, session: Session) {
        self.owners.lock().unwrap().remove(&key);
        if let Some(account) = session.account {
            self.np.logout_presence(account.id, key);
            let _ = self
                .store
                .update_last_logout(account.id, account.bot_score)
                .await;
        }
    }
    pub async fn disconnect(&mut self, link: LinkId) {
        self.links.remove(&link);
        let keys: Vec<_> = self
            .sessions
            .iter()
            .filter(|(_, s)| s.link == link)
            .map(|(k, _)| *k)
            .collect();
        for key in keys {
            if let Some(s) = self.sessions.remove(&key) {
                self.finish(key, s).await;
            }
        }
    }
    /// Close a pending anonymous session and notify its gate with D20.
    /// The caller supplies the trigger; no original expiry duration is assumed.
    pub async fn close_anonymous(&mut self, key: u64) -> io::Result<Vec<Output>> {
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
        self.finish(key, session).await;
        Ok(vec![output])
    }
    /// Local control operation. No management listener is exposed.
    pub async fn kick(&mut self, key: u64, reason: i8) -> io::Result<Vec<Output>> {
        let Some(session) = self.sessions.remove(&key) else {
            return Ok(vec![]);
        };
        let output = Output {
            link: session.link,
            packets: vec![
                if matches!(session.state, State::Init | State::RetainedInit) {
                    wire::notice(20, session.gate)?
                } else {
                    wire::kick(
                        key,
                        match reason {
                            1..=8 => reason,
                            10..=12 => reason - 1,
                            _ => 0,
                        },
                    )?
                },
            ],
        };
        self.finish(key, session).await;
        Ok(vec![output])
    }
    pub async fn handle(&mut self, link: LinkId, packet: &[u8]) -> io::Result<Vec<Output>> {
        let cmd = wire::command(packet)?;
        if cmd == 1 {
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
        let key = packet
            .get(4..12)
            .map(|v| u64::from_le_bytes(v.try_into().unwrap()))
            .unwrap_or(0);
        self.refresh_deletion_counts(key);
        let packets = match cmd {
            6 => self.init(link, packet, key)?,
            9 => self.start(link, packet, key).await?,
            12 => self.init_retained(link, packet, key).await?,
            15 => self.start_retained(link, packet, key)?,
            24 => self.enter_world(link, packet, key).await?,
            35 => self.characters(link, packet, key)?,
            38 => self.character_info(link, packet, key)?,
            // Build 307's D41 handler has no decode, session lookup or reply.
            41 => vec![],
            44 | 47 | 51 | 54 => self.character_mutation(link, packet, key, cmd).await?,
            103 => self.initial_user_name(link, packet, key).await?,
            64 | 67 | 70 | 73 | 76 => self.goods_unavailable(link, packet, key, cmd)?,
            // D79 GetServiceEntryListReq `hq` → D80 `hiqCqQqohq;` (02-links).
            // ❓ The 307 job is not ported. Local policy: an empty list, so the
            // client gets C72 and leaves its wait state.
            79 => {
                if wire::fields("hq", packet).is_err() {
                    vec![]
                } else {
                    wire::service_entries(key)?
                }
            }
            18 | 19 | 23 => {
                wire::fields("hq", packet)?;
                let client = if cmd == 18 {
                    self.sessions
                        .iter()
                        .find(|(_, s)| s.link == link && s.gate == key)
                        .map(|(k, _)| *k)
                } else {
                    Some(key)
                };
                let mut reply = vec![];
                if let Some(client) = client {
                    if self.sessions.get(&client).is_some_and(|s| s.link == link) {
                        if matches!(
                            self.state(client),
                            Some(State::Retained | State::WorldReady)
                        ) {
                            if cmd == 23 {
                                reply.push(wire::notice(21, client)?);
                            }
                            return Ok(if reply.is_empty() {
                                vec![]
                            } else {
                                vec![Output {
                                    link,
                                    packets: reply,
                                }]
                            });
                        }
                        let session = self.sessions.remove(&client).unwrap();
                        if cmd == 23 && session.state == State::Lobby {
                            reply.push(wire::notice(21, client)?);
                        }
                        self.finish(client, session).await;
                    }
                }
                reply
            }
            _ => {
                eprintln!("lobbyd: unsupported LobbyGate command {cmd}");
                return Err(wire::invalid("unsupported LobbyGate command"));
            }
        };
        Ok(if packets.is_empty() {
            vec![]
        } else {
            vec![Output { link, packets }]
        })
    }
    /// Local world-return control. The real GameDaemon return link is deferred.
    /// The caller must supply the lifetime; no binary default is assumed.
    pub fn retain(&mut self, key: u64, lifetime: Duration) -> io::Result<u64> {
        if lifetime.is_zero() {
            return Err(wire::invalid("token lifetime must be positive"));
        }
        let deadline = Instant::now()
            .checked_add(lifetime)
            .ok_or_else(|| wire::invalid("token lifetime overflow"))?;
        let session = self
            .sessions
            .get_mut(&key)
            .filter(|s| s.state == State::Lobby)
            .ok_or_else(|| wire::invalid("lobby session required"))?;
        let token = self
            .next_key
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |v| v.checked_add(1))
            .map_err(|_| wire::invalid("token key exhausted"))?;
        session.state = State::Retained;
        session.lobby_token = Some(token);
        session.expires = Some(deadline);
        Ok(token)
    }
    async fn init_retained(
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
            return Ok(vec![wire::failure(14, gate, 19)?]);
        };
        if session.expires.is_some_and(|v| v <= Instant::now()) {
            let session = self.sessions.remove(&owner).unwrap();
            self.finish(owner, session).await;
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
            self.finish(key, session).await;
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
                self.finish(key, session).await;
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
        let (format, fail) = match cmd {
            64 => ("hq", 66),
            67 => ("hqc", 69),
            70 => ("hqq", 72),
            73 => ("hqQq", 75),
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
            73 => wire::packet(
                "hqQE",
                &[
                    Value::I16(fail),
                    Value::U64(key),
                    values[2].clone(),
                    Value::I16(reason),
                ],
            )?,
            76 => wire::packet(
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
