//! Packet builders of the world entry. No I/O. Spec: Docs/Server/GameDaemon/04, 05, 10.
//! Hello 11 → 12 (RSA handshake, then AES-128 session), 32 (fragments), 41, 930.
use crate::db::Pc;
use bloodlib::{
    crypto::{DhParameters, RsaKey},
    frames::{decode_client, encode_client, ClientDecrypt, ClientEncrypt},
    packet::{compose, decompose, DecodeLimits, Value},
    session::SessionPacketCrypt,
};
use packet::{
    world_entry::{
        property_code, CraftUnion, CreatureProperties, EnterWorldOk, EntryHeader, Fragments,
        PropertyValue, SlateItem, SlatePage,
    },
    zone_entry::PrepareZone,
};
use rand::rngs::OsRng;
use std::io;

const PROTOCOL: i16 = 1278;

pub fn bad(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}

/// PC object id: type 1 in bits 48.., index in bits 0..16
/// (build 348 `World::initPcManager` 0x1403EDDC0, `GameObjectIdManager::Make` 0x14001D090).
/// ❓ Local: one fixed index. The original takes the next free index (1..19999).
pub const PC_OBJECT_ID: u64 = (1 << 48) + 1;
/// ❓ Local: the client must return this value in command 42.
pub const SPAWN_KEY: u32 = 1;
/// ❓ Local: size and content of the zone broadcast block. This server sends no broadcast packets.
const BROADCAST_BLOCK: [u8; 16] = [0; 16];

fn number(id: u16, value: u64) -> PropertyValue {
    match property_code(usize::from(id)) {
        Some('c') => PropertyValue::Byte(value as u8),
        Some('h') => PropertyValue::Short(value as u16),
        Some('i') => PropertyValue::Int(value as u32),
        Some('B' | 'F') => PropertyValue::Bit(value != 0),
        Some('s') => PropertyValue::String(Vec::new()),
        Some('b') => PropertyValue::Bytes(Vec::new()),
        _ => PropertyValue::Long(value),
    }
}

/// Command 32 for a new level-1 character. Property ids: 10-session-handlers.md, section gw1.
/// ❓ All properties not named here are zero. Max hp 303 and speed 62 come from `pcdata.xml`
/// record 1 (level 1); the original selects the record by race, sex, job and level.
pub fn enter_world(entry: &Pc) -> io::Result<Vec<Vec<u8>>> {
    let values: [PropertyValue; 422] = std::array::from_fn(|index| number(index as u16 + 1, 0));
    let mut properties = CreatureProperties::new(values).map_err(bad)?;
    let mut set = |id: u16, value: u64| properties.set(id, number(id, value)).map_err(bad);
    set(1, 1)?; // world-id
    set(2, entry.pc.race.into())?;
    set(3, entry.pc.sex.into())?;
    set(4, entry.pc.job.into())?;
    for (index, byte) in entry.pc.appearance.iter().enumerate() {
        set(5 + index as u16, (*byte).into())?;
    }
    set(98, entry.zone.into())?;
    set(99, entry.position[0] as u16 as u64)?;
    set(100, entry.position[1] as u16 as u64)?;
    set(101, entry.position[2] as u16 as u64)?;
    set(102, entry.yaw.into())?;
    set(103, entry.level.into())?;
    set(107, entry.hp)?;
    set(244, 303)?; // max-hp
    set(250, 62)?; // speed
    properties
        .set(97, PropertyValue::String(entry.name.clone()))
        .map_err(bad)?;
    let slate = |page| SlatePage {
        page,
        items: [SlateItem {
            item_id: 0,
            level: 0,
        }; 6],
        stones: [0; 9],
        new_stone_id: 0,
        new_stone_position: 0,
    };
    EnterWorldOk {
        header: EntryHeader {
            world_id: 1,
            object_id: PC_OBJECT_ID,
            jumping_character_state: 0,
            chat_address_bits: 0,
            chat_port: 0,
            chat_world_id: 1,
            manage_port: 0,
            time_zone_bias: 0,
        },
        properties,
        extra_properties: Vec::new(),
        opaque_broadcast_block: BROADCAST_BLOCK.to_vec(),
        recycle_remaining: [None; 25],
        acquired_skills: [Vec::new(), Vec::new()],
        config_bytes: [0; 4],
        pc_cafe_publisher: 0,
        pc_db_id: entry.pcid,
        content_bans_enabled: false,
        user: None,
        test_server_enabled: false,
        craft_unions: [CraftUnion::default(); 2],
        slate_pages: [slate(1), slate(2)],
    }
    .encode_fragments()
    .map_err(bad)
}

