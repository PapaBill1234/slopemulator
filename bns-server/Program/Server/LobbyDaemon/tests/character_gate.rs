//! Character lifecycle through the unchanged encrypted LobbyGate.
#[path = "end_to_end.rs"]
mod helpers;
use bloodlib::{
    packet::{compose, Value},
    session::SessionPacketCrypt,
};
use helpers::{config, connect_client, keys, receive, send};
use lobbydaemon::{
    character::CharacterPolicy,
    jobs::Settings,
    np::StubNp,
    protocol,
    store::{MemoryStore, Shared},
    Server as Lobby,
};
use lobbygate::{
    daemon::TcpLobbyDaemon,
    server::Server as Gate,
    wire::{inner_packet, read_frame},
};
use std::{net::Ipv4Addr, sync::Arc, time::Duration};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::watch,
    time::timeout,
};
async fn raw(client: &mut TcpStream, crypt: &mut SessionPacketCrypt, format: &str) -> Vec<Value> {
    let frame = timeout(Duration::from_secs(3), read_frame(client))
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let (plain, _) = crypt.decrypt(&frame, 4000).unwrap();
    protocol::fields(format, inner_packet(&plain).unwrap()).unwrap()
}
async fn rows(client: &mut TcpStream, crypt: &mut SessionPacketCrypt) -> Vec<Vec<Value>> {
    send(client, crypt, compose("h", &[Value::I16(27)]).unwrap()).await;
    let list = receive(client, crypt, 28, "qhCqohsbqQQi;").await;
    let Value::Repeat(rows) = &list[2] else {
        panic!()
    };
    rows.clone()
}
#[tokio::test]
async fn encrypted_character_create_delete_wait_cancel() {
    timeout(Duration::from_secs(15), async {
        let (np, token, portal, _) = StubNp::fixture(90);
        let slot = [94; 16];
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let cfg = config(listener.local_addr().unwrap().port());
        let lobby = Arc::new(
            Lobby::new(
                Shared::new(MemoryStore::default()),
                np,
                Settings {
                    character_policy: Some(CharacterPolicy {
                        slots: [slot].into(),
                        worlds: [1].into(),
                        min_name_units: 2,
                        max_name_units: 32,
                        deletion_wait_seconds: 60,
                    }),
                    ..Default::default()
                },
            )
            .unwrap(),
        );
        let (stop, shutdown) = watch::channel(false);
        let lobby_task = tokio::spawn(lobby.clone().serve(listener, shutdown.clone()));
        let daemon = TcpLobbyDaemon::start(&cfg, shutdown.clone()).await.unwrap();
        let (public, private, _) = keys(1024);
        let gate = Arc::new(Gate::new(&cfg, private, daemon).unwrap());
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let addr = listener.local_addr().unwrap();
        let gate_task = tokio::spawn(gate.serve(listener, shutdown));
        let (mut client, mut crypt) = connect_client(addr, &public).await;
        send(
            &mut client,
            &mut crypt,
            compose("hoo", &[Value::I16(3), Value::Id(token), Value::Id(portal)]).unwrap(),
        )
        .await;
        receive(&mut client, &mut crypt, 4, "qQoihCiiiii;CiiQiqq;QqiqiqK").await;
        assert!(rows(&mut client, &mut crypt).await.is_empty());
        // C41 is blocked by the gate. D41's no-op is tested directly in characters.rs.
        let name: Vec<u16> = "GeneratedHero".encode_utf16().collect();
        send(
            &mut client,
            &mut crypt,
            compose(
                "hohsbQ",
                &[
                    Value::I16(36),
                    Value::Id(slot),
                    Value::I16(1),
                    Value::String(name.clone()),
                    Value::Bytes(vec![2, 3]),
                    Value::I8(0),
                ],
            )
            .unwrap(),
        )
        .await;
        let created = raw(&mut client, &mut crypt, "hqohsbqQQii").await;
        assert_eq!(created[0], Value::I16(37));
        assert_eq!(created[4], Value::String(name));
        assert_eq!(rows(&mut client, &mut crypt).await[0][6], Value::I8(1));
        send(
            &mut client,
            &mut crypt,
            compose("ho", &[Value::I16(39), Value::Id(slot)]).unwrap(),
        )
        .await;
        let deleted = raw(&mut client, &mut crypt, "hoQi").await;
        assert_eq!(deleted[0], Value::I16(40));
        assert_eq!(deleted[2], Value::I8(2));
        assert_eq!(rows(&mut client, &mut crypt).await[0][6], Value::I8(2));
        send(
            &mut client,
            &mut crypt,
            compose("ho", &[Value::I16(43), Value::Id(slot)]).unwrap(),
        )
        .await;
        assert_eq!(
            raw(&mut client, &mut crypt, "ho").await,
            vec![Value::I16(44), Value::Id(slot)]
        );
        assert_eq!(rows(&mut client, &mut crypt).await[0][6], Value::I8(1));
        // A normal row is not a D54 recovery candidate. Check the unchanged failure relay.
        send(
            &mut client,
            &mut crypt,
            compose("ho", &[Value::I16(46), Value::Id(slot)]).unwrap(),
        )
        .await;
        assert_eq!(
            raw(&mut client, &mut crypt, "hoQE").await,
            vec![Value::I16(48), Value::Id(slot), Value::I8(1), Value::I16(2)]
        );
        drop(client);
        stop.send(true).unwrap();
        gate_task.await.unwrap().unwrap();
        lobby_task.await.unwrap().unwrap();
    })
    .await
    .unwrap();
}
