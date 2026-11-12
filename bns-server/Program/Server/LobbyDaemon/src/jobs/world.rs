//! D24 local job. Await external decisions before committing session state.
use super::*;
use crate::character::ProcedureError;
use crate::gamed::{ChargeMode, Eligibility};
use crate::store::{
    AccountExp, AccountId, ContentQuota, RecycleTime, SqlTimestamp, TimeBasedPlayInfo,
    RESULT_BLOB_CAPACITY,
};
use packet::gamed_link::{
    AccountExpInfo, AccountLists, Message, TimePlayInfo, UPDATE_ACCOUNT_CONTENT_QUOTA_FAIL_COMMAND,
    UPDATE_ACCOUNT_CONTENT_QUOTA_OK_COMMAND, UPDATE_ACCOUNT_DATA_FAIL_COMMAND,
    UPDATE_ACCOUNT_DATA_OK_COMMAND, UPDATE_ACCOUNT_EXP_FAIL_COMMAND, UPDATE_ACCOUNT_EXP_OK_COMMAND,
    UPDATE_ACCOUNT_RECYCLE_TIME_FAIL_COMMAND, UPDATE_ACCOUNT_RECYCLE_TIME_OK_COMMAND,
    UPDATE_CHARACTER_DATA_FAIL_COMMAND, UPDATE_CHARACTER_DATA_OK_COMMAND,
};

/// Most rows of one list in a gamed sync message. The limit is ours: the gamed link has no rate
/// limit, so a list must not make an unbounded number of database calls.
const MAX_SYNC_ROWS: usize = 512;

