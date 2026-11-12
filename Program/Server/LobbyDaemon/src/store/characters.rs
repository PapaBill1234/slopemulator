//! Local lifecycle persistence. Keep internal states outside wire-ready rows.
use super::*;
use crate::character::{
    procedure_error, CreateCharacter, CreatedCharacter, InternalState as State,
};

impl MemoryStore {
    pub(super) fn character_failure(&mut self, op: &'static str) -> io::Result<()> {
        match self.character_failures.remove(op) {
            Some(code) => Err(procedure_error(code)),
            None => Ok(()),
        }
    }
    fn character_mut(&mut self, pcid: u64) -> io::Result<&mut Character> {
        self.characters
            .values_mut()
            .flatten()
            .find(|c| c.pcid == pcid)
            .ok_or_else(absent)
    }
    pub(super) fn now(&self) -> SystemTime {
        self.clock.map_or_else(SystemTime::now, |clock| clock())
    }
    /// The state of the row is the truth, as in LobbyDB. `character_states` mirrors it
    /// for tests and keeps a tombstone for a PCID that left the list.
    pub(super) fn transition(
        &mut self,
        op: &'static str,
        pcid: u64,
        from: State,
        to: State,
    ) -> io::Result<()> {
        self.character_failure(op)?;
        let row = self.character_mut(pcid).map_err(|_| procedure_error(1))?;
        if row.state != from {
            return Err(procedure_error(1));
        }
        row.state = to;
        if to != State::Waiting {
            row.deletion_time = None;
        }
        self.character_states.insert(pcid, to);
        Ok(())
    }
    pub(super) fn expired_waiting(&mut self, max: u32) -> io::Result<Vec<ExpiredDeletion>> {
        self.character_failure("retrieve_expired")?;
        if max == 0 {
            return Err(procedure_error(1));
        }
        let now = self.now();
        let mut rows: Vec<_> = self
            .characters
            .iter()
            .flat_map(|(account, rows)| rows.iter().map(move |c| (*account, c)))
            .filter(|(_, c)| {
                c.state == State::Waiting && c.deletion_time.is_some_and(|time| time < now)
            })
            .map(|(account, c)| ExpiredDeletion {
                account,
                world: c.world,
                pcid: c.pcid,
            })
            .collect();
        rows.sort_by_key(|row| row.pcid);
        rows.truncate(max as usize);
        Ok(rows)
    }
    /// State 4 to 6 when the time has passed (`usp_ExpireCharacterDeletionWaiting`).
    /// The row leaves the list. The tombstone keeps the PCID out of the local ID source.
    pub(super) fn expire_waiting(&mut self, pcid: u64) -> io::Result<()> {
        self.character_failure("expire_delete")?;
        let now = self.now();
        let row = self.character_mut(pcid).map_err(|_| procedure_error(1))?;
        if row.state != State::Waiting || !row.deletion_time.is_some_and(|time| time < now) {
            return Err(procedure_error(1));
        }
        row.state = State::Deleted;
        row.deletion_time = None;
        self.character_states.insert(pcid, State::Deleted);
        for rows in self.characters.values_mut() {
            rows.retain(|c| c.pcid != pcid);
        }
        Ok(())
    }
    pub(super) fn check_name(&mut self, name: &[u16]) -> io::Result<()> {
        self.character_failure("check_name")?;
        if name.is_empty() || name.len() > 32 || name.contains(&0) {
            return Err(procedure_error(1));
        }
        let pure = |n: &[u16]| {
            String::from_utf16_lossy(n)
                .replace(' ', "")
                .to_ascii_lowercase()
        };
        let requested = pure(name);
        if self
            .characters
            .values()
            .flatten()
            .any(|c| pure(&c.name) == requested)
        {
            return Err(procedure_error(2));
        }
        if self.limited_names.iter().any(|n| pure(n) == requested) {
            return Err(procedure_error(3));
        }
        Ok(())
    }
    pub(super) fn create(&mut self, request: CreateCharacter) -> io::Result<u64> {
        self.character_failure("try_create")?;
        if !self.accounts.contains_key(&request.account)
            || request.slot == [0; 16]
            || request.world <= 0
            || request.right != 1
        {
            return Err(procedure_error(1));
        }
        self.check_name(&request.name)?;
        if self
            .characters
            .values()
            .flatten()
            .any(|c| c.slot == request.slot)
        {
            return Err(procedure_error(1));
        }
        let pcid = self
            .characters
            .values()
            .flatten()
            .map(|c| c.pcid)
            .chain(self.character_states.keys().copied())
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .filter(|n| *n <= i64::MAX as u64)
            .ok_or_else(|| procedure_error(1))?;
        self.characters
            .entry(request.account)
            .or_default()
            .push(Character {
                pcid,
                slot: request.slot,
                world: request.world,
                name: request.name,
                showcase: vec![],
                deletion_time: None,
                state: State::Creating,
                jumping_state: 0,
                last_play_end: 0,
            });
        self.character_states.insert(pcid, State::Creating);
        Ok(pcid)
    }
    pub(super) fn finish_create(&mut self, pcid: u64, created: CreatedCharacter) -> io::Result<()> {
        check_blob(&created.showcase)?;
        if created.job <= 0 || created.level <= 0 || created.showcase.is_empty() {
            return Err(procedure_error(1));
        }
        self.transition("finish_create", pcid, State::Creating, State::Normal)?;
        self.character_mut(pcid)?.showcase = created.showcase;
        Ok(())
    }
    pub(super) fn cancel_create(&mut self, pcid: u64) -> io::Result<()> {
        self.transition("cancel_create", pcid, State::Creating, State::Deleted)?;
        for rows in self.characters.values_mut() {
            rows.retain(|c| c.pcid != pcid);
        }
        // Keep a tombstone so the local ID source cannot silently reuse this PCID.
        Ok(())
    }
    pub(super) fn finish_delete(&mut self, pcid: u64, seconds: u32) -> io::Result<()> {
        if seconds > i32::MAX as u32 {
            return Err(procedure_error(1));
        }
        let state = if seconds == 0 {
            State::Deleted
        } else {
            State::Waiting
        };
        self.transition("finish_delete", pcid, State::Deleting, state)?;
        let time = self.now() + Duration::from_secs(seconds.into());
        let row = self.character_mut(pcid)?;
        row.last_play_end = seconds;
        row.deletion_time = (seconds > 0).then_some(time);
        if seconds == 0 {
            for rows in self.characters.values_mut() {
                rows.retain(|c| c.pcid != pcid);
            }
        }
        Ok(())
    }
}
