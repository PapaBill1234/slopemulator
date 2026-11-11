//! Local world interface. No GameDaemon transport or production eligibility claim.
use crate::store::Character;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

pub use shared::np_channel::{ChargeMode, GameFuture};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Eligibility {
    Ready,
    Restore,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WaitInfo {
    /// Opaque D27 fields. Their production meanings are not established.
    pub first: u32,
    pub second: u32,
    pub rows: Vec<(u8, i16)>,
}
pub struct Admission {
    pub wait: Option<WaitInfo>,
    release: Option<Box<dyn FnOnce() + Send + Sync>>,
}
impl Admission {
    /// The callback must release the local reservation without blocking.
    pub fn new(wait: Option<WaitInfo>, release: impl FnOnce() + Send + Sync + 'static) -> Self {
        Self {
            wait,
            release: Some(Box::new(release)),
        }
    }
}
impl Drop for Admission {
    fn drop(&mut self) {
        if let Some(release) = self.release.take() {
            release();
        }
    }
}
/// Futures must be cancellation safe. Returned admission owns its reservation.
/// Production adapters must obtain real metadata; wire Character.state is not eligibility.
pub trait GameDaemon: Send + Sync + 'static {
    fn create_character(
        &self,
        _request: crate::character::CreateCharacter,
        _pcid: u64,
        _appearance: Vec<u8>,
    ) -> GameFuture<'_, crate::character::CreatedCharacter> {
        Box::pin(async { Err(2) })
    }
    fn delete_character(&self, _world: i16, _pcid: u64) -> GameFuture<'_, i16> {
        Box::pin(async { Err(2) })
    }
    fn restore_character(&self, _world: i16, _pcid: u64) -> GameFuture<'_, ()> {
        Box::pin(async { Err(2) })
    }
    fn eligibility(&self, character: &Character) -> Result<Eligibility, i16>;
    fn restore(&self, world: i16, pcid: u64) -> GameFuture<'_, ()>;
    fn admit(
        &self,
        owner: u64,
        world: i16,
        pcid: u64,
        charge: ChargeMode,
    ) -> GameFuture<'_, Admission>;
}
#[derive(Clone, Debug)]
pub struct WorldFixture {
    pub slot: [u8; 16],
    pub eligibility: Result<Eligibility, i16>,
    pub restore: Result<(), i16>,
    pub admission: Result<Option<WaitInfo>, i16>,
}
#[derive(Default)]
struct Ledger {
    active: usize,
    released: usize,
    restored: usize,
}
#[derive(Default)]
pub struct StubGameDaemon {
    pub worlds: HashMap<(i16, u64), WorldFixture>,
    pub delay: Duration,
    pub create_error: Option<i16>,
    pub delete_error: Option<i16>,
    pub restore_error: Option<i16>,
    ledger: Arc<Mutex<Ledger>>,
}
impl StubGameDaemon {
    pub fn active(&self) -> usize {
        self.ledger.lock().unwrap().active
    }
    pub fn released(&self) -> usize {
        self.ledger.lock().unwrap().released
    }
    pub fn restored(&self) -> usize {
        self.ledger.lock().unwrap().restored
    }
}
impl GameDaemon for StubGameDaemon {
    fn create_character(
        &self,
        _request: crate::character::CreateCharacter,
        _pcid: u64,
        appearance: Vec<u8>,
    ) -> GameFuture<'_, crate::character::CreatedCharacter> {
        Box::pin(async move {
            tokio::time::sleep(self.delay).await;
            if let Some(error) = self.create_error {
                return Err(error);
            }
            Ok(crate::character::CreatedCharacter {
                job: 1,
                level: 1,
                showcase: appearance,
            })
        })
    }
    fn delete_character(&self, _world: i16, _pcid: u64) -> GameFuture<'_, i16> {
        Box::pin(async move {
            tokio::time::sleep(self.delay).await;
            self.delete_error.map_or(Ok(1), Err)
        })
    }
    fn restore_character(&self, _world: i16, _pcid: u64) -> GameFuture<'_, ()> {
        Box::pin(async move {
            tokio::time::sleep(self.delay).await;
            self.restore_error.map_or(Ok(()), Err)
        })
    }
    fn eligibility(&self, c: &Character) -> Result<Eligibility, i16> {
        self.worlds
            .get(&(c.world, c.pcid))
            .filter(|f| f.slot == c.slot)
            .ok_or(104i16)?
            .eligibility
    }
    fn restore(&self, world: i16, pcid: u64) -> GameFuture<'_, ()> {
        Box::pin(async move {
            tokio::time::sleep(self.delay).await;
            self.worlds.get(&(world, pcid)).ok_or(104i16)?.restore?;
            self.ledger.lock().unwrap().restored += 1;
            Ok(())
        })
    }
    fn admit(
        &self,
        _owner: u64,
        world: i16,
        pcid: u64,
        _charge: ChargeMode,
    ) -> GameFuture<'_, Admission> {
        Box::pin(async move {
            tokio::time::sleep(self.delay).await;
            let wait = self
                .worlds
                .get(&(world, pcid))
                .ok_or(104i16)?
                .admission
                .clone()?;
            self.ledger.lock().unwrap().active += 1;
            let ledger = self.ledger.clone();
            Ok(Admission::new(wait, move || {
                let mut ledger = ledger.lock().unwrap();
                ledger.active -= 1;
                ledger.released += 1;
            }))
        })
    }
}

