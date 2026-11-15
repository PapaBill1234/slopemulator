//! Packet builders of the world entry. No I/O. Spec: Docs/Server/GameDaemon/04, 05, 06.
//! Hello 11 → 12 (RSA handshake, then AES-128 session), 24, 25, 32 (fragments), 87, 90, 423,
//! 1122, 1126, 836, 41, 43, 44, 930, 931. Source rule: logic from 2019 + PDB, packets from 381.
use crate::db::Pc;
use bloodlib::{
    crypto::{DhParameters, RsaKey},
    frames::{decode_client, encode_client, ClientDecrypt, ClientEncrypt},
    packet::{compose, decompose, DecodeLimits, Value},
    session::SessionPacketCrypt,
};
use packet::{world_entry::Fragments, zone_entry::PrepareZone};
use rand::rngs::OsRng;
use std::io;

#[path = "property381.rs"]
pub mod property381;
#[path = "rules381.rs"]
pub mod rules381;

use rules381::{Kind, RULES};

const PROTOCOL: i16 = 1278;

pub fn bad(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}

/// PC object id: type 1 in bits 48.., index in bits 0..16
/// (build 348 `World::initPcManager` 0x1403EDDC0, `GameObjectIdManager::Make` 0x14001D090).
/// ❓ Local: one fixed index. The original takes the next free index (1..19999).
pub const PC_OBJECT_ID: u64 = (1 << 48) + 1;
/// Spawn action state in which command 42 is allowed (05 "Loading and command 42").
pub const SPAWN_STATE_WAIT: u8 = 2;
/// Reason of 44 for a wrong key or state of command 42 (05 "Loading and command 42").
pub const ENTER_ZONE_BAD_KEY: i16 = 1150;
/// Reason of 931 when a second list request comes inside 30 s (05 "Request handler").
pub const RULE_LIST_TOO_SOON: i16 = 2167;
/// Reason of 25 for a request that is too short. ❓ Local value.
pub const TOKEN_BAD_REQUEST: i16 = 2;
/// Reason of 33 for a second command 31. ❓ Local value.
pub const ENTER_WORLD_TWICE: i16 = 2;
/// Version of the rule list (05 "Command 930 body").
const RULE_MAJOR: i16 = 74;
const RULE_MINOR: i16 = 0;
/// Revision of the rule list before the first reload.
const RULE_REVISION: i32 = -1;
/// Longest element of 930 (05 "Command 930 body").
const ELEMENT_MAX: usize = 260;
/// Zone key of command 41. ❓ Local: the key is per zone and made at run time in the original,
/// and this server sends no broadcast packets. The fixed text keeps the field filled.
const ZONE_KEY: &[u8] = b"keyhere";
/// Bytes of the counted broadcast block of command 32. ❓ Local: size and content.
const BROADCAST_BLOCK: [u8; 16] = [0; 16];
/// Max hp and speed of level 1 (`pcdata.xml` record 1). ❓ The original selects the record by
/// race, sex, job and level.
const MAX_HP_LEVEL_1: u64 = 303;
const SPEED_LEVEL_1: u64 = 62;

fn make(format: &str, fields: &[Value]) -> io::Result<Vec<u8>> {
    compose(format, fields).map_err(bad)
}

fn size_and_command(command: u16, body: &[u8]) -> Vec<u8> {
    let mut packet = ((body.len() + 4) as u16).to_le_bytes().to_vec();
    packet.extend(command.to_le_bytes());
    packet.extend(body);
    packet
}

/// Command 24 `hQiQih` (05 "Command 24"). Free-to-play account: body
/// `00 FF FF FF FF 00 00 00 00 00 00 00`. `apply_type` is the first field.
pub fn token_ok(apply_type: u8) -> Vec<u8> {
    let mut body = vec![apply_type];
    body.extend((-1i32).to_le_bytes());
    body.extend([0; 7]);
    size_and_command(crate::ids::G_LobbyTokenAuthOkCmd, &body)
}

/// Command 25 `hh`, reason.
pub fn token_fail(reason: i16) -> io::Result<Vec<u8>> {
    make(
        "hh",
        &[
            Value::I16(crate::ids::G_LobbyTokenAuthFailCmd as i16),
            Value::I16(reason),
        ],
    )
}

