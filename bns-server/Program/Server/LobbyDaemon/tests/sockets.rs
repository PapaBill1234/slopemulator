use bloodlib::packet::Value;
use lobbydaemon::{
    jobs::Settings,
    np::StubNp,
    protocol as w,
    store::{MemoryStore, Shared},
    Server,
};
use std::{net::Ipv4Addr, sync::Arc, time::Duration};
use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
    sync::watch,
    time::timeout,
};
fn packet(f: &str, v: &[Value]) -> Vec<u8> {
    w::packet(f, v).unwrap()
}
async fn read(s: &mut TcpStream) -> Vec<u8> {
    timeout(Duration::from_secs(2), w::read(s))
        .await
        .unwrap()
        .unwrap()
        .unwrap()
}
async fn hello(s: &mut TcpStream) {
    s.write_all(&packet(
        "hhhh",
        &[Value::I16(1), Value::I16(1), Value::I16(58), Value::I16(0)],
    ))
    .await
    .unwrap();
    assert_eq!(w::command(&read(s).await).unwrap(), 2);
}
#[tokio::test]
async fn partial_read_survives_kick_output_and_disconnect_releases_sessions() {
    let (np, t, p, _) = StubNp::fixture(7);
    let server = Arc::new(
        Server::new(Shared::new(MemoryStore::default()), np, Settings::default()).unwrap(),
    );
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let address = listener.local_addr().unwrap();
    let (stop, shutdown) = watch::channel(false);
    let task = tokio::spawn(server.clone().serve(listener, shutdown));
    let mut client = TcpStream::connect(address).await.unwrap();
    hello(&mut client).await;
    client
        .write_all(&packet(
            "hqooi",
            &[
                Value::I16(6),
                Value::U64(1),
                Value::Id(t),
                Value::Id(p),
                Value::U32(0),
            ],
        ))
        .await
        .unwrap();
    let v = w::fields("hqqoo", &read(&mut client).await).unwrap();
    let Value::U64(key) = v[2] else { panic!("key") };
    client
        .write_all(&packet(
            "hqoo",
            &[Value::I16(9), Value::U64(key), Value::Id(t), Value::Id(p)],
        ))
        .await
        .unwrap();
    assert_eq!(w::command(&read(&mut client).await).unwrap(), 10);
    // Send one header byte, then receive an independent D22.
    let list = w::notice(35, key).unwrap();
    client.write_all(&list[..1]).await.unwrap();
    server.kick(key, -1).await.unwrap();
    // Original D22 conversion maps unsupported internal reasons to zero.
    assert_eq!(read(&mut client).await, w::kick(key, 0).unwrap());
    client.write_all(&list[1..]).await.unwrap();
    assert_eq!(
        w::fields("hqE", &read(&mut client).await).unwrap(),
        vec![Value::I16(37), Value::U64(key), Value::I16(4)]
    );
    client
        .write_all(&packet(
            "hqooi",
            &[
                Value::I16(6),
                Value::U64(2),
                Value::Id(t),
                Value::Id(p),
                Value::U32(0),
            ],
        ))
        .await
        .unwrap();
    read(&mut client).await;
    assert_eq!(server.session_count().await, 1);
    drop(client);
    timeout(Duration::from_secs(2), async {
        while server.session_count().await != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    stop.send(true).unwrap();
    timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}
#[tokio::test]
async fn shutdown_cancels_pending_np_without_retained_init_session() {
    let (mut np, t, p, _) = StubNp::fixture(8);
    np.delay = Duration::from_secs(60);
    let server = Arc::new(
        Server::new(Shared::new(MemoryStore::default()), np, Settings::default()).unwrap(),
    );
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let address = listener.local_addr().unwrap();
    let (stop, shutdown) = watch::channel(false);
    let task = tokio::spawn(server.clone().serve(listener, shutdown));
    let mut client = TcpStream::connect(address).await.unwrap();
    hello(&mut client).await;
    client
        .write_all(&packet(
            "hqooi",
            &[
                Value::I16(6),
                Value::U64(1),
                Value::Id(t),
                Value::Id(p),
                Value::U32(0),
            ],
        ))
        .await
        .unwrap();
    let v = w::fields("hqqoo", &read(&mut client).await).unwrap();
    let Value::U64(key) = v[2] else { panic!("key") };
    client
        .write_all(&packet(
            "hqoo",
            &[Value::I16(9), Value::U64(key), Value::Id(t), Value::Id(p)],
        ))
        .await
        .unwrap();
    tokio::task::yield_now().await;
    stop.send(true).unwrap();
    timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(server.session_count().await, 0);
}
