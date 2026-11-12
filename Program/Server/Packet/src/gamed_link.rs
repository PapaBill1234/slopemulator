//! LobbyDaemon ↔ GameDaemon link messages (GameDaemon dials LobbyDaemon).
//! Spec: Docs/Server/LobbyDaemon/07-gamed-link.md. Frame: `size:u16, command:u16, body`.
//! Sources: lobbyd 307 `0x1400722B0`, `0x140083D60`; GameDaemon 381 `0x14013F0F0` (20),
//! `0x140149920` (21), format strings `0x14099BD50`, `0x14099D280`, `0x1409A8D88`, `0x1409449E4`.
//! Build 381 formats: `HelloOk` string `0x14017ba60`, `EnterWorldOk` string `0x140179cd0`.
//! ❓ Command 6 sends empty lists and zero account fields except the PC id and the account id.
use bloodlib::packet::{compose, decompose, DecodeLimits, PacketError, Value};

/// Verified in build 381 (`HandleLobbyGame_HelloReq` `0x140083ee0`). Build 307 uses 80.
pub const PACKET_VERSION: i16 = 83;

const HELLO: &str = "hhhhh";
/// A rule entry has 5 values (h, i, i, i, i). The rule lists stay empty.
const HELLO_OK: &str = "hCQiiiqqChiiii;;qqqi";
const ERROR: &str = "hE";
const WORLD: &str = "hhh";
const ENTER: &str = "hqqqi";
const ENTER_OK: &str = "hiqqocbbbbssQhhiQCQiqqihcccc;Cqhs;iQQQsCiiiii;iCiiQiqq;Ciqq;qqiqCihqq;Chq;";
const FAILURE: &str = "hqE";
const CREATE: &str = "hqqobisbQh";
const CREATE_OK: &str = "hqhhb";
/// Original LobbyDaemon numbers (`07-gamed-link.md` §3): 8 LeaveWorldReq `hqqq`,
/// 9 LeaveWorldOk `hqq`, 10 LeaveWorldFail `hqE`, 12 LogoutNotify `hqq`.
const LEAVE: &str = "hqqq";
const LEAVE_OK: &str = "hqq";
const LOGOUT_NOTIFY: &str = "hqq";
/// 14 KickReq `hqqqqQ` (LobbyDaemon → GameDaemon), 15 KickOk `hq`, 16 KickFail `hqE`
/// (`07-gamed-link.md` lines 560, 561, 622).
const KICK: &str = "hqqqqQ";
const KICK_OK: &str = "hq";
const LEAVE_COMMAND: i16 = 8;
const LEAVE_OK_COMMAND: i16 = 9;
const LEAVE_FAIL_COMMAND: i16 = 10;
const LOGOUT_NOTIFY_COMMAND: i16 = 12;
const KICK_COMMAND: i16 = 14;
const KICK_OK_COMMAND: i16 = 15;
const KICK_FAIL_COMMAND: i16 = 16;

/// Command ids of the messages of `ld381-r1`. The names are the handler names of the
/// LobbyDaemon `LobbyGame` table (381) or the sender roles of `07-gamed-link.md` §3.
pub const UPDATE_BOT_SCORE_COMMAND: i16 = 11;
pub const DELETE_CHARACTER_COMMAND: i16 = 23;
pub const DELETE_CHARACTER_OK_COMMAND: i16 = 24;
pub const DELETE_CHARACTER_FAIL_COMMAND: i16 = 25;
pub const RESTORE_CHARACTER_COMMAND: i16 = 26;
pub const RESTORE_CHARACTER_OK_COMMAND: i16 = 27;
pub const RESTORE_CHARACTER_FAIL_COMMAND: i16 = 28;
pub const UPDATE_CHARACTER_DATA_COMMAND: i16 = 39;
pub const UPDATE_CHARACTER_DATA_OK_COMMAND: i16 = 40;
pub const UPDATE_CHARACTER_DATA_FAIL_COMMAND: i16 = 41;
pub const UPDATE_ACCOUNT_DATA_COMMAND: i16 = 42;
pub const UPDATE_ACCOUNT_DATA_OK_COMMAND: i16 = 43;
pub const UPDATE_ACCOUNT_DATA_FAIL_COMMAND: i16 = 44;
pub const UPDATE_ACCOUNT_EXP_COMMAND: i16 = 107;
pub const UPDATE_ACCOUNT_EXP_OK_COMMAND: i16 = 108;
pub const UPDATE_ACCOUNT_EXP_FAIL_COMMAND: i16 = 109;
pub const UPDATE_ACCOUNT_CONTENT_QUOTA_COMMAND: i16 = 110;
/// ❓ no verified name: the reply ids of 110 are not read (`24-compare-381.md` §9).
/// Both ends are ours (`step6-part2.md` §3 decision 3).
pub const UPDATE_ACCOUNT_CONTENT_QUOTA_OK_COMMAND: i16 = 111;
/// ❓ no verified name.
pub const UPDATE_ACCOUNT_CONTENT_QUOTA_FAIL_COMMAND: i16 = 112;
pub const UPDATE_ACCOUNT_RECYCLE_TIME_COMMAND: i16 = 113;
/// ❓ no verified name.
pub const UPDATE_ACCOUNT_RECYCLE_TIME_OK_COMMAND: i16 = 114;
/// ❓ no verified name.
pub const UPDATE_ACCOUNT_RECYCLE_TIME_FAIL_COMMAND: i16 = 115;

/// 23 `DeleteCharacterReq`: transaction, PC id, external key blob, client IPv4.
const DELETE: &str = "hqqbi";
/// 24 `DeleteCharacterOk`: transaction, result.
const DELETE_OK: &str = "hqh";
/// 26 `RestoreCharacterReq`.
const RESTORE: &str = "hqq";
const BOT_SCORE: &str = "hqqi";
const CHARACTER_DATA: &str = "hqqqhb";
/// 381 `UpdateAccountDataReq` (`24-compare-381.md` §8).
const ACCOUNT_DATA: &str = "hqqqiCQiqqihcccc;qiqCihqq;Chq;";
const ACCOUNT_EXP: &str = "hqqqqiq";
const CONTENT_QUOTA: &str = "hqqqCihqq;";
const RECYCLE_TIME: &str = "hqqqChq;";
const REPLY_TRANSACTION: &str = "hq";
const REPLY_PC: &str = "hqq";
const REPLY_PC_FAIL: &str = "hqqE";

/// Most rules in one schedule of `HelloOk` (`07-gamed-link.md` lines 930–957).
const MAX_RULES: usize = 6;
/// Number of values of 6 after the command number, and the places of the 381 lists.
const ENTER_OK_VALUES: usize = 33;
const ENTER_OK_TIME: usize = 16;
const ENTER_OK_EXP: usize = 28;

