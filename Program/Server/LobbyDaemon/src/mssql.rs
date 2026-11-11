//! SQL Server adapter for the 16 procedures exposed by Store.
//!
//! All batches are static. Query binds values through Tiberius' sp_executesql RPC.
//! No input becomes SQL text. This is not a direct named-procedure TDS RPC.
//! Calls run on the runtime of the caller through a connection pool.
//! There are no automatic retries after ambiguous writes.
use crate::store::*;
use shared::pool::Pool;
use std::{fmt, io, sync::Arc, time::Duration};
use tiberius::{time::chrono::NaiveDateTime, Client, Config, FromSql, Query, Row, Uuid};
use tokio::net::TcpStream;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

/// Default number of connections to LobbyDB.
pub const POOL_SIZE: usize = 8;
const TIMEOUT: Duration = Duration::from_secs(10);
const RETURN_COLUMN: &str = "__lobbyd_return";

/// SQL messages are deliberately omitted. They can contain account data.
#[derive(Debug, PartialEq, Eq)]
pub enum SqlStoreError {
    Procedure(i32),
    Driver,
    Timeout,
}
impl fmt::Display for SqlStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Procedure(code) => write!(f, "LobbyDB procedure returned {code}"),
            Self::Driver => f.write_str("LobbyDB driver operation failed"),
            Self::Timeout => f.write_str("LobbyDB operation timed out; outcome can be unknown"),
        }
    }
}
impl std::error::Error for SqlStoreError {}
fn driver(_: impl fmt::Display) -> io::Error {
    io::Error::other(SqlStoreError::Driver)
}
fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
type Connection = Client<Compat<TcpStream>>;
/// Connection settings are accepted only from LOBBYD_MSSQL_URL at runtime.
/// Calls run on the runtime of the caller. A failed, timed-out or cancelled
/// call drops its connection. The failed request is not sent again.
#[derive(Clone)]
pub struct MssqlStore {
    inner: Arc<Inner>,
}
struct Inner {
    config: Config,
    pool: Pool<Connection>,
}
impl MssqlStore {
    /// Pool size: env `LOBBYD_DB_POOL`, default [`POOL_SIZE`].
    pub async fn from_env() -> io::Result<Self> {
        let url = std::env::var("LOBBYD_MSSQL_URL")
            .map_err(|_| io::Error::new(io::ErrorKind::NotFound, "LOBBYD_MSSQL_URL is not set"))?;
        // Do not accept the driver's implicit server default.
        if !url
            .split(';')
            .filter_map(|part| part.split_once('='))
            .any(|(key, value)| {
                matches!(
                    key.trim().to_ascii_lowercase().as_str(),
                    "server" | "data source"
                ) && !value.trim().is_empty()
            })
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "ADO settings need an explicit server",
            ));
        }
        let config = Config::from_ado_string(&url).map_err(driver)?;
        let size = std::env::var("LOBBYD_DB_POOL")
            .ok()
            .and_then(|value| value.parse().ok())
            .filter(|size| *size > 0)
            .unwrap_or(POOL_SIZE);
        let store = Self {
            inner: Arc::new(Inner {
                config,
                pool: Pool::new(size),
            }),
        };
        // Fail at startup when the database does not answer.
        let lease = store.inner.pool.acquire().await;
        let connection = store.connect().await?;
        lease.give(connection);
        Ok(store)
    }
    async fn connect(&self) -> io::Result<Connection> {
        let config = self.inner.config.clone();
        let tcp = TcpStream::connect(config.get_addr())
            .await
            .map_err(driver)?;
        tcp.set_nodelay(true).map_err(driver)?;
        Client::connect(config, tcp.compat_write())
            .await
            .map_err(driver)
    }
    async fn call(&self, query: Query<'static>) -> io::Result<Vec<Row>> {
        let mut lease = self.inner.pool.acquire().await;
        let sets = tokio::time::timeout(TIMEOUT, async move {
            let mut connection = match lease.take() {
                Some(connection) => connection,
                None => self.connect().await?,
            };
            let sets = query
                .query(&mut connection)
                .await
                .map_err(driver)?
                .into_results()
                .await
                .map_err(driver)?;
            lease.give(connection);
            Ok::<_, io::Error>(sets)
        })
        .await
        .map_err(|_| io::Error::other(SqlStoreError::Timeout))??;
        check_return(sets)
    }
}
fn check_return(mut sets: Vec<Vec<Row>>) -> io::Result<Vec<Row>> {
    let status = sets
        .pop()
        .ok_or_else(|| invalid("missing procedure return"))?;
    if status.len() != 1
        || status[0].columns().len() != 1
        || status[0].columns()[0].name() != RETURN_COLUMN
    {
        return Err(invalid("invalid procedure return result"));
    }
    let code: i32 = required(&status[0], 0)?;
    if code != 0 {
        return Err(io::Error::other(SqlStoreError::Procedure(code)));
    }
    if sets.len() > 1 {
        return Err(invalid("unexpected procedure result sets"));
    }
    Ok(sets.pop().unwrap_or_default())
}
macro_rules! rpc {
    ($procedure:literal, $args:literal) => {
        Query::new(concat!(
            "DECLARE @rc int; EXEC @rc = dbo.",
            $procedure,
            " ",
            $args,
            "; SELECT @rc AS __lobbyd_return;"
        ))
    };
}
fn required<'a, T: FromSql<'a>>(row: &'a Row, index: usize) -> io::Result<T> {
    row.try_get(index)
        .map_err(driver)?
        .ok_or_else(|| invalid("required result column is NULL"))
}
fn optional<'a, T: FromSql<'a>>(row: &'a Row, index: usize) -> io::Result<Option<T>> {
    row.try_get(index).map_err(driver)
}
fn guid(id: AccountId) -> Uuid {
    Uuid::from_bytes_le(id)
}
fn id(row: &Row, index: usize) -> io::Result<AccountId> {
    Ok(required::<Uuid>(row, index)?.to_bytes_le())
}
fn positive(value: u64) -> io::Result<i64> {
    i64::try_from(value)
        .ok()
        .filter(|v| *v > 0)
        .ok_or_else(|| invalid("ID is outside positive SQL bigint range"))
}
fn unsigned(row: &Row, index: usize) -> io::Result<u64> {
    u64::try_from(required::<i64>(row, index)?).map_err(|_| invalid("negative SQL identifier"))
}
fn byte(row: &Row, index: usize) -> io::Result<i8> {
    i8::try_from(required::<u8>(row, index)?)
        .map_err(|_| invalid("SQL tinyint exceeds signed byte"))
}
fn name(row: &Row, index: usize, capacity: usize) -> io::Result<Vec<u16>> {
    let text: Option<&str> = optional(row, index)?;
    let units: Vec<u16> = text.unwrap_or_default().encode_utf16().collect();
    if units.len() > capacity {
        return Err(invalid("result name exceeds capacity"));
    }
    Ok(units)
}
fn blob(row: &Row, index: usize) -> io::Result<Vec<u8>> {
    let value: Option<&[u8]> = optional(row, index)?;
    let value = value.unwrap_or_default();
    if value.len() > RESULT_BLOB_CAPACITY {
        return Err(invalid("result blob exceeds 512 bytes"));
    }
    Ok(value.to_vec())
}
fn one(rows: Vec<Row>) -> io::Result<Option<Row>> {
    if rows.len() > 1 {
        return Err(invalid("expected at most one result row"));
    }
    Ok(rows.into_iter().next())
}
fn no_rows(rows: Vec<Row>) -> io::Result<()> {
    if !rows.is_empty() {
        return Err(invalid("unexpected procedure rows"));
    }
    Ok(())
}
fn character_name(name: &[u16]) -> io::Result<String> {
    if name.is_empty() || name.len() > 32 || name.contains(&0) {
        return Err(invalid("character name exceeds bounds"));
    }
    String::from_utf16(name).map_err(driver)
}
fn ip_text(ipv4: u32) -> String {
    std::net::Ipv4Addr::from(ipv4.to_le_bytes()).to_string()
}
// Character timestamp units are not yet specified. Reject rather than invent them.
fn null_character_time(row: &Row, index: usize) -> io::Result<()> {
    if optional::<NaiveDateTime>(row, index)?.is_some() {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "non-NULL character timestamp conversion is not specified",
        ));
    }
    Ok(())
}
fn timestamp(row: &Row, index: usize) -> io::Result<Option<SqlTimestamp>> {
    let Some(value) = optional::<NaiveDateTime>(row, index)? else {
        return Ok(None);
    };
    // Chrono traits are not a direct dependency. Display preserves SQL components.
    let text = value.to_string();
    let (whole, fraction) = text.split_once('.').unwrap_or((&text, ""));
    let p: Vec<i32> = whole
        .split(['-', ' ', ':'])
        .map(str::parse)
        .collect::<Result<_, _>>()
        .map_err(driver)?;
    if p.len() != 6 || fraction.len() > 9 {
        return Err(invalid("invalid SQL timestamp components"));
    }
    let nanos = if fraction.is_empty() {
        0
    } else {
        fraction.parse::<u32>().map_err(driver)? * 10u32.pow(9 - fraction.len() as u32)
    };
    Ok(Some(SqlTimestamp {
        year: p[0] as i16,
        month: p[1] as u16,
        day: p[2] as u16,
        hour: p[3] as u16,
        minute: p[4] as u16,
        second: p[5] as u16,
        fraction: nanos,
    }))
}
impl Store for MssqlStore {
    async fn check_character_name(&self, name: &[u16]) -> io::Result<()> {
        let mut q = rpc!("usp_CheckValidityOfCharacterName", "@P1");
        q.bind(character_name(name)?);
        no_rows(self.call(q).await?)
    }
    async fn try_character_creation(
        &self,
        request: crate::character::CreateCharacter,
    ) -> io::Result<u64> {
        if request.account == [0; 16]
            || request.slot == [0; 16]
            || request.world <= 0
            || request.right != 1
        {
            return Err(invalid("invalid local character creation"));
        }
        let mut q=Query::new("DECLARE @rc int, @pcid bigint = 0; EXEC @rc = dbo.usp_TryCharacterCreation @P1, @P2, @P3, @P4, @P5, @pcid OUTPUT; SELECT @pcid AS PCID; SELECT @rc AS __lobbyd_return;");
        q.bind(guid(request.account));
        q.bind(guid(request.slot));
        q.bind(request.world);
        q.bind(character_name(&request.name)?);
        q.bind(request.right);
        let row = one(self.call(q).await?)?.ok_or_else(|| invalid("missing created PCID"))?;
        let pcid = unsigned(&row, 0)?;
        positive(pcid)?;
        Ok(pcid)
    }
    async fn finish_character_creation(
        &self,
        pcid: u64,
        created: crate::character::CreatedCharacter,
    ) -> io::Result<()> {
        if created.job <= 0
            || created.level <= 0
            || created.showcase.is_empty()
            || created.showcase.len() > 512
        {
            return Err(invalid("invalid character creation output"));
        }
        let mut q = rpc!(
            "usp_FinishCharacterCreation",
            "@P1, @P2, @P3, @P4, @P5, @P6"
        );
        q.bind(positive(pcid)?);
        q.bind(created.job);
        q.bind(created.level);
        q.bind(0u8);
        q.bind(false);
        q.bind(created.showcase);
        no_rows(self.call(q).await?)
    }
    async fn cancel_character_creation(&self, pcid: u64) -> io::Result<()> {
        let mut q = rpc!("usp_CancelCharacterCreation", "@P1");
        q.bind(positive(pcid)?);
        no_rows(self.call(q).await?)
    }
    async fn try_character_deletion(&self, pcid: u64) -> io::Result<()> {
        let mut q = rpc!("usp_TryCharacterDeletion", "@P1");
        q.bind(positive(pcid)?);
        no_rows(self.call(q).await?)
    }
    async fn finish_character_deletion(
        &self,
        pcid: u64,
        seconds: u32,
    ) -> io::Result<Option<SqlTimestamp>> {
        let mut q=Query::new("DECLARE @rc int, @deleted datetime; EXEC @rc = dbo.usp_FinishCharacterDeletion @P1, @P2, @deleted OUTPUT; SELECT @deleted AS DeletionDelayTime; SELECT @rc AS __lobbyd_return;");
        q.bind(positive(pcid)?);
        q.bind(i32::try_from(seconds).map_err(|_| invalid("deletion duration exceeds SQL int"))?);
        let row = one(self.call(q).await?)?
            .ok_or_else(|| invalid("missing deletion timestamp output"))?;
        let time = timestamp(&row, 0)?;
        if (seconds == 0) != time.is_none() {
            return Err(invalid("unexpected deletion timestamp NULL"));
        }
        Ok(time)
    }
    async fn cancel_character_deletion(&self, pcid: u64) -> io::Result<()> {
        let mut q = rpc!("usp_CancelCharacterDeletion", "@P1");
        q.bind(positive(pcid)?);
        no_rows(self.call(q).await?)
    }
    async fn try_character_restoration(&self, pcid: u64) -> io::Result<()> {
        let mut q = rpc!("usp_TryCharacterRestoration", "@P1");
        q.bind(positive(pcid)?);
        no_rows(self.call(q).await?)
    }
    async fn load_game_account(&self, account: AccountId) -> io::Result<Option<Account>> {
        let mut q = rpc!("usp_LoadGameAccount", "@P1");
        q.bind(guid(account));
        let Some(row) = one(self.call(q).await?)? else {
            return Ok(None);
        };
        let pcid = optional::<i64>(&row, 3)?.unwrap_or(0);
        if required::<bool>(&row, 5)? {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "BotScoreIgnore is not represented by the local account",
            ));
        }
        Ok(Some(Account {
            id: id(&row, 0)?,
            name: name(&row, 1, 64)?,
            right: byte(&row, 2)?,
            representative_pcid: u64::try_from(pcid)
                .map_err(|_| invalid("negative representative PCID"))?,
            representative_world: 0,
            bot_score: required(&row, 4)?,
        }))
    }
    async fn register_game_account(&self, account: Account, ipv4: u32) -> io::Result<()> {
        if account.name.len() > 64 {
            return Err(invalid("account name exceeds capacity"));
        }
        let text = String::from_utf16(&account.name).map_err(driver)?;
        let right = u8::try_from(account.right).map_err(|_| invalid("negative account right"))?;
        let mut q = rpc!("usp_RegisterGameAccount", "@P1, @P2, @P3, @P4");
        q.bind(guid(account.id));
        q.bind(text);
        q.bind(right);
        q.bind(ip_text(ipv4));
        no_rows(self.call(q).await?)
    }
    async fn update_last_login(&self, account: AccountId, ipv4: u32) -> io::Result<()> {
        let mut q = rpc!("usp_UpdateLastLogin", "@P1, @P2");
        q.bind(guid(account));
        q.bind(ip_text(ipv4));
        no_rows(self.call(q).await?)
    }
    async fn update_last_logout(&self, account: AccountId, bot_score: i32) -> io::Result<()> {
        let mut q = rpc!("usp_UpdateLastLogout", "@P1, @P2");
        q.bind(guid(account));
        q.bind(bot_score);
        no_rows(self.call(q).await?)
    }
    async fn load_character_list(&self, account: AccountId) -> io::Result<Vec<Character>> {
        let mut q = rpc!("usp_LoadCharacterList", "@P1");
        q.bind(guid(account));
        self.call(q)
            .await?
            .iter()
            .map(|row| {
                null_character_time(row, 9)?;
                null_character_time(row, 10)?;
                // CharacterRight/Blocked cannot be represented by the local cache.
                // Refuse restricted rows instead of dropping their restrictions.
                if required::<u8>(row, 7)? != 1 || required::<bool>(row, 13)? {
                    return Err(io::Error::new(
                        io::ErrorKind::Unsupported,
                        "restricted character is not supported by local cache",
                    ));
                }
                Ok(Character {
                    pcid: unsigned(row, 0)?,
                    slot: optional::<Uuid>(row, 2)?
                        .map(|v| v.to_bytes_le())
                        .unwrap_or_default(),
                    world: required(row, 3)?,
                    name: name(row, 6, 32)?,
                    showcase: blob(row, 12)?,
                    deletion_time: 0,
                    state: byte(row, 8)?,
                    jumping_state: i8::try_from(optional::<u8>(row, 11)?.unwrap_or(0))
                        .map_err(|_| invalid("jumping state exceeds signed byte"))?,
                    last_play_end: 0,
                })
            })
            .collect()
    }
    async fn load_incomplete_lobby_service(
        &self,
        account: AccountId,
    ) -> io::Result<Vec<IncompleteService>> {
        let mut q = rpc!("usp_LoadIncompleteLobbyService", "@P1");
        q.bind(guid(account));
        self.call(q)
            .await?
            .iter()
            .map(|row| {
                Ok(IncompleteService {
                    id: unsigned(row, 0)?,
                    kind: byte(row, 2)?,
                    item_instance: unsigned(row, 3)?,
                    item_data: u32::try_from(required::<i32>(row, 4)?)
                        .map_err(|_| invalid("negative item data ID"))?,
                    state: byte(row, 5)?,
                })
            })
            .collect()
    }
    async fn finish_character_restoration(&self, pcid: u64) -> io::Result<()> {
        let mut q = rpc!("usp_FinishCharacterRestoration", "@P1");
        q.bind(positive(pcid)?);
        no_rows(self.call(q).await?)
    }
    async fn load_account_exp(&self, account: AccountId) -> io::Result<Option<AccountExp>> {
        let mut q = rpc!("usp_LoadGameAccountExp", "@P1");
        q.bind(guid(account));
        one(self.call(q).await?)?
            .map(|row| {
                Ok(AccountExp {
                    exp: required(&row, 1)?,
                    quota_per_day: required(&row, 2)?,
                    last_update: required(&row, 3)?,
                })
            })
            .transpose()
    }
    async fn insert_account_exp(&self, account: AccountId, row: AccountExp) -> io::Result<()> {
        let mut q = rpc!("usp_InsertGameAccountExp", "@P1, @P2, @P3, @P4");
        q.bind(guid(account));
        q.bind(row.exp);
        q.bind(row.quota_per_day);
        q.bind(row.last_update);
        no_rows(self.call(q).await?)
    }
    async fn load_jumping_creation_quota(
        &self,
        account: AccountId,
    ) -> io::Result<Option<JumpingCreationQuota>> {
        let mut q = rpc!("usp_LoadJumpingCharacterCreationQuota", "@P1");
        q.bind(guid(account));
        one(self.call(q).await?)?
            .map(|row| {
                Ok(JumpingCreationQuota {
                    quota: i32::from(required::<u8>(&row, 1)?),
                    count: i32::from(required::<u8>(&row, 2)?),
                })
            })
            .transpose()
    }
    async fn load_time_based_play_info(
        &self,
        account: AccountId,
    ) -> io::Result<Vec<TimeBasedPlayInfo>> {
        let mut q = rpc!("usp_LoadGameAccountTimeBasedPlayInfo", "@P1");
        q.bind(guid(account));
        self.call(q)
            .await?
            .iter()
            .map(|row| {
                Ok(TimeBasedPlayInfo {
                    schedule: required(row, 0)?,
                    target_type: required(row, 1)?,
                    expire: timestamp(row, 2)?,
                    duration: i64::from(required::<i32>(row, 3)?),
                    last_update: timestamp(row, 4)?,
                })
            })
            .collect()
    }
    async fn load_service_detail(
        &self,
        service: u64,
        kind: ServiceKind,
    ) -> io::Result<ServiceDetail> {
        let mut q = match kind {
            ServiceKind::SlotAddition => rpc!("usp_LoadSlotAdditionService", "@P1"),
            ServiceKind::Cosmesis => rpc!("usp_LoadCharacterCosmesisService", "@P1"),
            ServiceKind::Rename => rpc!("usp_LoadCharacterRenameService", "@P1"),
            ServiceKind::Sex => rpc!("usp_LoadChangeCharacterSexService", "@P1"),
            ServiceKind::Race => rpc!("usp_LoadChangeCharacterRaceService", "@P1"),
        };
        q.bind(positive(service)?);
        let row = one(self.call(q).await?)?
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "service detail row absent"))?;
        // Typed local payloads. Original appearance output propagation stays open.
        Ok(match kind {
            ServiceKind::SlotAddition => ServiceDetail::SlotAddition { slot: id(&row, 1)? },
            ServiceKind::Cosmesis => ServiceDetail::Cosmesis {
                pcid: unsigned(&row, 1)?,
                appearance: blob(&row, 2)?,
            },
            ServiceKind::Rename => ServiceDetail::Rename {
                pcid: unsigned(&row, 1)?,
                old_name: name(&row, 2, 32)?,
                new_name: name(&row, 3, 32)?,
            },
            ServiceKind::Sex => ServiceDetail::Sex {
                pcid: unsigned(&row, 1)?,
                sex: u16::try_from(required::<i16>(&row, 2)?)
                    .map_err(|_| invalid("negative sex"))?,
                appearance: blob(&row, 3)?,
            },
            ServiceKind::Race => ServiceDetail::Race {
                pcid: unsigned(&row, 1)?,
                race: u16::try_from(required::<i16>(&row, 2)?)
                    .map_err(|_| invalid("negative race"))?,
                sex: u16::try_from(required::<i16>(&row, 3)?)
                    .map_err(|_| invalid("negative sex"))?,
                appearance: blob(&row, 4)?,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_guid_round_trip_preserves_asymmetric_wire_bytes() {
        let bytes = std::array::from_fn(|n| n as u8);
        assert_eq!(guid(bytes).to_bytes_le(), bytes);
        assert_ne!(guid(bytes).as_bytes(), &bytes);
    }
    #[test]
    fn positive_sql_ids_reject_overflow_and_zero() {
        assert!(positive(0).is_err());
        assert!(positive(u64::MAX).is_err());
        assert_eq!(positive(i64::MAX as u64).unwrap(), i64::MAX);
    }
    #[test]
    fn missing_status_is_not_success() {
        assert!(check_return(vec![]).is_err());
        assert!(check_return(vec![vec![]]).is_err());
    }
    #[test]
    fn procedure_code_remains_typed() {
        let error = io::Error::other(SqlStoreError::Procedure(3));
        assert_eq!(
            error.get_ref().unwrap().downcast_ref::<SqlStoreError>(),
            Some(&SqlStoreError::Procedure(3))
        );
    }
}