/// Command 33 `hh`, reason. ❓ The format of 33 is not verified.
pub fn enter_world_fail(reason: i16) -> io::Result<Vec<u8>> {
    make(
        "hh",
        &[
            Value::I16(crate::ids::G_EnterWorldFailCmd as i16),
            Value::I16(reason),
        ],
    )
}

/// Command 44 `hh`, reason. ❓ The format of 44 is not verified.
pub fn enter_zone_fail(reason: i16) -> io::Result<Vec<u8>> {
    make(
        "hh",
        &[
            Value::I16(crate::ids::G_EnterZoneFailCmd as i16),
            Value::I16(reason),
        ],
    )
}

/// Command 931 `hE`, reason.
pub fn rule_list_fail(reason: i16) -> io::Result<Vec<u8>> {
    make(
        "hE",
        &[
            Value::I16(crate::ids::G_ContentsRuleListFailCmd as i16),
            Value::I16(reason),
        ],
    )
}

/// Command 17 `hqq`, sequence and timestamp (06 "Ordinary ping packets").
pub fn ping(sequence: u64, timestamp: u64) -> io::Result<Vec<u8>> {
    make(
        "hqq",
        &[
            Value::I16(crate::ids::G_ServerPingNotifyCmd as i16),
            Value::U64(sequence),
            Value::U64(timestamp),
        ],
    )
}

/// Command 43 `hq`. `enter_social_id` is 0 for a first entry (05 "Command 43").
pub fn enter_zone_ok(enter_social_id: u64) -> Vec<u8> {
    size_and_command(crate::ids::G_EnterZoneOkCmd, &enter_social_id.to_le_bytes())
}

/// Value of a fixed-size property row of a new level-1 PC (05 "Command 32 property table").
/// All ids that are not named here are 0.
fn scalar(entry: &Pc, id: u16) -> u64 {
    match id {
        1 => 1, // world-id
        2 => entry.pc.race.into(),
        3 => entry.pc.sex.into(),
        4 => entry.pc.job.into(),
        5..=96 => entry
            .pc
            .appearance
            .get(usize::from(id) - 5)
            .copied()
            .map_or(0, u64::from),
        98 => entry.zone.into(),
        99..=101 => entry.position[usize::from(id) - 99] as u16 as u64,
        102 => entry.yaw.into(),
        103 => entry.level.into(),
        107 => entry.hp,
        244 => MAX_HP_LEVEL_1,
        250 => SPEED_LEVEL_1,
        _ => 0,
    }
}

/// Command 32 for a new level-1 character, as fragments. The property blob follows the
/// 381 `GetPureData` rules (1,024 bytes + 2 for each name character).
fn initial_composite(entry: &Pc) -> io::Result<Vec<Vec<u8>>> {
    let mut out = Fragments::new(crate::ids::G_EnterWorldOkCmd);
    // Header, 9 fields. ❓ Values of fields 4–9 (chat address, chat port, chat world, manage
    // port, bias, session word) are local; the spec gives only the types.
    let mut header = 1u16.to_le_bytes().to_vec();
    header.extend(PC_OBJECT_ID.to_le_bytes());
    header.push(0);
    header.extend(0u32.to_le_bytes());
    header.extend(0u16.to_le_bytes());
    header.extend(1u16.to_le_bytes());
    header.extend(0u16.to_le_bytes());
    header.extend(0i16.to_le_bytes());
    header.extend(2u16.to_le_bytes());
    out.push(header).map_err(bad)?;
    out.push(property381::encode_blob(
        |id| scalar(entry, id),
        &entry.name,
        &[],
    ))
    .map_err(bad)?;
    out.push(0i16.to_le_bytes().to_vec()).map_err(bad)?; // extra properties, counted
    let mut block = (BROADCAST_BLOCK.len() as i16).to_le_bytes().to_vec();
    block.extend(BROADCAST_BLOCK);
    out.push(block).map_err(bad)?;
    out.push(0u16.to_le_bytes().to_vec()).map_err(bad)?; // recycle count
    out.push(2u16.to_le_bytes().to_vec()).map_err(bad)?;
    for page in 1..=2u8 {
        let mut row = vec![page];
        row.extend(0i16.to_le_bytes()); // acquired skills, counted
        out.push(row).map_err(bad)?;
    }
    out.push(vec![0; 4]).map_err(bad)?; // config bytes
    let mut identity = vec![0u8]; // pc cafe publisher
    identity.extend(entry.pcid.to_le_bytes());
    out.push(identity).map_err(bad)?;
    out.push(0u16.to_le_bytes().to_vec()).map_err(bad)?; // content bans
    out.push(vec![0]).map_err(bad)?; // test server flag
    out.push(0u16.to_le_bytes().to_vec()).map_err(bad)?; // craft unions
    out.push(0u16.to_le_bytes().to_vec()).map_err(bad)?; // time-based play
    out.push(2u16.to_le_bytes().to_vec()).map_err(bad)?;
    for page in 1..=2u8 {
        let mut row = vec![page];
        for _ in 0..6 {
            row.extend(0u32.to_le_bytes());
            row.push(0);
        }
        row.extend([0; 18]); // 9 stones
        row.extend(0u16.to_le_bytes());
        row.push(0);
        out.push(row).map_err(bad)?;
    }
    out.finish().map_err(bad)
}