/// The account data of a session for the 381 `EnterWorldOk` lists. Loaded at login, changed by
/// the gamed updates 42, 107, 110 and 113.
#[derive(Clone, Debug, Default)]
pub(super) struct AccountCache {
    time_based: Vec<TimeBasedPlayInfo>,
    exp: Option<AccountExp>,
    quotas: Vec<ContentQuota>,
    recycle: Vec<RecycleTime>,
}
impl AccountCache {
    /// Each load is best effort: an error gives an empty list (the loads are not a condition
    /// of the login).
    pub(super) async fn load<S: Store>(store: &S, id: AccountId) -> Self {
        Self {
            time_based: store
                .load_time_based_play_info(id)
                .await
                .unwrap_or_default(),
            exp: store.load_account_exp(id).await.ok().flatten(),
            quotas: store.load_content_quota(id).await.unwrap_or_default(),
            recycle: store.load_recycle_time(id).await.unwrap_or_default(),
        }
    }
    fn lists(&self) -> AccountLists {
        AccountLists {
            time_based: self.time_based.iter().map(time_to_wire).collect(),
            exp: self
                .exp
                .as_ref()
                .map(|e| AccountExpInfo {
                    exp: e.exp,
                    quota_per_day: e.quota_per_day,
                    last_update: e.last_update,
                })
                .unwrap_or_default(),
            quotas: self.quotas.clone(),
            recycle: self.recycle.clone(),
        }
    }
    /// A quota row replaces the row with the same (data id, version).
    fn put_quota(&mut self, row: ContentQuota) {
        match self
            .quotas
            .iter_mut()
            .find(|q| q.data_id == row.data_id && q.version == row.version)
        {
            Some(old) => *old = row,
            None => self.quotas.push(row),
        }
    }
    /// A recycle row replaces the row of its group. The larger time stays.
    fn put_recycle(&mut self, row: RecycleTime) {
        match self.recycle.iter_mut().find(|r| r.group == row.group) {
            Some(old) => old.time = old.time.max(row.time),
            None => self.recycle.push(row),
        }
    }
}
fn time_to_wire(row: &TimeBasedPlayInfo) -> TimePlayInfo {
    TimePlayInfo {
        target: row.target_type as i8,
        schedule: row.schedule,
        expire: row
            .expire
            .as_ref()
            .and_then(SqlTimestamp::to_unix)
            .unwrap_or(0),
        last_update: 0,
        duration: i32::try_from(row.duration).unwrap_or(i32::MAX),
        acknowledged: 0,
        rewards: [0; 4],
    }
}
/// `None`: a time that is not zero and not a valid SQL date, or a negative target.
fn time_to_store(row: &TimePlayInfo) -> Option<TimeBasedPlayInfo> {
    let stamp = |value: i64| {
        if value == 0 {
            Some(None)
        } else {
            SqlTimestamp::from_unix(value).map(Some)
        }
    };
    Some(TimeBasedPlayInfo {
        schedule: row.schedule,
        target_type: u8::try_from(row.target).ok()?,
        expire: stamp(row.expire)?,
        duration: i64::from(row.duration),
        last_update: stamp(row.last_update)?,
    })
}
/// The failure code of a store error: the code of a `ProcedureError`, else 2.
/// ❓ `SqlStoreError::Procedure` is not mapped.
fn failure_code(error: &io::Error) -> i16 {
    error
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<ProcedureError>())
        .and_then(|p| i16::try_from(p.0).ok())
        .filter(|code| *code != 0)
        .unwrap_or(2)
}
fn reply(command: i16, transaction: u64, pcid: Option<u64>, reason: Option<i16>) -> Message {
    Message::Reply {
        command,
        transaction,
        pcid,
        reason,
    }
}
/// The Fail reply of a gamed sync request. `None`: the message has no reply (11) or is not a
/// sync request.
pub(crate) fn sync_failure(message: &Message, reason: i16) -> Option<Message> {
    let reason = Some(reason);
    Some(match message {
        Message::UpdateCharacterData { transaction, .. } => reply(
            UPDATE_CHARACTER_DATA_FAIL_COMMAND,
            *transaction,
            None,
            reason,
        ),
        Message::UpdateAccountData { transaction, .. } => {
            reply(UPDATE_ACCOUNT_DATA_FAIL_COMMAND, *transaction, None, reason)
        }
        Message::UpdateAccountExp {
            transaction, pcid, ..
        } => reply(
            UPDATE_ACCOUNT_EXP_FAIL_COMMAND,
            *transaction,
            Some(*pcid),
            reason,
        ),
        Message::UpdateAccountContentQuota {
            transaction, pcid, ..
        } => reply(
            UPDATE_ACCOUNT_CONTENT_QUOTA_FAIL_COMMAND,
            *transaction,
            Some(*pcid),
            reason,
        ),
        Message::UpdateAccountRecycleTime {
            transaction, pcid, ..
        } => reply(
            UPDATE_ACCOUNT_RECYCLE_TIME_FAIL_COMMAND,
            *transaction,
            Some(*pcid),
            reason,
        ),
        _ => return None,
    })
}

