//! Return to lobby over the gamed link. Spec: `Docs/Server/LobbyDaemon/23-return-to-lobby.md`,
//! `13-jobs-a.md` §3.6. The job core runs on `Jobs`; the link runs on `Server` and
//! `LinkGameDaemon` with a real TCP link.
use bloodlib::packet::Value;
use lobbydaemon::{
    gamed::{ChargeMode, Eligibility, StubGameDaemon, WorldFixture},
    gamed_link::{Leave, LinkGameDaemon, LinkPeer},
    jobs::{Jobs, Settings, State},
    np::StubNp,
    protocol as w,
    store::{Character, MemoryStore, Shared},
    Server,
};
use packet::gamed_link::{Message, PACKET_VERSION};
use std::{net::Ipv4Addr, sync::Arc, time::Duration};
use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
    sync::watch,
    time::timeout,
};

const PCID: u64 = 77;
const IPV4: u32 = 123;
type Rig = Jobs<Shared<MemoryStore>, StubNp>;

fn packet(f: &str, v: &[Value]) -> Vec<u8> {
    w::packet(f, v).unwrap()
}
fn hello_packet() -> Vec<u8> {
    packet(
        "hhhh",
        &[Value::I16(1), Value::I16(0), Value::I16(58), Value::I16(0)],
    )
}
fn init_packet(gate: u64, t: [u8; 16], p: [u8; 16]) -> Vec<u8> {
    packet(
        "hqooi",
        &[
            Value::I16(6),
            Value::U64(gate),
            Value::Id(t),
            Value::Id(p),
            Value::U32(IPV4),
        ],
    )
}
fn start_packet(key: u64, t: [u8; 16], p: [u8; 16]) -> Vec<u8> {
    packet(
        "hqoo",
        &[Value::I16(9), Value::U64(key), Value::Id(t), Value::Id(p)],
    )
}
fn world_token_request(key: u64) -> Vec<u8> {
    packet(
        "hqo",
        &[Value::I16(24), Value::U64(key), Value::Id([9; 16])],
    )
}
/// LobbyGate 12 InitSessionWithLobbyTokenReq `hqqqi`: gate key, client key, token, IPv4.
fn token_login(gate: u64, key: u64, token: u64) -> Vec<u8> {
    packet(
        "hqqqi",
        &[
            Value::I16(12),
            Value::U64(gate),
            Value::U64(key),
            Value::U64(token),
            Value::U32(IPV4),
        ],
    )
}
fn character() -> Character {
    Character {
        pcid: PCID,
        slot: [9; 16],
        world: 1,
        name: vec![65],
        showcase: vec![],
        deletion_time: None,
        state: lobbydaemon::character::InternalState::Normal,
        jumping_state: 0,
        last_play_end: 0,
    }
}
type Fixture = (
    Shared<MemoryStore>,
    StubNp,
    Arc<StubGameDaemon>,
    [u8; 16],
    [u8; 16],
);
fn fixture() -> Fixture {
    let (mut np, t, p, id) = StubNp::fixture(61);
    np.world_results
        .insert((id.account, PCID, [9; 16]), Ok(ChargeMode::Paid));
    let mut store = MemoryStore::default();
    store.characters.insert(id.account, vec![character()]);
    store.restoration_allowed.insert(PCID);
    let mut g = StubGameDaemon::default();
    g.worlds.insert(
        (1, PCID),
        WorldFixture {
            slot: [9; 16],
            eligibility: Ok(Eligibility::Ready),
            restore: Ok(()),
            admission: Ok(None),
        },
    );
    (Shared::new(store), np, Arc::new(g), t, p)
}
async fn send(j: &mut Rig, p: Vec<u8>) -> Vec<Vec<u8>> {
    j.handle(1, &p)
        .await
        .unwrap()
        .into_iter()
        .flat_map(|o| o.packets)
        .collect()
}
fn token_of(packets: &[Vec<u8>]) -> u64 {
    let v = w::fields("hqqh", &packets[0]).unwrap();
    let Value::U64(token) = v[2] else { panic!() };
    token
}
/// A session in state 4.0 that plays `PCID` in world 1.
async fn in_world(settings: Settings) -> (Rig, u64) {
    let (store, np, g, t, p) = fixture();
    let mut j = Jobs::new(
        store,
        np,
        Settings {
            world_token_lifetime: Some(Duration::from_secs(10)),
            ..settings
        },
    )
    .unwrap()
    .with_game_daemon(g);
    send(&mut j, hello_packet()).await;
    let init = send(&mut j, init_packet(100, t, p)).await;
    let Value::U64(key) = w::fields("hqqoo", &init[0]).unwrap()[2] else {
        panic!()
    };
    send(&mut j, start_packet(key, t, p)).await;
    let world_token = token_of(&send(&mut j, world_token_request(key)).await);
    assert_eq!(j.consume_world(key, world_token, IPV4, 1), Ok(PCID));
    assert_eq!(j.state(key), Some(State::World));
    (j, key)
}