/// Command 87 `hC…;`: no active quest.
fn quest_list() -> io::Result<Vec<u8>> {
    make(
        "hChcccchchchch;",
        &[
            Value::I16(crate::ids::G_QuestListOkCmd as i16),
            Value::Repeat(Vec::new()),
        ],
    )
}

/// Command 90 `hbbb`: three empty lists. ❓ The exact empty length is not traced.
fn quest_complete_list() -> io::Result<Vec<u8>> {
    make(
        "hbbb",
        &[
            Value::I16(crate::ids::G_QuestCompleteListOkCmd as i16),
            Value::Bytes(Vec::new()),
            Value::Bytes(Vec::new()),
            Value::Bytes(Vec::new()),
        ],
    )
}

/// Command 423 `hq`. ❓ The unit is not checked: Unix seconds here.
fn current_utc_time(utc_secs: u64) -> io::Result<Vec<u8>> {
    make(
        "hq",
        &[
            Value::I16(crate::ids::G_CurrentUTCTimeOkCmd as i16),
            Value::U64(utc_secs),
        ],
    )
}

/// Command 1122 `hiCiq;`: count 0. ❓ Value of the field before the count.
fn collecting_list() -> io::Result<Vec<u8>> {
    make(
        "hiCiq;",
        &[
            Value::I16(crate::ids::G_CollectingListOkCmd as i16),
            Value::U32(0),
            Value::Repeat(Vec::new()),
        ],
    )
}

/// Command 1126 `hiChiiq;`: count 0. ❓ Value of the field before the count.
fn fishing_list() -> io::Result<Vec<u8>> {
    make(
        "hiChiiq;",
        &[
            Value::I16(crate::ids::G_FishingListOkCmd as i16),
            Value::U32(0),
            Value::Repeat(Vec::new()),
        ],
    )
}

/// Command 836 `hiiccqiiiiiiii`: 13 energy values. ❓ The meaning of the 13 values is open;
/// a new PC gives 0 in all of them.
fn energy_sync() -> io::Result<Vec<u8>> {
    let mut fields = vec![
        Value::I16(crate::ids::G_EnergyPropertySyncCmd as i16),
        Value::U32(0),
        Value::U32(0),
        Value::U8(0),
        Value::U8(0),
        Value::U64(0),
    ];
    fields.extend((0..8).map(|_| Value::U32(0)));
    make("hiiccqiiiiiiii", &fields)
}

/// Command 32 for a new level-1 character, then 87, 90, 423, 1122, 1126 and 836, in this
/// order (05 "Packets after command 32"). `utc_secs`: the current UTC time for 423.
/// 627, 1097 and 912 are not sent (their conditions are false for the default config).
pub fn enter_world_at(entry: &Pc, utc_secs: u64) -> io::Result<Vec<Vec<u8>>> {
    let mut packets = initial_composite(entry)?;
    packets.push(quest_list()?);
    packets.push(quest_complete_list()?);
    packets.push(current_utc_time(utc_secs)?);
    packets.push(collecting_list()?);
    packets.push(fishing_list()?);
    packets.push(energy_sync()?);
    Ok(packets)
}

/// [`enter_world_at`] with the system clock.
pub fn enter_world(entry: &Pc) -> io::Result<Vec<Vec<u8>>> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    enter_world_at(entry, now)
}

