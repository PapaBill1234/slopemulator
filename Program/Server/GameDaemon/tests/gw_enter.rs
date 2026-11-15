//! Card gw-enter-rust: 381 enter world to a standing PC. Spec: Docs/Server/GameDaemon/05, 06.
use bloodlib::packet::{decompose, DecodeLimits, Value};
use bytes::Bytes;
use gamedaemon::{
    conn::{log_line, log_lines, Dir, LogMode},
    db::Pc,
    entry::{
        enter_world_at, enter_zone_ok, prepare_zone, rule_list,
        rules381::{Kind, RULES},
        token_ok, SPAWN_STATE_WAIT,
    },
    ids,
    play::{self, Effect, Play, Spawn},
};
use packet::fragments::FragmentReader;
use shared::fsd::{MakePc, APPEARANCE_BYTES};

const START: [i16; 3] = [-3199, 9241, 599];

fn pc(name: usize) -> Pc {
    Pc {
        zone: 1101,
        position: START,
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
        name: vec![0x41; name],
    }
}

fn request(cmd: u16, body: &[u8]) -> Vec<u8> {
    let mut packet = ((body.len() + 4) as i16).to_le_bytes().to_vec();
    packet.extend(cmd.to_le_bytes());
    packet.extend(body);
    packet
}

fn rules() -> Vec<Bytes> {
    rule_list("")
        .unwrap()
        .into_iter()
        .map(Bytes::from)
        .collect()
}

fn sent(out: &[Effect]) -> Vec<Vec<u8>> {
    out.iter()
        .filter_map(|e| match e {
            Effect::Send(packet) => Some(packet.to_vec()),
            _ => None,
        })
        .collect()
}

fn command(packet: &[u8]) -> u16 {
    u16::from_le_bytes([packet[2], packet[3]])
}

/// Join the fragments of one command into the logical body.
fn join(fragments: &[Vec<u8>]) -> Vec<u8> {
    let mut reader = FragmentReader::default();
    let mut joined = None;
    for fragment in fragments {
        joined = reader.add(fragment).unwrap();
    }
    joined.unwrap().data.into_owned()
}

/// Run the effects of the shard that need no link or database.
fn run(play: &mut Play, rules: &[Bytes], packet: &[u8], pc: &Pc, ids_out: &mut Vec<u16>) {
    let mut effects = Vec::new();
    play::packet(play, rules, packet, &mut effects).unwrap();
    let mut index = 0;
    while index < effects.len() {
        match std::mem::replace(&mut effects[index], Effect::EnterZone(0)) {
            Effect::Send(p) => {
                // A fragmented command shows once.
                let id = command(&p);
                if ids_out.last() != Some(&id) {
                    ids_out.push(id);
                }
            }
            Effect::ConsumeToken { .. } => {
                play::entered(play, Ok(pc.pcid), &mut effects).unwrap();
            }
            Effect::LoadPc(_) => play::loaded(play, Some(pc.clone()), &mut effects).unwrap(),
            Effect::EnterZone(_) => play::joined(play, &mut effects).unwrap(),
            Effect::LeaveWorld { .. } => {}
        }
        index += 1;
    }
}

#[test]
fn loopback_order() {
    let rules = rules();
    let pc = pc(3);
    let mut play = Play {
        now_ms: 1000,
        spawn_next: 7,
        ..Play::default()
    };
    let mut seen = Vec::new();
    let mut token = 1i16.to_le_bytes().to_vec();
    token.extend(7u64.to_le_bytes());
    token.extend(9u64.to_le_bytes());
    run(
        &mut play,
        &rules,
        &request(ids::G_ContentsRuleListReqCmd, &[]),
        &pc,
        &mut seen,
    );
    run(
        &mut play,
        &rules,
        &request(ids::G_LobbyTokenAuthReqCmd, &token),
        &pc,
        &mut seen,
    );
    run(
        &mut play,
        &rules,
        &request(ids::G_EnterWorldReqCmd, &[0; 5]),
        &pc,
        &mut seen,
    );
    let mut zone = 7u32.to_le_bytes().to_vec();
    zone.extend([0; 8]);
    run(
        &mut play,
        &rules,
        &request(ids::G_EnterZoneReqCmd, &zone),
        &pc,
        &mut seen,
    );
    assert_eq!(
        seen,
        [
            ids::G_ContentsRuleListOkCmd,
            ids::G_LobbyTokenAuthOkCmd,
            ids::G_EnterWorldOkCmd,
            ids::G_QuestListOkCmd,
            ids::G_QuestCompleteListOkCmd,
            ids::G_CurrentUTCTimeOkCmd,
            ids::G_CollectingListOkCmd,
            ids::G_FishingListOkCmd,
            ids::G_EnergyPropertySyncCmd,
            ids::G_PrepareEnterZoneCmd,
            ids::G_EnterZoneOkCmd,
        ]
    );
    // The conditions of 627, 1097, 912 are false for the default config, and 17 is off.
    for id in [627u16, 1097, 912, ids::G_ServerPingNotifyCmd] {
        assert!(!seen.contains(&id));
    }
}