#[tokio::test]
async fn leave_ok_issues_type2_token() {
    let (mut j, key) = in_world(Settings::default()).await;
    let token = j.leave_world(key, PCID, 1).unwrap();
    assert_eq!(j.state(key), Some(State::Retained));
    // The token logs in once at LobbyGate: InitSession (12) → 13, with the same client key.
    assert_eq!(
        send(&mut j, token_login(200, key, token)).await,
        vec![packet(
            "hqq",
            &[Value::I16(13), Value::U64(200), Value::U64(key)]
        )]
    );
    assert_eq!(j.state(key), Some(State::RetainedInit));
}

#[tokio::test]
async fn leave_token_single_use() {
    let (mut j, key) = in_world(Settings::default()).await;
    let token = j.leave_world(key, PCID, 1).unwrap();
    assert_eq!(send(&mut j, token_login(200, key, token)).await.len(), 1);
    // The second use: no token found, reason 20 (`23-return-to-lobby.md` §3).
    assert_eq!(
        send(&mut j, token_login(201, key, token)).await,
        vec![w::failure(14, 201, 20).unwrap()]
    );
}

#[tokio::test]
async fn leave_no_session() {
    let (mut j, key) = in_world(Settings::default()).await;
    // Unknown key, unknown PC id and a zero PC id: reason 5.
    assert_eq!(j.leave_world(key + 1000, PCID, 1), Err(5));
    assert_eq!(j.leave_world(key, PCID + 1, 1), Err(5));
    assert_eq!(j.leave_world(key, 0, 1), Err(5));
    assert_eq!(j.state(key), Some(State::World));
}

#[tokio::test]
async fn leave_wrong_state() {
    let (store, np, g, t, p) = fixture();
    let mut j = Jobs::new(store, np, Settings::default())
        .unwrap()
        .with_game_daemon(g);
    send(&mut j, hello_packet()).await;
    let init = send(&mut j, init_packet(100, t, p)).await;
    let Value::U64(key) = w::fields("hqqoo", &init[0]).unwrap()[2] else {
        panic!()
    };
    send(&mut j, start_packet(key, t, p)).await;
    // State 2.0 (lobby): reason 6. The session does not change.
    assert_eq!(j.state(key), Some(State::Lobby));
    assert_eq!(j.leave_world(key, PCID, 1), Err(6));
    assert_eq!(j.state(key), Some(State::Lobby));
}

#[tokio::test]
async fn leave_playing_checks_and_token_failure() {
    // World differs: 22. The state stays 4.0 and the next call can pass.
    let (mut j, key) = in_world(Settings::default()).await;
    assert_eq!(j.leave_world(key, PCID, 2), Err(22));
    assert_eq!(j.state(key), Some(State::World));
    // A second leave after success: the state is 5.1, reason 6 (no second token).
    assert!(j.leave_world(key, PCID, 1).is_ok());
    assert_eq!(j.leave_world(key, PCID, 1), Err(6));
    // Token issue failure: reason 2, the session ends (state 6.0).
    let (mut j, key) = in_world(Settings {
        lobby_token_lifetime: None,
        ..Settings::default()
    })
    .await;
    assert_eq!(j.leave_world(key, PCID, 1), Err(2));
    assert_eq!(j.state(key), None);
    assert_eq!(j.take_logouts().len(), 1);
}

// ---- Server and link ----

