use bloodlib::packet::{decode_fields, Value};
use lobbydaemon::{
    jobs::{Jobs, Settings, State},
    np::{Np, NpError, StubNp},
    protocol as w,
    store::{
        Account, AccountId, Character, IncompleteService, MemoryStore, Shared, Store, SyncStore,
    },
};
use packet::fragments::FragmentReader;
use std::{io, time::Duration};

fn info_request(key: u64, slot: [u8; 16]) -> Vec<u8> {
    packet("hqo", &[Value::I16(38), Value::U64(key), Value::Id(slot)])
}
fn info_failure(packets: &[Vec<u8>], key: u64, slot: [u8; 16], reason: i16) {
    assert_eq!(packets.len(), 1);
    assert_eq!(
        w::fields("hqoE", &packets[0]).unwrap(),
        vec![
            Value::I16(40),
            Value::U64(key),
            Value::Id(slot),
            Value::I16(reason)
        ]
    );
}
#[tokio::test]
async fn r14_character_info_reads_full_slot_from_cached_snapshot() {
    let (mut j, t, p) = rig();
    let id = j.np.identities[&(t, p)].as_ref().unwrap().account;
    let row = character();
    let mut other = row.clone();
    other.pcid = 88;
    other.slot[15] ^= 1;
    j.store
        .lock()
        .characters
        .insert(id, vec![other, row.clone()]);
    let key = initialized(&mut j, t, p).await;
    send(&mut j, 1, start(key, t, p)).await;
    j.store.lock().characters.clear(); // No reload or write is needed by D38.
    let response = send(&mut j, 1, info_request(key, row.slot)).await;
    assert_eq!(response.len(), 1);
    assert_eq!(
        w::fields("hqqohsbqQQi", &response[0]).unwrap(),
        vec![
            Value::I16(39),
            Value::U64(key),
            Value::U64(row.pcid),
            Value::Id(row.slot),
            Value::I16(row.world),
            Value::String(row.name),
            Value::Bytes(row.showcase),
            Value::U64(0),
            Value::I8(0),
            Value::I8(0),
            Value::U32(0),
        ]
    );
    assert_eq!(j.state(key), Some(State::Lobby));
}
#[tokio::test]
async fn r14_character_info_checks_state_slot_session_and_link() {
    let (mut j, t, p) = rig();
    let id = j.np.identities[&(t, p)].as_ref().unwrap().account;
    j.store.lock().characters.insert(id, vec![character()]);
    let key = initialized(&mut j, t, p).await;
    info_failure(
        &send(&mut j, 1, info_request(key, [8; 16])).await,
        key,
        [8; 16],
        5,
    );
    info_failure(
        &send(&mut j, 1, info_request(u64::MAX, [8; 16])).await,
        u64::MAX,
        [8; 16],
        4,
    );
    send(&mut j, 1, start(key, t, p)).await;
    for slot in [[0; 16], [9; 16]] {
        info_failure(
            &send(&mut j, 1, info_request(key, slot)).await,
            key,
            slot,
            6,
        );
    }
    send(&mut j, 2, hello(58)).await;
    info_failure(
        &send(&mut j, 2, info_request(key, [8; 16])).await,
        key,
        [8; 16],
        4,
    );
    j.retain(key, Duration::from_secs(10)).unwrap();
    info_failure(
        &send(&mut j, 1, info_request(key, [8; 16])).await,
        key,
        [8; 16],
        5,
    );
}
#[tokio::test]
async fn r14_character_info_drops_malformed_without_losing_the_session() {
    let (mut j, t, p) = rig();
    let key = initialized(&mut j, t, p).await;
    send(&mut j, 1, start(key, t, p)).await;
    for malformed in [request(38, key), packet("h", &[Value::I16(38)])] {
        assert!(send(&mut j, 1, malformed).await.is_empty());
    }
    let mut extra = info_request(key, [8; 16]);
    extra.push(0);
    let len = extra.len() as u16;
    extra[..2].copy_from_slice(&len.to_le_bytes());
    // The shared codec accepts a decoded prefix. Keep that local policy.
    info_failure(&send(&mut j, 1, extra).await, key, [8; 16], 6);
    assert_eq!(j.state(key), Some(State::Lobby));
    assert_eq!(
        w::command(&send(&mut j, 1, request(35, key)).await[0]).unwrap(),
        36
    );
}
#[tokio::test]
async fn r14_check_name_is_a_no_op_with_any_body_or_session() {
    let (mut j, t, p) = rig();
    let bytes = packet("h", &[Value::I16(41)]);
    assert!(j.handle(1, &bytes).await.is_err()); // Channel Hello is still required.
    send(&mut j, 1, hello(58)).await;
    for bytes in [
        bytes,
        request(41, u64::MAX),
        packet(
            "hqs",
            &[
                Value::I16(41),
                Value::U64(0),
                Value::String(vec![0xD800, 0]),
            ],
        ),
    ] {
        assert!(send(&mut j, 1, bytes).await.is_empty());
    }
    let key = initialized_on_existing_link(&mut j, t, p).await;
    assert!(send(&mut j, 1, request(41, key)).await.is_empty());
    assert_eq!(j.state(key), Some(State::Init));
    send(&mut j, 1, start(key, t, p)).await;
    assert!(send(&mut j, 1, request(41, key)).await.is_empty());
    assert_eq!(j.state(key), Some(State::Lobby));
}
async fn initialized_on_existing_link(j: &mut Rig, t: [u8; 16], p: [u8; 16]) -> u64 {
    let response = send(j, 1, init(100, t, p)).await;
    let Value::U64(key) = w::fields("hqqoo", &response[0]).unwrap()[2] else {
        panic!("key")
    };
    key
}
#[test]
fn r14_character_info_serializer_rejects_invalid_bounds() {
    for (name, blob) in [(33, 0), (0, 513)] {
        let mut row = character();
        row.name = vec![65; name];
        row.showcase = vec![0; blob];
        assert!(w::character_info(1, &row).is_err());
    }
    assert_eq!(
        w::fields("hqoE", &w::character_info_failure(1, [2; 16], 2).unwrap()).unwrap()[3],
        Value::I16(2)
    );
}

