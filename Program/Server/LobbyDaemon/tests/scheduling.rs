use bloodlib::packet::Value;
use lobbydaemon::{
    jobs::Settings,
    np::{Identity, Np, NpFuture, StubNp},
    protocol as w,
    store::{AccountId, MemoryStore, Shared},
    Server,
};
use std::{net::Ipv4Addr, sync::Arc, time::Duration};
use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
    sync::{watch, Notify},
    time::timeout,
};
fn hello() -> Vec<u8> {
    w::packet(
        "hhhh",
        &[Value::I16(1), Value::I16(1), Value::I16(58), Value::I16(0)],
    )
    .unwrap()
}
fn init(gate: u64, t: [u8; 16], p: [u8; 16]) -> Vec<u8> {
    w::packet(
        "hqooi",
        &[
            Value::I16(6),
            Value::U64(gate),
            Value::Id(t),
            Value::Id(p),
            Value::U32(0),
        ],
    )
    .unwrap()
}
fn start(key: u64, t: [u8; 16], p: [u8; 16]) -> Vec<u8> {
    w::packet(
        "hqoo",
        &[Value::I16(9), Value::U64(key), Value::Id(t), Value::Id(p)],
    )
    .unwrap()
}
async fn read(s: &mut TcpStream) -> Vec<u8> {
    timeout(Duration::from_secs(2), w::read(s))
        .await
        .unwrap()
        .unwrap()
        .unwrap()
}
async fn connect(address: std::net::SocketAddr) -> TcpStream {
    let mut s = TcpStream::connect(address).await.unwrap();
    s.write_all(&hello()).await.unwrap();
    assert_eq!(w::command(&read(&mut s).await).unwrap(), 2);
    s
}
async fn initialize(s: &mut TcpStream, gate: u64, t: [u8; 16], p: [u8; 16]) -> u64 {
    s.write_all(&init(gate, t, p)).await.unwrap();
    let Value::U64(key) = w::fields("hqqoo", &read(s).await).unwrap()[2] else {
        panic!("key")
    };
    key
}
struct BlockNp {
    inner: StubNp,
    token: [u8; 16],
    entered: Notify,
    release: Notify,
}
impl Np for BlockNp {
    fn consume(&self, token: [u8; 16], portal: [u8; 16]) -> NpFuture<'_, Identity> {
        Box::pin(async move {
            if token == self.token {
                self.entered.notify_one();
                self.release.notified().await;
            }
            self.inner.consume(token, portal).await
        })
    }
    fn login_presence(&self, id: AccountId, owner: u64) -> NpFuture<'_, ()> {
        self.inner.login_presence(id, owner)
    }
    fn logout_presence(&self, id: AccountId, owner: u64) {
        self.inner.logout_presence(id, owner);
    }
}
#[tokio::test]
async fn r9_lanes_keep_order_without_blocking_other_links_and_share_capacity() {
    let (mut np, t, p, _) = StubNp::fixture(20);
    let (_, t2, p2, id2) = StubNp::fixture(30);
    np.identities.insert((t2, p2), Ok(id2));
    let np = Arc::new(BlockNp {
        inner: np,
        token: t,
        entered: Notify::new(),
        release: Notify::new(),
    });
    let server = Arc::new(
        Server::new(
            Shared::new(MemoryStore::default()),
            np.clone(),
            Settings {
                max_sessions: 2,
                ..Settings::default()
            },
        )
        .unwrap(),
    );
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let address = listener.local_addr().unwrap();
    let (stop, shutdown) = watch::channel(false);
    let task = tokio::spawn(server.clone().serve(listener, shutdown));
    let mut a = connect(address).await;
    let key = initialize(&mut a, 1, t, p).await;
    a.write_all(&[start(key, t, p), w::notice(35, key).unwrap()].concat())
        .await
        .unwrap();
    timeout(Duration::from_secs(2), np.entered.notified())
        .await
        .unwrap();
    // This complete login must finish while the first lane is blocked in NP.
    let mut b = connect(address).await;
    let key2 = initialize(&mut b, 1, t2, p2).await;
    assert_ne!(key, key2);
    b.write_all(&start(key2, t2, p2)).await.unwrap();
    assert_eq!(w::command(&read(&mut b).await).unwrap(), 10);
    b.write_all(&init(2, t2, p2)).await.unwrap();
    assert_eq!(
        w::fields("hqE", &read(&mut b).await).unwrap(),
        vec![Value::I16(8), Value::U64(2), Value::I16(6)]
    );
    // A control for B also must not wait on A.
    timeout(Duration::from_secs(2), server.kick(key2, 7))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(read(&mut b).await, w::kick(key2, 7).unwrap());
    np.release.notify_one();
    assert_eq!(w::command(&read(&mut a).await).unwrap(), 10);
    assert_eq!(w::command(&read(&mut a).await).unwrap(), 36);
    stop.send(true).unwrap();
    timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(server.session_count().await, 0);
}
#[tokio::test]
async fn r10_scheduled_expiry_preserves_partial_frame_and_releases_capacity() {
    let (np, t, p, _) = StubNp::fixture(40);
    let server = Arc::new(
        Server::new(
            Shared::new(MemoryStore::default()),
            np,
            Settings {
                init_timeout: Some(Duration::from_millis(30)),
                max_sessions: 1,
                ..Settings::default()
            },
        )
        .unwrap(),
    );
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let address = listener.local_addr().unwrap();
    let (stop, shutdown) = watch::channel(false);
    let task = tokio::spawn(server.clone().serve(listener, shutdown));
    let mut a = connect(address).await;
    let key = initialize(&mut a, 1, t, p).await;
    let request = w::notice(35, key).unwrap();
    a.write_all(&request[..1]).await.unwrap();
    assert_eq!(read(&mut a).await, w::notice(20, 1).unwrap());
    assert_eq!(server.session_count().await, 0);
    a.write_all(&request[1..]).await.unwrap();
    assert_eq!(
        w::fields("hqE", &read(&mut a).await).unwrap(),
        vec![Value::I16(37), Value::U64(key), Value::I16(4)]
    );
    let next = initialize(&mut a, 2, t, p).await;
    assert_ne!(key, next);
    server.close_anonymous(next).await.unwrap();
    assert_eq!(read(&mut a).await, w::notice(20, 2).unwrap());
    stop.send(true).unwrap();
    task.await.unwrap().unwrap();
    assert_eq!(server.session_count().await, 0);
}