/// Command 41. Field names: build 348 `PcSpawnAction::prepareSpawn` 0x1404B4E50.
/// `counter` is the PC transition counter before this send; the packet carries `counter + 1`.
/// ❓ Local: runtime zone id = geo zone id; the zone key is a fixed text.
pub fn prepare_zone(entry: &Pc, spawn_key: u32, hour: u8, counter: u16) -> io::Result<Vec<u8>> {
    PrepareZone {
        runtime_zone_id: entry.zone.into(),
        channel: 1,
        geo_zone_id: entry.zone,
        correlation: spawn_key,
        config_byte: hour % 24,
        action_flag: 0,
        zone_block: ZONE_KEY.to_vec(),
        position: entry.position,
        yaw: entry.yaw,
        pc_transition: counter.wrapping_add(1),
        summon_transition: 0,
    }
    .encode()
    .map_err(bad)
}

/// Remove XML comments. A comment can hold an option that is not active.
fn without_comments(config: &str) -> String {
    let mut text = String::new();
    let mut source = config;
    while let Some((before, after)) = source.split_once("<!--") {
        text.push_str(before);
        source = after.split_once("-->").map_or("", |(_, tail)| tail);
    }
    text.push_str(source);
    text
}

/// The value of rule `name` in the config text, if there is one.
fn config_value<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    let key = format!("<option name=\"{name}\" value=\"");
    let start = text.find(&key)? + key.len();
    text[start..].split_once('"').map(|(value, _)| value)
}

/// Text of one rule value. A config value that does not parse as the type of the row keeps the
/// default.
fn rule_value(kind: Kind, default: &'static str, config: Option<&str>) -> String {
    let Some(value) = config else {
        return default.to_owned();
    };
    let parsed = match kind {
        Kind::Int => value.trim().parse::<i32>().ok().map(|n| n.to_string()),
        Kind::Long => value.trim().parse::<i64>().ok().map(|n| n.to_string()),
        Kind::Bool => match value.trim() {
            "y" | "yes" | "true" => Some("yes".to_owned()),
            "n" | "no" | "false" => Some("no".to_owned()),
            _ => None,
        },
        // A quote or an angle bracket would break the element.
        Kind::Text => (!value.contains(['\'', '<', '>', '&'])).then(|| value.to_owned()),
    };
    parsed.unwrap_or_else(|| default.to_owned())
}

fn option_element(name: &str, value: &str) -> String {
    format!("<option name='{name}' value='{value}'/>")
}

/// Command 930 `hihhiCb;` (build 381 builder 0x140087D40). Body: major version 74, minor 0,
/// revision −1, count, then counted UTF-8 elements: XML header, one `<option>` for each of the
/// 167 rows in table order, `</config>`. A bool prints `yes` or `no`. A value of `config` (the
/// XML text of the config file) replaces the default of its row; `config` can be empty.
pub fn rule_list(config: &str) -> io::Result<Vec<Vec<u8>>> {
    let text = without_comments(config);
    let mut elements = Vec::with_capacity(RULES.len() + 2);
    elements.push(format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?><config major-version=\"{RULE_MAJOR}\" \
         minor-version=\"{RULE_MINOR}\" module=\"contents-rule-config\" \
         release-module=\"LocalizationData\" release-side=\"client\">"
    ));
    for rule in RULES.iter() {
        let value = rule_value(rule.kind, rule.default, config_value(&text, rule.name));
        let mut element = option_element(rule.name, &value);
        if element.len() > ELEMENT_MAX {
            element = option_element(rule.name, rule.default);
        }
        elements.push(element);
    }
    elements.push(String::from("</config>"));
    let mut out = Fragments::new(crate::ids::G_ContentsRuleListOkCmd);
    let mut head = RULE_MAJOR.to_le_bytes().to_vec();
    head.extend(RULE_MINOR.to_le_bytes());
    head.extend(RULE_REVISION.to_le_bytes());
    head.extend((elements.len() as u16).to_le_bytes());
    out.push(head).map_err(bad)?;
    for element in elements {
        let mut row = (element.len() as u16).to_le_bytes().to_vec();
        row.extend(element.into_bytes());
        out.push(row).map_err(bad)?;
    }
    out.finish().map_err(bad)
}

