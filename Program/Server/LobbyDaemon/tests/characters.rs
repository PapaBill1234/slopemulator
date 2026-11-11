use bloodlib::packet::{decode_fields, Value};
use lobbydaemon::{
    character::{
        CharacterPolicy, CreateCharacter, CreatedCharacter, InternalState, ProcedureError,
    },
    gamed::StubGameDaemon,
    jobs::{Jobs, Settings, State},
    np::{initial_name_reason, StubNp},
    protocol as w,
    store::{Account, MemoryStore, Shared, SyncStore},
};
use packet::fragments::FragmentReader;
use std::{sync::Arc, time::Duration};
type Rig = Jobs<Shared<MemoryStore>, StubNp>;
const SLOT: [u8; 16] = [91; 16];
fn game(
    create: Option<i16>,
    delete: Option<i16>,
    restore: Option<i16>,
    delay: u64,
) -> Arc<StubGameDaemon> {
    let mut g = StubGameDaemon::default();
    g.create_error = create;
    g.delete_error = delete;
    g.restore_error = restore;
    g.delay = Duration::from_secs(delay);
    Arc::new(g)
}
fn policy() -> CharacterPolicy {
    CharacterPolicy {
        slots: [SLOT].into(),
        worlds: [1].into(),
        min_name_units: 2,
        max_name_units: 16,
        deletion_wait_seconds: 60,
    }
}
fn packet(f: &str, v: &[Value]) -> Vec<u8> {
    w::packet(f, v).unwrap()
}
fn request(cmd: i16, key: u64) -> Vec<u8> {
    packet("hqo", &[Value::I16(cmd), Value::U64(key), Value::Id(SLOT)])
}
fn create(key: u64) -> Vec<u8> {
    packet(
        "hqohsbQ",
        &[
            Value::I16(44),
            Value::U64(key),
            Value::Id(SLOT),
            Value::I16(1),
            Value::String("Generated".encode_utf16().collect()),
            Value::Bytes(vec![1, 2]),
            Value::I8(0),
        ],
    )
}
async fn send(j: &mut Rig, p: Vec<u8>) -> Vec<Vec<u8>> {
    j.handle(1, &p)
        .await
        .unwrap()
        .into_iter()
        .flat_map(|o| o.packets)
        .collect()
}
async fn rig() -> (Rig, u64) {
    let (np, t, p, _) = StubNp::fixture(85);
    let mut j = Jobs::new(
        Shared::new(MemoryStore::default()),
        np,
        Settings {
            character_policy: Some(policy()),
            ..Default::default()
        },
    )
    .unwrap();
    send(
        &mut j,
        packet(
            "hhhh",
            &[Value::I16(1), Value::I16(0), Value::I16(58), Value::I16(0)],
        ),
    )
    .await;
    let init = send(
        &mut j,
        packet(
            "hqooi",
            &[
                Value::I16(6),
                Value::U64(1),
                Value::Id(t),
                Value::Id(p),
                Value::U32(1),
            ],
        ),
    )
    .await;
    let Value::U64(key) = w::fields("hqqoo", &init[0]).unwrap()[2] else {
        panic!()
    };
    send(
        &mut j,
        packet(
            "hqoo",
            &[Value::I16(9), Value::U64(key), Value::Id(t), Value::Id(p)],
        ),
    )
    .await;
    (j, key)
}
fn assert_failure(out: &[Vec<u8>], cmd: i16, reason: i16) {
    assert_eq!(out.len(), 1);
    let fields = w::fields(
        if cmd == 46 {
            "hqoE"
        } else if cmd == 105 {
            "hqE"
        } else {
            "hqoQE"
        },
        &out[0],
    )
    .unwrap();
    assert_eq!(fields[0], Value::I16(cmd));
    assert_eq!(fields.last(), Some(&Value::I16(reason)));
}
async fn list(j: &mut Rig, key: u64) -> Vec<Vec<Value>> {
    let packets = send(j, packet("hq", &[Value::I16(35), Value::U64(key)])).await;
    let mut reader = FragmentReader::default();
    let mut rows = None;
    for p in packets {
        if let Some(joined) = reader.add(&p).unwrap() {
            let fields = decode_fields("qqhCqohsbqQQi;", &joined.data, Default::default()).unwrap();
            let Value::Repeat(found) = &fields[3] else {
                panic!()
            };
            rows = Some(found.clone());
        }
    }
    rows.unwrap()
}
#[tokio::test(start_paused = true)]
async fn lifecycle_lists_normal_waiting_and_restored_rows() {
    let (mut j, key) = rig().await;
    assert!(list(&mut j, key).await.is_empty());
    assert!(
        send(&mut j, packet("hq", &[Value::I16(41), Value::U64(key)]))
            .await
            .is_empty()
    );
    let out = send(&mut j, create(key)).await;
    assert_eq!(w::command(&out[0]).unwrap(), 45);
    assert_eq!(list(&mut j, key).await[0][6], Value::I8(1));
    let out = send(&mut j, request(47, key)).await;
    assert_eq!(
        w::fields("hqoQi", &out[0]).unwrap()[3..],
        [Value::I8(2), Value::U32(60)]
    );
    tokio::time::advance(Duration::from_secs(7)).await;
    let rows = list(&mut j, key).await;
    assert_eq!(rows[0][6], Value::I8(2));
    assert_eq!(rows[0][8], Value::U32(53));
    assert_eq!(
        w::command(&send(&mut j, request(51, key)).await[0]).unwrap(),
        52
    );
    let rows = list(&mut j, key).await;
    assert_eq!(rows[0][6], Value::I8(1));
    assert_eq!(rows[0][8], Value::U32(0));
    // A second deletion/restoration is legal. No one-shot restore flag leaks.
    send(&mut j, request(47, key)).await;
    assert_eq!(
        w::command(&send(&mut j, request(51, key)).await[0]).unwrap(),
        52
    );
}
#[tokio::test]
async fn mutation_session_link_slot_and_decode_errors() {
    let (mut j, key) = rig().await;
    for (cmd, fail) in [(44, 46), (47, 49), (51, 53), (54, 56)] {
        assert_failure(
            &send(
                &mut j,
                if cmd == 44 {
                    create(key + 100)
                } else {
                    request(cmd, key + 100)
                },
            )
            .await,
            fail,
            4,
        );
        if cmd == 44 {
            assert!(send(&mut j, request(cmd, key)).await.is_empty());
        } else {
            assert_failure(&send(&mut j, request(cmd, key)).await, fail, 6);
        }
    }
}
#[tokio::test]
async fn create_validation_and_store_errors_do_not_add_rows() {
    for change in 0..8 {
        let (mut j, key) = rig().await;
        let mut v = w::fields("hqohsbQ", &create(key)).unwrap();
        match change {
            0 => v[2] = Value::Id([0; 16]),
            1 => v[2] = Value::Id([92; 16]),
            2 => v[3] = Value::I16(0),
            3 => v[4] = Value::String(vec![]),
            4 => v[5] = Value::Bytes(vec![]),
            5 => v[5] = Value::Bytes(vec![0; 257]),
            6 => v[4] = Value::String(vec![0]),
            _ => v[4] = Value::String(vec![0xD800]),
        }
        assert_failure(&send(&mut j, packet("hqohsbQ", &v)).await, 46, 6);
        assert!(list(&mut j, key).await.is_empty());
    }
    for (op, code) in [
        ("check_name", 1),
        ("check_name", 2),
        ("check_name", 3),
        ("try_create", 1),
        ("try_create", 2),
        ("try_create", 3),
        ("finish_create", 1),
    ] {
        let (mut j, key) = rig().await;
        j.store.lock().character_failures.insert(op, code);
        assert_failure(&send(&mut j, create(key)).await, 46, 2);
        assert!(list(&mut j, key).await.is_empty());
    }
}
#[tokio::test]
async fn game_create_rejection_rolls_back_and_ids_do_not_repeat() {
    let (mut j, key) = rig().await;
    j = j.with_game_daemon(game(Some(9), None, None, 0));
    assert_failure(&send(&mut j, create(key)).await, 46, 2);
    assert!(j
        .store
        .lock()
        .characters
        .values()
        .flatten()
        .next()
        .is_none());
    j = j.with_game_daemon(Arc::new(StubGameDaemon::default()));
    send(&mut j, create(key)).await;
    assert_eq!(list(&mut j, key).await[0][0], Value::U64(2));
}
#[tokio::test(start_paused = true)]
async fn timeouts_keep_uncertain_state_for_recovery() {
    let (mut j, key) = rig().await;
    send(&mut j, create(key)).await;
    j = j.with_game_daemon(game(None, None, None, 6));
    assert_failure(&send(&mut j, request(47, key)).await, 49, 2);
    assert_eq!(list(&mut j, key).await[0][6], Value::I8(3));
    j = j.with_game_daemon(Arc::new(StubGameDaemon::default()));
    assert_eq!(
        w::command(&send(&mut j, request(54, key)).await[0]).unwrap(),
        55
    );
    assert_eq!(list(&mut j, key).await[0][6], Value::I8(1));
}
#[tokio::test]
async fn delete_rejection_and_db_failure_recovery() {
    for op in ["try_delete", "finish_delete"] {
        let (mut j, key) = rig().await;
        send(&mut j, create(key)).await;
        j.store.lock().character_failures.insert(op, 1);
        assert_failure(&send(&mut j, request(47, key)).await, 49, 2);
        assert_eq!(
            list(&mut j, key).await[0][6],
            Value::I8(if op == "try_delete" { 1 } else { 3 })
        );
    }
    let (mut j, key) = rig().await;
    send(&mut j, create(key)).await;
    j = j.with_game_daemon(game(None, Some(3), None, 0));
    assert_failure(&send(&mut j, request(47, key)).await, 49, 2);
    assert_eq!(list(&mut j, key).await[0][6], Value::I8(1));
    j.store.lock().character_failures.insert("cancel_delete", 1);
    assert_failure(&send(&mut j, request(47, key)).await, 49, 2);
    j = j.with_game_daemon(game(None, None, Some(35), 0));
    assert_eq!(
        w::command(&send(&mut j, request(54, key)).await[0]).unwrap(),
        55
    );
}
#[tokio::test]
async fn restoration_failures_leave_recoverable_state_five() {
    for op in ["try_restore", "finish_restore"] {
        let (mut j, key) = rig().await;
        send(&mut j, create(key)).await;
        send(&mut j, request(47, key)).await;
        j.store.lock().character_failures.insert(op, 1);
        assert_failure(&send(&mut j, request(51, key)).await, 53, 2);
        if op == "finish_restore" {
            assert_eq!(
                w::command(&send(&mut j, request(54, key)).await[0]).unwrap(),
                55
            );
        }
    }
    let (mut j, key) = rig().await;
    send(&mut j, create(key)).await;
    send(&mut j, request(47, key)).await;
    j = j.with_game_daemon(game(None, None, Some(33), 0));
    assert_failure(&send(&mut j, request(51, key)).await, 53, 2);
    assert_failure(&send(&mut j, request(54, key)).await, 56, 2);
    j = j.with_game_daemon(Arc::new(StubGameDaemon::default()));
    assert_eq!(
        w::command(&send(&mut j, request(54, key)).await[0]).unwrap(),
        55
    );
    assert_failure(&send(&mut j, request(54, key)).await, 56, 2);
}
#[tokio::test(start_paused = true)]
async fn expired_wait_does_not_restore() {
    let (mut j, key) = rig().await;
    send(&mut j, create(key)).await;
    send(&mut j, request(47, key)).await;
    tokio::time::advance(Duration::from_secs(60)).await;
    assert_failure(&send(&mut j, request(51, key)).await, 53, 2);
    assert_eq!(list(&mut j, key).await[0][8], Value::U32(0));
}
#[tokio::test]
async fn initial_name_success_and_all_platform_maps() {
    for (np_code, wire) in [
        (4, 2),
        (48, 2),
        (3000, 2),
        (3002, 2),
        (10, 6),
        (3035, 6),
        (3003, 229),
        (3042, 229),
        (3041, 228),
        (3336, 231),
        (3743, 230),
        (99, 1),
    ] {
        assert_eq!(initial_name_reason(np_code), wire);
        let (mut j, key) = rig().await;
        let account = *j.store.lock().accounts.keys().next().unwrap();
        let name: Vec<u16> = "User307".encode_utf16().collect();
        j.np.initial_names
            .insert((account, name.clone()), Err(np_code));
        j.begin_initial_name(key).unwrap();
        assert_failure(
            &send(
                &mut j,
                packet(
                    "hqs",
                    &[Value::I16(103), Value::U64(key), Value::String(name)],
                ),
            )
            .await,
            105,
            wire,
        );
        assert_eq!(j.state(key), Some(State::InitialName));
    }
    let (mut j, key) = rig().await;
    let name: Vec<u16> = "User307".encode_utf16().collect();
    let req = packet(
        "hqs",
        &[
            Value::I16(103),
            Value::U64(key),
            Value::String(name.clone()),
        ],
    );
    assert_failure(&send(&mut j, req.clone()).await, 105, 5);
    j.begin_initial_name(key).unwrap();
    assert_failure(&send(&mut j, req.clone()).await, 105, 7);
    let account = *j.store.lock().accounts.keys().next().unwrap();
    j.np.initial_names.insert((account, name.clone()), Ok(()));
    let out = send(&mut j, req).await;
    assert_eq!(
        w::fields("hqs", &out[0]).unwrap(),
        vec![Value::I16(104), Value::U64(key), Value::String(name)]
    );
    assert_eq!(j.state(key), Some(State::Lobby));
}
#[tokio::test]
async fn initial_name_validation_maps() {
    for (name, reason) in [
        (vec![], 6),
        (vec![0], 6),
        (vec![65], 224),
        (vec![65; 17], 225),
        (vec![65, 32, 66], 226),
        (vec![65, 0xD800], 227),
    ] {
        let (mut j, key) = rig().await;
        j.begin_initial_name(key).unwrap();
        assert_failure(
            &send(
                &mut j,
                packet(
                    "hqs",
                    &[Value::I16(103), Value::U64(key), Value::String(name)],
                ),
            )
            .await,
            105,
            reason,
        );
    }
}
#[tokio::test(start_paused = true)]
async fn creation_restoration_and_initial_name_timeouts() {
    let (mut j, key) = rig().await;
    j = j.with_game_daemon(game(None, None, None, 6));
    assert_failure(&send(&mut j, create(key)).await, 46, 2);
    assert!(list(&mut j, key).await.is_empty());
    assert_eq!(j.store.lock().character_states[&1], InternalState::Creating);
    let (mut j, key) = rig().await;
    send(&mut j, create(key)).await;
    send(&mut j, request(47, key)).await;
    j = j.with_game_daemon(game(None, None, None, 6));
    assert_failure(&send(&mut j, request(51, key)).await, 53, 2);
    assert_eq!(
        j.store.lock().character_states[&1],
        InternalState::Restoring
    );
    assert_failure(&send(&mut j, request(54, key)).await, 56, 2);
    j = j.with_game_daemon(Arc::new(StubGameDaemon::default()));
    assert_eq!(
        w::command(&send(&mut j, request(54, key)).await[0]).unwrap(),
        55
    );
    let account = *j.store.lock().accounts.keys().next().unwrap();
    let name: Vec<u16> = "User307".encode_utf16().collect();
    j.np.initial_names.insert((account, name.clone()), Ok(()));
    j.np.delay = Duration::from_secs(6);
    j.begin_initial_name(key).unwrap();
    assert_failure(
        &send(
            &mut j,
            packet(
                "hqs",
                &[Value::I16(103), Value::U64(key), Value::String(name)],
            ),
        )
        .await,
        105,
        7,
    );
    assert_eq!(j.state(key), Some(State::InitialName));
}
#[tokio::test]
async fn immediate_delete_reply_and_list_are_consistent() {
    let (np, t, p, _) = StubNp::fixture(85);
    let mut config = policy();
    config.deletion_wait_seconds = 0;
    let mut j = Jobs::new(
        Shared::new(MemoryStore::default()),
        np,
        Settings {
            character_policy: Some(config),
            ..Default::default()
        },
    )
    .unwrap();
    send(
        &mut j,
        packet(
            "hhhh",
            &[Value::I16(1), Value::I16(0), Value::I16(58), Value::I16(0)],
        ),
    )
    .await;
    let init = send(
        &mut j,
        packet(
            "hqooi",
            &[
                Value::I16(6),
                Value::U64(1),
                Value::Id(t),
                Value::Id(p),
                Value::U32(1),
            ],
        ),
    )
    .await;
    let Value::U64(key) = w::fields("hqqoo", &init[0]).unwrap()[2] else {
        panic!()
    };
    send(
        &mut j,
        packet(
            "hqoo",
            &[Value::I16(9), Value::U64(key), Value::Id(t), Value::Id(p)],
        ),
    )
    .await;
    send(&mut j, create(key)).await;
    let out = send(&mut j, request(47, key)).await;
    assert_eq!(
        w::fields("hqoQi", &out[0]).unwrap()[3..],
        [Value::I8(0), Value::U32(0)]
    );
    assert!(list(&mut j, key).await.is_empty());
    assert!(j
        .store
        .lock()
        .characters
        .values()
        .flatten()
        .next()
        .is_none());
}
#[tokio::test]
async fn wrong_link_and_initial_name_state_block_mutations() {
    let (mut j, key) = rig().await;
    j.handle(
        2,
        &packet(
            "hhhh",
            &[Value::I16(1), Value::I16(0), Value::I16(58), Value::I16(0)],
        ),
    )
    .await
    .unwrap();
    for (cmd, fail) in [(44, 46), (47, 49), (51, 53), (54, 56)] {
        let p = if cmd == 44 {
            create(key)
        } else {
            request(cmd, key)
        };
        let out: Vec<_> = j
            .handle(2, &p)
            .await
            .unwrap()
            .into_iter()
            .flat_map(|o| o.packets)
            .collect();
        assert_failure(&out, fail, 4);
    }
    j.begin_initial_name(key).unwrap();
    for cmd in [44, 47, 51, 54] {
        assert!(send(
            &mut j,
            if cmd == 44 {
                create(key)
            } else {
                request(cmd, key)
            }
        )
        .await
        .is_empty());
    }
    assert!(j.store.lock().characters.is_empty());
    let req = packet(
        "hqs",
        &[
            Value::I16(103),
            Value::U64(key + 100),
            Value::String(vec![65, 66]),
        ],
    );
    assert_failure(&send(&mut j, req).await, 105, 4);
    assert!(
        send(&mut j, packet("hq", &[Value::I16(103), Value::U64(key)]))
            .await
            .is_empty()
    );
}

