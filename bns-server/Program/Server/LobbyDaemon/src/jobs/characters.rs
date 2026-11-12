//! Local R15 fixture lifecycle. The production NP checks are not implemented.
use super::*;
use crate::character::{CreateCharacter, InternalState as CState};
use crate::ids::gate::*;

fn reply(cmd: i16, key: u64, slot: [u8; 16], state: i8, reason: i16) -> io::Result<Vec<u8>> {
    let mut v = vec![Value::I16(cmd), Value::U64(key), Value::Id(slot)];
    if cmd != 46 {
        v.push(Value::I8(state));
    }
    v.push(Value::I16(reason));
    wire::packet(if cmd == 46 { "hqoE" } else { "hqoQE" }, &v)
}
/// Client reason of a gamed `DeleteCharacterFail` 25 (`16-jobs-d.md` §2.5):
/// 28 → 139, 29 → 140, 30 → 141, 31 → 148, 32 → 149, else 2.
fn delete_fail_code(reason: i16) -> i16 {
    match reason {
        28 => 139,
        29 => 140,
        30 => 141,
        31 => 148,
        32 => 149,
        _ => 2,
    }
}
fn success(cmd: i16, key: u64, slot: [u8; 16]) -> io::Result<Vec<u8>> {
    wire::packet("hqo", &[Value::I16(cmd), Value::U64(key), Value::Id(slot)])
}
impl<S: Store, N: Np> Jobs<S, N> {
    pub fn begin_initial_name(&mut self, key: u64) -> io::Result<()> {
        if self.settings.character_policy.is_none() {
            return Err(wire::invalid("character policy required"));
        }
        let s = self
            .sessions
            .get_mut(&key)
            .filter(|s| s.state == State::Lobby)
            .ok_or_else(|| wire::invalid("Lobby required"))?;
        s.state = State::InitialName;
        Ok(())
    }
    /// The remaining seconds of each deletion wait, from the stored wall-clock time.
    pub(super) fn refresh_deletion_counts(&mut self, key: u64) {
        let Some(s) = self.sessions.get_mut(&key) else {
            return;
        };
        if !s.characters.iter().any(|c| c.state == CState::Waiting) {
            return;
        }
        let now = (self.settings.clock)();
        for c in &mut s.characters {
            if let (CState::Waiting, Some(time)) = (c.state, c.deletion_time) {
                c.last_play_end = time
                    .duration_since(now)
                    .map_or(0, |left| left.as_secs().min(i32::MAX as u64) as u32);
            }
        }
    }
    fn set_character_state(&mut self, key: u64, pcid: u64, state: CState) {
        if let Some(s) = self.sessions.get_mut(&key) {
            if let Some(c) = s.characters.iter_mut().find(|c| c.pcid == pcid) {
                c.state = state;
                if state != CState::Waiting {
                    c.deletion_time = None;
                }
            }
        }
    }
    pub(super) async fn character_mutation(
        &mut self,
        link: LinkId,
        packet: &[u8],
        key: u64,
        cmd: i16,
    ) -> io::Result<Vec<Vec<u8>>> {
        let format = if cmd == CreateCharacterReq {
            "hqohsbQ"
        } else {
            "hqo"
        };
        let Ok(v) = wire::fields(format, packet) else {
            return Ok(vec![]);
        };
        let Value::Id(slot) = v[2] else {
            unreachable!()
        };
        // The fail IDs (46, 49, 53, 56) are not in the 307 handler tables.
        let fail = match cmd {
            CreateCharacterReq => 46,
            DeleteCharacterReq => 49,
            CancelWaitCharacterDeletionReq => 53,
            _ => 56,
        };
        let Some(s) = self.sessions.get(&key).filter(|s| s.link == link) else {
            return Ok(vec![reply(fail, key, slot, 0, 4)?]);
        };
        if s.state != State::Lobby {
            return Ok(vec![]);
        }
        let current = s.characters.iter().find(|c| c.slot == slot).cloned();
        let state = current.as_ref().map_or(0, |c| c.state.wire());
        let failure = |reason| -> io::Result<Vec<Vec<u8>>> {
            Ok(vec![reply(fail, key, slot, state, reason)?])
        };
        if slot == [0; 16] {
            return failure(6);
        }
        let Some(policy) = self.settings.character_policy.clone() else {
            return failure(2);
        };
        // An empty set accepts each slot id: the slots are per user in SlotSrv
        // and `lobbyd` has no Slot link ❓.
        if !policy.slots.is_empty() && !policy.slots.contains(&slot) {
            return failure(6);
        }
        if !s.services.is_empty() {
            return failure(2);
        }
        if cmd == CreateCharacterReq {
            let (
                Value::I16(world),
                Value::String(name),
                Value::Bytes(appearance),
                Value::I8(jumping),
            ) = (&v[3], &v[4], &v[5], &v[6])
            else {
                unreachable!()
            };
            if current.is_some()
                || *world == 0
                || name.is_empty()
                || appearance.is_empty()
                || appearance.len() > 256
                || name.len() > 32
            {
                return failure(6);
            }
            if *jumping != 0 || !policy.worlds.contains(world) {
                return failure(2);
            }
            if let Err(reason) = policy.validate_name(name, false) {
                return failure(reason);
            }
            let request = CreateCharacter {
                account: s.account.as_ref().unwrap().id,
                slot,
                world: *world,
                name: name.clone(),
                right: 1,
            };
            if self.store.check_character_name(name).await.is_err() {
                return failure(2);
            }
            let Ok(pcid) = self.store.try_character_creation(request.clone()).await else {
                return failure(2);
            };
            self.sessions
                .get_mut(&key)
                .unwrap()
                .characters
                .push(Character {
                    pcid,
                    slot,
                    world: *world,
                    name: name.clone(),
                    showcase: vec![],
                    deletion_time: None,
                    state: CState::Creating,
                    jumping_state: 0,
                    last_play_end: 0,
                });
            let result = tokio::time::timeout(
                self.settings.world_timeout,
                self.gamed
                    .create_character(request, pcid, appearance.clone()),
            )
            .await;
            let created = match result {
                Ok(Ok(created)) if created.showcase.len() <= 512 => created,
                other => {
                    if matches!(other, Ok(Err(_))) {
                        let _ = self.store.cancel_character_creation(pcid).await;
                    }
                    self.sessions
                        .get_mut(&key)
                        .unwrap()
                        .characters
                        .retain(|c| c.pcid != pcid);
                    return failure(2);
                }
            };
            if self
                .store
                .finish_character_creation(pcid, created.clone())
                .await
                .is_err()
            {
                self.sessions
                    .get_mut(&key)
                    .unwrap()
                    .characters
                    .retain(|c| c.pcid != pcid);
                return failure(2);
            }
            self.set_character_state(key, pcid, CState::Normal);
            let c = self
                .sessions
                .get_mut(&key)
                .unwrap()
                .characters
                .iter_mut()
                .find(|c| c.pcid == pcid)
                .unwrap();
            c.showcase = created.showcase;
            let mut values = vec![Value::I16(45), Value::U64(key)];
            values.extend(wire::character_row(c)?);
            values.push(Value::I32(0)); // Non-jumping fixture has no quota.
            return Ok(vec![wire::packet("hqqohsbqQQii", &values)?]);
        }
        let Some(c) = current else {
            return failure(6);
        };
        if !policy.worlds.contains(&c.world) {
            return failure(2);
        }
        // The lifecycle comes from the character list of the store. The store procedures
        // check it again, so a change through another link or the scheduler is refused there.
        let internal = Some(c.state);
        if cmd == DeleteCharacterReq {
            if internal != Some(CState::Normal)
                || self.store.try_character_deletion(c.pcid).await.is_err()
            {
                return failure(2);
            }
            self.set_character_state(key, c.pcid, CState::Deleting);
            match tokio::time::timeout(
                self.settings.world_timeout,
                self.gamed.delete_character(c.world, c.pcid),
            )
            .await
            {
                Ok(Ok(_level)) => {}
                Ok(Err(reason)) => {
                    if self.store.cancel_character_deletion(c.pcid).await.is_ok() {
                        self.set_character_state(key, c.pcid, CState::Normal);
                    }
                    return Ok(vec![reply(
                        fail,
                        key,
                        slot,
                        self.sessions[&key]
                            .characters
                            .iter()
                            .find(|r| r.pcid == c.pcid)
                            .unwrap()
                            .state
                            .wire(),
                        delete_fail_code(reason),
                    )?]);
                }
                Err(_) => return Ok(vec![reply(fail, key, slot, 3, 2)?]),
            }
            let seconds = policy.deletion_wait_seconds;
            if self
                .store
                .finish_character_deletion(c.pcid, seconds)
                .await
                .is_err()
            {
                return Ok(vec![reply(fail, key, slot, 3, 2)?]);
            }
            let next = if seconds == 0 {
                CState::Deleted
            } else {
                CState::Waiting
            };
            self.set_character_state(key, c.pcid, next);
            if seconds == 0 {
                self.sessions
                    .get_mut(&key)
                    .unwrap()
                    .characters
                    .retain(|r| r.pcid != c.pcid);
            } else {
                // The store keeps the same time (`usp_FinishCharacterDeletion`). The countdown
                // of this reply is exactly `seconds`, so the cache starts from the local clock.
                let time = (self.settings.clock)() + Duration::from_secs(seconds.into());
                if let Some(r) = self
                    .sessions
                    .get_mut(&key)
                    .and_then(|s| s.characters.iter_mut().find(|r| r.pcid == c.pcid))
                {
                    r.deletion_time = Some(time);
                    r.last_play_end = seconds;
                }
            }
            return Ok(vec![wire::packet(
                "hqoQi",
                &[
                    Value::I16(48),
                    Value::U64(key),
                    Value::Id(slot),
                    Value::I8(next.wire()),
                    Value::U32(seconds),
                ],
            )?]);
        }
        if cmd == CancelWaitCharacterDeletionReq {
            if internal != Some(CState::Waiting)
                || c.deletion_time
                    .is_none_or(|time| time <= (self.settings.clock)())
                || self.store.try_character_restoration(c.pcid).await.is_err()
            {
                return failure(2);
            }
            self.set_character_state(key, c.pcid, CState::Restoring);
            self.sessions
                .get_mut(&key)
                .unwrap()
                .characters
                .iter_mut()
                .find(|r| r.pcid == c.pcid)
                .unwrap()
                .last_play_end = 0;
        } else if !matches!(internal, Some(CState::Deleting | CState::Restoring)) {
            return failure(2);
        }
        let result = tokio::time::timeout(
            self.settings.world_timeout,
            self.gamed.restore_character(c.world, c.pcid),
        )
        .await;
        if !matches!(result, Ok(Ok(())))
            && !(cmd == RecoverCharacterReq && matches!(result, Ok(Err(35))))
        {
            return Ok(vec![reply(fail, key, slot, 3, 2)?]);
        }
        let done = if cmd == RecoverCharacterReq && internal == Some(CState::Deleting) {
            self.store.cancel_character_deletion(c.pcid).await
        } else {
            self.store.finish_character_restoration(c.pcid).await
        };
        if done.is_err() {
            return Ok(vec![reply(fail, key, slot, 3, 2)?]);
        }
        self.set_character_state(key, c.pcid, CState::Normal);
        Ok(vec![success(
            if cmd == CancelWaitCharacterDeletionReq {
                52
            } else {
                55
            },
            key,
            slot,
        )?])
    }
    pub(super) async fn initial_user_name(
        &mut self,
        link: LinkId,
        packet: &[u8],
        key: u64,
    ) -> io::Result<Vec<Vec<u8>>> {
        let Ok(v) = wire::fields("hqs", packet) else {
            return Ok(vec![]);
        };
        let Value::String(name) = &v[2] else {
            unreachable!()
        };
        let fail =
            |reason| -> io::Result<Vec<Vec<u8>>> { Ok(vec![wire::failure(105, key, reason)?]) };
        let Some(s) = self.sessions.get(&key).filter(|s| s.link == link) else {
            return fail(4);
        };
        if s.state != State::InitialName {
            return fail(5);
        }
        let Some(policy) = &self.settings.character_policy else {
            return fail(2);
        };
        if let Err(reason) = policy.validate_name(name, true) {
            return fail(reason);
        }
        let account = s.account.as_ref().unwrap().id;
        match tokio::time::timeout(
            self.settings.np_timeout,
            self.np.create_initial_user_name(account, name.clone()),
        )
        .await
        {
            Ok(Ok(())) => {}
            Ok(Err(reason)) => return fail(reason),
            Err(_) => return fail(7),
        }
        let s = self.sessions.get_mut(&key).unwrap();
        s.account.as_mut().unwrap().name = name.clone();
        s.state = State::Lobby;
        Ok(vec![wire::packet(
            "hqs",
            &[
                Value::I16(104),
                Value::U64(key),
                Value::String(name.clone()),
            ],
        )?])
    }
}
