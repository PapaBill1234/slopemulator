//! Real in-process LobbyGate, real lobbyd listener, generated encrypted client.
use bloodlib::{
    crypto::RsaKey,
    frames::{decode_client, encode_client, ClientDecrypt, ClientEncrypt},
    packet::{compose, decode_fields, decompose, Value},
    session::SessionPacketCrypt,
};
use lobbydaemon::{
    gamed::{ChargeMode, Eligibility, StubGameDaemon, WorldFixture},
    jobs::Settings,
    np::StubNp,
    protocol,
    store::{Account, Character, MemoryStore, Shared},
    Server as Lobby,
};
use lobbygate::{
    config::GateConfig,
    daemon::TcpLobbyDaemon,
    server::Server as Gate,
    wire::{inner_packet, read_frame, write_frame},
};
use num_bigint::BigUint;
use packet::fragments::FragmentReader;
use rand::{rngs::StdRng, SeedableRng};
use rsa::{
    traits::{PrivateKeyParts, PublicKeyParts},
    RsaPrivateKey,
};
use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};
use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
    sync::watch,
    time::timeout,
};

pub(crate) fn config(port: u16) -> GateConfig {
    GateConfig {
        root_path: ".".into(),
        log_path: ".".into(),
        config_path: ".".into(),
        cert: String::new(),
        worker_thread: 1,
        max_client: 8,
        dump_type: 0,
        timer_interval: 5,
        timer_max_time: 100,
        init_client_session_count: 0,
        client_session_hash_count: 8,
        predefined_world_id: 1,
        client_close_timeout: 20,
        internal_port: 0,
        client_port: 0,
        lobby_daemon_addr: Ipv4Addr::LOCALHOST,
        lobby_daemon_port: port,
        max_idle_time_enable: false,
        disconnect_after_idle_time: false,
        max_idle_time: 0,
        use_channel_delayed_release: false,
        alert_send_address: String::new(),
        cooldown_complete_delay: 0,
        allow_client_ip_address_in_logs: false,
    }
}
fn append_field(out: &mut Vec<u8>, data: &[u8]) {
    out.extend((data.len() as u32).to_le_bytes());
    out.extend(data);
}
pub(crate) fn keys(bits: usize) -> (RsaKey, RsaKey, Vec<u8>) {
    let key = RsaPrivateKey::new(&mut StdRng::seed_from_u64(307 + bits as u64), bits).unwrap();
    let mut public = vec![];
    append_field(&mut public, &key.n().to_bytes_le());
    let mut private = public.clone();
    let mut d = key.d().to_bytes_le();
    d.resize(bits / 8, 0);
    append_field(&mut private, &d);
    (
        RsaKey::from_public_blob(&public).unwrap(),
        RsaKey::from_private_blob(&private).unwrap(),
        public,
    )
}
pub(crate) async fn connect_client(
    address: SocketAddr,
    server_key: &RsaKey,
) -> (TcpStream, SessionPacketCrypt) {
    let (_, private, mut blob) = keys(512);
    let p = (BigUint::from(1u8) << 127usize) - BigUint::from(1u8);
    let g = BigUint::from(3u8);
    let exponent = BigUint::from(123456789u64);
    append_field(&mut blob, &g.to_bytes_le());
    append_field(&mut blob, &p.to_bytes_le());
    append_field(&mut blob, &g.modpow(&exponent, &p).to_bytes_le());
    let mut stream = TcpStream::connect(address).await.unwrap();
    let hello = compose("hhb", &[Value::I16(1), Value::I16(58), Value::Bytes(blob)]).unwrap();
    let frame = encode_client(
        &hello,
        ClientEncrypt::Handshake {
            key: server_key,
            code: 4,
        },
        &mut rand::rngs::OsRng,
    )
    .unwrap();
    write_frame(&mut stream, &frame).await.unwrap();
    let frame = timeout(Duration::from_secs(3), read_frame(&mut stream))
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let decoded = decode_client(
        &frame,
        ClientDecrypt::Handshake {
            key: &private,
            expected_code: 5,
        },
    )
    .unwrap()
    .unwrap();
    let values = decompose("hb", &decoded.packet, Default::default()).unwrap();
    let [Value::I16(2), Value::Bytes(b)] = values.as_slice() else {
        panic!("Hello reply")
    };
    let shared = BigUint::from_bytes_le(b)
        .modpow(&exponent, &p)
        .to_bytes_le();
    let mut key = [0; 16];
    for (i, b) in key.iter_mut().enumerate() {
        *b = shared[i % shared.len()];
    }
    (stream, SessionPacketCrypt::new(key))
}
pub(crate) async fn send(client: &mut TcpStream, crypt: &mut SessionPacketCrypt, packet: Vec<u8>) {
    write_frame(client, &crypt.encrypt(&packet, 4002, false).unwrap())
        .await
        .unwrap();
}
pub(crate) async fn receive(
    client: &mut TcpStream,
    crypt: &mut SessionPacketCrypt,
    cmd: i16,
    format: &str,
) -> Vec<Value> {
    let mut fragments = FragmentReader::default();
    loop {
        let frame = timeout(Duration::from_secs(3), read_frame(client))
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let (plain, _) = crypt.decrypt(&frame, 4000).unwrap();
        let packet = inner_packet(&plain).unwrap();
        assert_eq!(protocol::command(packet).unwrap(), cmd);
        if cmd == 10 {
            return protocol::fields("hQ", packet).unwrap()[1..].to_vec();
        }
        if cmd == 14 {
            return protocol::fields("hq", packet).unwrap()[1..].to_vec();
        }
        if let Some(joined) = fragments.add(packet).unwrap() {
            return decode_fields(format, &joined.data, Default::default()).unwrap();
        }
    }
}
#[tokio::test]
async fn generated_client_logs_in_through_existing_gate_and_loads_cached_characters() {
    timeout(Duration::from_secs(15), async {
        let (mut np, token, portal, identity) = StubNp::fixture(42);
        let (_, token2, portal2, identity2) = StubNp::fixture(43);
        np.identities
            .insert((token2, portal2), Ok(identity2.clone()));
        let mut store = MemoryStore::default();
        store.accounts.insert(
            identity.account,
            Account {
                id: identity.account,
                name: identity.login_name.clone(),
                right: 1,
                representative_pcid: 77,
                representative_world: 1,
                bot_score: 0,
            },
        );
        let row = Character {
            pcid: 77,
            slot: [45; 16],
            world: 1,
            name: "generated-character".encode_utf16().collect(),
            showcase: vec![7; 512],
            deletion_time: None,
            state: lobbydaemon::character::InternalState::Normal,
            jumping_state: 0,
            last_play_end: 0,
        };
        store
            .characters
            .insert(identity.account, vec![row.clone(); 10]);
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let cfg = config(listener.local_addr().unwrap().port());
        np.world_results
            .insert((identity.account, 77, [45; 16]), Ok(ChargeMode::Paid));
        let mut gamed = StubGameDaemon::default();
        gamed.worlds.insert(
            (1, 77),
            WorldFixture {
                slot: [45; 16],
                eligibility: Ok(Eligibility::Ready),
                restore: Ok(()),
                admission: Ok(None),
            },
        );
        let gamed = Arc::new(gamed);
        let lobby = Arc::new(
            Lobby::new(
                Shared::new(store),
                np,
                Settings {
                    world_token_lifetime: Some(Duration::from_secs(10)),
                    ..Settings::default()
                },
            )
            .unwrap()
            .with_game_daemon(gamed.clone()),
        );
        let (stop, shutdown) = watch::channel(false);
        let lobby_task = tokio::spawn(lobby.clone().serve(listener, shutdown.clone()));
        let daemon = TcpLobbyDaemon::start(&cfg, shutdown.clone()).await.unwrap();
        let (public, private, _) = keys(1024);
        let gate = Arc::new(Gate::new(&cfg, private, daemon).unwrap());
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        let gate_task = tokio::spawn(gate.serve(listener, shutdown));
        let (mut client, mut crypt) = connect_client(address, &public).await;
        send(
            &mut client,
            &mut crypt,
            compose("hoo", &[Value::I16(3), Value::Id(token), Value::Id(portal)]).unwrap(),
        )
        .await;
        let login = receive(&mut client, &mut crypt, 4, "qQoihCiiiii;CiiQiqq;QqiqiqK").await;
        let Value::U64(key) = login[0] else {
            panic!("client key")
        };
        assert_ne!(key, 0);
        assert_eq!(login[2], Value::Id(identity.account));
        assert_eq!(lobby.session_count().await, 1);
        // C27 -> D35 -> D36 -> C28. Gate removes cliKey from the reply.
        send(
            &mut client,
            &mut crypt,
            compose("h", &[Value::I16(27)]).unwrap(),
        )
        .await;
        let list = receive(&mut client, &mut crypt, 28, "qhCqohsbqQQi;").await;
        assert_eq!(list[0], Value::U64(77));
        assert_eq!(list[1], Value::I16(1));
        let Value::Repeat(rows) = &list[2] else {
            panic!("character rows")
        };
        assert_eq!(rows.len(), 10);
        assert_eq!(
            rows[0],
            vec![
                Value::U64(77),
                Value::Id(row.slot),
                Value::I16(1),
                Value::String(row.name),
                Value::Bytes(row.showcase),
                Value::U64(0),
                Value::I8(1), // Normal row: wire state 1.
                Value::I8(0),
                Value::U32(0)
            ]
        );
        // C30 -> D38 -> D39 -> C31 is an unchanged byte relay.
        send(
            &mut client,
            &mut crypt,
            compose("ho", &[Value::I16(30), Value::Id(row.slot)]).unwrap(),
        )
        .await;
        let frame = timeout(Duration::from_secs(3), read_frame(&mut client))
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let (plain, _) = crypt.decrypt(&frame, 4000).unwrap();
        let info = protocol::fields("hqohsbqQQi", inner_packet(&plain).unwrap()).unwrap();
        let mut expected = vec![Value::I16(31)];
        expected.extend(rows[0].clone());
        assert_eq!(info, expected);
        let missing_slot = [46; 16];
        send(
            &mut client,
            &mut crypt,
            compose("ho", &[Value::I16(30), Value::Id(missing_slot)]).unwrap(),
        )
        .await;
        let frame = timeout(Duration::from_secs(3), read_frame(&mut client))
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let (plain, _) = crypt.decrypt(&frame, 4000).unwrap();
        assert_eq!(
            protocol::fields("hoE", inner_packet(&plain).unwrap()).unwrap(),
            vec![Value::I16(32), Value::Id(missing_slot), Value::I16(6)]
        );
        // Generated world-return control, then C6 -> D12/D15 -> C7 reconnect.
        let lobby_token = lobby.retain(key, Duration::from_secs(10)).await.unwrap();
        client.shutdown().await.unwrap();
        drop(client);
        let (mut client, mut crypt) = connect_client(address, &public).await;
        send(
            &mut client,
            &mut crypt,
            compose(
                "hqq",
                &[Value::I16(6), Value::U64(key), Value::U64(lobby_token)],
            )
            .unwrap(),
        )
        .await;
        let retained = receive(&mut client, &mut crypt, 7, "qCiiiii;CiiQiqq;iqiqK").await;
        assert_eq!(retained[0], Value::U64(key));
        assert_eq!(lobby.session_count().await, 1);
        send(
            &mut client,
            &mut crypt,
            compose("h", &[Value::I16(27)]).unwrap(),
        )
        .await;
        let cached = receive(&mut client, &mut crypt, 28, "qhCqohsbqQQi;").await;
        assert_eq!(cached, list);
        // C13 -> D24 -> D25 -> C14 through the unchanged gate.
        send(
            &mut client,
            &mut crypt,
            compose("ho", &[Value::I16(13), Value::Id([45; 16])]).unwrap(),
        )
        .await;
        let result = receive(&mut client, &mut crypt, 14, "q").await;
        let Value::U64(world_token) = result[0] else {
            panic!("world token")
        };
        assert_ne!(world_token, 0);
        assert_eq!(gamed.active(), 1);
        assert_eq!(
            lobby
                .consume_world(key, world_token, u32::from_le_bytes([127, 0, 0, 1]), 1)
                .await,
            Ok(77)
        );
        assert_eq!(gamed.active(), 0);
        assert_eq!(
            lobby
                .consume_world(key, world_token, u32::from_le_bytes([127, 0, 0, 1]), 1)
                .await,
            Err(16)
        );
        // D22 -> C10, followed by the existing gate's delayed close.
        lobby.kick(key, 7).await.unwrap();
        assert_eq!(
            receive(&mut client, &mut crypt, 10, "Q").await,
            vec![Value::I8(7)]
        );
        assert!(timeout(Duration::from_secs(3), read_frame(&mut client))
            .await
            .unwrap()
            .unwrap()
            .is_none());
        assert_eq!(lobby.session_count().await, 0);

        // A second generated client uses C12 -> D23 -> D21 -> gate close.
        let (mut client, mut crypt) = connect_client(address, &public).await;
        send(
            &mut client,
            &mut crypt,
            compose(
                "hoo",
                &[Value::I16(3), Value::Id(token2), Value::Id(portal2)],
            )
            .unwrap(),
        )
        .await;
        let login = receive(&mut client, &mut crypt, 4, "qQoihCiiiii;CiiQiqq;QqiqiqK").await;
        assert_eq!(login[2], Value::Id(identity2.account));
        assert_ne!(login[0], Value::U64(key));
        assert_eq!(lobby.session_count().await, 1);
        send(
            &mut client,
            &mut crypt,
            compose("h", &[Value::I16(12)]).unwrap(),
        )
        .await;
        assert!(timeout(Duration::from_secs(3), read_frame(&mut client))
            .await
            .unwrap()
            .unwrap()
            .is_none());
        assert_eq!(lobby.session_count().await, 0);
        // Shutdown must close both listeners.
        client.shutdown().await.unwrap();
        stop.send(true).unwrap();
        gate_task.await.unwrap().unwrap();
        lobby_task.await.unwrap().unwrap();
        assert_eq!(lobby.session_count().await, 0);
    })
    .await
    .unwrap();
}