/// Command 11 `hhqsqb` → command 12 `hb`. The blob layout is the LobbyGate layout.
pub fn hello(key: &RsaKey, frame: &[u8]) -> io::Result<(Vec<u8>, SessionPacketCrypt)> {
    let (reply, session, _) = hello_sizes(key, frame)?;
    Ok((reply, session))
}

/// [`hello`] with the sizes of the plain packets 11 and 12, for the packet log.
pub fn hello_sizes(
    key: &RsaKey,
    frame: &[u8],
) -> io::Result<(Vec<u8>, SessionPacketCrypt, [usize; 2])> {
    let decoded = decode_client(
        frame,
        ClientDecrypt::Handshake {
            key,
            expected_code: 4,
        },
    )
    .map_err(bad)?
    .ok_or_else(|| bad("incomplete Hello"))?;
    let limits = DecodeLimits {
        blob_bytes: 4000,
        ..DecodeLimits::default()
    };
    let fields = decompose("hhqsqb", &decoded.packet, limits).map_err(bad)?;
    let [Value::I16(11), Value::I16(PROTOCOL), _, _, _, Value::Bytes(blob)] = fields.as_slice()
    else {
        return Err(bad("invalid Hello command or version"));
    };
    if blob.len() < 68 || u32::from_le_bytes(blob[..4].try_into().unwrap()) != 64 {
        return Err(bad("client RSA key must be 512 bits"));
    }
    let client = RsaKey::from_public_blob(&blob[..68]).map_err(bad)?;
    let dh = DhParameters::from_blob(&blob[68..])
        .map_err(bad)?
        .exchange(&mut OsRng);
    let reply = compose("hb", &[Value::I16(12), Value::Bytes(dh.reply)]).map_err(bad)?;
    let sizes = [decoded.packet.len(), reply.len()];
    let reply = encode_client(
        &reply,
        ClientEncrypt::Handshake {
            key: &client,
            code: 5,
        },
        &mut OsRng,
    )
    .map_err(bad)?;
    Ok((reply, SessionPacketCrypt::new(dh.session_key), sizes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::fsd::{MakePc, APPEARANCE_BYTES};

    fn pc(name: Vec<u16>) -> Pc {
        Pc {
            zone: 1101,
            position: [-3199, 9241, 599],
            yaw: 45,
            level: 1,
            hp: 303,
            pcid: 5,
            pc: MakePc {
                race: 1,
                sex: 2,
                job: 3,
                appearance: [7; APPEARANCE_BYTES],
            },
            name,
        }
    }

    #[test]
    fn entry_packets_encode() {
        let entry = pc(vec![0x41, 0x42]);
        let packets = enter_world_at(&entry, 1000).unwrap();
        let commands: Vec<u16> = packets
            .iter()
            .map(|p| u16::from_le_bytes([p[2], p[3]]))
            .collect();
        assert_eq!(commands.last(), Some(&crate::ids::G_EnergyPropertySyncCmd));
        // Header: world `h`, object id `q` at offset 8 + 2.
        assert_eq!(packets[0][10..18], PC_OBJECT_ID.to_le_bytes());
        let prepare = prepare_zone(&entry, 1, 3, 0).unwrap();
        assert_eq!(
            prepare[2..4],
            crate::ids::G_PrepareEnterZoneCmd.to_le_bytes()
        );
    }

    #[test]
    fn rule_config_overrides_the_default() {
        let config = "<!-- <option name=\"teleport-duration\" value=\"1\"/> -->\n\
                      <option name=\"teleport-default-distance\" value=\"55\"/>\n\
                      <option name=\"teleport-price-percent\" value=\"abc\"/>";
        let text = without_comments(config);
        assert_eq!(config_value(&text, "teleport-duration"), None);
        assert_eq!(config_value(&text, "teleport-default-distance"), Some("55"));
        // A value that does not parse keeps the default.
        let kind = Kind::Int;
        assert_eq!(rule_value(kind, "10", Some("abc")), "10");
        assert_eq!(rule_value(kind, "10", Some("55")), "55");
        assert_eq!(rule_value(Kind::Bool, "no", Some("y")), "yes");
        assert_eq!(rule_value(Kind::Text, ";", Some("a'b")), ";");
    }
}
