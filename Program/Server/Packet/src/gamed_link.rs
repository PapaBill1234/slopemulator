//! LobbyDaemon ↔ GameDaemon link messages (GameDaemon dials LobbyDaemon).
//! Spec: Docs/Server/LobbyDaemon/07-gamed-link.md. Frame: `size:u16, command:u16, body`.
//! Sources: lobbyd 307 `0x1400722B0`, `0x140083D60`; GameDaemon 381 `0x14013F0F0` (20),
//! `0x140149920` (21), format strings `0x14099BD50`, `0x14099D280`, `0x1409A8D88`, `0x1409449E4`.
//! ❓ Command 6 uses the build-307 field list. The build-381 list is not verified.
//! ❓ Command 6 sends empty lists and zero account fields except the PC id and the account id.
use bloodlib::packet::{compose, decompose, DecodeLimits, PacketError, Value};

/// ❓ Build 348 value. Build 307 uses 80. The build-381 value is not verified.
pub const PACKET_VERSION: i16 = 83;

const HELLO: &str = "hhhhh";
const HELLO_OK: &str = "hCQiiiqqChii;;qqqi";
const ERROR: &str = "hE";
const WORLD: &str = "hhh";
const ENTER: &str = "hqqqi";
const ENTER_OK: &str = "hiqqocbbbbssQhhiQCQiqqihcc;Cqhs;iQQQsCiiiii;iCiiQiqq;Ciqq;qqiq";
const FAILURE: &str = "hqE";
const CREATE: &str = "hqqobisbQh";
const CREATE_OK: &str = "hqhhb";

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
                    ],
                )
            }
            Self::EnterFail {
                transaction,
                reason,
            } => compose(FAILURE, &[I16(7), U64(*transaction), I16(*reason)]),
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
            7 | 22 => FAILURE,
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
        // Hello reply: 96 bytes + 10 for each rule (07-gamed-link.md). ❓ The spec size has other framing.
        assert_eq!(
            Message::HelloOk.encode().unwrap().len(),
            2 + 2 + 2 + 2 * 31 + 28
        );
        assert_eq!(Message::decode(&[4, 0, 99, 0]).unwrap(), None);
    }
}