#[test]
fn memory_store_preserves_procedure_codes_and_enforces_states() {
    let mut store = MemoryStore::default();
    let account = [80; 16];
    store.accounts.insert(
        account,
        Account {
            id: account,
            name: vec![],
            right: 1,
            representative_pcid: 0,
            representative_world: 0,
            bot_score: 0,
        },
    );
    let request = CreateCharacter {
        account,
        slot: SLOT,
        world: 1,
        name: vec![65, 66],
        right: 1,
    };
    let pcid = store.try_character_creation(request.clone()).unwrap();
    let error = store.try_character_creation(request).unwrap_err();
    assert_eq!(
        error.get_ref().unwrap().downcast_ref::<ProcedureError>(),
        Some(&ProcedureError(2))
    );
    assert!(store.try_character_deletion(pcid).is_err());
    store
        .finish_character_creation(
            pcid,
            CreatedCharacter {
                job: 1,
                level: 1,
                showcase: vec![0; 512],
            },
        )
        .unwrap();
    store.try_character_deletion(pcid).unwrap();
    store.finish_character_deletion(pcid, 60).unwrap();
    assert!(store.cancel_character_deletion(pcid).is_err());
    store.try_character_restoration(pcid).unwrap();
    store.finish_character_restoration(pcid).unwrap();
    assert_eq!(store.character_states[&pcid], InternalState::Normal);
    store.try_character_deletion(pcid).unwrap();
    assert!(store.finish_character_deletion(pcid, 0).unwrap().is_none());
    assert!(store.characters.values().flatten().next().is_none());
    assert_eq!(store.character_states[&pcid], InternalState::Deleted);
}
