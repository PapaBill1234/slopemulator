//! `gamedaemon --listen <addr:port> --handshake-key <file> [--lobby <addr:port>]`
//!
//! Client listener of the play stack (card world-1). Spec: Docs/Server/GameDaemon/04, 05.
//! Done: Hello 11/12 (RSA handshake, then AES-128 session), token 23 → 24, log of each command.
//! Enter world: 31 → 32 (fragments) and 41; 42 → 43. Spec: 10-session-handlers.md.
//! ❓ Not parity: the token is not consumed through LobbyDaemon (no link). `--lobby` is not used yet.
//! ❓ Not parity: the character comes from `shared::local_entry`, not from the game DB.
//! 929 → 930: rule list from `--contents-rule <ContentsRule.gamed.config.xml>`.
//! ❓ The sync packets after 32 have no reply yet.
use bloodlib::{
    crypto::{DhParameters, RsaKey},
    frames::{decode_client, encode_client, ClientDecrypt, ClientEncrypt},
    packet::{compose, decompose, DecodeLimits, Value},
    session::SessionPacketCrypt,
};
use lobbygate::wire::{inner_packet, read_frame, write_frame};
use packet::{
    world_entry::{
        property_code, CraftUnion, CreatureProperties, EnterWorldOk, EntryHeader, Fragments,
        PropertyValue, SlateItem, SlatePage,
    },
    zone_entry::{EnterZoneOk, PrepareZone},
};
use rand::rngs::OsRng;
use shared::local_entry::LocalEntry;
use std::{io, sync::Arc};
use tokio::net::{TcpListener, TcpStream};

const PROTOCOL: i16 = 1278;

fn bad(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}

fn hex(data: &[u8]) -> String {
    data.iter().map(|b| format!("{b:02x}")).collect()
}

