//! Game DB (`BlGame`) access: create and load of a PC.
//! ❓ Local: the original GameDaemon sends the create to CacheDaemon through CacheGate
//! (381 `0x140070580` → `0x140323AE0`). CacheDaemon calls `BlGame.dbo.usp_CreateCharacter`.
//! CacheDaemon is not ported. This module calls the same procedures directly.
//! ❓ Local: only the known start values are set. All other values are the type default.
//! The original takes them from `PcInitialSetting` (table 2019).
//! The connection string comes from the environment variable `GAMED_MSSQL_URL` only.
use shared::fsd::{MakePc, APPEARANCE_BYTES};
use shared::pool::Pool;
use std::{fmt::Write, io, time::Duration};
use tiberius::{Client, Config, Row};
use tokio::net::TcpStream;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

const TIMEOUT: Duration = Duration::from_secs(5);
/// Default count of connections to the game database (`scale-rules.md` rule D1).
const POOL_SIZE: usize = 8;

/// A PC as the world-entry packets need it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pc {
    pub pcid: u64,
    pub pc: MakePc,
    pub name: Vec<u16>,
    pub zone: u32,
    pub position: [i16; 3],
    pub yaw: u16,
    pub level: u16,
    pub hp: u64,
}

/// Start values of a new PC.
pub struct NewPc<'a> {
    pub pcid: u64,
    pub account: [u8; 16],
    pub world: i16,
    pub pc: &'a MakePc,
    pub name: &'a [u16],
    pub zone: u32,
    pub position: [i16; 3],
    pub yaw: u16,
    pub level: u16,
    pub hp: u64,
}

fn driver(error: impl std::fmt::Display) -> io::Error {
    // The driver text can hold the server name. Keep only the kind.
    let text = error.to_string();
    io::Error::other(text.chars().take(200).collect::<String>())
}

type Connection = Client<Compat<TcpStream>>;

pub struct Db {
    config: Config,
    /// Connections to `BlGame` (rule D1). Size: `GAMED_DB_POOL`, default 8.
    pool: Pool<Connection>,
    /// Parameter names and SQL types of `usp_CreateCharacter`, in order.
    parameters: Vec<(String, String)>,
}

/// The value of one create parameter as SQL text. `@P1` is the name, `@P2` the account id.
fn create_value(name: &str, kind: &str, new: &NewPc<'_>) -> String {
    let name = name.trim_start_matches('@');
    // Optional groups: all NULL means "no row".
    let quest = name
        .strip_prefix("Quest")
        .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()));
    if quest || name.starts_with("Costume_") || name.starts_with("Weapon_") {
        return "NULL".into();
    }
    if name == "pcid" || name.ends_with("_pcid") || name.contains("_pcid_") {
        return new.pcid.to_string();
    }
    if let Some(index) = name.strip_prefix("appearance") {
        if let Ok(index) = index.parse::<usize>() {
            return new
                .pc
                .appearance
                .get(index.wrapping_sub(1))
                .copied()
                .unwrap_or(0)
                .to_string();
        }
    }
    // Key columns of the two option rows and the two random-store rows.
    for prefix in ["ClientOption_optionType_", "randomStore"] {
        if let Some(rest) = name.strip_prefix(prefix) {
            let digit = rest.chars().next().filter(char::is_ascii_digit);
            if let Some(digit) = digit {
                if prefix != "randomStore" || rest[1..] == *"_random_store_number" {
                    return digit.to_string();
                }
            }
        }
    }
    match name {
        "name" => "@P1".into(),
        "game_account_id" => "@P2".into(),
        "world_id" => new.world.to_string(),
        "race" => new.pc.race.to_string(),
        "sex" => new.pc.sex.to_string(),
        "job" => new.pc.job.to_string(),
        "geo_zone" => new.zone.to_string(),
        "x" => new.position[0].to_string(),
        "y" => new.position[1].to_string(),
        "z" => new.position[2].to_string(),
        "yaw" => new.yaw.to_string(),
        "level" => new.level.to_string(),
        "hp" => new.hp.to_string(),
        _ => match kind {
            "nvarchar" => "N''".into(),
            "varbinary" => "0x".into(),
            "uniqueidentifier" => "'00000000-0000-0000-0000-000000000000'".into(),
            _ => "0".into(),
        },
    }
}