/// One record of the time-based play list `CQiqqihcccc;`.
/// ❓ The meaning of the two `q` and the four `c` is open (`07-gamed-link.md` line 989).
/// `target` is the index (0, 1), `schedule` the schedule number, `expire` and `last_update`
/// are Unix seconds (0: none), `duration` and `acknowledged` are the play time values,
/// `rewards` are the four flag bytes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TimePlayInfo {
    pub target: i8,
    pub schedule: i32,
    pub expire: i64,
    pub last_update: i64,
    pub duration: i32,
    pub acknowledged: i16,
    pub rewards: [u8; 4],
}
/// Account exp record `qiq`: exp, quota for each day, last update time.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AccountExpInfo {
    pub exp: i64,
    pub quota_per_day: i32,
    pub last_update: i64,
}
/// Content quota record `ihqq`: data id, version, quantity, last update time
/// (`usp_UpdateGameAccountContentQuota`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ContentQuota {
    pub data_id: i32,
    pub version: i16,
    pub quantity: i64,
    pub last_update: i64,
}
/// Recycle time record `hq`: group id, time (`usp_UpdateGameAccountRecycleTime`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RecycleTime {
    pub group: i16,
    pub time: i64,
}
/// The account lists of `EnterWorldOk` and `UpdateAccountDataReq` (381).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AccountLists {
    pub time_based: Vec<TimePlayInfo>,
    pub exp: AccountExpInfo,
    pub quotas: Vec<ContentQuota>,
    pub recycle: Vec<RecycleTime>,
}
/// One rule of a schedule: `hiiii` (381). ❓ The meanings are open.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScheduleRule {
    pub id: i16,
    pub values: [i32; 4],
}
/// One schedule of `HelloOk`: `Qiiiqq C[hiiii]`. `mode` 0 is a daily window, 1 the whole
/// schedule (`07-gamed-link.md` lines 930–957). At most 6 rules.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlaySchedule {
    pub mode: i8,
    pub reset_hour: i32,
    pub utc_offset: i32,
    pub source: i32,
    pub start: i64,
    pub end: i64,
    pub rules: Vec<ScheduleRule>,
}
/// The data of `HelloOk`: the 2 time-based play season schedules, the 3 random store
/// initialization numbers and the mileage effective duration in days.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HelloData {
    pub schedules: [PlaySchedule; 2],
    pub random_store: [u64; 3],
    pub mileage_days: i32,
}
impl Default for HelloData {
    fn default() -> Self {
        Self {
            schedules: [
                PlaySchedule::default(),
                PlaySchedule {
                    mode: 1,
                    ..PlaySchedule::default()
                },
            ],
            random_store: [0; 3],
            mileage_days: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    /// 1. `port` is the internal port of the GameDaemon.
    Hello {
        cluster: i16,
        world: i16,
        version: i16,
        port: i16,
    },
    /// 2 with the default [`HelloData`] (two empty schedules and a zero tail).
    /// `decode` gives this variant when the data equals the default.
    HelloOk,
    /// 2 with other data.
    HelloOkData(Box<HelloData>),
    /// 3. Reasons 1, 2, 12.
    HelloFail(i16),
    /// 4. The world is live. No reply.
    WorldLive { cluster: i16, world: i16 },
    /// 5.
    Enter {
        transaction: u64,
        client_key: u64,
        token: u64,
        address: u32,
    },
    /// 6.
    EnterOk {
        transaction: u64,
        pcid: u64,
        account: [u8; 16],
    },
    /// 7. Reasons 16, 15, 17, 4, 2, 6.
    EnterFail { transaction: u64, reason: i16 },
    /// 8 LeaveWorldReq (GameDaemon → LobbyDaemon). Return to lobby: step 7 of the transaction.
    Leave {
        transaction: u64,
        pc_db_id: u64,
        cli_key: u64,
    },
    /// 9 LeaveWorldOk. `token` is the key of the new type-2 lobby token.
    LeaveOk { transaction: u64, token: u64 },
    /// 10 LeaveWorldFail. Reasons 2, 5, 6, 21, 22, 23.
    LeaveFail { transaction: u64, reason: i16 },
    /// 12 LogoutNotify (GameDaemon → LobbyDaemon). No reply.
    LogoutNotify { pc_db_id: u64, cli_key: u64 },
    /// 14 KickReq (LobbyDaemon → GameDaemon): disconnect the PC. Reply 15 or 16.
    /// ❓ The wire format `hqqqqQ` has one more `q` after the three named ones. Its meaning is
    /// not verified: it is sent as 0 and ignored on decode. `reason` is the `Q` (one signed byte).
    Kick {
        transaction: u64,
        client: u64,
        pcid: u64,
        reason: i8,
    },
    /// 15 KickOk.
    KickOk { transaction: u64 },
    /// 16 KickFail. Reason 24 means that the kick is done already (LobbyDaemon result 1).
    KickFail { transaction: u64, reason: i16 },
    /// 20. ❓ `external` is the external account data blob. ❓ `address` is the client IPv4.
    Create {
        transaction: u64,
        pcid: u64,
        account: [u8; 16],
        external: Vec<u8>,
        address: u32,
        name: Vec<u16>,
        make_pc: Vec<u8>,
        jumping: bool,
        top_level: i16,
    },
    /// 21.
    CreateOk {
        transaction: u64,
        job: i16,
        level: i16,
        showcase: Vec<u8>,
    },
    /// 22.
    CreateFail { transaction: u64, reason: i16 },
    /// 6 with account lists. `decode` gives [`Message::EnterOk`] when all lists are empty.
    EnterOkData {
        transaction: u64,
        pcid: u64,
        account: [u8; 16],
        lists: Box<AccountLists>,
    },
    /// 11 `UpdateBotScoreNotify` (GameDaemon → LobbyDaemon). No reply.
    UpdateBotScore { pcid: u64, cli_key: u64, score: i32 },
    /// 23 `DeleteCharacterReq` (LobbyDaemon → GameDaemon). Reply 24 or 25.
    /// ❓ `external` is the external account data blob, `address` is the client IPv4
    /// (the card names `wait` and `seconds`; the 381 format `hqqbi` does not fit them).
    DeleteCharacter {
        transaction: u64,
        pcid: u64,
        external: Vec<u8>,
        address: u32,
    },
    /// 24 `DeleteCharacterOk`. `result` 1 means done.
    DeleteCharacterOk { transaction: u64, result: i16 },
    /// 25 `DeleteCharacterFail`.
    DeleteCharacterFail { transaction: u64, reason: i16 },
    /// 26 `RestoreCharacterReq` (LobbyDaemon → GameDaemon). Reply 27 or 28.
    RestoreCharacter { transaction: u64, pcid: u64 },
    /// 27 `RestoreCharacterOk`.
    RestoreCharacterOk { transaction: u64 },
    /// 28 `RestoreCharacterFail`.
    RestoreCharacterFail { transaction: u64, reason: i16 },
    /// 39 `UpdateCharacterDataReq`. Reply 40 or 41.
    UpdateCharacterData {
        transaction: u64,
        pcid: u64,
        cli_key: u64,
        level: i16,
        showcase: Vec<u8>,
    },
    /// 42 `UpdateAccountDataReq` (381 format). Reply 43 or 44.
    UpdateAccountData {
        transaction: u64,
        pcid: u64,
        cli_key: u64,
        bot_score: i32,
        lists: Box<AccountLists>,
    },
    /// 107 `UpdateAccountExpReq`. Reply 108 or 109.
    UpdateAccountExp {
        transaction: u64,
        pcid: u64,
        cli_key: u64,
        exp: AccountExpInfo,
    },
    /// 110 `UpdateAccountContentQuotaReq`. Reply 111 or 112 (❓ no verified names).
    UpdateAccountContentQuota {
        transaction: u64,
        pcid: u64,
        cli_key: u64,
        quotas: Vec<ContentQuota>,
    },
    /// 113 `UpdateAccountRecycleTimeReq`. Reply 114 or 115 (❓ no verified names).
    UpdateAccountRecycleTime {
        transaction: u64,
        pcid: u64,
        cli_key: u64,
        recycle: Vec<RecycleTime>,
    },
    /// Replies 40, 41, 43, 44, 108, 109, 111, 112, 114 and 115 (LobbyDaemon → GameDaemon).
    /// `pcid` is set for 108, 109, 111, 112, 114 and 115. `reason` is set for the Fail commands
    /// 41, 44, 109, 112 and 115. A wrong combination is a `ValueCount` error.
    Reply {
        command: i16,
        transaction: u64,
        pcid: Option<u64>,
        reason: Option<i16>,
    },
}

fn list() -> Value {
    Value::Repeat(Vec::new())
}

fn bad_shape<T>() -> Result<T, PacketError> {
    Err(PacketError::ValueCount)
}

fn time_value(rows: &[TimePlayInfo]) -> Value {
    Value::Repeat(
        rows.iter()
            .map(|t| {
                vec![
                    Value::I8(t.target),
                    Value::U32(t.schedule as u32),
                    Value::U64(t.expire as u64),
                    Value::U64(t.last_update as u64),
                    Value::U32(t.duration as u32),
                    Value::I16(t.acknowledged),
                    Value::U8(t.rewards[0]),
                    Value::U8(t.rewards[1]),
                    Value::U8(t.rewards[2]),
                    Value::U8(t.rewards[3]),
                ]
            })
            .collect(),
    )
}

fn quota_value(rows: &[ContentQuota]) -> Value {
    Value::Repeat(
        rows.iter()
            .map(|q| {
                vec![
                    Value::U32(q.data_id as u32),
                    Value::I16(q.version),
                    Value::U64(q.quantity as u64),
                    Value::U64(q.last_update as u64),
                ]
            })
            .collect(),
    )
}

fn recycle_value(rows: &[RecycleTime]) -> Value {
    Value::Repeat(
        rows.iter()
            .map(|r| vec![Value::I16(r.group), Value::U64(r.time as u64)])
            .collect(),
    )
}

fn rows_of(value: &Value) -> Result<&[Vec<Value>], PacketError> {
    match value {
        Value::Repeat(rows) => Ok(rows),
        _ => bad_shape(),
    }
}

fn time_from(value: &Value) -> Result<Vec<TimePlayInfo>, PacketError> {
    use Value::*;
    rows_of(value)?
        .iter()
        .map(|row| match row.as_slice() {
            [I8(target), U32(schedule), U64(expire), U64(last_update), U32(duration), I16(acknowledged), U8(a), U8(b), U8(c), U8(d)] => {
                Ok(TimePlayInfo {
                    target: *target,
                    schedule: *schedule as i32,
                    expire: *expire as i64,
                    last_update: *last_update as i64,
                    duration: *duration as i32,
                    acknowledged: *acknowledged,
                    rewards: [*a, *b, *c, *d],
                })
            }
            _ => bad_shape(),
        })
        .collect()
}

fn quota_from(value: &Value) -> Result<Vec<ContentQuota>, PacketError> {
    use Value::*;
    rows_of(value)?
        .iter()
        .map(|row| match row.as_slice() {
            [U32(data_id), I16(version), U64(quantity), U64(last_update)] => Ok(ContentQuota {
                data_id: *data_id as i32,
                version: *version,
                quantity: *quantity as i64,
                last_update: *last_update as i64,
            }),
            _ => bad_shape(),
        })
        .collect()
}

fn recycle_from(value: &Value) -> Result<Vec<RecycleTime>, PacketError> {
    use Value::*;
    rows_of(value)?
        .iter()
        .map(|row| match row.as_slice() {
            [I16(group), U64(time)] => Ok(RecycleTime {
                group: *group,
                time: *time as i64,
            }),
            _ => bad_shape(),
        })
        .collect()
}

fn exp_from(
    exp: &Value,
    quota: &Value,
    last_update: &Value,
) -> Result<AccountExpInfo, PacketError> {
    match (exp, quota, last_update) {
        (Value::U64(exp), Value::U32(quota), Value::U64(last_update)) => Ok(AccountExpInfo {
            exp: *exp as i64,
            quota_per_day: *quota as i32,
            last_update: *last_update as i64,
        }),
        _ => bad_shape(),
    }
}

fn exp_values(exp: &AccountExpInfo) -> [Value; 3] {
    [
        Value::U64(exp.exp as u64),
        Value::U32(exp.quota_per_day as u32),
        Value::U64(exp.last_update as u64),
    ]
}

/// The values of 2 `HelloOk` (the command number is the first value).
fn hello_values(data: &HelloData) -> Result<Vec<Value>, PacketError> {
    use Value::*;
    let mut schedules = Vec::with_capacity(data.schedules.len());
    for s in &data.schedules {
        if s.rules.len() > MAX_RULES {
            return bad_shape();
        }
        let rules = s
            .rules
            .iter()
            .map(|r| {
                let mut row = vec![I16(r.id)];
                row.extend(r.values.iter().map(|v| U32(*v as u32)));
                row
            })
            .collect();
        schedules.push(vec![
            I8(s.mode),
            U32(s.reset_hour as u32),
            U32(s.utc_offset as u32),
            U32(s.source as u32),
            U64(s.start as u64),
            U64(s.end as u64),
            Repeat(rules),
        ]);
    }
    Ok(vec![
        I16(2),
        Repeat(schedules),
        U64(data.random_store[0]),
        U64(data.random_store[1]),
        U64(data.random_store[2]),
        U32(data.mileage_days as u32),
    ])
}

fn hello_from(body: &[Value]) -> Result<HelloData, PacketError> {
    use Value::*;
    let [Repeat(rows), U64(a), U64(b), U64(c), U32(mileage)] = body else {
        return bad_shape();
    };
    let mut schedules = Vec::new();
    for row in rows {
        let [I8(mode), U32(reset_hour), U32(utc_offset), U32(source), U64(start), U64(end), Repeat(rules)] =
            row.as_slice()
        else {
            return bad_shape();
        };
        let mut parsed = Vec::new();
        for rule in rules {
            let [I16(id), U32(v0), U32(v1), U32(v2), U32(v3)] = rule.as_slice() else {
                return bad_shape();
            };
            parsed.push(ScheduleRule {
                id: *id,
                values: [*v0 as i32, *v1 as i32, *v2 as i32, *v3 as i32],
            });
        }
        schedules.push(PlaySchedule {
            mode: *mode,
            reset_hour: *reset_hour as i32,
            utc_offset: *utc_offset as i32,
            source: *source as i32,
            start: *start as i64,
            end: *end as i64,
            rules: parsed,
        });
    }
    let schedules: [PlaySchedule; 2] = schedules.try_into().or_else(|_| bad_shape())?;
    Ok(HelloData {
        schedules,
        random_store: [*a, *b, *c],
        mileage_days: *mileage as i32,
    })
}

/// The values of 6 `EnterWorldOk` (the command number is the first value). The account
/// fields that are not in the lists stay zero.
fn enter_ok_values(
    transaction: u64,
    pcid: u64,
    account: &[u8; 16],
    lists: &AccountLists,
) -> Vec<Value> {
    use Value::*;
    let blob = || Bytes(Vec::new());
    let text = || String(Vec::new());
    let [exp, quota, last_update] = exp_values(&lists.exp);
    vec![
        I16(6),
        U32(0),
        U64(transaction),
        U64(pcid),
        Id(*account),
        U8(0),
        blob(),
        blob(),
        blob(),
        blob(),
        text(),
        text(),
        I8(0),
        I16(0),
        I16(0),
        U32(0),
        I8(0),
        time_value(&lists.time_based),
        list(),
        U32(0),
        I8(0),
        I8(0),
        I8(0),
        text(),
        list(),
        U32(0),
        list(),
        list(),
        U64(0),
        exp,
        quota,
        last_update,
        quota_value(&lists.quotas),
        recycle_value(&lists.recycle),
    ]
}

/// The lists of 6 from the body values (without the command number).
fn enter_lists(body: &[Value]) -> Result<AccountLists, PacketError> {
    if body.len() != ENTER_OK_VALUES {
        return bad_shape();
    }
    Ok(AccountLists {
        time_based: time_from(&body[ENTER_OK_TIME])?,
        exp: exp_from(
            &body[ENTER_OK_EXP],
            &body[ENTER_OK_EXP + 1],
            &body[ENTER_OK_EXP + 2],
        )?,
        quotas: quota_from(&body[ENTER_OK_EXP + 3])?,
        recycle: recycle_from(&body[ENTER_OK_EXP + 4])?,
    })
}

/// The shape of a reply: format, has PC id, has reason.
fn reply_shape(command: i16) -> Option<(&'static str, bool, bool)> {
    Some(match command {
        UPDATE_CHARACTER_DATA_OK_COMMAND | UPDATE_ACCOUNT_DATA_OK_COMMAND => {
            (REPLY_TRANSACTION, false, false)
        }
        UPDATE_CHARACTER_DATA_FAIL_COMMAND | UPDATE_ACCOUNT_DATA_FAIL_COMMAND => {
            (FAILURE, false, true)
        }
        UPDATE_ACCOUNT_EXP_OK_COMMAND
        | UPDATE_ACCOUNT_CONTENT_QUOTA_OK_COMMAND
        | UPDATE_ACCOUNT_RECYCLE_TIME_OK_COMMAND => (REPLY_PC, true, false),
        UPDATE_ACCOUNT_EXP_FAIL_COMMAND
        | UPDATE_ACCOUNT_CONTENT_QUOTA_FAIL_COMMAND
        | UPDATE_ACCOUNT_RECYCLE_TIME_FAIL_COMMAND => (REPLY_PC_FAIL, true, true),
        _ => return None,
    })
}

impl Message {
    pub fn encode(&self) -> Result<Vec<u8>, PacketError> {
        use Value::*;
        match self {
            Self::Hello {
                cluster,
                world,
                version,
                port,
            } => compose(
                HELLO,
                &[
                    I16(1),
                    I16(*cluster),
                    I16(*world),
                    I16(*version),
                    I16(*port),
                ],
            ),
            Self::HelloOk => compose(HELLO_OK, &hello_values(&HelloData::default())?),
            Self::HelloOkData(data) => compose(HELLO_OK, &hello_values(data)?),
            Self::HelloFail(reason) => compose(ERROR, &[I16(3), I16(*reason)]),
            Self::WorldLive { cluster, world } => {
                compose(WORLD, &[I16(4), I16(*cluster), I16(*world)])
            }
            Self::Enter {
                transaction,
                client_key,
                token,
                address,
            } => compose(
                ENTER,
                &[
                    I16(5),
                    U64(*transaction),
                    U64(*client_key),
                    U64(*token),
                    U32(*address),
                ],
            ),
            Self::EnterOk {
                transaction,
                pcid,
                account,
            } => compose(
                ENTER_OK,
                &enter_ok_values(*transaction, *pcid, account, &AccountLists::default()),
            ),
            Self::EnterOkData {
                transaction,
                pcid,
                account,
                lists,
            } => compose(
                ENTER_OK,
                &enter_ok_values(*transaction, *pcid, account, lists),
            ),
            Self::EnterFail {
                transaction,
                reason,
            } => compose(FAILURE, &[I16(7), U64(*transaction), I16(*reason)]),
            Self::Leave {
                transaction,
                pc_db_id,
                cli_key,
            } => compose(
                LEAVE,
                &[
                    I16(LEAVE_COMMAND),
                    U64(*transaction),
                    U64(*pc_db_id),
                    U64(*cli_key),
                ],
            ),
            Self::LeaveOk { transaction, token } => compose(
                LEAVE_OK,
                &[I16(LEAVE_OK_COMMAND), U64(*transaction), U64(*token)],
            ),
            Self::LeaveFail {
                transaction,
                reason,
            } => compose(
                FAILURE,
                &[I16(LEAVE_FAIL_COMMAND), U64(*transaction), I16(*reason)],
            ),
            Self::LogoutNotify { pc_db_id, cli_key } => compose(
                LOGOUT_NOTIFY,
                &[I16(LOGOUT_NOTIFY_COMMAND), U64(*pc_db_id), U64(*cli_key)],
            ),
            Self::Kick {
                transaction,
                client,
                pcid,
                reason,
            } => compose(
                KICK,
                &[
                    I16(KICK_COMMAND),
                    U64(*transaction),
                    U64(*client),
                    U64(*pcid),
                    // ❓ The fourth `q` is not verified.
                    U64(0),
                    I8(*reason),
                ],
            ),
            Self::KickOk { transaction } => {
                compose(KICK_OK, &[I16(KICK_OK_COMMAND), U64(*transaction)])
            }
            Self::KickFail {
                transaction,
                reason,
            } => compose(
                FAILURE,
                &[I16(KICK_FAIL_COMMAND), U64(*transaction), I16(*reason)],
            ),
            Self::Create {
                transaction,
                pcid,
                account,
                external,
                address,
                name,
                make_pc,
                jumping,
                top_level,
            } => compose(
                CREATE,
                &[
                    I16(20),
                    U64(*transaction),
                    U64(*pcid),
                    Id(*account),
                    Bytes(external.clone()),
                    U32(*address),
                    String(name.clone()),
                    Bytes(make_pc.clone()),
                    I8(i8::from(*jumping)),
                    I16(*top_level),
                ],
            ),
            Self::CreateOk {
                transaction,
                job,
                level,
                showcase,
            } => compose(
                CREATE_OK,
                &[
                    I16(21),
                    U64(*transaction),
                    I16(*job),
                    I16(*level),
                    Bytes(showcase.clone()),
                ],
            ),
            Self::CreateFail {
                transaction,
                reason,
            } => compose(FAILURE, &[I16(22), U64(*transaction), I16(*reason)]),
            Self::UpdateBotScore {
                pcid,
                cli_key,
                score,
            } => compose(
                BOT_SCORE,
                &[
                    I16(UPDATE_BOT_SCORE_COMMAND),
                    U64(*pcid),
                    U64(*cli_key),
                    U32(*score as u32),
                ],
            ),
            Self::DeleteCharacter {
                transaction,
                pcid,
                external,
                address,
            } => compose(
                DELETE,
                &[
                    I16(DELETE_CHARACTER_COMMAND),
                    U64(*transaction),
                    U64(*pcid),
                    Bytes(external.clone()),
                    U32(*address),
                ],
            ),
            Self::DeleteCharacterOk {
                transaction,
                result,
            } => compose(
                DELETE_OK,
                &[
                    I16(DELETE_CHARACTER_OK_COMMAND),
                    U64(*transaction),
                    I16(*result),
                ],
            ),
            Self::DeleteCharacterFail {
                transaction,
                reason,
            } => compose(
                FAILURE,
                &[
                    I16(DELETE_CHARACTER_FAIL_COMMAND),
                    U64(*transaction),
                    I16(*reason),
                ],
            ),
            Self::RestoreCharacter { transaction, pcid } => compose(
                RESTORE,
                &[
                    I16(RESTORE_CHARACTER_COMMAND),
                    U64(*transaction),
                    U64(*pcid),
                ],
            ),
            Self::RestoreCharacterOk { transaction } => compose(
                REPLY_TRANSACTION,
                &[I16(RESTORE_CHARACTER_OK_COMMAND), U64(*transaction)],
            ),
            Self::RestoreCharacterFail {
                transaction,
                reason,
            } => compose(
                FAILURE,
                &[
                    I16(RESTORE_CHARACTER_FAIL_COMMAND),
                    U64(*transaction),
                    I16(*reason),
                ],
            ),
            Self::UpdateCharacterData {
                transaction,
                pcid,
                cli_key,
                level,
                showcase,
            } => compose(
                CHARACTER_DATA,
                &[
                    I16(UPDATE_CHARACTER_DATA_COMMAND),
                    U64(*transaction),
                    U64(*pcid),
                    U64(*cli_key),
                    I16(*level),
                    Bytes(showcase.clone()),
                ],
            ),
            Self::UpdateAccountData {
                transaction,
                pcid,
                cli_key,
                bot_score,
                lists,
            } => {
                let [exp, quota, last_update] = exp_values(&lists.exp);
                compose(
                    ACCOUNT_DATA,
                    &[
                        I16(UPDATE_ACCOUNT_DATA_COMMAND),
                        U64(*transaction),
                        U64(*pcid),
                        U64(*cli_key),
                        U32(*bot_score as u32),
                        time_value(&lists.time_based),
                        exp,
                        quota,
                        last_update,
                        quota_value(&lists.quotas),
                        recycle_value(&lists.recycle),
                    ],
                )
            }
            Self::UpdateAccountExp {
                transaction,
                pcid,
                cli_key,
                exp,
            } => {
                let [exp, quota, last_update] = exp_values(exp);
                compose(
                    ACCOUNT_EXP,
                    &[
                        I16(UPDATE_ACCOUNT_EXP_COMMAND),
                        U64(*transaction),
                        U64(*pcid),
                        U64(*cli_key),
                        exp,
                        quota,
                        last_update,
                    ],
                )
            }
            Self::UpdateAccountContentQuota {
                transaction,
                pcid,
                cli_key,
                quotas,
            } => compose(
                CONTENT_QUOTA,
                &[
                    I16(UPDATE_ACCOUNT_CONTENT_QUOTA_COMMAND),
                    U64(*transaction),
                    U64(*pcid),
                    U64(*cli_key),
                    quota_value(quotas),
                ],
            ),
            Self::UpdateAccountRecycleTime {
                transaction,
                pcid,
                cli_key,
                recycle,
            } => compose(
                RECYCLE_TIME,
                &[
                    I16(UPDATE_ACCOUNT_RECYCLE_TIME_COMMAND),
                    U64(*transaction),
                    U64(*pcid),
                    U64(*cli_key),
                    recycle_value(recycle),
                ],
            ),
            Self::Reply {
                command,
                transaction,
                pcid,
                reason,
            } => {
                let Some((format, has_pcid, has_reason)) = reply_shape(*command) else {
                    return bad_shape();
                };
                if pcid.is_some() != has_pcid || reason.is_some() != has_reason {
                    return bad_shape();
                }
                let mut values = vec![I16(*command), U64(*transaction)];
                values.extend(pcid.map(U64));
                values.extend(reason.map(I16));
                compose(format, &values)
            }
        }
    }

    /// `packet` is one full frame. An unknown command gives `Ok(None)`.
    pub fn decode(packet: &[u8]) -> Result<Option<Self>, PacketError> {
        use Value::*;
        let command = packet
            .get(2..4)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .ok_or(PacketError::InvalidSize)?;
        let format = match command {
            1 => HELLO,
            2 => HELLO_OK,
            3 => ERROR,
            4 => WORLD,
            5 => ENTER,
            6 => ENTER_OK,
            7
            | 22
            | LEAVE_FAIL_COMMAND
            | KICK_FAIL_COMMAND
            | DELETE_CHARACTER_FAIL_COMMAND
            | RESTORE_CHARACTER_FAIL_COMMAND => FAILURE,
            UPDATE_BOT_SCORE_COMMAND => BOT_SCORE,
            DELETE_CHARACTER_COMMAND => DELETE,
            DELETE_CHARACTER_OK_COMMAND => DELETE_OK,
            RESTORE_CHARACTER_COMMAND => RESTORE,
            RESTORE_CHARACTER_OK_COMMAND => REPLY_TRANSACTION,
            UPDATE_CHARACTER_DATA_COMMAND => CHARACTER_DATA,
            UPDATE_ACCOUNT_DATA_COMMAND => ACCOUNT_DATA,
            UPDATE_ACCOUNT_EXP_COMMAND => ACCOUNT_EXP,
            UPDATE_ACCOUNT_CONTENT_QUOTA_COMMAND => CONTENT_QUOTA,
            UPDATE_ACCOUNT_RECYCLE_TIME_COMMAND => RECYCLE_TIME,
            KICK_COMMAND => KICK,
            KICK_OK_COMMAND => KICK_OK,
            LEAVE_COMMAND => LEAVE,
            LEAVE_OK_COMMAND => LEAVE_OK,
            LOGOUT_NOTIFY_COMMAND => LOGOUT_NOTIFY,
            20 => CREATE,
            21 => CREATE_OK,
            other => match reply_shape(other) {
                Some((format, _, _)) => format,
                None => return Ok(None),
            },
        };
        // Limits of command 20: 64 blob bytes are the first blob; 256 is the largest blob.
        let limits = DecodeLimits {
            blob_bytes: 4096,
            string_units: 256,
            property_bytes: 0,
        };
        let v = decompose(format, packet, limits)?;
        let message = match (command, &v[1..]) {
            (1, [I16(cluster), I16(world), I16(version), I16(port)]) => Self::Hello {
                cluster: *cluster,
                world: *world,
                version: *version,
                port: *port,
            },
            (2, body) => {
                let data = hello_from(body)?;
                if data == HelloData::default() {
                    Self::HelloOk
                } else {
                    Self::HelloOkData(Box::new(data))
                }
            }
            (3, [I16(reason)]) => Self::HelloFail(*reason),
            (4, [I16(cluster), I16(world)]) => Self::WorldLive {
                cluster: *cluster,
                world: *world,
            },
            (5, [U64(transaction), U64(client_key), U64(token), U32(address)]) => Self::Enter {
                transaction: *transaction,
                client_key: *client_key,
                token: *token,
                address: *address,
            },
            (6, body @ [_, U64(transaction), U64(pcid), Id(account), ..]) => {
                let lists = enter_lists(body)?;
                if lists == AccountLists::default() {
                    Self::EnterOk {
                        transaction: *transaction,
                        pcid: *pcid,
                        account: *account,
                    }
                } else {
                    Self::EnterOkData {
                        transaction: *transaction,
                        pcid: *pcid,
                        account: *account,
                        lists: Box::new(lists),
                    }
                }
            }
            (7, [U64(transaction), I16(reason)]) => Self::EnterFail {
                transaction: *transaction,
                reason: *reason,
            },
            (22, [U64(transaction), I16(reason)]) => Self::CreateFail {
                transaction: *transaction,
                reason: *reason,
            },
            (LEAVE_COMMAND, [U64(transaction), U64(pc_db_id), U64(cli_key)]) => Self::Leave {
                transaction: *transaction,
                pc_db_id: *pc_db_id,
                cli_key: *cli_key,
            },
            (LEAVE_OK_COMMAND, [U64(transaction), U64(token)]) => Self::LeaveOk {
                transaction: *transaction,
                token: *token,
            },
            (LEAVE_FAIL_COMMAND, [U64(transaction), I16(reason)]) => Self::LeaveFail {
                transaction: *transaction,
                reason: *reason,
            },
            (LOGOUT_NOTIFY_COMMAND, [U64(pc_db_id), U64(cli_key)]) => Self::LogoutNotify {
                pc_db_id: *pc_db_id,
                cli_key: *cli_key,
            },
            (KICK_COMMAND, [U64(transaction), U64(client), U64(pcid), U64(_), I8(reason)]) => {
                Self::Kick {
                    transaction: *transaction,
                    client: *client,
                    pcid: *pcid,
                    reason: *reason,
                }
            }
            (KICK_OK_COMMAND, [U64(transaction)]) => Self::KickOk {
                transaction: *transaction,
            },
            (KICK_FAIL_COMMAND, [U64(transaction), I16(reason)]) => Self::KickFail {
                transaction: *transaction,
                reason: *reason,
            },
            (
                20,
                [U64(transaction), U64(pcid), Id(account), Bytes(external), U32(address), String(name), Bytes(make_pc), I8(jumping), I16(top_level)],
            ) => Self::Create {
                transaction: *transaction,
                pcid: *pcid,
                account: *account,
                external: external.clone(),
                address: *address,
                name: name.clone(),
                make_pc: make_pc.clone(),
                jumping: *jumping != 0,
                top_level: *top_level,
            },
            (21, [U64(transaction), I16(job), I16(level), Bytes(showcase)]) => Self::CreateOk {
                transaction: *transaction,
                job: *job,
                level: *level,
                showcase: showcase.clone(),
            },
            (UPDATE_BOT_SCORE_COMMAND, [U64(pcid), U64(cli_key), U32(score)]) => {
                Self::UpdateBotScore {
                    pcid: *pcid,
                    cli_key: *cli_key,
                    score: *score as i32,
                }
            }
            (
                DELETE_CHARACTER_COMMAND,
                [U64(transaction), U64(pcid), Bytes(external), U32(address)],
            ) => Self::DeleteCharacter {
                transaction: *transaction,
                pcid: *pcid,
                external: external.clone(),
                address: *address,
            },
            (DELETE_CHARACTER_OK_COMMAND, [U64(transaction), I16(result)]) => {
                Self::DeleteCharacterOk {
                    transaction: *transaction,
                    result: *result,
                }
            }
            (DELETE_CHARACTER_FAIL_COMMAND, [U64(transaction), I16(reason)]) => {
                Self::DeleteCharacterFail {
                    transaction: *transaction,
                    reason: *reason,
                }
            }
            (RESTORE_CHARACTER_COMMAND, [U64(transaction), U64(pcid)]) => Self::RestoreCharacter {
                transaction: *transaction,
                pcid: *pcid,
            },
            (RESTORE_CHARACTER_OK_COMMAND, [U64(transaction)]) => Self::RestoreCharacterOk {
                transaction: *transaction,
            },
            (RESTORE_CHARACTER_FAIL_COMMAND, [U64(transaction), I16(reason)]) => {
                Self::RestoreCharacterFail {
                    transaction: *transaction,
                    reason: *reason,
                }
            }
            (
                UPDATE_CHARACTER_DATA_COMMAND,
                [U64(transaction), U64(pcid), U64(cli_key), I16(level), Bytes(showcase)],
            ) => Self::UpdateCharacterData {
                transaction: *transaction,
                pcid: *pcid,
                cli_key: *cli_key,
                level: *level,
                showcase: showcase.clone(),
            },
            (
                UPDATE_ACCOUNT_DATA_COMMAND,
                [U64(transaction), U64(pcid), U64(cli_key), U32(bot_score), time, exp, quota, last_update, quotas, recycle],
            ) => Self::UpdateAccountData {
                transaction: *transaction,
                pcid: *pcid,
                cli_key: *cli_key,
                bot_score: *bot_score as i32,
                lists: Box::new(AccountLists {
                    time_based: time_from(time)?,
                    exp: exp_from(exp, quota, last_update)?,
                    quotas: quota_from(quotas)?,
                    recycle: recycle_from(recycle)?,
                }),
            },
            (
                UPDATE_ACCOUNT_EXP_COMMAND,
                [U64(transaction), U64(pcid), U64(cli_key), exp, quota, last_update],
            ) => Self::UpdateAccountExp {
                transaction: *transaction,
                pcid: *pcid,
                cli_key: *cli_key,
                exp: exp_from(exp, quota, last_update)?,
            },
            (
                UPDATE_ACCOUNT_CONTENT_QUOTA_COMMAND,
                [U64(transaction), U64(pcid), U64(cli_key), quotas],
            ) => Self::UpdateAccountContentQuota {
                transaction: *transaction,
                pcid: *pcid,
                cli_key: *cli_key,
                quotas: quota_from(quotas)?,
            },
            (
                UPDATE_ACCOUNT_RECYCLE_TIME_COMMAND,
                [U64(transaction), U64(pcid), U64(cli_key), recycle],
            ) => Self::UpdateAccountRecycleTime {
                transaction: *transaction,
                pcid: *pcid,
                cli_key: *cli_key,
                recycle: recycle_from(recycle)?,
            },
            (command, [U64(transaction), rest @ ..]) if reply_shape(command).is_some() => {
                let (_, has_pcid, has_reason) = reply_shape(command).unwrap_or(("", false, false));
                let (pcid, rest) = match (has_pcid, rest) {
                    (true, [U64(pcid), rest @ ..]) => (Some(*pcid), rest),
                    (false, rest) => (None, rest),
                    _ => return Err(PacketError::ValueCount),
                };
                let reason = match (has_reason, rest) {
                    (true, [I16(reason)]) => Some(*reason),
                    (false, []) => None,
                    _ => return Err(PacketError::ValueCount),
                };
                Self::Reply {
                    command,
                    transaction: *transaction,
                    pcid,
                    reason,
                }
            }
            _ => return Err(PacketError::ValueCount),
        };
        Ok(Some(message))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let messages = [
            Message::Hello {
                cluster: 1,
                world: 2,
                version: PACKET_VERSION,
                port: 3,
            },
            Message::HelloOk,
            Message::HelloFail(12),
            Message::WorldLive {
                cluster: 1,
                world: 2,
            },
            Message::Enter {
                transaction: 9,
                client_key: 8,
                token: 7,
                address: 6,
            },
            Message::EnterOk {
                transaction: 9,
                pcid: 60005,
                account: [4; 16],
            },
            Message::EnterFail {
                transaction: 9,
                reason: 16,
            },
            Message::Create {
                transaction: 1,
                pcid: 2,
                account: [3; 16],
                external: vec![1, 2],
                address: 5,
                name: vec![0x41],
                make_pc: vec![9; 30],
                jumping: false,
                top_level: 60,
            },
            Message::CreateOk {
                transaction: 1,
                job: 2,
                level: 1,
                showcase: vec![7; 40],
            },
            Message::CreateFail {
                transaction: 1,
                reason: 25,
            },
        ];
        for message in messages {
            let packet = message.encode().unwrap();
            assert_eq!(Message::decode(&packet).unwrap(), Some(message));
        }
        // Hello reply: 96 bytes + 18 for each rule (381 rule entry `hiiii`; 07-gamed-link.md). ❓ The spec size has other framing.
        assert_eq!(
            Message::HelloOk.encode().unwrap().len(),
            2 + 2 + 2 + 2 * 31 + 28
        );
        assert_eq!(Message::decode(&[4, 0, 99, 0]).unwrap(), None);
    }

    #[test]
    fn hello_ok_381_format() {
        assert_eq!(HELLO_OK, "hCQiiiqqChiiii;;qqqi");
        assert_eq!(PACKET_VERSION, 83);
    }

    #[test]
    fn enter_ok_381_tail() {
        let message = Message::EnterOk {
            transaction: 9,
            pcid: 60005,
            account: [4; 16],
        };
        let packet = message.encode().unwrap();
        let limits = DecodeLimits {
            blob_bytes: 4096,
            string_units: 256,
            property_bytes: 0,
        };
        let mut values = decompose(ENTER_OK, &packet, limits).unwrap();
        let n = values.len();
        assert_eq!(values[n - 2], Value::Repeat(Vec::new()));
        assert_eq!(values[n - 1], Value::Repeat(Vec::new()));
        // Non-empty new lists: content quota (i, h, q, q) and recycle time (h, q).
        values[n - 2] = Value::Repeat(vec![vec![
            Value::U32(1),
            Value::I16(2),
            Value::U64(3),
            Value::U64(4),
        ]]);
        values[n - 1] = Value::Repeat(vec![vec![Value::I16(5), Value::U64(6)]]);
        let packet = compose(ENTER_OK, &values).unwrap();
        let Some(Message::EnterOkData { lists, .. }) = Message::decode(&packet).unwrap() else {
            panic!("EnterOkData expected");
        };
        assert_eq!(lists.quotas.len(), 1);
        assert_eq!(lists.quotas[0].version, 2);
        assert_eq!(lists.recycle, vec![RecycleTime { group: 5, time: 6 }]);
    }

    fn sample_lists() -> AccountLists {
        AccountLists {
            time_based: vec![TimePlayInfo {
                target: 1,
                schedule: 7,
                expire: 1_700_000_000,
                last_update: 1_699_999_000,
                duration: 3600,
                acknowledged: 2,
                rewards: [1, 0, 1, 0],
            }],
            exp: AccountExpInfo {
                exp: 500,
                quota_per_day: 40,
                last_update: 1_699_990_000,
            },
            quotas: vec![ContentQuota {
                data_id: 9,
                version: 3,
                quantity: 12,
                last_update: 1_699_980_000,
            }],
            recycle: vec![RecycleTime {
                group: 4,
                time: 1_699_970_000,
            }],
        }
    }

    #[test]
    fn message_round_trip_381() {
        let messages = [
            Message::UpdateBotScore {
                pcid: 1,
                cli_key: 2,
                score: -29,
            },
            Message::DeleteCharacter {
                transaction: 3,
                pcid: 4,
                external: vec![1, 2, 3],
                address: 0x0100_007f,
            },
            Message::DeleteCharacterOk {
                transaction: 3,
                result: 1,
            },
            Message::DeleteCharacterFail {
                transaction: 3,
                reason: 2,
            },
            Message::RestoreCharacter {
                transaction: 5,
                pcid: 4,
            },
            Message::RestoreCharacterOk { transaction: 5 },
            Message::RestoreCharacterFail {
                transaction: 5,
                reason: 2,
            },
            Message::UpdateCharacterData {
                transaction: 6,
                pcid: 4,
                cli_key: 77,
                level: 45,
                showcase: vec![8; 12],
            },
            Message::UpdateAccountData {
                transaction: 7,
                pcid: 4,
                cli_key: 77,
                bot_score: 29,
                lists: Box::new(sample_lists()),
            },
            Message::UpdateAccountExp {
                transaction: 8,
                pcid: 4,
                cli_key: 77,
                exp: sample_lists().exp,
            },
            Message::UpdateAccountContentQuota {
                transaction: 9,
                pcid: 4,
                cli_key: 77,
                quotas: sample_lists().quotas,
            },
            Message::UpdateAccountRecycleTime {
                transaction: 10,
                pcid: 4,
                cli_key: 77,
                recycle: sample_lists().recycle,
            },
            Message::EnterOkData {
                transaction: 11,
                pcid: 4,
                account: [5; 16],
                lists: Box::new(sample_lists()),
            },
            Message::HelloOkData(Box::new(sample_hello())),
        ];
        for message in messages {
            let packet = message.encode().unwrap();
            assert_eq!(Message::decode(&packet).unwrap(), Some(message));
        }
        let replies = [
            (UPDATE_CHARACTER_DATA_OK_COMMAND, None, None),
            (UPDATE_CHARACTER_DATA_FAIL_COMMAND, None, Some(5)),
            (UPDATE_ACCOUNT_DATA_OK_COMMAND, None, None),
            (UPDATE_ACCOUNT_DATA_FAIL_COMMAND, None, Some(5)),
            (UPDATE_ACCOUNT_EXP_OK_COMMAND, Some(4), None),
            (UPDATE_ACCOUNT_EXP_FAIL_COMMAND, Some(4), Some(5)),
            (UPDATE_ACCOUNT_CONTENT_QUOTA_OK_COMMAND, Some(4), None),
            (UPDATE_ACCOUNT_CONTENT_QUOTA_FAIL_COMMAND, Some(4), Some(5)),
            (UPDATE_ACCOUNT_RECYCLE_TIME_OK_COMMAND, Some(4), None),
            (UPDATE_ACCOUNT_RECYCLE_TIME_FAIL_COMMAND, Some(4), Some(5)),
        ];
        for (command, pcid, reason) in replies {
            let message = Message::Reply {
                command,
                transaction: 12,
                pcid,
                reason,
            };
            let packet = message.encode().unwrap();
            assert_eq!(Message::decode(&packet).unwrap(), Some(message));
        }
        // A reply with the wrong fields for its command is an error, not a wrong frame.
        let wrong = Message::Reply {
            command: UPDATE_ACCOUNT_EXP_OK_COMMAND,
            transaction: 1,
            pcid: None,
            reason: None,
        };
        assert!(wrong.encode().is_err());
        let unknown = Message::Reply {
            command: 99,
            transaction: 1,
            pcid: None,
            reason: None,
        };
        assert!(unknown.encode().is_err());
    }

    fn sample_hello() -> HelloData {
        let mut data = HelloData::default();
        data.schedules[0].rules.push(ScheduleRule {
            id: 3,
            values: [1, 2, 3, 4],
        });
        data.schedules[1].start = 1_700_000_000;
        data.random_store = [11, 22, 33];
        data.mileage_days = 30;
        data
    }

    #[test]
    fn hello_ok_data_and_rule_limit() {
        let packet = Message::HelloOkData(Box::new(sample_hello()))
            .encode()
            .unwrap();
        // 2 + 2 + 2 + (2 schedules of 31 bytes) + 18 for the one rule + 28 tail.
        assert_eq!(packet.len(), 2 + 2 + 2 + 2 * 31 + 18 + 28);
        let mut data = HelloData::default();
        data.schedules[0].rules = vec![ScheduleRule::default(); MAX_RULES + 1];
        assert!(Message::HelloOkData(Box::new(data)).encode().is_err());
        // The default data decodes as the unit variant.
        let packet = Message::HelloOkData(Box::default()).encode().unwrap();
        assert_eq!(Message::decode(&packet).unwrap(), Some(Message::HelloOk));
    }

    #[test]
    fn update_account_data_381_bytes() {
        assert_eq!(ACCOUNT_DATA, "hqqqiCQiqqihcccc;qiqCihqq;Chq;");
        // Build the bytes with the format by hand: one entry in each list.
        let mut body = Vec::new();
        body.extend(UPDATE_ACCOUNT_DATA_COMMAND.to_le_bytes());
        body.extend(1u64.to_le_bytes()); // transaction
        body.extend(2u64.to_le_bytes()); // pcid
        body.extend(3u64.to_le_bytes()); // client key
        body.extend(29u32.to_le_bytes()); // bot score
        body.extend(1i16.to_le_bytes()); // time list count
        body.push(1); // Q target
        body.extend(7u32.to_le_bytes()); // i schedule
        body.extend(100u64.to_le_bytes()); // q expire
        body.extend(50u64.to_le_bytes()); // q last update
        body.extend(60u32.to_le_bytes()); // i duration
        body.extend(2i16.to_le_bytes()); // h
        body.extend([1, 2, 3, 4]); // cccc
        body.extend(500u64.to_le_bytes()); // exp q
        body.extend(40u32.to_le_bytes()); // quota i
        body.extend(900u64.to_le_bytes()); // last update q
        body.extend(1i16.to_le_bytes()); // quota list count
        body.extend(9u32.to_le_bytes());
        body.extend(3i16.to_le_bytes());
        body.extend(12u64.to_le_bytes());
        body.extend(13u64.to_le_bytes());
        body.extend(1i16.to_le_bytes()); // recycle list count
        body.extend(4i16.to_le_bytes());
        body.extend(77u64.to_le_bytes());
        let mut packet = Vec::new();
        packet.extend(((body.len() + 2) as u16).to_le_bytes());
        packet.extend(&body);
        let Some(Message::UpdateAccountData {
            transaction,
            pcid,
            cli_key,
            bot_score,
            lists,
        }) = Message::decode(&packet).unwrap()
        else {
            panic!("UpdateAccountData expected");
        };
        assert_eq!((transaction, pcid, cli_key, bot_score), (1, 2, 3, 29));
        assert_eq!(lists.time_based[0].schedule, 7);
        assert_eq!(lists.time_based[0].rewards, [1, 2, 3, 4]);
        assert_eq!(lists.exp.exp, 500);
        assert_eq!(lists.quotas[0].quantity, 12);
        assert_eq!(lists.recycle[0].time, 77);
        // The Ok reply is `hq`: size, command, transaction.
        let reply = Message::Reply {
            command: UPDATE_ACCOUNT_DATA_OK_COMMAND,
            transaction: 1,
            pcid: None,
            reason: None,
        };
        assert_eq!(reply.encode().unwrap().len(), 12);
        // A cut packet is an error, not a panic.
        let mut cut = packet.clone();
        cut.truncate(cut.len() - 5);
        assert!(Message::decode(&cut).is_err());
    }

    #[test]
    fn message_round_trip() {
        let messages = [
            Message::LogoutNotify {
                pc_db_id: 60005,
                cli_key: 77,
            },
            Message::Kick {
                transaction: 4,
                client: 77,
                pcid: 60005,
                reason: 8,
            },
            Message::KickOk { transaction: 4 },
            Message::KickFail {
                transaction: 5,
                reason: 24,
            },
        ];
        for message in messages {
            let packet = message.encode().unwrap();
            assert_eq!(Message::decode(&packet).unwrap(), Some(message));
        }
        // Frame sizes: `hqqqqQ` = 2 + 2 + 4 * 8 + 1, `hq` = 2 + 2 + 8, `hqE` = 2 + 2 + 8 + 2.
        let size = |m: Message| m.encode().unwrap().len();
        assert_eq!(
            size(Message::Kick {
                transaction: 1,
                client: 2,
                pcid: 3,
                reason: 4
            }),
            37
        );
        assert_eq!(size(Message::KickOk { transaction: 1 }), 12);
        // A short body is an error, not a panic.
        let mut short = Message::KickOk { transaction: 1 }.encode().unwrap();
        short.truncate(short.len() - 3);
        assert!(Message::decode(&short).is_err());
    }

    #[test]
    fn leave_messages_round_trip() {
        let messages = [
            Message::Leave {
                transaction: 1,
                pc_db_id: 60005,
                cli_key: 77,
            },
            Message::LeaveOk {
                transaction: 2,
                token: 99,
            },
            Message::LeaveFail {
                transaction: 3,
                reason: 22,
            },
            Message::LogoutNotify {
                pc_db_id: 60005,
                cli_key: 77,
            },
        ];
        for message in messages {
            let packet = message.encode().unwrap();
            assert_eq!(Message::decode(&packet).unwrap(), Some(message));
        }
        // A short body is an error, not a panic.
        let mut short = Message::LeaveOk {
            transaction: 2,
            token: 99,
        }
        .encode()
        .unwrap();
        short.truncate(short.len() - 3);
        assert!(Message::decode(&short).is_err());
    }
}