/// Start zone of each level-1 `PcInitialSetting` record: geo zone 1101.
/// Spawn `1101_SP1` (`zonepcspawndata.1101.xml`): position and yaw. ❓ Units.
const START_ZONE: u32 = 1101;
const START_POSITION: [i16; 3] = [-3199, 9241, 599];
const START_YAW: u16 = 45;
/// PC object id: type 1 in bits 48.., index in bits 0..16
/// (build 348 `World::initPcManager` 0x1403EDDC0, `GameObjectIdManager::Make` 0x14001D090).
/// ❓ Local: one fixed index. The original takes the next free index (1..19999).
const PC_OBJECT_ID: u64 = (1 << 48) + 1;
/// ❓ Local: the client must return this value in command 42.
const SPAWN_KEY: u32 = 1;
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
fn enter_world(entry: &LocalEntry) -> io::Result<Vec<Vec<u8>>> {
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
    set(98, START_ZONE.into())?;
    set(99, START_POSITION[0] as u16 as u64)?;
    set(100, START_POSITION[1] as u16 as u64)?;
    set(101, START_POSITION[2] as u16 as u64)?;
    set(102, START_YAW.into())?;
    set(103, 1)?; // level
    set(107, 303)?; // hp
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
fn prepare_zone() -> io::Result<Vec<u8>> {
    PrepareZone {
        runtime_zone_id: START_ZONE.into(),
        channel: 1,
        geo_zone_id: START_ZONE,
        correlation: SPAWN_KEY,
        config_byte: 0,
        action_flag: 0,
        zone_block: BROADCAST_BLOCK.to_vec(),
        position: START_POSITION,
        yaw: START_YAW,
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
fn rule_list(config: &str) -> io::Result<Vec<Vec<u8>>> {
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
fn hello(key: &RsaKey, frame: &[u8]) -> io::Result<(Vec<u8>, SessionPacketCrypt)> {
    let decoded = decode_client(
        frame,
        ClientDecrypt::Handshake {
            key,
            expected_code: 4,
        },
    )
    .map_err(bad)?
    .ok_or_else(|| bad("incomplete Hello"))?;
    eprintln!("gamed: hello packet {}", hex(&decoded.packet));
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

async fn connection(
    key: Arc<RsaKey>,
    rules: Arc<Vec<Vec<u8>>>,
    stream: TcpStream,
) -> io::Result<()> {
    stream.set_nodelay(true)?;
    let (mut reader, mut writer) = stream.into_split();
    let Some(frame) = read_frame(&mut reader).await? else {
        return Ok(());
    };
    let (reply, mut session) = hello(&key, &frame)?;
    write_frame(&mut writer, &reply).await?;
    eprintln!("gamed: hello ok");
    let mut entry: Option<LocalEntry> = None;
    while let Some(frame) = read_frame(&mut reader).await? {
        let (plain, _) = session.decrypt(&frame, 4000).map_err(bad)?;
        let packet = inner_packet(&plain)?;
        let cmd = i16::from_le_bytes([packet[2], packet[3]]);
        eprintln!("gamed: client cmd {cmd} body {}", hex(&packet[4..]));
        let mut replies = Vec::new();
        match cmd {
            // 23 `hhqq` world, client key, lobby token → 24 `hQiQih` (10-session-handlers.md §23).
            23 => {
                if packet.len() >= 14 {
                    let client_key = u64::from_le_bytes(packet[6..14].try_into().unwrap());
                    entry = LocalEntry::read(client_key);
                    eprintln!(
                        "gamed: client key {client_key}, character {:?}",
                        entry.as_ref().map(|e| e.pcid)
                    );
                }
                replies.push(
                    compose(
                        "hQiQih",
                        &[
                            Value::I16(24),
                            Value::I8(0),
                            Value::I32(0),
                            Value::I8(0),
                            Value::I32(0),
                            Value::I16(0),
                        ],
                    )
                    .map_err(bad)?,
                );
            }
            // 31 `hic` client version, character → 32 fragments, then 41.
            31 => match &entry {
                Some(entry) => {
                    replies.extend(enter_world(entry)?);
                    replies.push(prepare_zone()?);
                }
                None => eprintln!("gamed: command 31 without a character record, no reply"),
            },
            929 => replies.extend(rules.iter().cloned()),
            // 42 `hiuuuh` → 43 `hq`. ❓ The spawn context is the PC object id here.
            42 => {
                let key = packet
                    .get(4..8)
                    .map(|k| u32::from_le_bytes(k.try_into().unwrap()));
                if key == Some(SPAWN_KEY) {
                    replies.push(
                        EnterZoneOk {
                            context: PC_OBJECT_ID,
                        }
                        .encode()
                        .map_err(bad)?,
                    );
                } else {
                    eprintln!("gamed: command 42 with key {key:?}, no reply");
                }
            }
            _ => {}
        }
        for reply in replies {
            eprintln!(
                "gamed: reply cmd {} ({} bytes)",
                i16::from_le_bytes([reply[2], reply[3]]),
                reply.len()
            );
            let frame = session.encrypt(&reply, 4002, false).map_err(bad)?;
            write_frame(&mut writer, &frame).await?;
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let mut args = std::env::args().skip(1);
    let (mut listen, mut key_path, mut rule_path) = (None, None, None);
    while let Some(arg) = args.next() {
        let value = args.next().ok_or_else(|| bad("missing option value"))?;
        match arg.as_str() {
            "--listen" => listen = Some(value),
            "--handshake-key" => key_path = Some(value),
            "--contents-rule" => rule_path = Some(value),
            "--lobby" => {}
            _ => {
                return Err(bad(
                    "usage: gamedaemon --listen <addr:port> --handshake-key <file>",
                ))
            }
        }
    }
    let listen = listen.ok_or_else(|| bad("--listen is necessary"))?;
    let key_path = key_path.ok_or_else(|| bad("--handshake-key is necessary"))?;
    let key = Arc::new(lobbygate::server::load_handshake_key(key_path.as_ref())?);
    // No file: no reply to command 929 (the client then waits).
    let rules = Arc::new(match rule_path {
        Some(path) => rule_list(&std::fs::read_to_string(path)?)?,
        None => Vec::new(),
    });
    eprintln!("gamed: rule list, {} fragments", rules.len());
    let listener = TcpListener::bind(&listen).await?;
    eprintln!("gamed: listening on {listen}");
    loop {
        let (stream, peer) = listener.accept().await?;
        eprintln!("gamed: client {peer}");
        let (key, rules) = (key.clone(), rules.clone());
        tokio::spawn(async move {
            let result = connection(key, rules, stream).await;
            eprintln!("gamed: client {peer} closed: {result:?}");
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::fsd::{MakePc, APPEARANCE_BYTES};

    #[test]
    fn entry_packets_encode() {
        let entry = LocalEntry {
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
        let prepare = prepare_zone().unwrap();
        assert_eq!(prepare[2..4], 41u16.to_le_bytes());
    }
}