#[tokio::test]
async fn r8_anonymous_close_uses_gate_key_and_preserves_authenticated_sessions() {
    let (mut j, t, p) = rig();
    let key = initialized(&mut j, t, p).await;
    let output = j.close_anonymous(key).await.unwrap();
    assert_eq!(output[0].link, 1);
    assert_eq!(output[0].packets, vec![w::notice(20, 100).unwrap()]);
    assert_eq!(j.session_count(), 0);
    assert!(j.close_anonymous(key).await.unwrap().is_empty());
    let response = send(&mut j, 1, init(101, t, p)).await;
    let Value::U64(key) = w::fields("hqqoo", &response[0]).unwrap()[2] else {
        panic!("key")
    };
    send(&mut j, 1, start(key, t, p)).await;
    assert!(j.close_anonymous(key).await.unwrap().is_empty());
    assert_eq!(j.state(key), Some(State::Lobby));
}
#[tokio::test]
async fn r6_invalid_character_bounds_block_login() {
    for (name_len, blob_len, reason) in [(33, 0, 2), (0, 513, 16)] {
        let (mut j, t, p) = rig();
        let id = j.np.identities[&(t, p)].as_ref().unwrap().account;
        let mut row = character();
        row.name = vec![65; name_len];
        row.showcase = vec![0; blob_len];
        j.store.lock().characters.insert(id, vec![row]);
        let key = initialized(&mut j, t, p).await;
        fail(&send(&mut j, 1, start(key, t, p)).await, 11, key, reason);
        assert_eq!(j.session_count(), 0);
        j.np.login_presence(id, 999).await.unwrap();
    }
}
#[tokio::test]
async fn r9_link_limit_and_malformed_frame_rejection() {
    let mut j = Jobs::new(
        Shared::new(MemoryStore::default()),
        StubNp::default(),
        Settings {
            max_links: 1,
            ..Settings::default()
        },
    )
    .unwrap();
    send(&mut j, 1, hello(58)).await;
    assert_eq!(w::command(&send(&mut j, 2, hello(58)).await[0]).unwrap(), 3);
    j.disconnect(1).await;
    assert_eq!(w::command(&send(&mut j, 2, hello(58)).await[0]).unwrap(), 2);
    assert!(j.handle(2, &[0, 0, 1, 0]).await.is_err());
    // An unknown command gets no reply and the link stays.
    assert!(j.handle(2, &request(0, 0)).await.unwrap().is_empty());
}

#[tokio::test(start_paused = true)]
async fn r4_presence_timeout_and_cancel_release_ownership() {
    for cancel in [false, true] {
        let (mut j, t, p) = rig();
        let id = j.np.identities[&(t, p)].as_ref().unwrap().account;
        j.np.presence_delay = Duration::from_secs(10);
        let key = initialized(&mut j, t, p).await;
        if cancel {
            assert!(
                tokio::time::timeout(Duration::from_secs(1), j.handle(1, &start(key, t, p)))
                    .await
                    .is_err()
            );
            assert_eq!(j.state(key), Some(State::Init));
            j.disconnect(1).await;
        } else {
            fail(&send(&mut j, 1, start(key, t, p)).await, 11, key, 2);
        }
        assert_eq!(j.session_count(), 0);
        j.np.presence_delay = Duration::ZERO;
        j.np.login_presence(id, 999).await.unwrap();
    }
}

