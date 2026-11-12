//! Card lp2-1: session states, LogoutJob table, wait info, give up wait, unknown command.
use bloodlib::packet::Value;
use lobbydaemon::{
    character::CharacterPolicy,
    gamed::{ChargeMode, Eligibility, StubGameDaemon, WaitInfo, WorldFixture},
    jobs::{Jobs, Settings, State},
    np::StubNp,
    protocol as w,
    store::{Character, MemoryStore, Shared},
};
use std::{sync::Arc, time::Duration};

type Rig = Jobs<Shared<MemoryStore>, StubNp>;
const LINK: u64 = 1;
const GATE: u64 = 100;
const IPV4: u32 = 123;

fn packet(f: &str, v: &[Value]) -> Vec<u8> {
    w::packet(f, v).unwrap()
}
async fn send(j: &mut Rig, p: Vec<u8>) -> Vec<Vec<u8>> {
    j.handle(LINK, &p)
        .await
        .unwrap()
        .into_iter()
        .flat_map(|o| o.packets)
        .collect()
}
fn request(cmd: i16, key: u64) -> Vec<u8> {
    w::notice(cmd, key).unwrap()
}
fn enter(key: u64) -> Vec<u8> {
    packet(
        "hqo",
        &[Value::I16(24), Value::U64(key), Value::Id([9; 16])],
    )
}
struct Fixture {
    j: Rig,
    g: Arc<StubGameDaemon>,
    key: u64,
}
/// Init an anonymous session on the link (6), then start it (9) when `start` is true.
async fn session(j: &mut Rig, token: [u8; 16], portal: [u8; 16], gate: u64, start: bool) -> u64 {
    let init = send(
        j,
        packet(
            "hqooi",
            &[
                Value::I16(6),
                Value::U64(gate),
                Value::Id(token),
                Value::Id(portal),
                Value::U32(IPV4),
            ],
        ),
    )
    .await;
    let Value::U64(key) = w::fields("hqqoo", &init[0]).unwrap()[2] else {
        panic!()
    };
    if start {
        send(
            j,
            packet(
                "hqoo",
                &[
                    Value::I16(9),
                    Value::U64(key),
                    Value::Id(token),
                    Value::Id(portal),
                ],
            ),
        )
        .await;
    }
    key
}
/// A rig with one session. `start` false: the session stays in state `Init`.
async fn rig(queued: bool, start: bool) -> Fixture {
    let (mut np, t, p, id) = StubNp::fixture(61);
    np.world_results
        .insert((id.account, 77, [9; 16]), Ok(ChargeMode::Paid));
    let c = Character {
        pcid: 77,
        slot: [9; 16],
        world: 1,
        name: vec![65],
        showcase: vec![],
        deletion_time: None,
        state: lobbydaemon::character::InternalState::Normal,
        jumping_state: 0,
        last_play_end: 0,
    };
    let mut store = MemoryStore::default();
    store.characters.insert(id.account, vec![c]);
    store.restoration_allowed.insert(77);
    let mut g = StubGameDaemon::default();
    g.worlds.insert(
        (1, 77),
        WorldFixture {
            slot: [9; 16],
            eligibility: Ok(Eligibility::Ready),
            restore: Ok(()),
            admission: Ok(queued.then_some(WaitInfo {
                first: 3,
                second: 8,
                rows: vec![(1, 2)],
            })),
        },
    );
    let g = Arc::new(g);
    let mut j = Jobs::new(
        Shared::new(store),
        np,
        Settings {
            character_policy: Some(CharacterPolicy {
                slots: [[9; 16]].into(),
                worlds: [1].into(),
                min_name_units: 2,
                max_name_units: 16,
                deletion_wait_seconds: 60,
            }),
            world_token_lifetime: Some(Duration::from_secs(10)),
            ..Settings::default()
        },
    )
    .unwrap()
    .with_game_daemon(g.clone());
    send(
        &mut j,
        packet(
            "hhhh",
            &[Value::I16(1), Value::I16(0), Value::I16(58), Value::I16(0)],
        ),
    )
    .await;
    let key = session(&mut j, t, p, GATE, start).await;
    Fixture { j, g, key }
}

#[tokio::test(start_paused = true)]
async fn wait_info_in_queue() {
    let Fixture { mut j, key, .. } = rig(true, true).await;
    send(&mut j, enter(key)).await;
    assert_eq!(j.state(key), Some(State::WorldQueued));
    let p = send(&mut j, request(28, key)).await;
    assert_eq!(p.len(), 1);
    assert_eq!(
        w::fields("hqiiCch;", &p[0]).unwrap(),
        vec![
            Value::I16(29),
            Value::U64(key),
            Value::U32(3),
            Value::U32(8),
            Value::Repeat(vec![vec![Value::U8(1), Value::I16(2)]])
        ]
    );
}

#[tokio::test(start_paused = true)]
async fn wait_info_no_session() {
    let Fixture { mut j, .. } = rig(true, true).await;
    let p = send(&mut j, request(28, 999_999)).await;
    assert_eq!(p, vec![w::failure(30, 999_999, 4).unwrap()]);
}

#[tokio::test(start_paused = true)]
async fn wait_info_wrong_state_and_bad_decode() {
    let Fixture { mut j, key, .. } = rig(true, true).await;
    // ❓ code 5 outside 3.2 is a local choice (20-handlers §4.1 gives no number).
    assert_eq!(
        send(&mut j, request(28, key)).await,
        vec![w::failure(30, key, 5).unwrap()]
    );
    // A short packet has no `hq` body: no reply.
    assert!(send(&mut j, packet("h", &[Value::I16(28)]))
        .await
        .is_empty());
}

