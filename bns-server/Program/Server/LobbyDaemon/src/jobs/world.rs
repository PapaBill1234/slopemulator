//! D24 local job. Await external decisions before committing session state.
use super::*;
use crate::gamed::{ChargeMode, Eligibility};

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
        Ok(pending.pcid)
    }
}