#[tokio::test(start_paused = true)]
async fn r10_init_expiry_is_once_and_success_cancels_it() {
    let (np, t, p, _) = StubNp::fixture(10);
    let mut j = Jobs::new(
        Shared::new(MemoryStore::default()),
        np,
        Settings {
            init_timeout: Some(Duration::from_secs(2)),
            ..Settings::default()
        },
    )
    .unwrap();
    let key = initialized(&mut j, t, p).await;
    tokio::time::advance(Duration::from_secs(2)).await;
    let out = j.expire().await.unwrap();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].packets, vec![w::notice(20, 100).unwrap()]);
    assert!(j.expire().await.unwrap().is_empty());
    assert_eq!(j.state(key), None);
    let out = send(&mut j, 1, init(101, t, p)).await;
    let Value::U64(key) = w::fields("hqqoo", &out[0]).unwrap()[2] else {
        panic!("key")
    };
    send(&mut j, 1, start(key, t, p)).await;
    tokio::time::advance(Duration::from_secs(3)).await;
    assert!(j.expire().await.unwrap().is_empty());
    assert_eq!(j.state(key), Some(State::Lobby));
}
#[tokio::test(start_paused = true)]
async fn r11_generated_token_expiry_rejects_at_boundary_and_after_sweep() {
    for sweep in [false, true] {
        let (mut np, t, p, _) = StubNp::fixture(10);
        np.token_expiry
            .insert(t, tokio::time::Instant::now() + Duration::from_secs(2));
        tokio::time::advance(Duration::from_secs(2)).await;
        if sweep {
            np.expire();
        }
        assert_eq!(np.consume(t, p).await, Err(NpError::RejectedToken));
        np.expire();
        assert_eq!(np.consume(t, p).await, Err(NpError::RejectedToken));
    }
}
#[tokio::test]
async fn r8_init_kick_uses_original_anonymous_close_effect() {
    let (mut j, t, p) = rig();
    let key = initialized(&mut j, t, p).await;
    assert_eq!(
        j.kick(key, 7).await.unwrap()[0].packets,
        vec![w::notice(20, 100).unwrap()]
    );
    assert!(j.kick(key, 7).await.unwrap().is_empty());
    assert!(j.store.lock().logout_updates.is_empty());
}