#[tokio::test(start_paused = true)]
async fn giveup_wait_ok() {
    let Fixture { mut j, g, key } = rig(true, true).await;
    send(&mut j, enter(key)).await;
    assert_eq!(g.active(), 1);
    let p = send(&mut j, request(31, key)).await;
    assert_eq!(p, vec![w::notice(32, key).unwrap()]);
    assert_eq!(j.state(key), Some(State::Lobby));
    assert_eq!(g.active(), 0);
    assert_eq!(g.released(), 1);
    // A new 24 works.
    let p = send(&mut j, enter(key)).await;
    assert_eq!(p[0], w::failure(26, key, 107).unwrap());
    assert_eq!(j.state(key), Some(State::WorldQueued));
}

#[tokio::test(start_paused = true)]
async fn giveup_wait_wrong_state() {
    let Fixture { mut j, key, .. } = rig(true, true).await;
    assert_eq!(j.state(key), Some(State::Lobby));
    assert_eq!(
        send(&mut j, request(31, key)).await,
        vec![w::failure(33, key, 5).unwrap()]
    );
    assert_eq!(j.state(key), Some(State::Lobby));
}

#[tokio::test(start_paused = true)]
async fn giveup_wait_no_session() {
    let Fixture { mut j, .. } = rig(true, true).await;
    assert_eq!(
        send(&mut j, request(31, 999_999)).await,
        vec![w::failure(33, 999_999, 4).unwrap()]
    );
}

#[tokio::test(start_paused = true)]
async fn unknown_command_keeps_link() {
    let Fixture { mut j, key, .. } = rig(true, true).await;
    let (np2, t2, p2, _) = StubNp::fixture(70);
    j.np.identities.extend(np2.identities);
    let other = session(&mut j, t2, p2, GATE + 1, true).await;
    assert_ne!(key, other);
    assert!(send(&mut j, request(200, key)).await.is_empty());
    assert_eq!(j.state(key), Some(State::Lobby));
    let p = send(&mut j, request(35, other)).await;
    assert!(!p.is_empty());
    assert_eq!(w::command(&p[0]).unwrap(), 36);
    assert_eq!(j.session_count(), 2);
}

#[tokio::test(start_paused = true)]
async fn logout_table() {
    // Command 23 in each local state. Init 1.1: no notify, session gone.
    let Fixture { mut j, key, .. } = rig(false, false).await;
    assert_eq!(j.state(key), Some(State::Init));
    assert!(send(&mut j, request(23, key)).await.is_empty());
    assert_eq!(j.state(key), None);
    // Lobby 2.0
    let Fixture { mut j, key, .. } = rig(false, true).await;
    assert_eq!(
        send(&mut j, request(23, key)).await,
        vec![w::notice(21, key).unwrap()]
    );
    assert_eq!(j.state(key), None);
    // Initial name (local sub-state of 2.0)
    let Fixture { mut j, key, .. } = rig(false, true).await;
    j.begin_initial_name(key).unwrap();
    assert_eq!(j.state(key), Some(State::InitialName));
    assert_eq!(
        send(&mut j, request(23, key)).await,
        vec![w::notice(21, key).unwrap()]
    );
    assert_eq!(j.state(key), None);
    // WorldQueued 3.2: session gone, place released, notify
    let Fixture { mut j, g, key } = rig(true, true).await;
    send(&mut j, enter(key)).await;
    assert_eq!(j.state(key), Some(State::WorldQueued));
    assert_eq!(
        send(&mut j, request(23, key)).await,
        vec![w::notice(21, key).unwrap()]
    );
    assert_eq!(j.state(key), None);
    assert_eq!(g.active(), 0);
    // WorldReady 3.3: absorbed, notify
    let Fixture { mut j, key, .. } = rig(false, true).await;
    send(&mut j, enter(key)).await;
    assert_eq!(j.state(key), Some(State::WorldReady));
    assert_eq!(
        send(&mut j, request(23, key)).await,
        vec![w::notice(21, key).unwrap()]
    );
    assert_eq!(j.state(key), Some(State::WorldReady));
    // Retained 5.1: invalid state, no change, no notify
    let Fixture { mut j, key, .. } = rig(false, true).await;
    let lobby_token = j.retain(key, Duration::from_secs(10)).unwrap();
    assert_eq!(j.state(key), Some(State::Retained));
    assert!(send(&mut j, request(23, key)).await.is_empty());
    assert_eq!(j.state(key), Some(State::Retained));
    // RetainedInit 5.2: session gone, no notify
    send(
        &mut j,
        packet(
            "hqqqi",
            &[
                Value::I16(12),
                Value::U64(200),
                Value::U64(key),
                Value::U64(lobby_token),
                Value::U32(IPV4),
            ],
        ),
    )
    .await;
    assert_eq!(j.state(key), Some(State::RetainedInit));
    assert!(send(&mut j, request(23, key)).await.is_empty());
    assert_eq!(j.state(key), None);
}

#[tokio::test(start_paused = true)]
async fn logout_world_state_is_kept() {
    let Fixture { mut j, key, .. } = rig(false, true).await;
    let p = send(&mut j, enter(key)).await;
    let Value::U64(token) = w::fields("hqqh", &p[0]).unwrap()[2] else {
        panic!()
    };
    assert_eq!(j.consume_world(key, token, IPV4, 1), Ok(77));
    assert_eq!(j.state(key), Some(State::World));
    // World 4.0: "invalid state" in LogoutJob (13-jobs-a.md §3.3). No change, no notify.
    assert!(send(&mut j, request(23, key)).await.is_empty());
    assert_eq!(j.state(key), Some(State::World));
}
