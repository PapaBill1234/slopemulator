//! Charge, grade and account-inventory item requests with the feature off (card lp2-2).
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
const SLOT: [u8; 16] = [7; 16];
fn blob() -> Value {
    Value::Bytes(vec![1, 2, 3])
}
fn name() -> Value {
    Value::String("Ab".encode_utf16().collect())
}
/// A valid request for each item service command: (format, fields after the key, Fail command).
/// `label` and `item` replace the label id and the item instance id.
fn item_request(
    cmd: i16,
    key: u64,
    slot: [u8; 16],
    label: u64,
    item: u64,
) -> (Vec<u8>, &'static str, i16) {
    let (c, k) = (Value::I16(cmd), Value::U64(key));
    let (s, l, i, n) = (
        Value::Id(slot),
        Value::U64(label),
        Value::U64(item),
        Value::U32(0),
    );
    match cmd {
        82 => (packet("hqobqqi", &[c, k, s, blob(), l, i, n]), "hqoqqE", 84),
        85 => (
            packet("hqocbqqi", &[c, k, s, Value::U8(1), blob(), l, i, n]),
            "hqoqqE",
            87,
        ),
        88 => (
            packet(
                "hqoccbqqi",
                &[c, k, s, Value::U8(1), Value::U8(2), blob(), l, i, n],
            ),
            "hqoqqE",
            90,
        ),
        91 => (packet("hqosqqi", &[c, k, s, name(), l, i, n]), "hqoqqE", 93),
        94 => (packet("hqqqi", &[c, k, l, i, n]), "hqqqE", 96),
        97 => (packet("hqsqqi", &[c, k, name(), l, i, n]), "hqqqE", 99),
        _ => (packet("hqqqi", &[c, k, l, i, n]), "hqqqE", 102),
    }
}
const SERVICES: [i16; 7] = [82, 85, 88, 91, 94, 97, 100];
#[tokio::test]
async fn charge_info_off() {
    let (mut j, key) = rig().await;
    let out = send(&mut j, 1, packet("hq", &[Value::I16(60), Value::U64(key)])).await;
    assert_eq!(out.len(), 1);
    let f = w::fields("hqE", &out[0]).unwrap();
    assert_eq!(f, [Value::I16(62), Value::U64(key), Value::I16(7)]);
}
#[tokio::test]
async fn charge_info_no_session() {
    let (mut j, key) = rig().await;
    let out = send(
        &mut j,
        1,
        packet("hq", &[Value::I16(60), Value::U64(key + 99)]),
    )
    .await;
    assert_eq!(out.len(), 1);
    let f = w::fields("hqE", &out[0]).unwrap();
    assert_eq!(f[0], Value::I16(62));
    assert_eq!(f[2], Value::I16(4));
}
#[tokio::test]
async fn charge_info_on_is_not_ported() {
    let (np, t, p, _) = StubNp::fixture(85);
    let settings = Settings {
        charge: true,
        ..Settings::default()
    };
    let mut j = Jobs::new(Shared::new(MemoryStore::default()), np, settings).unwrap();
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
    let out = send(&mut j, 1, packet("hq", &[Value::I16(60), Value::U64(key)])).await;
    let f = w::fields("hqE", &out[0]).unwrap();
    assert_eq!(f[2], Value::I16(2));
}
#[tokio::test]
async fn grade_off_before_session() {
    let (mut j, key) = rig().await;
    let out = send(
        &mut j,
        1,
        packet(
            "hqQi",
            &[
                Value::I16(106),
                Value::U64(key + 99),
                Value::I8(5),
                Value::U32(77),
            ],
        ),
    )
    .await;
    assert_eq!(out.len(), 1);
    let f = w::fields("hqQiE", &out[0]).unwrap();
    assert_eq!(f[0], Value::I16(108));
    assert_eq!(f[1], Value::U64(key + 99));
    assert_eq!(f[2], Value::I8(5));
    assert_eq!(f[3], Value::U32(77));
    assert_eq!(f[4], Value::I16(7));
}
#[tokio::test]
async fn grade_off_with_session_and_malformed() {
    let (mut j, key) = rig().await;
    let req = |key| {
        packet(
            "hqQi",
            &[
                Value::I16(106),
                Value::U64(key),
                Value::I8(1),
                Value::U32(2),
            ],
        )
    };
    let out = send(&mut j, 1, req(key)).await;
    assert_eq!(
        w::fields("hqQiE", &out[0]).unwrap().last(),
        Some(&Value::I16(7))
    );
    let short = packet("hq", &[Value::I16(106), Value::U64(key)]);
    assert!(send(&mut j, 1, short).await.is_empty());
}
#[tokio::test]
async fn item_service_off() {
    let (mut j, key) = rig().await;
    for cmd in SERVICES {
        let (req, format, fail) = item_request(cmd, key, SLOT, 11, 22);
        let out = send(&mut j, 1, req).await;
        assert_eq!(out.len(), 1, "command {cmd}");
        let f = w::fields(format, &out[0]).unwrap();
        assert_eq!(f[0], Value::I16(fail), "command {cmd}");
        assert_eq!(f[1], Value::U64(key));
        if format == "hqoqqE" {
            assert_eq!(f[2], Value::Id(SLOT));
            assert_eq!(f[3], Value::U64(11));
            assert_eq!(f[4], Value::U64(22));
            assert_eq!(f[5], Value::I16(7));
        } else {
            assert_eq!(f[2], Value::U64(11));
            assert_eq!(f[3], Value::U64(22));
            assert_eq!(f[4], Value::I16(7));
        }
    }
}
#[tokio::test]
async fn item_service_no_session() {
    let (mut j, key) = rig().await;
    send(&mut j, 2, hello()).await;
    for cmd in SERVICES {
        let (req, format, fail) = item_request(cmd, key, SLOT, 11, 22);
        let out = send(&mut j, 2, req).await;
        let f = w::fields(format, &out[0]).unwrap();
        assert_eq!(f[0], Value::I16(fail));
        assert_eq!(f.last(), Some(&Value::I16(4)), "command {cmd}");
    }
}
#[tokio::test]
async fn item_service_zero_input() {
    let (mut j, key) = rig().await;
    // 82 with label id 0, and the other zero inputs of the character services: no reply.
    for cmd in [82, 85, 88, 91] {
        for (slot, label, item) in [(SLOT, 0, 22), (SLOT, 11, 0), ([0; 16], 11, 22)] {
            let (req, _, _) = item_request(cmd, key, slot, label, item);
            assert!(send(&mut j, 1, req).await.is_empty(), "command {cmd}");
        }
    }
    // Account services: bad parameter gives code 1 ❓ (`19-jobs-g.md`).
    for cmd in [94, 97, 100] {
        let (req, format, _) = item_request(cmd, key, SLOT, 0, 22);
        let out = send(&mut j, 1, req).await;
        assert_eq!(
            w::fields(format, &out[0]).unwrap().last(),
            Some(&Value::I16(1)),
            "command {cmd}"
        );
    }
}
#[tokio::test]
async fn item_service_malformed() {
    let (mut j, key) = rig().await;
    let short = packet("hq", &[Value::I16(85), Value::U64(key)]);
    assert!(send(&mut j, 1, short).await.is_empty());
    // The link stays: the next request is answered.
    let out = send(&mut j, 1, packet("hq", &[Value::I16(60), Value::U64(key)])).await;
    assert_eq!(out.len(), 1);
}
#[tokio::test]
async fn rename_fail_command() {
    let (mut j, key) = rig().await;
    let (req, format, _) = item_request(91, key, SLOT, 11, 22);
    let out = send(&mut j, 1, req).await;
    let f = w::fields(format, &out[0]).unwrap();
    assert_eq!(f[0], Value::I16(93));
    assert_ne!(f[0], Value::I16(84));
}