/// Command 41. Field names: build 348 `PcSpawnAction::prepareSpawn` 0x1404B4E50.
/// ❓ Local: runtime zone id = geo zone id, channel 1, world clock byte 0.
pub fn prepare_zone(entry: &Pc) -> io::Result<Vec<u8>> {
    PrepareZone {
        runtime_zone_id: entry.zone.into(),
        channel: 1,
        geo_zone_id: entry.zone,
        correlation: SPAWN_KEY,
        config_byte: 0,
        action_flag: 0,
        zone_block: BROADCAST_BLOCK.to_vec(),
        position: entry.position,
        yaw: entry.yaw,
        pc_transition: 1,
        summon_transition: 0,
    }
    .encode()
    .map_err(bad)
}

/// Command 930 `hihhiCb;` (build 381 builder 0x140087D40; build 348
/// `ContentsRuleConfig::MakeListPacket` 0x140089C50). Body: major version 74, minor 0,
/// revision `i`, count, then counted UTF-8 elements: XML header, one `<option>` for
/// each rule, `</config>`. Type 4 (bool) prints `yes` or `no`.
/// ❓ Local: file order, not definition-table order (0x140ECAD88). Revision 0.
/// ❓ Local: an option that the file does not have is absent; the original sends its default.
pub fn rule_list(config: &str) -> io::Result<Vec<Vec<u8>>> {
    let mut elements = vec![String::from(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?><config major-version=\"74\" \
         minor-version=\"0\" module=\"contents-rule-config\" \
         release-module=\"LocalizationData\" release-side=\"client\">",
    )];
    // Remove XML comments. A comment can hold an option that is not active.
    let mut text = String::new();
    let mut source = config;
    while let Some((before, after)) = source.split_once("<!--") {
        text.push_str(before);
        source = after.split_once("-->").map_or("", |(_, tail)| tail);
    }
    text.push_str(source);
    let mut rest = text.as_str();
    while let Some(start) = rest.find("<option name=\"") {
        rest = &rest[start + 14..];
        let Some((name, tail)) = rest.split_once("\" value=\"") else {
            break;
        };
        let Some((value, tail)) = tail.split_once('"') else {
            break;
        };
        rest = tail;
        let value = match value {
            "y" | "yes" => "yes",
            "n" | "no" => "no",
            other => other,
        };
        elements.push(format!("<option name='{name}' value='{value}'/>"));
    }
    elements.push(String::from("</config>"));
    let mut out = Fragments::new(930);
    let mut head = 74u16.to_le_bytes().to_vec();
    head.extend(0u16.to_le_bytes());
    head.extend(0u32.to_le_bytes());
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
    let reply = encode_client(
        &reply,
        ClientEncrypt::Handshake {
            key: &client,
            code: 5,
        },
        &mut OsRng,
    )
    .map_err(bad)?;
    Ok((reply, SessionPacketCrypt::new(dh.session_key)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::fsd::{MakePc, APPEARANCE_BYTES};

    #[test]
    fn entry_packets_encode() {
        let entry = Pc {
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
            name: vec![0x41, 0x42],
        };
        let fragments = enter_world(&entry).unwrap();
        assert!(fragments.iter().all(|f| f[2..4] == 32u16.to_le_bytes()));
        // Header: world `h`, object id `q` at offset 8 + 2.
        assert_eq!(fragments[0][10..18], PC_OBJECT_ID.to_le_bytes());
        let rules = rule_list(
            "<!-- <option name=\"x\" value=\"1\"/> -->\n<option name=\"a-b\" value=\"y\"/>",
        )
        .unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0][16..18], 3u16.to_le_bytes());
        assert_eq!(rules[0][2..4], 930u16.to_le_bytes());
        assert!(rules[0].ends_with(b"<option name='a-b' value='yes'/>\x09\0</config>"));
        let prepare = prepare_zone(&entry).unwrap();
        assert_eq!(prepare[2..4], 41u16.to_le_bytes());
    }
}