#[test]
fn golden_930() {
    let frames = rule_list("").unwrap();
    assert!(frames.iter().all(|f| command(f) == 930));
    let data = join(&frames);
    assert_eq!(data[..2], 74i16.to_le_bytes());
    assert_eq!(data[2..4], 0i16.to_le_bytes());
    assert_eq!(data[4..8], [0xFF; 4]);
    let count = usize::from(u16::from_le_bytes([data[8], data[9]]));
    let mut elements = Vec::new();
    let mut at = 10;
    while at < data.len() {
        let length = usize::from(u16::from_le_bytes([data[at], data[at + 1]]));
        elements.push(String::from_utf8(data[at + 2..at + 2 + length].to_vec()).unwrap());
        at += 2 + length;
    }
    assert_eq!(at, data.len());
    assert_eq!(elements.len(), count);
    assert_eq!(elements.len(), 167 + 2);
    assert!(elements[0].starts_with("<?xml"));
    assert!(elements[0].contains("major-version=\"74\""));
    assert_eq!(elements.last().unwrap(), "</config>");
    assert_eq!(
        elements[1],
        "<option name='limit-money-market-sale-price' value='15000000000'/>"
    );
    let options = elements
        .iter()
        .filter(|e| e.starts_with("<option "))
        .count();
    assert_eq!(options, 167);
    // Table order and bool text.
    for (rule, element) in RULES.iter().zip(&elements[1..]) {
        assert!(element.starts_with(&format!("<option name='{}'", rule.name)));
        if rule.kind == Kind::Bool {
            assert!(element.ends_with("value='yes'/>") || element.ends_with("value='no'/>"));
        }
    }
    assert!(elements.iter().any(|e| e.ends_with("value='yes'/>")));
    assert!(elements.iter().any(|e| e.ends_with("value='no'/>")));
}

#[test]
fn rule_list_rate() {
    let rules = rules();
    let mut play = Play {
        now_ms: 31_000,
        ..Play::default()
    };
    let mut out = Vec::new();
    let packet = request(ids::G_ContentsRuleListReqCmd, &[]);
    play::packet(&mut play, &rules, &packet, &mut out).unwrap();
    let first = sent(&out);
    assert!(!first.is_empty());
    assert!(first
        .iter()
        .all(|p| command(p) == ids::G_ContentsRuleListOkCmd));
    out.clear();
    play.now_ms = 32_000;
    play::packet(&mut play, &rules, &packet, &mut out).unwrap();
    let second = sent(&out);
    assert_eq!(second.len(), 1);
    assert_eq!(command(&second[0]), ids::G_ContentsRuleListFailCmd);
    assert_eq!(second[0][4..6], 2167i16.to_le_bytes());
    // After 30 s the next list goes out.
    out.clear();
    play.now_ms = 62_000;
    play::packet(&mut play, &rules, &packet, &mut out).unwrap();
    assert_eq!(command(&sent(&out)[0]), ids::G_ContentsRuleListOkCmd);
}

#[test]
fn golden_24() {
    let packet = token_ok(0);
    assert_eq!(packet[2..4], [0x18, 0x00]);
    assert_eq!(
        packet[4..],
        [0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0, 0, 0, 0, 0, 0, 0]
    );
    assert_eq!(usize::from(packet[0]), packet.len());
}

#[test]
fn golden_32() {
    for n in [0usize, 1, 12] {
        let entry = pc(n);
        let packets = enter_world_at(&entry, 100).unwrap();
        let fragments: Vec<Vec<u8>> = packets
            .iter()
            .take_while(|p| command(p) == ids::G_EnterWorldOkCmd)
            .cloned()
            .collect();
        let data = join(&fragments);
        // Header, 9 fields: h q c i h h h h h.
        assert_eq!(data[..2], 1u16.to_le_bytes());
        assert_eq!(data[2..10], ((1u64 << 48) + 1).to_le_bytes());
        let header = 2 + 8 + 1 + 4 + 2 + 2 + 2 + 2 + 2;
        assert_eq!(data[header - 2..header], 2u16.to_le_bytes());
        // Blob: 1,024 + 2n. The first property (id 1, world id) is at blob offset 0.
        let blob = 1024 + 2 * n;
        assert_eq!(data[header..header + 2], 1u16.to_le_bytes());
        // The group after the blob: the counted extra properties, empty.
        assert_eq!(data[header + blob..header + blob + 2], [0, 0]);
    }
}

