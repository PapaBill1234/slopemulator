//! Build 307 LobbyGate packets. Shared codec is used without changes.
use crate::store::{Account, Character};
use bloodlib::packet::{compose, decompose, DecodeLimits, Value};
use packet::fragments::compose_fragments;
use std::io;
use tokio::io::{AsyncRead, AsyncReadExt};

pub const HELLO: &str = "hiChhsQQiihhiQQQQQQQ;QQqCqqiQiic;";
pub const START: &str = "hiqQoihCiiiii;CiiQiqq;Qqiqiq";
pub const CHARACTERS: &str = "hiqqhCqohsbqQQi;";
pub fn invalid(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}
pub fn packet(format: &str, fields: &[Value]) -> io::Result<Vec<u8>> {
    compose(format, fields).map_err(invalid)
}
pub fn fields(format: &str, packet: &[u8]) -> io::Result<Vec<Value>> {
    decompose(
        format,
        packet,
        DecodeLimits {
            blob_bytes: 1024,
            string_units: 65,
            property_bytes: 1024,
        },
    )
    .map_err(invalid)
}
/// The first `q` field of a LobbyGate packet (client key or gate id). 0 if absent.
pub fn key(packet: &[u8]) -> u64 {
    packet
        .get(4..12)
        .and_then(|v| v.first_chunk::<8>())
        .map_or(0, |v| u64::from_le_bytes(*v))
}
pub fn command(packet: &[u8]) -> io::Result<i16> {
    if packet.len() < 4
        || packet.len() > 4000
        || i16::from_le_bytes([packet[0], packet[1]]) as usize != packet.len()
    {
        return Err(invalid("invalid trusted frame"));
    }
    Ok(i16::from_le_bytes([packet[2], packet[3]]))
}
pub async fn read(reader: &mut (impl AsyncRead + Unpin)) -> io::Result<Option<Vec<u8>>> {
    let mut header = [0; 2];
    if reader.read(&mut header[..1]).await? == 0 {
        return Ok(None);
    }
    reader.read_exact(&mut header[1..]).await?;
    let size = i16::from_le_bytes(header);
    if !(4..=4000).contains(&size) {
        return Err(invalid("invalid trusted length"));
    }
    let mut packet = vec![0; size as usize];
    packet[..2].copy_from_slice(&header);
    reader.read_exact(&mut packet[2..]).await?;
    Ok(Some(packet))
}
pub fn failure(cmd: i16, key: u64, reason: i16) -> io::Result<Vec<u8>> {
    packet(
        "hqE",
        &[Value::I16(cmd), Value::U64(key), Value::I16(reason)],
    )
}
/// The trailing short is an explicit local zero until its source is checked.
pub fn world_token(key: u64, token: u64) -> io::Result<Vec<u8>> {
    packet(
        "hqqh",
        &[
            Value::I16(25),
            Value::U64(key),
            Value::U64(token),
            Value::I16(0),
        ],
    )
}
pub fn world_wait(key: u64, wait: &crate::gamed::WaitInfo) -> io::Result<Vec<u8>> {
    packet(
        "hqiiCch;",
        &[
            Value::I16(27),
            Value::U64(key),
            Value::U32(wait.first),
            Value::U32(wait.second),
            Value::Repeat(
                wait.rows
                    .iter()
                    .map(|(a, b)| vec![Value::U8(*a), Value::I16(*b)])
                    .collect(),
            ),
        ],
    )
}
pub fn notice(cmd: i16, key: u64) -> io::Result<Vec<u8>> {
    packet("hq", &[Value::I16(cmd), Value::U64(key)])
}
pub fn kick(key: u64, reason: i8) -> io::Result<Vec<u8>> {
    packet("hqQ", &[Value::I16(22), Value::U64(key), Value::I8(reason)])
}
/// Local empty world/schedule snapshot. No live world or charge service is claimed.
/// One world row of HelloOk (D2). Field order: LobbyGate spec 05 §2.2.
/// Local fixture only: the original builds rows from the world registry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorldInfo {
    pub id: i16,
    pub cluster: i16,
    /// At most 16 UTF-16 units (LobbyGate decode limit).
    pub name: String,
    pub public_ip: std::net::Ipv4Addr,
    pub internal_ip: std::net::Ipv4Addr,
    pub public_port: u16,
    /// ❓ worldType/playType values are not established; 0 is a local choice.
    pub world_type: u8,
    pub play_type: u8,
}
impl WorldInfo {
    fn row(&self) -> io::Result<Vec<Value>> {
        let name: Vec<u16> = self.name.encode_utf16().collect();
        if name.is_empty() || name.len() > 16 {
            return Err(invalid("world name must have 1..=16 UTF-16 units"));
        }
        let flag = |v: bool| Value::I8(v as i8);
        Ok(vec![
            Value::I16(self.id),
            Value::I16(self.cluster),
            Value::String(name),
            Value::I8(self.world_type as i8),
            Value::I8(self.play_type as i8),
            // Raw in_addr bytes, as LobbyGate writes client IPs (server.rs).
            Value::U32(u32::from_le_bytes(self.public_ip.octets())),
            Value::U32(u32::from_le_bytes(self.internal_ip.octets())),
            Value::U16(self.public_port),
            Value::U16(0),
            Value::I32(0),
            flag(true),   // worldLive
            flag(true),   // entrance
            flag(true),   // characterCreation
            flag(false),  // characterCreationLimitedOnNewAccount
            flag(false),  // recommend
            flag(false),  // brandnew
            Value::I8(0), // crowdness
        ])
    }
}
pub fn hello(worlds: &[WorldInfo]) -> io::Result<Vec<Vec<u8>>> {
    let rows = worlds
        .iter()
        .map(WorldInfo::row)
        .collect::<io::Result<Vec<_>>>()?;
    compose_fragments(
        2,
        HELLO,
        &[
            Value::Repeat(rows),
            Value::I8(0),
            Value::I8(0),
            Value::U64(0),
            Value::Repeat(vec![]),
        ],
    )
    .map_err(invalid)
}
pub fn start(key: u64, account: &Account) -> io::Result<Vec<Vec<u8>>> {
    compose_fragments(
        10,
        START,
        &[
            Value::U64(key),
            Value::I8(0),
            Value::Id(account.id),
            Value::U32(0),
            Value::U16(0),
            Value::Repeat(vec![]),
            Value::Repeat(vec![]),
            Value::I8(0),
            Value::U64(0),
            Value::I32(0),
            Value::U64(0),
            Value::I32(0),
            Value::U64(0),
        ],
    )
    .map_err(invalid)
}
/// Retained login uses the existing empty optional charge/EXP policy.
pub fn start_retained(key: u64) -> io::Result<Vec<Vec<u8>>> {
    compose_fragments(
        16,
        "hiqCiiiii;CiiQiqq;iqiq",
        &[
            Value::U64(key),
            Value::Repeat(vec![]),
            Value::Repeat(vec![]),
            Value::I32(0),
            Value::U64(0),
            Value::I32(0),
            Value::U64(0),
        ],
    )
    .map_err(invalid)
}
/// Current local cache fields are wire-ready fixtures, not target DB states/times.
/// See spec/lobbyd/11-character-select.md before adding nonzero timestamp support.
pub(crate) fn character_row(c: &Character) -> io::Result<Vec<Value>> {
    if c.name.len() > 32 || c.showcase.len() > 512 {
        return Err(invalid("character exceeds DB bounds"));
    }
    Ok(vec![
        Value::U64(c.pcid),
        Value::Id(c.slot),
        Value::I16(c.world),
        Value::String(c.name.clone()),
        Value::Bytes(c.showcase.clone()),
        Value::U64(c.deletion_time),
        Value::I8(c.state),
        Value::I8(c.jumping_state),
        Value::U32(c.last_play_end),
    ])
}
pub fn character_info(key: u64, character: &Character) -> io::Result<Vec<u8>> {
    let mut values = vec![Value::I16(39), Value::U64(key)];
    values.extend(character_row(character)?);
    packet("hqqohsbqQQi", &values)
}
pub fn character_info_failure(key: u64, slot: [u8; 16], reason: i16) -> io::Result<Vec<u8>> {
    packet(
        "hqoE",
        &[
            Value::I16(40),
            Value::U64(key),
            Value::Id(slot),
            Value::I16(reason),
        ],
    )
}
/// D80 GetServiceEntryListOk with no rows.
pub fn service_entries(key: u64) -> io::Result<Vec<Vec<u8>>> {
    compose_fragments(80, "hiqCqQqohq;", &[Value::U64(key), Value::Repeat(vec![])]).map_err(invalid)
}
pub fn characters(key: u64, account: &Account, rows: &[Character]) -> io::Result<Vec<Vec<u8>>> {
    let rows = rows
        .iter()
        .map(character_row)
        .collect::<io::Result<Vec<_>>>()?;
    compose_fragments(
        36,
        CHARACTERS,
        &[
            Value::U64(key),
            Value::U64(account.representative_pcid),
            Value::I16(account.representative_world),
            Value::Repeat(rows),
        ],
    )
    .map_err(invalid)
}