async fn read_message(stream: &mut TcpStream) -> Message {
    let frame = timeout(Duration::from_secs(2), w::read(stream))
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    Message::decode(&frame).unwrap().unwrap()
}
async fn write_message(stream: &mut TcpStream, message: Message) {
    stream.write_all(&message.encode().unwrap()).await.unwrap();
}
/// A GameDaemon link that is live for world 1. `None` for `server`: no leave hook.
async fn game_link(server: Option<Arc<Server<Shared<MemoryStore>, StubNp>>>) -> TcpStream {
    let gamed = Arc::new(LinkGameDaemon::new(vec![LinkPeer {
        world: 1,
        cluster: 1,
        address: std::net::Ipv4Addr::LOCALHOST.into(),
    }]));
    if let Some(server) = server {
        let leave: Leave = Arc::new(move |pcid, key, world| {
            let server = server.clone();
            Box::pin(async move { server.leave_world(key, pcid, world).await })
        });
        gamed.set_leave(leave);
    }
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let address = listener.local_addr().unwrap();
    let consume: lobbydaemon::gamed_link::Consume =
        Arc::new(|_, _, _, _| Box::pin(async { Err(15) }));
    tokio::spawn(gamed.serve(listener, consume));
    let mut stream = TcpStream::connect(address).await.unwrap();
    write_message(
        &mut stream,
        Message::Hello {
            cluster: 1,
            world: 1,
            version: PACKET_VERSION,
            port: 0,
        },
    )
    .await;
    assert_eq!(read_message(&mut stream).await, Message::HelloOk);
    write_message(
        &mut stream,
        Message::WorldLive {
            cluster: 1,
            world: 1,
        },
    )
    .await;
    stream
}
async fn read_gate(stream: &mut TcpStream) -> Vec<u8> {
    timeout(Duration::from_secs(2), w::read(stream))
        .await
        .unwrap()
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn leave_wrong_cli_key() {
    let (store, np, g, t, p) = fixture();
    let server = Arc::new(
        Server::new(
            store,
            np,
            Settings {
                world_token_lifetime: Some(Duration::from_secs(10)),
                ..Settings::default()
            },
        )
        .unwrap()
        .with_game_daemon(g),
    );
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let address = listener.local_addr().unwrap();
    let (stop, shutdown) = watch::channel(false);
    let lobby = tokio::spawn(server.clone().serve(listener, shutdown));
    // LobbyGate side: log in, start, get the world token.
    let mut gate = TcpStream::connect(address).await.unwrap();
    gate.write_all(&hello_packet()).await.unwrap();
    read_gate(&mut gate).await;
    gate.write_all(&init_packet(100, t, p)).await.unwrap();
    let init = read_gate(&mut gate).await;
    let Value::U64(key) = w::fields("hqqoo", &init).unwrap()[2] else {
        panic!()
    };
    gate.write_all(&start_packet(key, t, p)).await.unwrap();
    read_gate(&mut gate).await;
    gate.write_all(&world_token_request(key)).await.unwrap();
    let world_token = token_of(&[read_gate(&mut gate).await]);
    assert_eq!(
        server.consume_world(key, world_token, IPV4, 1).await,
        Ok(PCID)
    );

    let mut link = game_link(Some(server.clone())).await;
    // Other client key: reason 5. The session stays in state 4.0.
    write_message(
        &mut link,
        Message::Leave {
            transaction: 1,
            pc_db_id: PCID,
            cli_key: key + 1000,
        },
    )
    .await;
    assert_eq!(
        read_message(&mut link).await,
        Message::LeaveFail {
            transaction: 1,
            reason: 5
        }
    );
    // The right key: LeaveOk with a token, and the token works once at LobbyGate.
    write_message(
        &mut link,
        Message::Leave {
            transaction: 2,
            pc_db_id: PCID,
            cli_key: key,
        },
    )
    .await;
    let Message::LeaveOk {
        transaction: 2,
        token,
    } = read_message(&mut link).await
    else {
        panic!("LeaveOk expected")
    };
    gate.write_all(&token_login(200, key, token)).await.unwrap();
    assert_eq!(
        read_gate(&mut gate).await,
        packet("hqq", &[Value::I16(13), Value::U64(200), Value::U64(key)])
    );
    gate.write_all(&token_login(201, key, token)).await.unwrap();
    assert_eq!(read_gate(&mut gate).await, w::failure(14, 201, 20).unwrap());
    // A second leave after the session left the world: reason 6.
    write_message(
        &mut link,
        Message::Leave {
            transaction: 3,
            pc_db_id: PCID,
            cli_key: key,
        },
    )
    .await;
    assert_eq!(
        read_message(&mut link).await,
        Message::LeaveFail {
            transaction: 3,
            reason: 6
        }
    );
    // A LogoutNotify gets no reply and does not close the link.
    write_message(
        &mut link,
        Message::LogoutNotify {
            pc_db_id: PCID,
            cli_key: key,
        },
    )
    .await;
    write_message(
        &mut link,
        Message::Leave {
            transaction: 4,
            pc_db_id: PCID,
            cli_key: key + 1,
        },
    )
    .await;
    assert_eq!(
        read_message(&mut link).await,
        Message::LeaveFail {
            transaction: 4,
            reason: 5
        }
    );
    stop.send(true).unwrap();
    let _ = timeout(Duration::from_secs(2), lobby).await;
}

/// `quirk_leave_fail_as_enter_fail`: the original sends 7 EnterWorldFail when the job cannot
/// start. The port sends 10 LeaveWorldFail, reason 2.
#[tokio::test]
async fn leave_job_start_fail_sends_leave_fail() {
    // No leave hook: the job cannot start.
    let mut link = game_link(None).await;
    write_message(
        &mut link,
        Message::Leave {
            transaction: 9,
            pc_db_id: PCID,
            cli_key: 5,
        },
    )
    .await;
    assert_eq!(
        read_message(&mut link).await,
        Message::LeaveFail {
            transaction: 9,
            reason: 2
        }
    );
    // A server with no link for the key: the job cannot start; reason 5 (no session).
    let (store, np, _, _, _) = fixture();
    let server = Server::new(store, np, Settings::default()).unwrap();
    assert_eq!(server.leave_world(5, PCID, 1).await, Err(5));
}