#[derive(Clone, Debug)]
pub(super) struct WorldToken {
    pub key: u64,
    pub world: i16,
    pub pcid: u64,
}
/// The character that plays in a game world (state 4.0).
#[derive(Clone, Copy, Debug)]
pub(super) struct Playing {
    pub world: i16,
    pub pcid: u64,
}
impl<S: Store, N: Np> Jobs<S, N> {
    /// LeaveGameWorldJob (`13-jobs-a.md` §3.6; `23-return-to-lobby.md` §3). `world` is the
    /// world of the GameDaemon link. `Ok` is the key of the new type-2 lobby token (state 5.1).
    /// `Err` is the reason for LeaveWorldFail: 5 no session for the PC id, 6 state is not 4.0,
    /// 21 no playing character, 23 PC id differs, 22 world differs, 2 token issue failed.
    pub fn leave_world(&mut self, key: u64, pcid: u64, world: i16) -> Result<u64, i16> {
        // Any zero input ends the job. A zero PC id or key has no session.
        let session = self
            .sessions
            .get_mut(&key)
            .filter(|s| pcid != 0 && s.characters.iter().any(|c| c.pcid == pcid))
            .ok_or(5i16)?;
        if session.state != State::World {
            return Err(6);
        }
        let playing = session.playing.ok_or(21i16)?;
        if playing.pcid != pcid {
            return Err(23);
        }
        if playing.world != world {
            return Err(22);
        }
        // `Character::EndGameWorldPlay`. `OnLeave` of the service location has no local state.
        session.playing = None;
        self.play.ends.push(pcid);
        let issued = self
            .settings
            .lobby_token_lifetime
            .ok_or_else(|| wire::invalid("no lobby token lifetime"))
            .and_then(|lifetime| self.issue_lobby_token(key, lifetime));
        issued.map_err(|_| {
            // 6.0: the session ends. The presence is closed in `finish`.
            if let Some(session) = self.sessions.remove(&key) {
                self.finish(key, session);
            }
            2
        })
    }
    /// LogoutInGameWorldJob (`13-jobs-a.md` §3.7). `world` is the world of the GameDaemon link.
    /// The session must be in state 4.0 with the same playing character and world, else there
    /// is no change. Then the session ends (state 6.0): the NP presence is released and the
    /// last-logout write goes to the logout list. `true`: the session ended.
    pub fn logout_in_world(&mut self, key: u64, pcid: u64, world: i16) -> bool {
        // Step 1: "world id (not 0)". A zero PC id never matches a playing character.
        let same = world != 0
            && pcid != 0
            && self.sessions.get(&key).is_some_and(|s| {
                s.state == State::World
                    && s.playing
                        .is_some_and(|p| p.pcid == pcid && p.world == world)
            });
        if !same {
            return false;
        }
        // Step 2: `EndGameWorldPlay` and `OnLeave` have no local state. `finalize` is `finish`.
        if let Some(session) = self.sessions.remove(&key) {
            self.finish(key, session);
        }
        true
    }
    /// Client keys and PC ids of the sessions in state 4.0 that play in `world`.
    pub fn world_players(&self, world: i16) -> Vec<(u64, u64)> {
        self.sessions
            .iter()
            .filter(|(_, s)| s.state == State::World)
            .filter_map(|(key, s)| {
                let playing = s.playing.filter(|p| p.world == world)?;
                Some((*key, playing.pcid))
            })
            .collect()
    }
    /// KickJob (`18-jobs-f.md` §5) for a session in state 4.0: world kick through the GameDaemon
    /// link, then `finalize` and the kick packet (D22) for the client. Result 1 = kicked (reply
    /// ok, or fail reason 24), 2 = world kick failed (other fail reason, or no reply in time).
    /// Both results end the session, so the result has no other effect here.
    /// No outputs: the session is not in state 4.0 (the caller uses the other kick paths).
    /// The wait runs in a job task (rule 3 of step 6).
    pub async fn kick_world(&mut self, key: u64, reason: i8) -> io::Result<Vec<Output>> {
        let Some(session) = self.sessions.get(&key) else {
            return Ok(vec![]);
        };
        if session.state != State::World {
            return Ok(vec![]);
        }
        // A 4.0 session without a playing character has no GameWorld presence: result 2.
        let result = match session.playing {
            Some(playing) => self
                .gamed
                .kick(playing.world, key, playing.pcid, kick_reason(reason))
                .await
                .map(|()| 1u8)
                .or_else(|reason| if reason == 24 { Ok(1) } else { Err(reason) }),
            None => Err(2),
        };
        if let Err(reason) = result {
            eprintln!("lobbyd: world kick of {key} failed, reason {reason}");
        }
        let Some(session) = self.sessions.remove(&key) else {
            return Ok(vec![]);
        };
        let output = Output {
            link: session.link,
            packets: vec![wire::kick(key, kick_reason(reason))?],
        };
        self.finish(key, session);
        Ok(vec![output])
    }
    pub fn with_game_daemon(mut self, gamed: Arc<dyn crate::gamed::GameDaemon>) -> Self {
        self.gamed = gamed;
        self
    }
    pub(super) async fn enter_world(
        &mut self,
        link: LinkId,
        packet: &[u8],
        key: u64,
    ) -> io::Result<Vec<Vec<u8>>> {
        let Ok(v) = wire::fields("hqo", packet) else {
            return Ok(vec![wire::failure(26, key, 2)?]);
        };
        let Some(session) = self.sessions.get(&key).filter(|s| s.link == link) else {
            return Ok(vec![wire::failure(26, key, 4)?]);
        };
        if session.state != State::Lobby {
            return Ok(vec![wire::failure(26, key, 5)?]);
        }
        let Value::Id(slot) = v[2] else {
            unreachable!()
        };
        let Some(character) = session.characters.iter().find(|c| c.slot == slot).cloned() else {
            return Ok(vec![wire::failure(26, key, 101)?]);
        };
        if !session.services.is_empty() {
            return Ok(vec![wire::failure(26, key, 9)?]);
        }
        let Some(lifetime) = self.settings.world_token_lifetime else {
            return Ok(vec![wire::failure(26, key, 2)?]);
        };
        let account = session.account.as_ref().unwrap().id;
        let ipv4 = session.ipv4;
        let restored = session.restored.contains(&character.pcid);
        let decision = async {
            let eligibility = self.gamed.eligibility(&character)?;
            if eligibility == Eligibility::Restore && !restored {
                tokio::time::timeout(
                    self.settings.world_timeout,
                    self.gamed.restore(character.world, character.pcid),
                )
                .await
                .map_err(|_| 104i16)??;
                self.store
                    .finish_character_restoration(character.pcid)
                    .await
                    .map_err(|_| 2i16)?;
                self.sessions
                    .get_mut(&key)
                    .unwrap()
                    .restored
                    .insert(character.pcid);
            }
            let charge: ChargeMode = tokio::time::timeout(
                self.settings.np_timeout,
                self.np
                    .world_entry(account, character.pcid, character.slot, ipv4),
            )
            .await
            .map_err(|_| 7i16)??;
            tokio::time::timeout(
                self.settings.world_timeout,
                self.gamed
                    .admit(key, character.world, character.pcid, charge),
            )
            .await
            .map_err(|_| 104i16)?
        }
        .await;
        let admission = match decision {
            Ok(admission) => admission,
            Err(reason) => return Ok(vec![wire::failure(26, key, reason)?]),
        };
        let token = self
            .next_key
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |v| v.checked_add(1))
            .map_err(|_| wire::invalid("token key exhausted"))?;
        let deadline = Instant::now()
            .checked_add(lifetime)
            .ok_or_else(|| wire::invalid("token lifetime overflow"))?;
        let packets = if let Some(wait) = &admission.wait {
            vec![wire::failure(26, key, 107)?, wire::world_wait(key, wait)?]
        } else {
            vec![wire::world_token(key, token)?]
        };
        let session = self.sessions.get_mut(&key).unwrap();
        session.state = if admission.wait.is_some() {
            State::WorldQueued
        } else {
            State::WorldReady
        };
        session.world_token = Some(WorldToken {
            key: token,
            world: character.world,
            pcid: character.pcid,
        });
        session.admission = Some(Arc::new(admission));
        session.expires = Some(deadline);
        Ok(packets)
    }
    /// D28 GetWaitInfoReq (`20-handlers.md` §4.1). No session → Fail 30 code 4.
    /// ❓ `GetGameWorldWaitInfo` error outside 3.2: the code is not read; local code 5.
    pub(super) fn wait_info(&self, link: LinkId, key: u64) -> io::Result<Vec<Vec<u8>>> {
        let Some(session) = self.sessions.get(&key).filter(|s| s.link == link) else {
            return Ok(vec![wire::failure(30, key, 4)?]);
        };
        let wait = match (&session.state, &session.admission) {
            (State::WorldQueued, Some(admission)) => admission.wait.as_ref(),
            _ => None,
        };
        let Some(wait) = wait else {
            return Ok(vec![wire::failure(30, key, 5)?]);
        };
        Ok(vec![wire::wait_packet(29, key, wait)?])
    }
    /// D31 GiveupWaitReq and GiveupWaitGameWorldJob (`13-jobs-a.md` §3.4).
    /// No session → Fail 33 code 4. Not 3.2 → 5. No world location → 2.
    /// `leave`: the drop of the admission releases the place in the queue.
    pub(super) fn giveup_wait(&mut self, link: LinkId, key: u64) -> io::Result<Vec<Vec<u8>>> {
        let Some(session) = self.sessions.get_mut(&key).filter(|s| s.link == link) else {
            return Ok(vec![wire::failure(33, key, 4)?]);
        };
        if session.state != State::WorldQueued {
            return Ok(vec![wire::failure(33, key, 5)?]);
        }
        if session.world_token.is_none() {
            return Ok(vec![wire::failure(33, key, 2)?]);
        }
        session.admission = None;
        session.world_token = None;
        session.expires = None;
        session.state = State::Lobby;
        Ok(vec![wire::notice(32, key)?])
    }
    /// Local queue fixture control. No automatic or real GameDaemon promotion.
    pub fn promote_world(&mut self, key: u64) -> io::Result<Vec<Output>> {
        let session = self
            .sessions
            .get_mut(&key)
            .filter(|s| s.state == State::WorldQueued)
            .ok_or_else(|| wire::invalid("queued session required"))?;
        if session.expires.is_some_and(|d| d <= Instant::now()) {
            return Err(wire::invalid("queue expired"));
        }
        let token = session.world_token.as_ref().unwrap().key;
        let packets = vec![wire::world_token(key, token)?];
        session.state = State::WorldReady;
        Ok(vec![Output {
            link: session.link,
            packets,
        }])
    }
    /// Local kind-1 verification. Wrong identity fields do not consume the token.
    /// Valid identity consumes it before world validation, as in build 307.
    pub fn consume_world(
        &mut self,
        key: u64,
        token: u64,
        ipv4: u32,
        world: i16,
    ) -> Result<u64, i16> {
        let session = self.sessions.get_mut(&key).ok_or(16i16)?;
        if session.state != State::WorldReady {
            return Err(16);
        }
        if session.expires.is_some_and(|d| d <= Instant::now()) {
            return Err(16);
        }
        let pending = session.world_token.as_ref().ok_or(16i16)?;
        if pending.key != token || (self.settings.check_retained_ipv4 && session.ipv4 != ipv4) {
            return Err(15);
        }
        let pending = session.world_token.take().unwrap();
        session.admission = None;
        session.expires = None;
        // Local failure-hook policy. The original hook is not fully specified.
        if pending.world != world {
            session.state = State::Lobby;
            return Err(17);
        }
        session.state = State::World;
        session.playing = Some(Playing {
            world,
            pcid: pending.pcid,
        });
        // EnterGameWorldJob: `usp_UpdateLastPlayStart`, written by the link task.
        self.play.starts.push((pending.pcid, ipv4));
        Ok(pending.pcid)
    }
    /// The account lists of the session for the 381 `EnterWorldOk`. `None`: no session.
    pub fn account_lists(&self, key: u64) -> Option<AccountLists> {
        self.sessions.get(&key).map(|s| s.cache.lists())
    }
    /// The account id of session `key` when it plays `pcid` in `world` (state 4.0).
    /// This is the identity check of every gamed sync request (`20-handlers.md` lines 97–121):
    /// the client key, the PC id and the world of the link must all match.
    fn playing_account(&self, key: u64, world: i16, pcid: u64) -> Option<AccountId> {
        let session = self.sessions.get(&key)?;
        let playing = session.playing?;
        (pcid != 0
            && session.state == State::World
            && playing.world == world
            && playing.pcid == pcid)
            .then_some(())?;
        session.account.as_ref().map(|a| a.id)
    }
    /// A gamed sync request of the world `world`: 11, 39, 42, 107, 110 and 113. `key` is the
    /// client key of the message. `Some` is the reply for the GameDaemon (`None` for 11).
    /// Reason 5: no session for the key, the session does not play this PC in this world, or
    /// the message is not a sync request. Reason 2: a list of more than 512 rows, a showcase of
    /// more than 512 bytes, a time that is not valid, or a database failure without a code.
    /// The wait for the database runs in a job task (rule 3 of step 6).
    pub async fn world_sync(&mut self, key: u64, world: i16, message: Message) -> Option<Message> {
        let pcid = match &message {
            Message::UpdateBotScore { pcid, .. }
            | Message::UpdateCharacterData { pcid, .. }
            | Message::UpdateAccountData { pcid, .. }
            | Message::UpdateAccountExp { pcid, .. }
            | Message::UpdateAccountContentQuota { pcid, .. }
            | Message::UpdateAccountRecycleTime { pcid, .. } => *pcid,
            _ => return None,
        };
        let Some(id) = self.playing_account(key, world, pcid) else {
            return sync_failure(&message, 5);
        };
        let too_long = match &message {
            Message::UpdateCharacterData { showcase, .. } => showcase.len() > RESULT_BLOB_CAPACITY,
            Message::UpdateAccountData { lists, .. } => {
                lists.time_based.len() > MAX_SYNC_ROWS
                    || lists.quotas.len() > MAX_SYNC_ROWS
                    || lists.recycle.len() > MAX_SYNC_ROWS
            }
            Message::UpdateAccountContentQuota { quotas, .. } => quotas.len() > MAX_SYNC_ROWS,
            Message::UpdateAccountRecycleTime { recycle, .. } => recycle.len() > MAX_SYNC_ROWS,
            _ => false,
        };
        if too_long {
            return sync_failure(&message, 2);
        }
        match message {
            Message::UpdateBotScore { score, .. } => {
                // The score goes to `usp_UpdateLastLogout` when the session ends.
                if let Some(account) = self.sessions.get_mut(&key).and_then(|s| s.account.as_mut())
                {
                    account.bot_score = score;
                }
                None
            }
            Message::UpdateCharacterData {
                transaction,
                level,
                showcase,
                ..
            } => {
                let written = self
                    .store
                    .update_character_data(pcid, level, showcase.clone())
                    .await;
                Some(match written {
                    Ok(()) => {
                        if let Some(character) = self
                            .sessions
                            .get_mut(&key)
                            .and_then(|s| s.characters.iter_mut().find(|c| c.pcid == pcid))
                        {
                            character.showcase = showcase;
                        }
                        reply(UPDATE_CHARACTER_DATA_OK_COMMAND, transaction, None, None)
                    }
                    Err(error) => reply(
                        UPDATE_CHARACTER_DATA_FAIL_COMMAND,
                        transaction,
                        None,
                        Some(failure_code(&error)),
                    ),
                })
            }
            Message::UpdateAccountData {
                transaction,
                bot_score,
                lists,
                ..
            } => {
                if let Some(account) = self.sessions.get_mut(&key).and_then(|s| s.account.as_mut())
                {
                    account.bot_score = bot_score;
                }
                // All four steps run. The first failure code goes in the reply.
                let mut first: Option<i16> = None;
                let steps = [
                    self.sync_time(key, id, &lists.time_based).await,
                    self.sync_exp(key, id, &lists.exp).await,
                    self.sync_quotas(key, id, &lists.quotas, false).await,
                    self.sync_recycle(key, id, &lists.recycle, false).await,
                ];
                for step in steps {
                    if let Err(code) = step {
                        first.get_or_insert(code);
                    }
                }
                Some(match first {
                    None => reply(UPDATE_ACCOUNT_DATA_OK_COMMAND, transaction, None, None),
                    Some(code) => reply(
                        UPDATE_ACCOUNT_DATA_FAIL_COMMAND,
                        transaction,
                        None,
                        Some(code),
                    ),
                })
            }
            Message::UpdateAccountExp {
                transaction, exp, ..
            } => Some(match self.sync_exp(key, id, &exp).await {
                Ok(()) => reply(UPDATE_ACCOUNT_EXP_OK_COMMAND, transaction, Some(pcid), None),
                Err(code) => reply(
                    UPDATE_ACCOUNT_EXP_FAIL_COMMAND,
                    transaction,
                    Some(pcid),
                    Some(code),
                ),
            }),
            Message::UpdateAccountContentQuota {
                transaction,
                quotas,
                ..
            } => Some(match self.sync_quotas(key, id, &quotas, true).await {
                Ok(()) => reply(
                    UPDATE_ACCOUNT_CONTENT_QUOTA_OK_COMMAND,
                    transaction,
                    Some(pcid),
                    None,
                ),
                Err(code) => reply(
                    UPDATE_ACCOUNT_CONTENT_QUOTA_FAIL_COMMAND,
                    transaction,
                    Some(pcid),
                    Some(code),
                ),
            }),
            Message::UpdateAccountRecycleTime {
                transaction,
                recycle,
                ..
            } => Some(match self.sync_recycle(key, id, &recycle, true).await {
                Ok(()) => reply(
                    UPDATE_ACCOUNT_RECYCLE_TIME_OK_COMMAND,
                    transaction,
                    Some(pcid),
                    None,
                ),
                Err(code) => reply(
                    UPDATE_ACCOUNT_RECYCLE_TIME_FAIL_COMMAND,
                    transaction,
                    Some(pcid),
                    Some(code),
                ),
            }),
            _ => None,
        }
    }
    /// `usp_UpdateGameAccountTimeBasedPlayInfo`. The list replaces the cached list.
    async fn sync_time(
        &mut self,
        key: u64,
        id: AccountId,
        rows: &[TimePlayInfo],
    ) -> Result<(), i16> {
        let rows: Vec<TimeBasedPlayInfo> = rows
            .iter()
            .map(time_to_store)
            .collect::<Option<_>>()
            .ok_or(2i16)?;
        self.store
            .update_time_based_play_info(id, rows.clone())
            .await
            .map_err(|e| failure_code(&e))?;
        if let Some(session) = self.sessions.get_mut(&key) {
            session.cache.time_based = rows;
        }
        Ok(())
    }
    /// `usp_UpdateGameAccountExp`.
    async fn sync_exp(&mut self, key: u64, id: AccountId, exp: &AccountExpInfo) -> Result<(), i16> {
        let row = AccountExp {
            exp: exp.exp,
            quota_per_day: exp.quota_per_day,
            last_update: exp.last_update,
        };
        self.store
            .update_account_exp(id, row.clone())
            .await
            .map_err(|e| failure_code(&e))?;
        if let Some(session) = self.sessions.get_mut(&key) {
            session.cache.exp = Some(row);
        }
        Ok(())
    }
    /// `usp_UpdateGameAccountContentQuota`, one call for each row, in order. `stop`: the first
    /// failure ends the list. Else all rows run. `Err` is the first failure code. A row that
    /// was written goes to the cache.
    async fn sync_quotas(
        &mut self,
        key: u64,
        id: AccountId,
        rows: &[ContentQuota],
        stop: bool,
    ) -> Result<(), i16> {
        let mut first = None;
        for row in rows {
            match self.store.update_content_quota(id, row.clone()).await {
                Ok(()) => {
                    if let Some(session) = self.sessions.get_mut(&key) {
                        session.cache.put_quota(row.clone());
                    }
                }
                Err(error) => {
                    first.get_or_insert(failure_code(&error));
                    if stop {
                        break;
                    }
                }
            }
        }
        first.map_or(Ok(()), Err)
    }
    /// `usp_UpdateGameAccountRecycleTime`, one call for each row, in order. A group keeps the
    /// larger time, so the row that is written is the larger of the cached and the new time
    /// (❓ the 381 merge rule is not verified). `stop` as for [`Self::sync_quotas`].
    async fn sync_recycle(
        &mut self,
        key: u64,
        id: AccountId,
        rows: &[RecycleTime],
        stop: bool,
    ) -> Result<(), i16> {
        let mut first = None;
        for row in rows {
            let mut row = row.clone();
            if let Some(old) = self
                .sessions
                .get(&key)
                .and_then(|s| s.cache.recycle.iter().find(|r| r.group == row.group))
            {
                row.time = row.time.max(old.time);
            }
            match self.store.update_recycle_time(id, row.clone()).await {
                Ok(()) => {
                    if let Some(session) = self.sessions.get_mut(&key) {
                        session.cache.put_recycle(row);
                    }
                }
                Err(error) => {
                    first.get_or_insert(failure_code(&error));
                    if stop {
                        break;
                    }
                }
            }
        }
        first.map_or(Ok(()), Err)
    }
}