fn integer(row: &Row, column: &str) -> i64 {
    if let Ok(Some(v)) = row.try_get::<i16, _>(column) {
        return v.into();
    }
    if let Ok(Some(v)) = row.try_get::<i32, _>(column) {
        return v.into();
    }
    if let Ok(Some(v)) = row.try_get::<i64, _>(column) {
        return v;
    }
    if let Ok(Some(v)) = row.try_get::<u8, _>(column) {
        return v.into();
    }
    0
}

impl Db {
    pub fn from_env() -> io::Result<Option<Self>> {
        let Ok(url) = std::env::var("GAMED_MSSQL_URL") else {
            return Ok(None);
        };
        let config = Config::from_ado_string(&url).map_err(driver)?;
        Ok(Some(Self {
            config,
            pool: Pool::new(
                std::env::var("GAMED_DB_POOL")
                    .ok()
                    .and_then(|size| size.parse().ok())
                    .unwrap_or(POOL_SIZE),
            ),
            parameters: Vec::new(),
        }))
    }

    async fn open(&self) -> io::Result<Connection> {
        let tcp = TcpStream::connect(self.config.get_addr()).await?;
        tcp.set_nodelay(true)?;
        Client::connect(self.config.clone(), tcp.compat_write())
            .await
            .map_err(driver)
    }

    /// Run one batch on a pool connection. Calls run in parallel up to the pool size.
    /// A failure or a timeout drops the connection; a later call opens a new one.
    async fn rows(&self, sql: &str, values: &[&dyn tiberius::ToSql]) -> io::Result<Vec<Vec<Row>>> {
        let mut lease = self.pool.acquire().await;
        let work = async {
            let mut client = match lease.take() {
                Some(client) => client,
                None => self.open().await?,
            };
            let sets = client
                .query(sql, values)
                .await
                .map_err(driver)?
                .into_results()
                .await
                .map_err(driver)?;
            Ok((client, sets))
        };
        match tokio::time::timeout(TIMEOUT, work).await {
            Ok(Ok((client, sets))) => {
                lease.give(client);
                Ok(sets)
            }
            Ok(Err(error)) => Err(error),
            Err(_) => Err(io::Error::new(io::ErrorKind::TimedOut, "game DB timeout")),
        }
    }

    /// Read the parameter list of `usp_CreateCharacter`. Call it one time at start.
    pub async fn prepare(&mut self) -> io::Result<()> {
        let sets = self
            .rows(
                "select name, type_name(user_type_id) from sys.parameters \
                 where object_id = object_id('dbo.usp_CreateCharacter') order by parameter_id",
                &[],
            )
            .await?;
        self.parameters = sets
            .into_iter()
            .flatten()
            .filter_map(|row| {
                Some((
                    row.try_get::<&str, _>(0).ok()??.to_string(),
                    row.try_get::<&str, _>(1).ok()??.to_string(),
                ))
            })
            .collect();
        if self.parameters.is_empty() {
            return Err(io::Error::other("usp_CreateCharacter is absent"));
        }
        Ok(())
    }

    pub fn create_statement(&self, new: &NewPc<'_>) -> String {
        let mut sql = String::from("declare @rc int; exec @rc = dbo.usp_CreateCharacter ");
        for (index, (name, kind)) in self.parameters.iter().enumerate() {
            let comma = if index == 0 { "" } else { ", " };
            let _ = write!(sql, "{comma}{name}={}", create_value(name, kind, new));
        }
        sql.push_str("; select @rc;");
        sql
    }

