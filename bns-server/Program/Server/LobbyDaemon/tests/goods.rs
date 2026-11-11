//! Goods requests with account inventory off: build-307 Fail replies.
use bloodlib::packet::Value;
use lobbydaemon::{
    jobs::{Jobs, Settings},
    np::StubNp,
    protocol as w,
    store::{MemoryStore, Shared},
};
type Rig = Jobs<Shared<MemoryStore>, StubNp>;
fn packet(f: &str, v: &[Value]) -> Vec<u8> {
    w::packet(f, v).unwrap()
}
async fn send(j: &mut Rig, link: u64, p: Vec<u8>) -> Vec<Vec<u8>> {
    j.handle(link as _, &p)
        .await
        .unwrap()
        .into_iter()
        .flat_map(|o| o.packets)
        .collect()
}
fn hello() -> Vec<u8> {
    packet(
        "hhhh",
        &[Value::I16(1), Value::I16(0), Value::I16(58), Value::I16(0)],
    )
}
async fn rig() -> (Rig, u64) {
    let (np, t, p, _) = StubNp::fixture(85);
    let mut j = Jobs::new(Shared::new(MemoryStore::default()), np, Settings::default()).unwrap();
    send(&mut j, 1, hello()).await;
    let init = send(
        &mut j,
        1,
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
        1,
        packet(
            "hqoo",
            &[Value::I16(9), Value::U64(key), Value::Id(t), Value::Id(p)],
        ),
    )
    .await;
    (j, key)
}
fn requests(key: u64) -> Vec<(Vec<u8>, &'static str, i16)> {
    let k = Value::U64(key);
    vec![
        (packet("hq", &[Value::I16(64), k.clone()]), "hqE", 66),
        (
            packet("hqc", &[Value::I16(67), k.clone(), Value::U8(0)]),
            "hqE",
            69,
        ),
        (
            packet("hqq", &[Value::I16(70), k.clone(), Value::U64(9)]),
            "hqE",
            72,
        ),
        (
            packet(
                "hqQq",
                &[Value::I16(73), k.clone(), Value::I8(3), Value::U64(0)],
            ),
            "hqQE",
            75,
        ),
        (
            packet("hqq", &[Value::I16(76), k, Value::U64(9)]),
            "hqqE",
            78,
        ),
    ]
}
#[tokio::test]
async fn goods_requests_fail_with_account_inventory_unavailable() {
    let (mut j, key) = rig().await;
    for (req, format, fail) in requests(key) {
        let out = send(&mut j, 1, req).await;
        assert_eq!(out.len(), 1);
        let f = w::fields(format, &out[0]).unwrap();
        assert_eq!(f[0], Value::I16(fail));
        assert_eq!(f[1], Value::U64(key));
        assert_eq!(f.last(), Some(&Value::I16(7)));
        match fail {
            75 => assert_eq!(f[2], Value::I8(3)),
            78 => assert_eq!(f[2], Value::U64(9)),
            _ => {}
        }
    }
}
#[tokio::test]
async fn goods_requests_without_session_fail_with_4_and_malformed_are_dropped() {
    let (mut j, key) = rig().await;
    send(&mut j, 2, hello()).await;
    for (req, format, _) in requests(key) {
        let out = send(&mut j, 2, req).await;
        assert_eq!(
            w::fields(format, &out[0]).unwrap().last(),
            Some(&Value::I16(4))
        );
    }
    assert!(send(&mut j, 1, packet("h", &[Value::I16(64)]))
        .await
        .is_empty());
}
