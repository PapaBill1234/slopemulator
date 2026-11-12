//! Local persistence boundary. No SQL connection or sample data is used.
use crate::character::InternalState;
use std::{
    collections::HashMap,
    future::Future,
    io,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
mod characters;

pub use shared::np_channel::AccountId;
pub const RESULT_BLOB_CAPACITY: usize = 512;

/// One character row. The store owns the lifecycle (`state`, `deletion_time`). A job
/// reads it with the character list and writes it back through the store procedures,
/// so each LobbyGate link sees the same lifecycle (`11-character-select.md` §15).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Character {
    pub pcid: u64,
    pub slot: [u8; 16],
    pub world: i16,
    pub name: Vec<u16>,
    pub showcase: Vec<u8>,
    /// End of the deletion wait (wall clock). `Some` only in `InternalState::Waiting`.
    pub deletion_time: Option<SystemTime>,
    pub state: InternalState,
    pub jumping_state: i8,
    /// Wire field. In a deletion wait the jobs set it to the remaining seconds.
    pub last_play_end: u32,
}
/// One row of `usp_RetrieveExpiredDeletionWaitingList`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExpiredDeletion {
    pub account: AccountId,
    pub world: i16,
    pub pcid: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IncompleteService {
    pub id: u64,
    pub kind: i8,
    pub item_instance: u64,
    pub item_data: u32,
    pub state: i8,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    pub id: AccountId,
    pub name: Vec<u16>,
    pub right: i8,
    pub representative_pcid: u64,
    pub representative_world: i16,
    pub bot_score: i32,
}