/// Stand-in for the GameDaemon link while no GameDaemon server exists
/// (card play-1). It is not a port of the link (`Docs/Server/LobbyDaemon/07-gamed-link.md`).
/// Create: the showcase comes from the client make-PC record, as
/// `CreateCharacterTran` does it in GameDaemon, but without the
/// `PcInitialSetting` data ❓. World admission fails with reason 2.
/// ❓ Local: it keeps each make-PC record and hands it to `gamedaemon`
/// through `shared::local_entry` at world admission.
#[derive(Default)]
pub struct LocalGameDaemon {
    created: Mutex<HashMap<u64, (shared::fsd::MakePc, Vec<u16>)>>,
}

impl GameDaemon for LocalGameDaemon {
    fn create_character(
        &self,
        request: crate::character::CreateCharacter,
        pcid: u64,
        appearance: Vec<u8>,
    ) -> GameFuture<'_, crate::character::CreatedCharacter> {
        Box::pin(async move {
            let Some(pc) = shared::fsd::decode_make_pc(&appearance) else {
                eprintln!(
                    "lobbyd: create character, bad make-PC record ({} bytes)",
                    appearance.len()
                );
                return Err(2);
            };
            let showcase = shared::fsd::compose_new_showcase(&pc, &request.name, 1);
            eprintln!(
                "lobbyd: create character, race {} sex {} job {}, showcase {} bytes",
                pc.race,
                pc.sex,
                pc.job,
                showcase.len()
            );
            let job = i16::from(pc.job);
            self.created
                .lock()
                .unwrap()
                .insert(pcid, (pc, request.name.clone()));
            Ok(crate::character::CreatedCharacter {
                job,
                level: 1,
                showcase,
            })
        })
    }
    fn delete_character(&self, _world: i16, _pcid: u64) -> GameFuture<'_, i16> {
        Box::pin(async { Ok(1) })
    }
    fn restore_character(&self, _world: i16, _pcid: u64) -> GameFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
    fn eligibility(&self, _character: &Character) -> Result<Eligibility, i16> {
        Ok(Eligibility::Ready)
    }
    fn restore(&self, _world: i16, _pcid: u64) -> GameFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
    fn admit(
        &self,
        owner: u64,
        _world: i16,
        pcid: u64,
        _charge: ChargeMode,
    ) -> GameFuture<'_, Admission> {
        // ❓ Local: no GameDaemon link. Each entry is admitted with no wait.
        let created = self.created.lock().unwrap().get(&pcid).cloned();
        if let Some((pc, name)) = created {
            let entry = shared::local_entry::LocalEntry { pcid, pc, name };
            let result = entry.write(owner);
            eprintln!("lobbyd: world entry hand-off, owner {owner} pcid {pcid}: {result:?}");
        } else {
            eprintln!("lobbyd: world entry, no make-PC record for pcid {pcid}");
        }
        Box::pin(async { Ok(Admission::new(None, || {})) })
    }
}
