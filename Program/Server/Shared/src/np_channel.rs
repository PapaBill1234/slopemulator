//! Deterministic local platform. This module has no transport.
use std::{
    collections::{HashMap, HashSet},
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};
use tokio::time::Instant;

pub type AccountId = [u8; 16];
pub type GameFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, i16>> + Send + 'a>>;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChargeMode {
    Paid,
    Trial,
    Free,
}

/// Build 307 callback arithmetic at 0x14002FAB0; NP errors are not wire reasons.
pub fn initial_name_reason(code: i32) -> i16 {
    match code {
        4 | 48 | 3000 | 3002 => 2,
        10 | 3035 => 6,
        3003 | 3042 => 229,
        3041 => 228,
        3336 => 231,
        3743 => 230,
        _ => 1,
    }
}
pub type NpFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, NpError>> + Send + 'a>>;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NpError {
    RejectedToken,
    MissingData,
    PresenceConflict,
    Restricted,
    Unavailable,
}
impl NpError {
    pub fn reason(self) -> i16 {
        match self {
            Self::RejectedToken => 15,
            Self::PresenceConflict => 18,
            Self::Restricted => 16,
            Self::MissingData | Self::Unavailable => 2,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    pub account: AccountId,
    pub login_name: Vec<u16>,
    pub user_name: Vec<u16>,
    pub slot_id: u64,
    pub profile: Vec<u16>,
    pub grades: Vec<i32>,
}
/// Local interface, not the NP ABI. Operations can run on different links.
/// Futures must not commit side effects before a pending await.
pub trait Np: Send + Sync + 'static {
    fn create_initial_user_name(&self, _account: AccountId, _name: Vec<u16>) -> GameFuture<'_, ()> {
        Box::pin(async { Err(7) })
    }
    fn consume(&self, token: [u8; 16], portal: [u8; 16]) -> NpFuture<'_, Identity>;
    fn login_presence(&self, account: AccountId, owner: u64) -> NpFuture<'_, ()>;
    fn logout_presence(&self, account: AccountId, owner: u64);
    /// Aggregate fixture, not the production NP protocol.
    fn world_entry(
        &self,
        _account: AccountId,
        _pcid: u64,
        _slot: [u8; 16],
        _ipv4: u32,
    ) -> GameFuture<'_, ChargeMode> {
        Box::pin(async { Err(2) })
    }
    fn expire(&self) {}
}
impl<N: Np> Np for Arc<N> {
    fn create_initial_user_name(&self, account: AccountId, name: Vec<u16>) -> GameFuture<'_, ()> {
        (**self).create_initial_user_name(account, name)
    }
    fn world_entry(
        &self,
        account: AccountId,
        pcid: u64,
        slot: [u8; 16],
        ipv4: u32,
    ) -> GameFuture<'_, ChargeMode> {
        (**self).world_entry(account, pcid, slot, ipv4)
    }
    fn consume(&self, token: [u8; 16], portal: [u8; 16]) -> NpFuture<'_, Identity> {
        (**self).consume(token, portal)
    }
    fn login_presence(&self, account: AccountId, owner: u64) -> NpFuture<'_, ()> {
        (**self).login_presence(account, owner)
    }
    fn logout_presence(&self, account: AccountId, owner: u64) {
        (**self).logout_presence(account, owner);
    }
    fn expire(&self) {
        (**self).expire();
    }
}
#[derive(Default)]
struct Presence {
    consumed: HashSet<[u8; 16]>,
    owners: HashMap<AccountId, u64>,
}
#[derive(Default)]
pub struct StubNp {
    pub identities: HashMap<([u8; 16], [u8; 16]), Result<Identity, NpError>>,
    pub presence_error: Option<NpError>,
    /// Explicit generated initial-name results. Errors are NP codes.
    pub initial_names: HashMap<(AccountId, Vec<u16>), Result<(), i32>>,
    pub delay: std::time::Duration,
    pub presence_delay: std::time::Duration,
    pub world_delay: std::time::Duration,
    pub world_results: HashMap<(AccountId, u64, [u8; 16]), Result<ChargeMode, i16>>,
    /// Local generated-fixture deadlines. These are not recovered NP lifetimes.
    pub token_expiry: HashMap<[u8; 16], Instant>,
    /// True: a new login of an account takes the presence of the old login.
    /// ❓ The original PresenceSrv kicks the old login. Local policy for `lobbyd`.
    pub presence_replace: bool,
    /// Result of a world entry that has no fixture. `None` gives reason 2.
    /// ❓ Local policy for `lobbyd`: the charge services are not ported.
    pub world_default: Option<ChargeMode>,
    presence: Mutex<Presence>,
}
impl StubNp {
    /// All identifiers and text are generated locally. Unknown tokens are rejected.
    pub fn fixture(seed: u8) -> (Self, [u8; 16], [u8; 16], Identity) {
        let token = [seed.max(1); 16];
        let portal = [seed.wrapping_add(1).max(1); 16];
        let identity = Identity {
            account: [seed.wrapping_add(2).max(1); 16],
            login_name: format!("generated-{seed}").encode_utf16().collect(),
            user_name: format!("generated-user-{seed}").encode_utf16().collect(),
            slot_id: u64::from(seed) + 1,
            profile: vec![],
            grades: vec![],
        };
        let mut stub = Self::default();
        stub.identities
            .insert((token, portal), Ok(identity.clone()));
        (stub, token, portal, identity)
    }
}
impl Np for StubNp {
    fn create_initial_user_name(&self, account: AccountId, name: Vec<u16>) -> GameFuture<'_, ()> {
        Box::pin(async move {
            tokio::time::sleep(self.delay).await;
            match self.initial_names.get(&(account, name)) {
                Some(Ok(())) => Ok(()),
                Some(Err(code)) => Err(initial_name_reason(*code)),
                None => Err(7),
            }
        })
    }
    fn world_entry(
        &self,
        account: AccountId,
        pcid: u64,
        slot: [u8; 16],
        _ipv4: u32,
    ) -> GameFuture<'_, ChargeMode> {
        Box::pin(async move {
            tokio::time::sleep(self.world_delay).await;
            self.world_results
                .get(&(account, pcid, slot))
                .copied()
                .unwrap_or(self.world_default.ok_or(2))
        })
    }
    fn consume(&self, token: [u8; 16], portal: [u8; 16]) -> NpFuture<'_, Identity> {
        Box::pin(async move {
            tokio::time::sleep(self.delay).await;
            self.expire();
            let identity = self
                .identities
                .get(&(token, portal))
                .cloned()
                .unwrap_or(Err(NpError::RejectedToken))?;
            if !self.presence.lock().unwrap().consumed.insert(token) {
                return Err(NpError::RejectedToken);
            }
            if identity.account == [0; 16]
                || identity.login_name.is_empty()
                || identity.user_name.is_empty()
            {
                return Err(NpError::MissingData);
            }
            Ok(identity)
        })
    }
    fn login_presence(&self, account: AccountId, owner: u64) -> NpFuture<'_, ()> {
        Box::pin(async move {
            tokio::time::sleep(self.presence_delay).await;
            if let Some(error) = self.presence_error {
                return Err(error);
            }
            let mut presence = self.presence.lock().unwrap();
            if !self.presence_replace
                && presence
                    .owners
                    .get(&account)
                    .is_some_and(|old| *old != owner)
            {
                return Err(NpError::PresenceConflict);
            }
            presence.owners.insert(account, owner);
            Ok(())
        })
    }
    fn logout_presence(&self, account: AccountId, owner: u64) {
        let mut presence = self.presence.lock().unwrap();
        if presence.owners.get(&account) == Some(&owner) {
            presence.owners.remove(&account);
        }
    }
    fn expire(&self) {
        let now = Instant::now();
        let mut presence = self.presence.lock().unwrap();
        for (token, deadline) in &self.token_expiry {
            if *deadline <= now {
                presence.consumed.insert(*token);
            }
        }
    }
}