fn retained_init(gate: u64, key: u64, token: u64, ipv4: u32) -> Vec<u8> {
    packet(
        "hqqqi",
        &[
            Value::I16(12),
            Value::U64(gate),
            Value::U64(key),
            Value::U64(token),
            Value::U32(ipv4),
        ],
    )
}
#[tokio::test]
async fn r13_retained_rebind_single_use_cache_and_link_checks() {
    let (mut j, t, p) = rig();
    let id = j.np.identities[&(t, p)].as_ref().unwrap().account;
    j.store.lock().characters.insert(id, vec![character()]);
    let key = initialized(&mut j, t, p).await;
    fail(&send(&mut j, 1, request(15, key)).await, 17, key, 5);
    send(&mut j, 1, start(key, t, p)).await;
    let old_list = send(&mut j, 1, request(35, key)).await;
    assert!(j.retain(key, Duration::ZERO).is_err());
    let token = j.retain(key, Duration::from_secs(10)).unwrap();
    assert!(j.retain(key, Duration::from_secs(10)).is_err());
    send(&mut j, 2, hello(58)).await;
    // No token found (other link, other key): 20. Only an expired token gives 19
    // (`23-return-to-lobby.md` §3, InitSessionWithLobbyTokenReq).
    fail(
        &send(&mut j, 2, retained_init(200, key, token, 7)).await,
        14,
        200,
        20,
    );
    fail(
        &send(&mut j, 1, retained_init(200, key, token + 1, 7)).await,
        14,
        200,
        20,
    );
    fail(
        &send(&mut j, 1, retained_init(200, key + 999, token, 7)).await,
        14,
        200,
        20,
    );
    fail(
        &send(&mut j, 1, retained_init(200, key, token, 8)).await,
        14,
        200,
        20,
    );
    assert!(send(&mut j, 1, request(19, key)).await.is_empty());
    // 13-jobs-a.md §3.3: 5.1 is "invalid state" for LogoutJob. No notify, no change.
    assert!(send(&mut j, 1, request(23, key)).await.is_empty());
    assert_eq!(j.state(key), Some(State::Retained));
    assert_eq!(
        send(&mut j, 1, retained_init(200, key, token, 7)).await,
        vec![packet(
            "hqq",
            &[Value::I16(13), Value::U64(200), Value::U64(key)]
        )]
    );
    fail(
        &send(&mut j, 1, retained_init(201, key, token, 7)).await,
        14,
        201,
        20,
    );
    fail(&send(&mut j, 1, request(35, key)).await, 37, key, 5);
    j.store.lock().characters.clear();
    j.store.lock().services.clear();
    let updates = j.store.lock().login_updates.clone();
    let reply = send(&mut j, 1, request(15, key)).await;
    assert_eq!(w::command(&reply[0]).unwrap(), 16);
    assert_eq!(joined(&reply, "qCiiiii;CiiQiqq;iqiq")[0], Value::U64(key));
    assert_eq!(send(&mut j, 1, request(35, key)).await, old_list);
    assert_eq!(j.store.lock().login_updates, updates);
    assert_eq!(j.np.consume(t, p).await, Err(NpError::RejectedToken));
    fail(&send(&mut j, 1, request(15, key)).await, 17, key, 5);
    send(&mut j, 1, request(23, key)).await;
    j.np.login_presence(id, 999).await.unwrap();
}
#[tokio::test(start_paused = true)]
async fn r13_retained_expiry_boundary_sweep_and_cleanup() {
    for sweep in [false, true] {
        let (mut j, t, p) = rig();
        let id = j.np.identities[&(t, p)].as_ref().unwrap().account;
        let key = initialized(&mut j, t, p).await;
        send(&mut j, 1, start(key, t, p)).await;
        let token = j.retain(key, Duration::from_secs(2)).unwrap();
        tokio::time::advance(Duration::from_secs(2)).await;
        if sweep {
            assert_eq!(
                j.expire().await.unwrap()[0].packets,
                vec![w::kick(key, 8).unwrap()]
            );
            assert!(j.expire().await.unwrap().is_empty());
        }
        fail(
            &send(&mut j, 1, retained_init(200, key, token, 7)).await,
            14,
            200,
            // Expired and not swept: 19. Swept: the token is gone, 20.
            if sweep { 20 } else { 19 },
        );
        assert_eq!(j.session_count(), 0);
        assert_eq!(j.store.lock().logout_updates.len(), 1);
        j.np.login_presence(id, 999).await.unwrap();
    }
}
#[tokio::test]
async fn r13_retained_binding_collision_and_missing_session() {
    let (mut j, t, p) = rig();
    let key = initialized(&mut j, t, p).await;
    send(&mut j, 1, start(key, t, p)).await;
    let token = j.retain(key, Duration::from_secs(10)).unwrap();
    send(&mut j, 1, init(200, t, p)).await;
    fail(
        &send(&mut j, 1, retained_init(200, key, token, 7)).await,
        14,
        200,
        2,
    );
    assert_eq!(j.state(key), None);
    fail(&send(&mut j, 1, request(15, key)).await, 17, key, 4);
    fail(&send(&mut j, 1, request(12, 200)).await, 14, 200, 2);
}
type Rig = Jobs<Shared<MemoryStore>, StubNp>;
fn packet(f: &str, v: &[Value]) -> Vec<u8> {
    w::packet(f, v).unwrap()
}
fn request(cmd: i16, key: u64) -> Vec<u8> {
    w::notice(cmd, key).unwrap()
}
fn hello(version: i16) -> Vec<u8> {
    packet(
        "hhhh",
        &[
            Value::I16(1),
            Value::I16(1),
            Value::I16(version),
            Value::I16(0),
        ],
    )
}
fn init(key: u64, t: [u8; 16], p: [u8; 16]) -> Vec<u8> {
    packet(
        "hqooi",
        &[
            Value::I16(6),
            Value::U64(key),
            Value::Id(t),
            Value::Id(p),
            Value::U32(7),
        ],
    )
}
fn start(key: u64, t: [u8; 16], p: [u8; 16]) -> Vec<u8> {
    packet(
        "hqoo",
        &[Value::I16(9), Value::U64(key), Value::Id(t), Value::Id(p)],
    )
}
fn rig() -> (Rig, [u8; 16], [u8; 16]) {
    let (np, t, p, _) = StubNp::fixture(10);
    (
        Jobs::new(Shared::new(MemoryStore::default()), np, Settings::default()).unwrap(),
        t,
        p,
    )
}
async fn send<S: Store>(j: &mut Jobs<S, StubNp>, link: u64, bytes: Vec<u8>) -> Vec<Vec<u8>> {
    j.handle(link, &bytes)
        .await
        .unwrap()
        .into_iter()
        .flat_map(|o| {
            assert_eq!(o.link, link);
            o.packets
        })
        .collect()
}
async fn initialized<S: Store>(j: &mut Jobs<S, StubNp>, t: [u8; 16], p: [u8; 16]) -> u64 {
    send(j, 1, hello(58)).await;
    let result = send(j, 1, init(100, t, p)).await;
    let v = w::fields("hqqoo", &result[0]).unwrap();
    let Value::U64(key) = v[2] else {
        panic!("no key")
    };
    assert_ne!(key, 0);
    assert_eq!(v[1], Value::U64(100));
    assert_eq!(v[3], Value::Id(t));
    assert_eq!(v[4], Value::Id(p));
    key
}
fn fail(bytes: &[Vec<u8>], cmd: i16, key: u64, reason: i16) {
    assert_eq!(
        w::fields("hqE", &bytes[0]).unwrap(),
        vec![Value::I16(cmd), Value::U64(key), Value::I16(reason)]
    );
}
fn joined(packets: &[Vec<u8>], f: &str) -> Vec<Value> {
    let mut reader = FragmentReader::default();
    let mut body = None;
    for p in packets {
        if let Some(v) = reader.add(p).unwrap() {
            body = Some(v.data.into_owned());
        }
    }
    decode_fields(f, &body.unwrap(), Default::default()).unwrap()
}
fn character() -> Character {
    Character {
        pcid: 77,
        slot: [8; 16],
        world: 1,
        name: "generated-character".encode_utf16().collect(),
        showcase: vec![9; 512],
        deletion_time: 0,
        state: 0,
        jumping_state: 0,
        last_play_end: 0,
    }
}
#[tokio::test]
async fn r1_hello_version_format_order_and_duplicate() {
    let (mut j, t, p) = rig();
    assert!(j.handle(1, &init(1, t, p)).await.is_err());
    let bad = send(&mut j, 1, hello(57)).await;
    assert_eq!(
        w::fields("hh", &bad[0]).unwrap(),
        vec![Value::I16(3), Value::I16(2)]
    );
    let ok = send(&mut j, 1, hello(58)).await;
    assert_eq!(
        joined(&ok, &w::HELLO[2..]),
        vec![
            Value::Repeat(vec![]),
            Value::I8(0),
            Value::I8(0),
            Value::U64(0),
            Value::Repeat(vec![])
        ]
    );
    assert_eq!(w::command(&send(&mut j, 1, hello(58)).await[0]).unwrap(), 3);
}
#[tokio::test]
async fn r2_init_rejections_echo_uniqueness_and_no_consumption() {
    let (mut j, t, p) = rig();
    let key = initialized(&mut j, t, p).await;
    assert_eq!(j.state(key), Some(State::Init));
    fail(&send(&mut j, 1, init(100, t, p)).await, 8, 100, 6);
    for (g, t, p) in [(0, t, p), (101, [0; 16], p), (101, t, [0; 16])] {
        fail(&send(&mut j, 1, init(g, t, p)).await, 8, g, 2);
    }
    assert!(j.np.consume(t, p).await.is_ok());
    send(&mut j, 1, request(18, 100)).await;
    let other = send(&mut j, 1, init(100, t, p)).await;
    assert_ne!(w::fields("hqqoo", &other[0]).unwrap()[2], Value::U64(key));
}
#[tokio::test]
async fn r3_state_guid_link_and_decode_failures() {
    let (mut j, t, p) = rig();
    let key = initialized(&mut j, t, p).await;
    send(&mut j, 2, hello(58)).await;
    fail(&send(&mut j, 2, start(key, t, p)).await, 11, key, 4);
    fail(&send(&mut j, 1, start(999, t, p)).await, 11, 999, 4);
    fail(&send(&mut j, 1, start(key, [4; 16], p)).await, 11, key, 5);
    fail(&send(&mut j, 1, request(9, key)).await, 11, key, 2);
    send(&mut j, 1, start(key, t, p)).await;
    fail(&send(&mut j, 1, start(key, t, p)).await, 11, key, 5);
    fail(
        &send(&mut j, 1, packet("h", &[Value::I16(6)])).await,
        8,
        0,
        2,
    );
}
#[tokio::test]
async fn r4_rejection_missing_fields_and_presence_error_maps() {
    for (error, reason) in [
        (NpError::RejectedToken, 15),
        (NpError::MissingData, 2),
        (NpError::Unavailable, 2),
        (NpError::PresenceConflict, 18),
        (NpError::Restricted, 16),
    ] {
        let (mut j, t, p) = rig();
        let key = initialized(&mut j, t, p).await;
        if matches!(error, NpError::PresenceConflict | NpError::Restricted) {
            j.np.presence_error = Some(error);
        } else {
            j.np.identities.insert((t, p), Err(error));
        }
        fail(&send(&mut j, 1, start(key, t, p)).await, 11, key, reason);
        assert_eq!(j.session_count(), 0);
    }
    let (mut j, t, p) = rig();
    let key = initialized(&mut j, t, p).await;
    let identity = j.np.identities.get_mut(&(t, p)).unwrap().as_mut().unwrap();
    identity.user_name.clear();
    fail(&send(&mut j, 1, start(key, t, p)).await, 11, key, 2);
}
#[tokio::test(start_paused = true)]
async fn r4_np_timeout_is_explicit_local_duration() {
    let (mut j, t, p) = rig();
    j.np.delay = Duration::from_secs(6);
    let key = initialized(&mut j, t, p).await;
    let before = tokio::time::Instant::now();
    fail(&send(&mut j, 1, start(key, t, p)).await, 11, key, 2);
    assert_eq!(before.elapsed(), Duration::from_secs(5));
    assert_eq!(j.session_count(), 0);
}
#[tokio::test]
async fn r4_stub_rejects_unknown_replay_and_wrong_presence_owner() {
    let (np, t, p, id) = StubNp::fixture(2);
    assert_eq!(np.consume([1; 16], p).await, Err(NpError::RejectedToken));
    assert_eq!(np.consume(t, p).await.unwrap(), id);
    assert_eq!(np.consume(t, p).await, Err(NpError::RejectedToken));
    np.login_presence(id.account, 1).await.unwrap();
    assert_eq!(
        np.login_presence(id.account, 2).await,
        Err(NpError::PresenceConflict)
    );
    np.logout_presence(id.account, 2);
    assert_eq!(
        np.login_presence(id.account, 2).await,
        Err(NpError::PresenceConflict)
    );
    np.logout_presence(id.account, 1);
    np.login_presence(id.account, 2).await.unwrap();
}
#[tokio::test]
async fn r5_nonempty_services_fail_and_release_presence() {
    let (mut j, t, p) = rig();
    let id = j.np.identities[&(t, p)].as_ref().unwrap().account;
    j.store.lock().services.insert(
        id,
        vec![IncompleteService {
            id: 1,
            kind: 1,
            item_instance: 0,
            item_data: 0,
            state: 0,
        }],
    );
    let key = initialized(&mut j, t, p).await;
    fail(&send(&mut j, 1, start(key, t, p)).await, 11, key, 2);
    assert_eq!(j.session_count(), 0);
    j.np.login_presence(id, 999).await.unwrap();
}
#[tokio::test]
async fn r12_kind6_recovery_caches_without_completing_service() {
    let (mut j, t, p) = rig();
    let id = j.np.identities[&(t, p)].as_ref().unwrap().account;
    let row = IncompleteService {
        id: 67,
        kind: 6,
        item_instance: 68,
        item_data: 69,
        state: 1,
    };
    j.store.lock().services.insert(id, vec![row.clone()]);
    let key = initialized(&mut j, t, p).await;
    assert_eq!(
        w::command(&send(&mut j, 1, start(key, t, p)).await[0]).unwrap(),
        10
    );
    assert_eq!(j.pending_services(key), Some([row.clone()].as_slice()));
    assert_eq!(j.store.lock().services[&id], vec![row]);
    send(&mut j, 1, request(23, key)).await;
    assert!(j.pending_services(key).is_none());
    assert_eq!(j.store.lock().services[&id].len(), 1);
}
#[tokio::test]
async fn r8_original_d22_reason_conversion() {
    for (reason, wire_reason) in [
        (-1, 0),
        (0, 0),
        (1, 1),
        (8, 8),
        (9, 0),
        (10, 9),
        (11, 10),
        (12, 11),
        (13, 0),
    ] {
        let (mut j, t, p) = rig();
        let key = initialized(&mut j, t, p).await;
        send(&mut j, 1, start(key, t, p)).await;
        assert_eq!(
            j.kick(key, reason).await.unwrap()[0].packets,
            vec![w::kick(key, wire_reason).unwrap()]
        );
    }
}
#[tokio::test]
async fn r5_r6_register_required_loads_cache_and_fragmented_character_list() {
    let (mut j, t, p) = rig();
    let id = j.np.identities[&(t, p)].as_ref().unwrap().account;
    j.store.lock().characters.insert(id, vec![character(); 12]);
    let key = initialized(&mut j, t, p).await;
    fail(&send(&mut j, 1, request(35, key)).await, 37, key, 5);
    let ok = send(&mut j, 1, start(key, t, p)).await;
    assert_eq!(w::command(&ok[0]).unwrap(), 10);
    assert_eq!(joined(&ok, &w::START[2..])[2], Value::Id(id));
    assert_eq!(j.store.lock().accounts[&id].right, 1);
    assert_eq!(j.state(key), Some(State::Lobby));
    assert_eq!(j.store.lock().login_updates, vec![(id, 7)]);
    j.store.lock().characters.clear();
    let list = send(&mut j, 1, request(35, key)).await;
    assert!(list.len() > 1);
    let got = joined(&list, &w::CHARACTERS[2..]);
    let Value::Repeat(rows) = &got[3] else {
        panic!("rows")
    };
    assert_eq!(rows.len(), 12);
    assert_eq!(rows[0][0], Value::U64(77));
    fail(&send(&mut j, 1, request(35, 999)).await, 37, 999, 4);
    send(&mut j, 2, hello(58)).await;
    fail(&send(&mut j, 2, request(35, key)).await, 37, key, 4);
}
#[tokio::test]
async fn r7_close_lookup_logout_notice_and_idempotence() {
    for cmd in [18, 19, 23] {
        for authed in [false, true] {
            let (mut j, t, p) = rig();
            let key = initialized(&mut j, t, p).await;
            if authed {
                send(&mut j, 1, start(key, t, p)).await;
            }
            send(&mut j, 2, hello(58)).await;
            let lookup = if cmd == 18 { 100 } else { key };
            assert!(send(&mut j, 2, request(cmd, lookup)).await.is_empty());
            assert_eq!(j.session_count(), 1);
            let got = send(&mut j, 1, request(cmd, lookup)).await;
            if cmd == 23 && authed {
                assert_eq!(got, vec![w::notice(21, key).unwrap()]);
            } else {
                assert!(got.is_empty());
            }
            assert_eq!(j.session_count(), 0);
            assert!(send(&mut j, 1, request(cmd, lookup)).await.is_empty());
            assert_eq!(j.store.lock().logout_updates.len(), usize::from(authed));
        }
    }
}
#[tokio::test]
async fn r8_d20_d21_d22_fixtures_and_kick_once() {
    assert_eq!(
        w::notice(20, 0x0102).unwrap(),
        vec![12, 0, 20, 0, 2, 1, 0, 0, 0, 0, 0, 0]
    );
    assert_eq!(
        w::notice(21, 0x0102).unwrap(),
        vec![12, 0, 21, 0, 2, 1, 0, 0, 0, 0, 0, 0]
    );
    assert_eq!(
        w::kick(0x0102, -1).unwrap(),
        vec![13, 0, 22, 0, 2, 1, 0, 0, 0, 0, 0, 0, 255]
    );
    let (mut j, t, p) = rig();
    let key = initialized(&mut j, t, p).await;
    send(&mut j, 1, start(key, t, p)).await;
    let got = j.kick(key, 7).await.unwrap();
    assert_eq!(got[0].link, 1);
    assert_eq!(got[0].packets, vec![w::kick(key, 7).unwrap()]);
    assert!(j.kick(key, 7).await.unwrap().is_empty());
    assert_eq!(j.store.lock().logout_updates.len(), 1);
}
#[tokio::test]
async fn r9_capacity_disconnect_and_limits() {
    let (np, t, p, _) = StubNp::fixture(10);
    let mut j = Jobs::new(
        Shared::new(MemoryStore::default()),
        np,
        Settings {
            max_sessions: 1,
            ..Settings::default()
        },
    )
    .unwrap();
    let key = initialized(&mut j, t, p).await;
    fail(&send(&mut j, 1, init(101, t, p)).await, 8, 101, 6);
    send(&mut j, 1, start(key, t, p)).await;
    j.disconnect(1).await;
    assert_eq!(j.session_count(), 0);
    assert_eq!(j.store.lock().logout_updates.len(), 1);
    assert!(j.handle(1, &init(100, t, p)).await.is_err());
    assert!(Jobs::new(
        Shared::new(MemoryStore::default()),
        StubNp::default(),
        Settings {
            np_timeout: Duration::ZERO,
            ..Settings::default()
        }
    )
    .is_err());
}
#[tokio::test]
async fn r1_frame_lengths_split_and_coalesced() {
    use tokio::io::AsyncWriteExt;
    for n in [-1i16, 0, 2, 3, 4001] {
        assert!(w::read(&mut n.to_le_bytes().as_slice()).await.is_err());
    }
    assert!(w::read(&mut [4, 0, 1].as_slice()).await.is_err());
    let (mut tx, mut rx) = tokio::io::duplex(100);
    let a = hello(58);
    let expected = a.clone();
    let task = tokio::spawn(async move {
        tx.write_all(&a[..1]).await.unwrap();
        tokio::task::yield_now().await;
        tx.write_all(&[a[1..].to_vec(), a].concat()).await.unwrap();
    });
    assert_eq!(w::read(&mut rx).await.unwrap(), Some(expected.clone()));
    assert_eq!(w::read(&mut rx).await.unwrap(), Some(expected));
    task.await.unwrap();
    assert!(w::read(&mut rx).await.unwrap().is_none());
}

