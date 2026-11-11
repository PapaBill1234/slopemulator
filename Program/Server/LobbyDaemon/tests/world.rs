use bloodlib::packet::Value;
use lobbydaemon::{
    gamed::{ChargeMode, Eligibility, StubGameDaemon, WaitInfo, WorldFixture},
    jobs::{Jobs, Settings, State},
    np::StubNp,
    protocol as w,
    store::{Character, MemoryStore, Shared},
};
use std::{sync::Arc, time::Duration};

type Rig = Jobs<Shared<MemoryStore>, StubNp>;
fn packet(f: &str, v: &[Value]) -> Vec<u8> {
    w::packet(f, v).unwrap()
}
async fn send(j: &mut Rig, p: Vec<u8>) -> Vec<Vec<u8>> {
    j.handle(1, &p)
        .await
        .unwrap()
        .into_iter()
        .flat_map(|o| o.packets)
        .collect()
}
fn request(key: u64) -> Vec<u8> {
    packet(
        "hqo",
        &[Value::I16(24), Value::U64(key), Value::Id([9; 16])],
    )
}
fn failure(p: &[Vec<u8>], key: u64, r: i16) {
    assert_eq!(p, &[w::failure(26, key, r).unwrap()]);
}
fn token(p: &[Vec<u8>]) -> u64 {
    let v = w::fields("hqqh", &p[0]).unwrap();
    assert_eq!(v[0], Value::I16(25));
    let Value::U64(t) = v[2] else { panic!() };
    t
}
async fn rig(
    eligibility: Eligibility,
    queued: bool,
    game_delay: Duration,
) -> (Rig, Arc<StubGameDaemon>, u64) {
    let (mut np, t, p, id) = StubNp::fixture(61);
    np.world_results
        .insert((id.account, 77, [9; 16]), Ok(ChargeMode::Paid));
    let c = Character {
        pcid: 77,
        slot: [9; 16],
        world: 1,
        name: vec![65],
        showcase: vec![],
        deletion_time: 0,
        state: 0,
        jumping_state: 0,
        last_play_end: 0,
    };
    let mut store = MemoryStore::default();
    store.characters.insert(id.account, vec![c]);
    store.restoration_allowed.insert(77);
    let mut g = StubGameDaemon::default();
    g.delay = game_delay;
    g.worlds.insert(
        (1, 77),
        WorldFixture {
            slot: [9; 16],
            eligibility: Ok(eligibility),
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
    let init = send(
        &mut j,
        packet(
            "hqooi",
            &[
                Value::I16(6),
                Value::U64(100),
                Value::Id(t),
                Value::Id(p),
                Value::U32(123),
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
    (j, g, key)
}
#[tokio::test(start_paused = true)]
async fn success_single_use_and_identity_checks() {
    let (mut j, g, key) = rig(Eligibility::Ready, false, Duration::ZERO).await;
    let t = token(&send(&mut j, request(key)).await);
    assert_eq!(g.active(), 1);
    assert_eq!(j.consume_world(key, t, 124, 1), Err(15));
    assert_eq!(g.active(), 1);
    assert_eq!(j.consume_world(key, t, 123, 1), Ok(77));
    assert_eq!(g.active(), 0);
    assert_eq!(g.released(), 1);
    assert_eq!(j.consume_world(key, t, 123, 1), Err(16));
    assert_eq!(j.state(key), Some(State::World));
}
#[tokio::test(start_paused = true)]
async fn rejection_does_not_reserve_and_can_retry() {
    let (mut j, g, key) = rig(Eligibility::Ready, false, Duration::ZERO).await;
    let fixture = j.np.world_results.values_mut().next().unwrap();
    *fixture = Err(114);
    failure(&send(&mut j, request(key)).await, key, 114);
    assert_eq!(g.active(), 0);
    assert_eq!(j.state(key), Some(State::Lobby));
    *j.np.world_results.values_mut().next().unwrap() = Ok(ChargeMode::Free);
    token(&send(&mut j, request(key)).await);
    failure(&send(&mut j, request(key)).await, key, 5);
    j.disconnect(1).await;
    assert_eq!(g.active(), 0);
}
#[tokio::test(start_paused = true)]
async fn restoration_commits_store_once_even_after_platform_failure() {
    let (mut j, g, key) = rig(Eligibility::Restore, false, Duration::ZERO).await;
    *j.np.world_results.values_mut().next().unwrap() = Err(111);
    failure(&send(&mut j, request(key)).await, key, 111);
    assert_eq!(j.store.lock().restored, vec![77]);
    *j.np.world_results.values_mut().next().unwrap() = Ok(ChargeMode::Trial);
    token(&send(&mut j, request(key)).await);
    assert_eq!(j.store.lock().restored, vec![77]);
    assert_eq!(g.restored(), 1);
    j.kick(key, 7).await.unwrap();
    assert_eq!(g.active(), 0);
}
#[tokio::test(start_paused = true)]
async fn restoration_store_failure_blocks_entry() {
    let (mut j, g, key) = rig(Eligibility::Restore, false, Duration::ZERO).await;
    j.store.lock().restoration_allowed.clear();
    failure(&send(&mut j, request(key)).await, key, 2);
    assert_eq!(g.active(), 0);
    assert_eq!(j.state(key), Some(State::Lobby));
}
#[tokio::test(start_paused = true)]
async fn timeouts_and_cancellation_do_not_leak_admission() {
    for restore in [false, true] {
        let (mut j, g, key) = rig(
            if restore {
                Eligibility::Restore
            } else {
                Eligibility::Ready
            },
            false,
            Duration::from_secs(20),
        )
        .await;
        failure(&send(&mut j, request(key)).await, key, 104);
        assert_eq!(g.active(), 0);
        assert_eq!(j.state(key), Some(State::Lobby));
        assert!(
            tokio::time::timeout(Duration::from_secs(1), j.handle(1, &request(key)))
                .await
                .is_err()
        );
        assert_eq!(g.active(), 0);
        assert_eq!(j.state(key), Some(State::Lobby));
    }
    let (mut j, g, key) = rig(Eligibility::Ready, false, Duration::ZERO).await;
    j.np.world_delay = Duration::from_secs(20);
    failure(&send(&mut j, request(key)).await, key, 7);
    assert_eq!(g.active(), 0);
}
#[tokio::test(start_paused = true)]
async fn queued_reply_order_promotion_and_cleanup() {
    let (mut j, g, key) = rig(Eligibility::Ready, true, Duration::ZERO).await;
    let p = send(&mut j, request(key)).await;
    assert_eq!(p[0], w::failure(26, key, 107).unwrap());
    assert_eq!(
        w::fields("hqiiCch;", &p[1]).unwrap(),
        vec![
            Value::I16(27),
            Value::U64(key),
            Value::U32(3),
            Value::U32(8),
            Value::Repeat(vec![vec![Value::U8(1), Value::I16(2)]])
        ]
    );
    assert_eq!(j.state(key), Some(State::WorldQueued));
    assert_eq!(g.active(), 1);
    let out = j.promote_world(key).unwrap();
    let t = token(&out[0].packets);
    assert!(j.promote_world(key).is_err());
    assert_eq!(j.consume_world(key, t, 123, 1), Ok(77));
    assert_eq!(g.active(), 0);
}
#[tokio::test(start_paused = true)]
async fn exact_expiry_and_wrong_world_consume_cleanup() {
    for queued in [false, true] {
        let (mut j, g, key) = rig(Eligibility::Ready, queued, Duration::ZERO).await;
        send(&mut j, request(key)).await;
        tokio::time::advance(Duration::from_secs(10)).await;
        assert!(j.promote_world(key).is_err());
        assert_eq!(j.expire().await.unwrap().len(), 1);
        assert_eq!(g.active(), 0);
        assert!(j.expire().await.unwrap().is_empty());
    }
    let (mut j, g, key) = rig(Eligibility::Ready, false, Duration::ZERO).await;
    let t = token(&send(&mut j, request(key)).await);
    assert_eq!(j.consume_world(key, t, 123, 2), Err(17));
    assert_eq!(g.active(), 0);
    assert_eq!(j.consume_world(key, t, 123, 1), Err(16));
}
#[tokio::test(start_paused = true)]
async fn unknown_platform_and_wrong_link_reject() {
    let (mut j, g, key) = rig(Eligibility::Ready, false, Duration::ZERO).await;
    j.np.world_results.clear();
    failure(&send(&mut j, request(key)).await, key, 2);
    assert_eq!(g.active(), 0);
    send(
        &mut j,
        packet(
            "hhhh",
            &[Value::I16(1), Value::I16(0), Value::I16(58), Value::I16(0)],
        ),
    )
    .await;
    assert!(j.handle(2, &request(key)).await.is_err());
    failure(&send(&mut j, request(999)).await, 999, 4);
}