/// Typed local fixtures, not a claim that the original wrapper returns every field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceKind {
    SlotAddition,
    Cosmesis,
    Rename,
    Sex,
    Race,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServiceDetail {
    SlotAddition {
        slot: [u8; 16],
    },
    Cosmesis {
        pcid: u64,
        appearance: Vec<u8>,
    },
    Rename {
        pcid: u64,
        old_name: Vec<u16>,
        new_name: Vec<u16>,
    },
    Sex {
        pcid: u64,
        sex: u16,
        appearance: Vec<u8>,
    },
    Race {
        pcid: u64,
        race: u16,
        sex: u16,
        appearance: Vec<u8>,
    },
}
impl ServiceDetail {
    pub fn kind(&self) -> ServiceKind {
        match self {
            Self::SlotAddition { .. } => ServiceKind::SlotAddition,
            Self::Cosmesis { .. } => ServiceKind::Cosmesis,
            Self::Rename { .. } => ServiceKind::Rename,
            Self::Sex { .. } => ServiceKind::Sex,
            Self::Race { .. } => ServiceKind::Race,
        }
    }
    fn validate(&self) -> io::Result<()> {
        match self {
            Self::Cosmesis { appearance, .. }
            | Self::Sex { appearance, .. }
            | Self::Race { appearance, .. } => check_blob(appearance),
            Self::Rename {
                old_name, new_name, ..
            } if old_name.len() > 32 || new_name.len() > 32 => {
                Err(invalid_data("service name exceeds capacity"))
            }
            _ => Ok(()),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountExp {
    pub exp: i64,
    pub quota_per_day: i32,
    pub last_update: i64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JumpingCreationQuota {
    pub quota: i32,
    pub count: i32,
}
/// Preserve SQL timestamp components. No epoch or time-zone conversion is assumed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqlTimestamp {
    pub year: i16,
    pub month: u16,
    pub day: u16,
    pub hour: u16,
    pub minute: u16,
    pub second: u16,
    pub fraction: u32,
}
impl SqlTimestamp {
    /// The wall-clock time of the components. ❓ The components are read as UTC. LobbyDB
    /// writes `GETDATE()` (server local time), so the SQL Server host must run in UTC.
    /// `None`: the components are not a valid date.
    pub fn to_system_time(&self) -> Option<SystemTime> {
        let (month, day) = (i64::from(self.month), i64::from(self.day));
        if !(1..=12).contains(&month)
            || !(1..=31).contains(&day)
            || self.hour > 23
            || self.minute > 59
            || self.second > 59
            || self.fraction >= 1_000_000_000
        {
            return None;
        }
        // Days from 1970-01-01 (civil calendar, proleptic Gregorian).
        let year = i64::from(self.year) - i64::from(month <= 2);
        let era = year.div_euclid(400);
        let year_of_era = year.rem_euclid(400);
        let shifted_month = (month + 9) % 12;
        let day_of_year = (153 * shifted_month + 2) / 5 + day - 1;
        let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
        let days = era * 146_097 + day_of_era - 719_468;
        let seconds = days * 86_400
            + i64::from(self.hour) * 3600
            + i64::from(self.minute) * 60
            + i64::from(self.second);
        let time = if seconds >= 0 {
            UNIX_EPOCH.checked_add(Duration::from_secs(seconds.unsigned_abs()))?
        } else {
            UNIX_EPOCH.checked_sub(Duration::from_secs(seconds.unsigned_abs()))?
        };
        time.checked_add(Duration::from_nanos(self.fraction.into()))
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimeBasedPlayInfo {
    pub schedule: i32,
    pub target_type: u8,
    pub expire: Option<SqlTimestamp>,
    pub duration: i64,
    pub last_update: Option<SqlTimestamp>,
}
pub use packet::gamed_link::{ContentQuota, RecycleTime};
impl SqlTimestamp {
    /// The components of a Unix time in seconds (UTC). `None`: out of the SQL year range.
    pub fn from_unix(seconds: i64) -> Option<Self> {
        let days = seconds.div_euclid(86_400);
        let rest = seconds.rem_euclid(86_400);
        // Civil date from days since 1970-01-01 (proleptic Gregorian).
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let day_of_era = z.rem_euclid(146_097);
        let year_of_era =
            (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
        let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
        let shifted_month = (5 * day_of_year + 2) / 153;
        let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
        let month = if shifted_month < 10 {
            shifted_month + 3
        } else {
            shifted_month - 9
        };
        let year = year_of_era + era * 400 + i64::from(month <= 2);
        if !(1753..=9999).contains(&year) {
            return None;
        }
        Some(Self {
            year: i16::try_from(year).ok()?,
            month: u16::try_from(month).ok()?,
            day: u16::try_from(day).ok()?,
            hour: u16::try_from(rest / 3600).ok()?,
            minute: u16::try_from(rest % 3600 / 60).ok()?,
            second: u16::try_from(rest % 60).ok()?,
            fraction: 0,
        })
    }
    /// Unix seconds (UTC) of the components. `None`: not a valid date.
    pub fn to_unix(&self) -> Option<i64> {
        let time = self.to_system_time()?;
        match time.duration_since(UNIX_EPOCH) {
            Ok(after) => i64::try_from(after.as_secs()).ok(),
            Err(before) => i64::try_from(before.duration().as_secs()).ok().map(|s| -s),
        }
    }
}
fn unsupported() -> io::Error {
    io::Error::new(io::ErrorKind::Unsupported, "Store operation unsupported")
}
fn absent() -> io::Error {
    io::Error::new(io::ErrorKind::NotFound, "explicit Store fixture absent")
}
fn invalid_data(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn check_blob(bytes: &[u8]) -> io::Result<()> {
    if bytes.len() > RESULT_BLOB_CAPACITY {
        Err(invalid_data("result blob exceeds 512 bytes"))
    } else {
        Ok(())
    }
}

/// Updates are best-effort in Jobs. Required load failures block fresh login.
/// Optional methods reject by default. They do not enable optional login features.
/// Synchronous model of the storage. Use it for in-memory stores only.
pub trait SyncStore: Send + 'static {
    fn check_character_name(&mut self, _name: &[u16]) -> io::Result<()> {
        Err(unsupported())
    }
    fn try_character_creation(
        &mut self,
        _request: crate::character::CreateCharacter,
    ) -> io::Result<u64> {
        Err(unsupported())
    }
    fn finish_character_creation(
        &mut self,
        _pcid: u64,
        _created: crate::character::CreatedCharacter,
    ) -> io::Result<()> {
        Err(unsupported())
    }
    fn cancel_character_creation(&mut self, _pcid: u64) -> io::Result<()> {
        Err(unsupported())
    }
    fn try_character_deletion(&mut self, _pcid: u64) -> io::Result<()> {
        Err(unsupported())
    }
    fn finish_character_deletion(
        &mut self,
        _pcid: u64,
        _seconds: u32,
    ) -> io::Result<Option<SqlTimestamp>> {
        Err(unsupported())
    }
    fn cancel_character_deletion(&mut self, _pcid: u64) -> io::Result<()> {
        Err(unsupported())
    }
    fn try_character_restoration(&mut self, _pcid: u64) -> io::Result<()> {
        Err(unsupported())
    }
    /// At most `max` characters in deletion wait whose time has passed.
    fn retrieve_expired_deletion_waiting_list(
        &mut self,
        _max: u32,
    ) -> io::Result<Vec<ExpiredDeletion>> {
        Err(unsupported())
    }
    fn expire_character_deletion_waiting(&mut self, _pcid: u64) -> io::Result<()> {
        Err(unsupported())
    }
    fn load_service_detail(&mut self, _id: u64, _kind: ServiceKind) -> io::Result<ServiceDetail> {
        Err(unsupported())
    }
    fn load_account_exp(&mut self, _id: AccountId) -> io::Result<Option<AccountExp>> {
        Err(unsupported())
    }
    fn insert_account_exp(&mut self, _id: AccountId, _row: AccountExp) -> io::Result<()> {
        Err(unsupported())
    }
    fn load_time_based_play_info(&mut self, _id: AccountId) -> io::Result<Vec<TimeBasedPlayInfo>> {
        Err(unsupported())
    }
    fn load_jumping_creation_quota(
        &mut self,
        _id: AccountId,
    ) -> io::Result<Option<JumpingCreationQuota>> {
        Err(unsupported())
    }
    fn finish_character_restoration(&mut self, _pcid: u64) -> io::Result<()> {
        Err(unsupported())
    }
    fn update_last_play_start(&mut self, _pcid: u64, _address: u32) -> io::Result<()> {
        Err(unsupported())
    }
    fn update_last_play_end(&mut self, _pcid: u64) -> io::Result<()> {
        Err(unsupported())
    }
    fn update_character_data(
        &mut self,
        _pcid: u64,
        _level: i16,
        _showcase: Vec<u8>,
    ) -> io::Result<()> {
        Err(unsupported())
    }
    fn update_character_level(&mut self, _pcid: u64, _level: i16) -> io::Result<()> {
        Err(unsupported())
    }
    fn update_time_based_play_info(
        &mut self,
        _id: AccountId,
        _rows: Vec<TimeBasedPlayInfo>,
    ) -> io::Result<()> {
        Err(unsupported())
    }
    fn update_account_exp(&mut self, _id: AccountId, _row: AccountExp) -> io::Result<()> {
        Err(unsupported())
    }
    fn load_content_quota(&mut self, _id: AccountId) -> io::Result<Vec<ContentQuota>> {
        Err(unsupported())
    }
    fn update_content_quota(&mut self, _id: AccountId, _row: ContentQuota) -> io::Result<()> {
        Err(unsupported())
    }
    fn load_recycle_time(&mut self, _id: AccountId) -> io::Result<Vec<RecycleTime>> {
        Err(unsupported())
    }
    fn update_recycle_time(&mut self, _id: AccountId, _row: RecycleTime) -> io::Result<()> {
        Err(unsupported())
    }
    fn load_random_store_schedule(&mut self) -> io::Result<[u64; 3]> {
        Err(unsupported())
    }
    fn load_game_account(&mut self, id: AccountId) -> io::Result<Option<Account>>;
    fn register_game_account(&mut self, account: Account, ipv4: u32) -> io::Result<()>;
    fn update_last_login(&mut self, id: AccountId, ipv4: u32) -> io::Result<()>;
    fn update_last_logout(&mut self, id: AccountId, bot_score: i32) -> io::Result<()>;
    fn load_character_list(&mut self, id: AccountId) -> io::Result<Vec<Character>>;
    fn load_incomplete_lobby_service(
        &mut self,
        id: AccountId,
    ) -> io::Result<Vec<IncompleteService>>;
}

// Share short synchronous Store calls, never an NP wait.
/// Async storage interface of the jobs. A call does not block a runtime thread.
/// Each implementation is cheap to clone and safe to share between tasks.
pub trait Store: Clone + Send + Sync + 'static {
    fn check_character_name(&self, name: &[u16]) -> impl Future<Output = io::Result<()>> + Send;
    fn try_character_creation(
        &self,
        request: crate::character::CreateCharacter,
    ) -> impl Future<Output = io::Result<u64>> + Send;
    fn finish_character_creation(
        &self,
        pcid: u64,
        created: crate::character::CreatedCharacter,
    ) -> impl Future<Output = io::Result<()>> + Send;
    fn cancel_character_creation(&self, pcid: u64) -> impl Future<Output = io::Result<()>> + Send;
    fn try_character_deletion(&self, pcid: u64) -> impl Future<Output = io::Result<()>> + Send;
    fn finish_character_deletion(
        &self,
        pcid: u64,
        seconds: u32,
    ) -> impl Future<Output = io::Result<Option<SqlTimestamp>>> + Send;
    fn cancel_character_deletion(&self, pcid: u64) -> impl Future<Output = io::Result<()>> + Send;
    fn try_character_restoration(&self, pcid: u64) -> impl Future<Output = io::Result<()>> + Send;
    /// `usp_RetrieveExpiredDeletionWaitingList`: at most `max` rows, `max` above zero.
    fn retrieve_expired_deletion_waiting_list(
        &self,
        max: u32,
    ) -> impl Future<Output = io::Result<Vec<ExpiredDeletion>>> + Send;
    /// `usp_ExpireCharacterDeletionWaiting`: state 4 to 6 when the time has passed.
    fn expire_character_deletion_waiting(
        &self,
        pcid: u64,
    ) -> impl Future<Output = io::Result<()>> + Send;
    fn load_service_detail(
        &self,
        id: u64,
        kind: ServiceKind,
    ) -> impl Future<Output = io::Result<ServiceDetail>> + Send;
    fn load_account_exp(
        &self,
        id: AccountId,
    ) -> impl Future<Output = io::Result<Option<AccountExp>>> + Send;
    fn insert_account_exp(
        &self,
        id: AccountId,
        row: AccountExp,
    ) -> impl Future<Output = io::Result<()>> + Send;
    fn load_time_based_play_info(
        &self,
        id: AccountId,
    ) -> impl Future<Output = io::Result<Vec<TimeBasedPlayInfo>>> + Send;
    fn load_jumping_creation_quota(
        &self,
        id: AccountId,
    ) -> impl Future<Output = io::Result<Option<JumpingCreationQuota>>> + Send;
    fn finish_character_restoration(
        &self,
        pcid: u64,
    ) -> impl Future<Output = io::Result<()>> + Send;
    /// `usp_UpdateLastPlayStart`. The card names `world`; the procedure takes the client IPv4.
    fn update_last_play_start(
        &self,
        pcid: u64,
        address: u32,
    ) -> impl Future<Output = io::Result<()>> + Send;
    /// `usp_UpdateLastPlayEnd`.
    fn update_last_play_end(&self, pcid: u64) -> impl Future<Output = io::Result<()>> + Send;
    /// `usp_UpdateCharacterData`. ❓ The job argument is not in the gamed message.
    fn update_character_data(
        &self,
        pcid: u64,
        level: i16,
        showcase: Vec<u8>,
    ) -> impl Future<Output = io::Result<()>> + Send;
    /// `usp_UpdateCharacterLevel`.
    fn update_character_level(
        &self,
        pcid: u64,
        level: i16,
    ) -> impl Future<Output = io::Result<()>> + Send;
    /// `usp_UpdateGameAccountTimeBasedPlayInfo`, one call for each row.
    fn update_time_based_play_info(
        &self,
        id: AccountId,
        rows: Vec<TimeBasedPlayInfo>,
    ) -> impl Future<Output = io::Result<()>> + Send;
    /// `usp_UpdateGameAccountExp`.
    fn update_account_exp(
        &self,
        id: AccountId,
        row: AccountExp,
    ) -> impl Future<Output = io::Result<()>> + Send;
    /// `usp_LoadGameAccountContentQuota` (381 only).
    fn load_content_quota(
        &self,
        id: AccountId,
    ) -> impl Future<Output = io::Result<Vec<ContentQuota>>> + Send;
    /// `usp_UpdateGameAccountContentQuota` (381 only).
    fn update_content_quota(
        &self,
        id: AccountId,
        row: ContentQuota,
    ) -> impl Future<Output = io::Result<()>> + Send;
    /// `usp_LoadGameAccountRecycleTime` (381 only).
    fn load_recycle_time(
        &self,
        id: AccountId,
    ) -> impl Future<Output = io::Result<Vec<RecycleTime>>> + Send;
    /// `usp_UpdateGameAccountRecycleTime` (381 only).
    fn update_recycle_time(
        &self,
        id: AccountId,
        row: RecycleTime,
    ) -> impl Future<Output = io::Result<()>> + Send;
    /// `usp_LoadRandomStoreInitializationSchedule`: three bigint output values.
    fn load_random_store_schedule(&self) -> impl Future<Output = io::Result<[u64; 3]>> + Send;
    fn load_game_account(
        &self,
        id: AccountId,
    ) -> impl Future<Output = io::Result<Option<Account>>> + Send;
    fn register_game_account(
        &self,
        account: Account,
        ipv4: u32,
    ) -> impl Future<Output = io::Result<()>> + Send;
    fn update_last_login(
        &self,
        id: AccountId,
        ipv4: u32,
    ) -> impl Future<Output = io::Result<()>> + Send;
    fn update_last_logout(
        &self,
        id: AccountId,
        bot_score: i32,
    ) -> impl Future<Output = io::Result<()>> + Send;
    fn load_character_list(
        &self,
        id: AccountId,
    ) -> impl Future<Output = io::Result<Vec<Character>>> + Send;
    fn load_incomplete_lobby_service(
        &self,
        id: AccountId,
    ) -> impl Future<Output = io::Result<Vec<IncompleteService>>> + Send;
}
/// Adapter for an in-memory [`SyncStore`]. The lock is never held across `.await`.
pub struct Shared<S>(std::sync::Arc<std::sync::Mutex<S>>);
impl<S> Shared<S> {
    pub fn new(store: S) -> Self {
        Self(std::sync::Arc::new(std::sync::Mutex::new(store)))
    }
    pub fn lock(&self) -> std::sync::MutexGuard<'_, S> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
impl<S> Clone for Shared<S> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<S: SyncStore> Store for Shared<S> {
    async fn check_character_name(&self, name: &[u16]) -> io::Result<()> {
        self.lock().check_character_name(name)
    }
    async fn try_character_creation(
        &self,
        request: crate::character::CreateCharacter,
    ) -> io::Result<u64> {
        self.lock().try_character_creation(request)
    }
    async fn finish_character_creation(
        &self,
        pcid: u64,
        created: crate::character::CreatedCharacter,
    ) -> io::Result<()> {
        self.lock().finish_character_creation(pcid, created)
    }
    async fn cancel_character_creation(&self, pcid: u64) -> io::Result<()> {
        self.lock().cancel_character_creation(pcid)
    }
    async fn try_character_deletion(&self, pcid: u64) -> io::Result<()> {
        self.lock().try_character_deletion(pcid)
    }
    async fn finish_character_deletion(
        &self,
        pcid: u64,
        seconds: u32,
    ) -> io::Result<Option<SqlTimestamp>> {
        self.lock().finish_character_deletion(pcid, seconds)
    }
    async fn cancel_character_deletion(&self, pcid: u64) -> io::Result<()> {
        self.lock().cancel_character_deletion(pcid)
    }
    async fn try_character_restoration(&self, pcid: u64) -> io::Result<()> {
        self.lock().try_character_restoration(pcid)
    }
    async fn retrieve_expired_deletion_waiting_list(
        &self,
        max: u32,
    ) -> io::Result<Vec<ExpiredDeletion>> {
        self.lock().retrieve_expired_deletion_waiting_list(max)
    }
    async fn expire_character_deletion_waiting(&self, pcid: u64) -> io::Result<()> {
        self.lock().expire_character_deletion_waiting(pcid)
    }
    async fn load_service_detail(&self, id: u64, kind: ServiceKind) -> io::Result<ServiceDetail> {
        self.lock().load_service_detail(id, kind)
    }
    async fn load_account_exp(&self, id: AccountId) -> io::Result<Option<AccountExp>> {
        self.lock().load_account_exp(id)
    }
    async fn insert_account_exp(&self, id: AccountId, row: AccountExp) -> io::Result<()> {
        self.lock().insert_account_exp(id, row)
    }
    async fn load_time_based_play_info(&self, id: AccountId) -> io::Result<Vec<TimeBasedPlayInfo>> {
        self.lock().load_time_based_play_info(id)
    }
    async fn load_jumping_creation_quota(
        &self,
        id: AccountId,
    ) -> io::Result<Option<JumpingCreationQuota>> {
        self.lock().load_jumping_creation_quota(id)
    }
    async fn finish_character_restoration(&self, pcid: u64) -> io::Result<()> {
        self.lock().finish_character_restoration(pcid)
    }
    async fn update_last_play_start(&self, pcid: u64, address: u32) -> io::Result<()> {
        self.lock().update_last_play_start(pcid, address)
    }
    async fn update_last_play_end(&self, pcid: u64) -> io::Result<()> {
        self.lock().update_last_play_end(pcid)
    }
    async fn update_character_data(
        &self,
        pcid: u64,
        level: i16,
        showcase: Vec<u8>,
    ) -> io::Result<()> {
        self.lock().update_character_data(pcid, level, showcase)
    }
    async fn update_character_level(&self, pcid: u64, level: i16) -> io::Result<()> {
        self.lock().update_character_level(pcid, level)
    }
    async fn update_time_based_play_info(
        &self,
        id: AccountId,
        rows: Vec<TimeBasedPlayInfo>,
    ) -> io::Result<()> {
        self.lock().update_time_based_play_info(id, rows)
    }
    async fn update_account_exp(&self, id: AccountId, row: AccountExp) -> io::Result<()> {
        self.lock().update_account_exp(id, row)
    }
    async fn load_content_quota(&self, id: AccountId) -> io::Result<Vec<ContentQuota>> {
        self.lock().load_content_quota(id)
    }
    async fn update_content_quota(&self, id: AccountId, row: ContentQuota) -> io::Result<()> {
        self.lock().update_content_quota(id, row)
    }
    async fn load_recycle_time(&self, id: AccountId) -> io::Result<Vec<RecycleTime>> {
        self.lock().load_recycle_time(id)
    }
    async fn update_recycle_time(&self, id: AccountId, row: RecycleTime) -> io::Result<()> {
        self.lock().update_recycle_time(id, row)
    }
    async fn load_random_store_schedule(&self) -> io::Result<[u64; 3]> {
        self.lock().load_random_store_schedule()
    }
    async fn load_game_account(&self, id: AccountId) -> io::Result<Option<Account>> {
        self.lock().load_game_account(id)
    }
    async fn register_game_account(&self, account: Account, ipv4: u32) -> io::Result<()> {
        self.lock().register_game_account(account, ipv4)
    }
    async fn update_last_login(&self, id: AccountId, ipv4: u32) -> io::Result<()> {
        self.lock().update_last_login(id, ipv4)
    }
    async fn update_last_logout(&self, id: AccountId, bot_score: i32) -> io::Result<()> {
        self.lock().update_last_logout(id, bot_score)
    }
    async fn load_character_list(&self, id: AccountId) -> io::Result<Vec<Character>> {
        self.lock().load_character_list(id)
    }
    async fn load_incomplete_lobby_service(
        &self,
        id: AccountId,
    ) -> io::Result<Vec<IncompleteService>> {
        self.lock().load_incomplete_lobby_service(id)
    }
}

#[derive(Default)]
pub struct MemoryStore {
    pub accounts: HashMap<AccountId, Account>,
    pub character_states: HashMap<u64, crate::character::InternalState>,
    pub limited_names: std::collections::HashSet<Vec<u16>>,
    /// One-shot operation failures for generated tests. Values are procedure returns.
    pub character_failures: HashMap<&'static str, i32>,
    pub restoration_allowed: std::collections::HashSet<u64>,
    pub restored: Vec<u64>,
    pub characters: HashMap<AccountId, Vec<Character>>,
    pub services: HashMap<AccountId, Vec<IncompleteService>>,
    pub service_details: HashMap<u64, ServiceDetail>,
    /// Missing key is unsupported; None is an explicit successful no-row result.
    pub account_exp: HashMap<AccountId, Option<AccountExp>>,
    pub time_based_play_info: HashMap<AccountId, Vec<TimeBasedPlayInfo>>,
    pub jumping_creation_quota: HashMap<AccountId, Option<JumpingCreationQuota>>,
    pub login_updates: Vec<(AccountId, u32)>,
    pub logout_updates: Vec<(AccountId, i32)>,
    /// Content quota and recycle time rows. A missing key is an empty list.
    pub content_quota: HashMap<AccountId, Vec<ContentQuota>>,
    pub recycle_time: HashMap<AccountId, Vec<RecycleTime>>,
    /// Result of `load_random_store_schedule`.
    pub random_store: [u64; 3],
    /// Play start (PC id, client IPv4) and play end (PC id) writes.
    pub play_starts: Vec<(u64, u32)>,
    pub play_ends: Vec<u64>,
    /// `update_character_data` (PC id, level, showcase) and `update_character_level` writes.
    pub character_data: Vec<(u64, i16, Vec<u8>)>,
    pub character_levels: Vec<(u64, i16)>,
    /// Names of the account update calls of `ld381-r1`, in call order. A call that fails by
    /// `character_failures` is not in the log.
    pub write_log: Vec<&'static str>,
    /// Wall clock of this store. `None`: the system clock. Tests set a fixed clock.
    pub clock: Option<fn() -> SystemTime>,
}
impl SyncStore for MemoryStore {
    fn retrieve_expired_deletion_waiting_list(
        &mut self,
        max: u32,
    ) -> io::Result<Vec<ExpiredDeletion>> {
        self.expired_waiting(max)
    }
    fn expire_character_deletion_waiting(&mut self, pcid: u64) -> io::Result<()> {
        self.expire_waiting(pcid)
    }
    fn check_character_name(&mut self, name: &[u16]) -> io::Result<()> {
        self.check_name(name)
    }
    fn try_character_creation(
        &mut self,
        request: crate::character::CreateCharacter,
    ) -> io::Result<u64> {
        self.create(request)
    }
    fn finish_character_creation(
        &mut self,
        pcid: u64,
        created: crate::character::CreatedCharacter,
    ) -> io::Result<()> {
        self.finish_create(pcid, created)
    }
    fn cancel_character_creation(&mut self, pcid: u64) -> io::Result<()> {
        self.cancel_create(pcid)
    }
    fn try_character_deletion(&mut self, pcid: u64) -> io::Result<()> {
        self.transition(
            "try_delete",
            pcid,
            crate::character::InternalState::Normal,
            crate::character::InternalState::Deleting,
        )
    }
    fn finish_character_deletion(
        &mut self,
        pcid: u64,
        seconds: u32,
    ) -> io::Result<Option<SqlTimestamp>> {
        self.finish_delete(pcid, seconds)?;
        Ok(None)
    }
    fn cancel_character_deletion(&mut self, pcid: u64) -> io::Result<()> {
        self.transition(
            "cancel_delete",
            pcid,
            crate::character::InternalState::Deleting,
            crate::character::InternalState::Normal,
        )
    }
    fn try_character_restoration(&mut self, pcid: u64) -> io::Result<()> {
        self.transition(
            "try_restore",
            pcid,
            crate::character::InternalState::Waiting,
            crate::character::InternalState::Restoring,
        )?;
        self.characters
            .values_mut()
            .flatten()
            .filter(|c| c.pcid == pcid)
            .for_each(|c| c.last_play_end = 0);
        Ok(())
    }
    fn load_service_detail(&mut self, id: u64, kind: ServiceKind) -> io::Result<ServiceDetail> {
        let row = self.service_details.get(&id).ok_or_else(absent)?;
        if row.kind() != kind {
            return Err(invalid_data("service detail kind mismatch"));
        }
        row.validate()?;
        Ok(row.clone())
    }
    fn load_account_exp(&mut self, id: AccountId) -> io::Result<Option<AccountExp>> {
        self.account_exp.get(&id).cloned().ok_or_else(absent)
    }
    fn insert_account_exp(&mut self, id: AccountId, row: AccountExp) -> io::Result<()> {
        let fixture = self.account_exp.get_mut(&id).ok_or_else(absent)?;
        if fixture.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "EXP row exists",
            ));
        }
        *fixture = Some(row);
        Ok(())
    }
    fn load_time_based_play_info(&mut self, id: AccountId) -> io::Result<Vec<TimeBasedPlayInfo>> {
        self.time_based_play_info
            .get(&id)
            .cloned()
            .ok_or_else(absent)
    }
    fn load_jumping_creation_quota(
        &mut self,
        id: AccountId,
    ) -> io::Result<Option<JumpingCreationQuota>> {
        self.jumping_creation_quota
            .get(&id)
            .cloned()
            .ok_or_else(absent)
    }
    fn finish_character_restoration(&mut self, pcid: u64) -> io::Result<()> {
        if self.character_states.contains_key(&pcid) {
            self.transition(
                "finish_restore",
                pcid,
                crate::character::InternalState::Restoring,
                crate::character::InternalState::Normal,
            )?;
            self.characters
                .values_mut()
                .flatten()
                .filter(|c| c.pcid == pcid)
                .for_each(|c| c.last_play_end = 0);
            return Ok(());
        }
        if pcid == 0 || self.restored.contains(&pcid) || !self.restoration_allowed.remove(&pcid) {
            return Err(absent());
        }
        for row in self
            .characters
            .values_mut()
            .flatten()
            .filter(|row| row.pcid == pcid)
        {
            row.state = InternalState::Normal;
            row.deletion_time = None;
        }
        self.restored.push(pcid);
        Ok(())
    }
    fn update_last_play_start(&mut self, pcid: u64, address: u32) -> io::Result<()> {
        self.character_failure("update_last_play_start")?;
        self.play_starts.push((pcid, address));
        Ok(())
    }
    fn update_last_play_end(&mut self, pcid: u64) -> io::Result<()> {
        self.character_failure("update_last_play_end")?;
        self.play_ends.push(pcid);
        Ok(())
    }
    fn update_character_data(
        &mut self,
        pcid: u64,
        level: i16,
        showcase: Vec<u8>,
    ) -> io::Result<()> {
        self.character_failure("update_character_data")?;
        check_blob(&showcase)?;
        if level <= 0 {
            return Err(crate::character::procedure_error(1));
        }
        for row in self
            .characters
            .values_mut()
            .flatten()
            .filter(|row| row.pcid == pcid)
        {
            row.showcase = showcase.clone();
        }
        self.character_data.push((pcid, level, showcase));
        self.write_log.push("update_character_data");
        Ok(())
    }
    fn update_character_level(&mut self, pcid: u64, level: i16) -> io::Result<()> {
        self.character_failure("update_character_level")?;
        if level <= 0 {
            return Err(crate::character::procedure_error(1));
        }
        self.character_levels.push((pcid, level));
        self.write_log.push("update_character_level");
        Ok(())
    }
    fn update_time_based_play_info(
        &mut self,
        id: AccountId,
        rows: Vec<TimeBasedPlayInfo>,
    ) -> io::Result<()> {
        self.character_failure("update_time_based_play_info")?;
        self.time_based_play_info.insert(id, rows);
        self.write_log.push("update_time_based_play_info");
        Ok(())
    }
    fn update_account_exp(&mut self, id: AccountId, row: AccountExp) -> io::Result<()> {
        self.character_failure("update_account_exp")?;
        // As the procedure: no row is an error, a negative value is an error.
        if row.exp < 0 || row.quota_per_day < 0 || row.last_update < 0 {
            return Err(crate::character::procedure_error(1));
        }
        match self.account_exp.get_mut(&id) {
            Some(Some(slot)) => *slot = row,
            _ => return Err(crate::character::procedure_error(1)),
        }
        self.write_log.push("update_account_exp");
        Ok(())
    }
    fn load_content_quota(&mut self, id: AccountId) -> io::Result<Vec<ContentQuota>> {
        Ok(self.content_quota.get(&id).cloned().unwrap_or_default())
    }
    fn update_content_quota(&mut self, id: AccountId, row: ContentQuota) -> io::Result<()> {
        self.character_failure("update_content_quota")?;
        let rows = self.content_quota.entry(id).or_default();
        match rows.iter_mut().find(|r| r.data_id == row.data_id) {
            Some(slot) => *slot = row,
            None => rows.push(row),
        }
        self.write_log.push("update_content_quota");
        Ok(())
    }
    fn load_recycle_time(&mut self, id: AccountId) -> io::Result<Vec<RecycleTime>> {
        Ok(self.recycle_time.get(&id).cloned().unwrap_or_default())
    }
    fn update_recycle_time(&mut self, id: AccountId, row: RecycleTime) -> io::Result<()> {
        self.character_failure("update_recycle_time")?;
        let rows = self.recycle_time.entry(id).or_default();
        // As the procedure (MERGE): the later time stays.
        match rows.iter_mut().find(|r| r.group == row.group) {
            Some(slot) => slot.time = slot.time.max(row.time),
            None => rows.push(row),
        }
        self.write_log.push("update_recycle_time");
        Ok(())
    }
    fn load_random_store_schedule(&mut self) -> io::Result<[u64; 3]> {
        Ok(self.random_store)
    }
    fn load_game_account(&mut self, id: AccountId) -> io::Result<Option<Account>> {
        Ok(self.accounts.get(&id).cloned())
    }
    fn register_game_account(&mut self, account: Account, ipv4: u32) -> io::Result<()> {
        if self.accounts.contains_key(&account.id) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "account exists",
            ));
        }
        self.login_updates.push((account.id, ipv4));
        self.accounts.insert(account.id, account);
        Ok(())
    }
    fn update_last_login(&mut self, id: AccountId, ipv4: u32) -> io::Result<()> {
        if !self.accounts.contains_key(&id) {
            return Err(absent());
        }
        self.login_updates.push((id, ipv4));
        Ok(())
    }
    fn update_last_logout(&mut self, id: AccountId, bot_score: i32) -> io::Result<()> {
        let account = self.accounts.get_mut(&id).ok_or_else(absent)?;
        account.bot_score = bot_score;
        self.logout_updates.push((id, bot_score));
        Ok(())
    }
    fn load_character_list(&mut self, id: AccountId) -> io::Result<Vec<Character>> {
        let rows = self.characters.get(&id).cloned().unwrap_or_default();
        for row in &rows {
            check_blob(&row.showcase)?;
        }
        Ok(rows)
    }
    fn load_incomplete_lobby_service(
        &mut self,
        id: AccountId,
    ) -> io::Result<Vec<IncompleteService>> {
        Ok(self.services.get(&id).cloned().unwrap_or_default())
    }
}