#[tokio::test(start_paused = true)]
async fn r13_retained_and_rebound_cleanup_release_presence_once() {
    for rebound in [false, true] {
        for trigger in 0..3 {
            let (np, t, p, id) = StubNp::fixture(31);
            let mut j = Jobs::new(
                Shared::new(MemoryStore::default()),
                np,
                Settings {
                    init_timeout: Some(Duration::from_secs(2)),
                    ..Settings::default()
                },
            )
            .unwrap();
            let key = initialized(&mut j, t, p).await;
            send(&mut j, 1, start(key, t, p)).await;
            let token = j.retain(key, Duration::from_secs(5)).unwrap();
            if rebound {
                let reply = send(&mut j, 1, retained_init(200, key, token, 7)).await;
                assert_eq!(w::command(&reply[0]).unwrap(), 13);
                fail(
                    &send(&mut j, 1, retained_init(201, key, token, 7)).await,
                    14,
                    201,
                    20,
                );
            }
            match trigger {
                0 => {
                    tokio::time::advance(Duration::from_secs(if rebound { 2 } else { 5 })).await;
                    let outputs = j.expire().await.unwrap();
                    assert_eq!(outputs.len(), 1);
                    assert_eq!(
                        outputs[0].packets,
                        vec![if rebound {
                            w::notice(20, 200).unwrap()
                        } else {
                            w::kick(key, 8).unwrap()
                        }]
                    );
                    assert!(j.expire().await.unwrap().is_empty());
                }
                1 => {
                    j.disconnect(1).await;
                    j.disconnect(1).await;
                }
                _ => {
                    let outputs = j.kick(key, 7).await.unwrap();
                    assert_eq!(
                        outputs[0].packets,
                        vec![if rebound {
                            w::notice(20, 200).unwrap()
                        } else {
                            w::kick(key, 7).unwrap()
                        }]
                    );
                    assert!(j.kick(key, 7).await.unwrap().is_empty());
                }
            }
            assert_eq!(j.session_count(), 0);
            assert_eq!(j.store.lock().logout_updates.len(), 1);
            j.np.login_presence(id.account, 999).await.unwrap();
        }
    }
}

