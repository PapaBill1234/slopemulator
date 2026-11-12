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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    /// 1. `port` is the internal port of the GameDaemon.
    Hello {
        cluster: i16,
        world: i16,
        version: i16,
        port: i16,
    },
    /// 2. Two schedules without rules and a zero tail.
    HelloOk,
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
}

fn list() -> Value {
    Value::Repeat(Vec::new())
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
            Self::HelloOk => {
                let schedule =
                    |kind| vec![I8(kind), U32(0), U32(0), U32(0), U64(0), U64(0), list()];
                compose(
                    HELLO_OK,
                    &[
                        I16(2),
                        Repeat(vec![schedule(0), schedule(1)]),
                        U64(0),
                        U64(0),
                        U64(0),
                        U32(0),
                    ],
                )
            }
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
            } => {
                let blob = || Bytes(Vec::new());
                let text = || String(Vec::new());
                compose(
                    ENTER_OK,
                    &[
                        I16(6),
                        U32(0),
                        U64(*transaction),
                        U64(*pcid),
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
                        list(),
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
                        U64(0),
                        U32(0),
                        U64(0),
                        // Content quota list (i, h, q, q) and recycle time list (h, q): empty.
                        list(),
                        list(),
                    ],
                )
            }
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
            7 | 22 | LEAVE_FAIL_COMMAND | KICK_FAIL_COMMAND => FAILURE,
            KICK_COMMAND => KICK,
            KICK_OK_COMMAND => KICK_OK,
            LEAVE_COMMAND => LEAVE,
            LEAVE_OK_COMMAND => LEAVE_OK,
            LOGOUT_NOTIFY_COMMAND => LOGOUT_NOTIFY,
            20 => CREATE,
            21 => CREATE_OK,
            _ => return Ok(None),
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
            (2, _) => Self::HelloOk,
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
            (6, [_, U64(transaction), U64(pcid), Id(account), ..]) => Self::EnterOk {
                transaction: *transaction,
                pcid: *pcid,
                account: *account,
            },
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
        assert_eq!(Message::decode(&packet).unwrap(), Some(message));
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