    /// `Ok(true)`: the procedure returned 0.
    pub async fn create(&self, new: &NewPc<'_>) -> io::Result<bool> {
        let name = String::from_utf16_lossy(new.name);
        let account = tiberius::Uuid::from_bytes_le(new.account);
        let sets = self
            .rows(&self.create_statement(new), &[&name, &account])
            .await?;
        let code = sets
            .last()
            .and_then(|set| set.first())
            .and_then(|row| row.try_get::<i32, _>(0).ok().flatten());
        Ok(code == Some(0))
    }

    /// Test help: delete all rows of one PC id. Not a game function.
    #[doc(hidden)]
    pub async fn remove_test_rows(&self, pcid: u64) -> io::Result<()> {
        let sql = "set quoted_identifier on; declare @s nvarchar(max) = N''; \
            select @s = @s + N'delete from dbo.' + quotename(t.name) + N' where pcid = ' \
            + cast(@P1 as nvarchar(20)) + N';' from sys.tables t where exists \
            (select 1 from sys.columns c where c.object_id = t.object_id and c.name = 'pcid'); \
            exec(@s); select 0;";
        self.rows(sql, &[&(pcid as i64)]).await.map(|_| ())
    }

    /// `usp_LoadCreatureProperty`: one row, or none for an absent or deleted PC.
    pub async fn load(&self, pcid: u64) -> io::Result<Option<Pc>> {
        let sets = self
            .rows("exec dbo.usp_LoadCreatureProperty @P1", &[&(pcid as i64)])
            .await?;
        let Some(row) = sets.into_iter().flatten().next() else {
            return Ok(None);
        };
        let mut appearance = [0u8; APPEARANCE_BYTES];
        for (index, byte) in appearance.iter_mut().enumerate() {
            *byte = integer(&row, &format!("appearance{}", index + 1)) as u8;
        }
        let name: &str = row.try_get("name").map_err(driver)?.unwrap_or_default();
        Ok(Some(Pc {
            pcid,
            pc: MakePc {
                race: integer(&row, "race") as u8,
                sex: integer(&row, "sex") as u8,
                job: integer(&row, "job") as u8,
                appearance,
            },
            name: name.encode_utf16().collect(),
            zone: integer(&row, "geo_zone") as u32,
            position: [
                integer(&row, "x") as i16,
                integer(&row, "y") as i16,
                integer(&row, "z") as i16,
            ],
            yaw: integer(&row, "yaw") as u16,
            level: integer(&row, "level") as u16,
            hp: integer(&row, "hp") as u64,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_values() {
        let pc = MakePc {
            race: 1,
            sex: 2,
            job: 3,
            appearance: [7; APPEARANCE_BYTES],
        };
        let new = NewPc {
            pcid: 60001,
            account: [0; 16],
            world: 1,
            pc: &pc,
            name: &[0x41],
            zone: 1101,
            position: [-1, 2, 3],
            yaw: 45,
            level: 1,
            hp: 303,
        };
        let value = |name, kind| create_value(name, kind, &new);
        assert_eq!(value("@pcid", "bigint"), "60001");
        assert_eq!(value("@ClientOption_pcid_2", "bigint"), "60001");
        assert_eq!(value("@ClientOption_optionType_2", "smallint"), "2");
        assert_eq!(value("@randomStore2_random_store_number", "smallint"), "2");
        assert_eq!(value("@randomStore2_draw_1", "int"), "0");
        assert_eq!(value("@randomStore1_pcid", "bigint"), "60001");
        assert_eq!(value("@appearance92", "smallint"), "7");
        assert_eq!(value("@x", "smallint"), "-1");
        assert_eq!(value("@name", "nvarchar"), "@P1");
        assert_eq!(value("@Quest1_data_id", "smallint"), "NULL");
        assert_eq!(value("@Weapon_depot", "tinyint"), "NULL");
        assert_eq!(value("@QuestList_counts", "varbinary"), "0x");
        assert_eq!(value("@skill_page_name1", "nvarchar"), "N''");
        assert_eq!(value("@money", "bigint"), "0");
    }
}