#[test]
fn golden_41() {
    let entry = pc(2);
    let packet = prepare_zone(&entry, 1, 17, 0).unwrap();
    let fields = decompose(
        "hqhiiccKuuuhhh",
        &packet,
        DecodeLimits {
            blob_bytes: 4000,
            ..DecodeLimits::default()
        },
    )
    .unwrap();
    assert_eq!(fields[0], Value::I16(41));
    assert_eq!(fields[1], Value::U64(1101));
    assert_eq!(fields[2], Value::I16(1), "channel");
    assert_eq!(fields[3], Value::U32(1101), "zone");
    assert_eq!(fields[4], Value::U32(1), "spawn key");
    assert_eq!(fields[5], Value::U8(17), "world clock hour");
    assert_eq!(fields[6], Value::U8(0), "arenaWaitCancel");
    assert_eq!(fields[7], Value::Bytes(b"keyhere".to_vec()));
    assert_eq!(fields[8], Value::U16(START[0] as u16));
    assert_eq!(fields[9], Value::U16(START[1] as u16));
    assert_eq!(fields[10], Value::U16(START[2] as u16));
    assert_eq!(fields[11], Value::I16(45), "yaw");
    assert_eq!(fields[12], Value::I16(1), "PC transition counter");
    assert_eq!(fields[13], Value::I16(0), "summoned counter");
}

fn waiting(entry: &Pc, key: u32) -> Play {
    Play {
        pc: Some(entry.clone()),
        spawn: Some(Spawn {
            key,
            state: SPAWN_STATE_WAIT,
        }),
        ..Play::default()
    }
}

fn enter_zone_request(key: u32, x: i16, y: i16, z: i16, yaw: i16) -> Vec<u8> {
    let mut body = key.to_le_bytes().to_vec();
    for value in [x, y, z, yaw] {
        body.extend(value.to_le_bytes());
    }
    request(ids::G_EnterZoneReqCmd, &body)
}

#[test]
fn enter_zone_bad_key() {
    let mut play = waiting(&pc(1), 5);
    let mut out = Vec::new();
    let packet = enter_zone_request(6, 0, 0, 0, 0);
    play::packet(&mut play, &[], &packet, &mut out).unwrap();
    let replies = sent(&out);
    assert_eq!(replies.len(), 1);
    assert_eq!(command(&replies[0]), ids::G_EnterZoneFailCmd);
    assert_eq!(replies[0][4..6], 1150i16.to_le_bytes());
    assert!(out.iter().all(|e| matches!(e, Effect::Send(_))));
    // The spawn action stays open for the right key.
    out.clear();
    play::packet(&mut play, &[], &enter_zone_request(5, 0, 0, 0, 0), &mut out).unwrap();
    assert_eq!(out, [Effect::EnterZone(1101)]);
}

#[test]
fn enter_zone_clamp() {
    let entry = pc(1);
    let mut play = waiting(&entry, 5);
    let mut out = Vec::new();
    let packet = enter_zone_request(5, 30_000, -30_000, 30_000, 12_345);
    play::packet(&mut play, &[], &packet, &mut out).unwrap();
    assert_eq!(out, [Effect::EnterZone(1101)]);
    // The server keeps its own position. The client value does not move the PC or the summon.
    assert_eq!(play.pc.as_ref().unwrap().position, START);
    assert_eq!(play.summon, START);
    assert_eq!(play.summon_yaw, 45);
    // A value near the PC is allowed for the summoned object only.
    let mut play = waiting(&entry, 5);
    let packet = enter_zone_request(5, -3190, 9241, 599, 90);
    play::packet(&mut play, &[], &packet, &mut Vec::new()).unwrap();
    assert_eq!(play.summon, [-3190, 9241, 599]);
    assert_eq!(play.pc.as_ref().unwrap().position, START);
}

#[test]
fn golden_43() {
    let packet = enter_zone_ok(0);
    assert_eq!(packet[2..4], [0x2B, 0x00]);
    assert_eq!(packet[4..], [0; 8]);
    assert_eq!(usize::from(packet[0]), 12);
}

#[test]
fn packet_log_line() {
    let packet = enter_zone_ok(0);
    let lines = log_lines(LogMode::parse(Some("1")), 5123, Dir::Send, &packet);
    assert_eq!(lines, ["5123 S 43 G_EnterZoneOkCmd 12"]);
    assert_eq!(
        log_line(7, Dir::Recv, 42, 20),
        "7 R 42 G_EnterZoneReqCmd 20"
    );
    let lines = log_lines(LogMode::parse(Some("hex")), 5123, Dir::Send, &packet);
    assert_eq!(
        lines,
        ["5123 S 43 G_EnterZoneOkCmd 12", "  0000000000000000"]
    );
    // The key bytes of 41 are never in the dump.
    let entry = pc(1);
    let prepare = prepare_zone(&entry, 1, 0, 0).unwrap();
    let lines = log_lines(LogMode::Hex, 1, Dir::Send, &prepare);
    assert!(lines[1].contains("keyhere"));
    assert!(!lines[1].contains("6b657968657265"));
}

#[test]
fn packet_log_off() {
    let packet = enter_zone_ok(0);
    assert_eq!(LogMode::parse(None), LogMode::Off);
    assert_eq!(LogMode::parse(Some("")), LogMode::Off);
    assert!(log_lines(LogMode::parse(None), 1, Dir::Send, &packet).is_empty());
}