#[tokio::test(start_paused = true)]
async fn r13_retained_start_cancels_rebound_init_deadline() {
    let (np, t, p, _) = StubNp::fixture(32);
    let mut j = Jobs::new(
        Shared::new(MemoryStore::default()),
        np,
        Settings {
            init_timeout: Some(Duration::from_secs(2)),
            ..Settings::default()
        },
    )
    .unwrap();
    let key = initialized(&mut j, t, p).await;
    send(&mut j, 1, start(key, t, p)).await;
    let token = j.retain(key, Duration::from_secs(5)).unwrap();
    send(&mut j, 1, retained_init(200, key, token, 7)).await;
    assert!(j.next_expiry().is_some());
    let reply = send(&mut j, 1, request(15, key)).await;
    assert_eq!(w::command(&reply[0]).unwrap(), 16);
    assert!(j.next_expiry().is_none());
    tokio::time::advance(Duration::from_secs(5)).await;
    assert!(j.expire().await.unwrap().is_empty());
    assert_eq!(j.state(key), Some(State::Lobby));
    assert!(j.store.lock().logout_updates.is_empty());
}

#[derive(Default)]
struct FaultStore {
    inner: MemoryStore,
    fail: &'static str,
}
impl SyncStore for FaultStore {
    fn load_game_account(&mut self, id: AccountId) -> io::Result<Option<Account>> {
        if self.fail == "account" {
            Err(io::Error::other("generated failure"))
        } else {
            self.inner.load_game_account(id)
        }
    }
    fn register_game_account(&mut self, a: Account, ip: u32) -> io::Result<()> {
        if self.fail == "register" {
            Err(io::Error::other("generated failure"))
        } else {
            self.inner.register_game_account(a, ip)
        }
    }
    fn update_last_login(&mut self, id: AccountId, ip: u32) -> io::Result<()> {
        self.inner.update_last_login(id, ip)?;
        Err(io::Error::other("generated update failure"))
    }
    fn update_last_logout(&mut self, id: AccountId, score: i32) -> io::Result<()> {
        self.inner.update_last_logout(id, score)?;
        Err(io::Error::other("generated update failure"))
    }
    fn load_character_list(&mut self, id: AccountId) -> io::Result<Vec<Character>> {
        if self.fail == "characters" {
            Err(io::Error::other("generated failure"))
        } else {
            self.inner.load_character_list(id)
        }
    }
    fn load_incomplete_lobby_service(
        &mut self,
        id: AccountId,
    ) -> io::Result<Vec<IncompleteService>> {
        if self.fail == "services" {
            Err(io::Error::other("generated failure"))
        } else {
            self.inner.load_incomplete_lobby_service(id)
        }
    }
}
#[tokio::test]
async fn r5_required_store_failures_and_best_effort_updates() {
    for (stage, reason) in [
        ("account", 16),
        ("register", 16),
        ("characters", 16),
        ("services", 16),
        ("", 0),
    ] {
        let (np, t, p, id) = StubNp::fixture(10);
        let mut store = FaultStore {
            fail: stage,
            ..FaultStore::default()
        };
        if stage.is_empty() {
            store.inner.accounts.insert(
                id.account,
                Account {
                    id: id.account,
                    name: id.login_name.clone(),
                    right: 1,
                    representative_pcid: 77,
                    representative_world: 1,
                    bot_score: 4,
                },
            );
        }
        let mut j = Jobs::new(Shared::new(store), np, Settings::default()).unwrap();
        let key = initialized(&mut j, t, p).await;
        let result = send(&mut j, 1, start(key, t, p)).await;
        if reason != 0 {
            fail(&result, 11, key, reason);
            assert_eq!(j.session_count(), 0);
        } else {
            assert_eq!(w::command(&result[0]).unwrap(), 10);
            assert_eq!(j.store.lock().inner.login_updates.len(), 1);
            send(&mut j, 1, request(23, key)).await;
            assert_eq!(j.store.lock().inner.logout_updates, vec![(id.account, 4)]);
        }
        j.np.login_presence(id.account, 999).await.unwrap();
    }
}
