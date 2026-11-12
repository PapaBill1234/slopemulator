//! Logout in the world and kick over the GameDaemon link. Spec: `13-jobs-a.md` §3.7 (LogoutNotify),
//! `18-jobs-f.md` §5 (KickJob), `07-gamed-link.md` (messages 12, 14, 15, 16 and link close).
//! A real `Server` with a real TCP LobbyGate link and a real GameDaemon link. The test is the
//! peer of both links.
use bloodlib::packet::Value;
use lobbydaemon::{
    gamed::{
        Admission, ChargeMode, Eligibility, GameDaemon, GameFuture, StubGameDaemon, WorldFixture,
    },
    gamed_link::LinkGameDaemon,
    jobs::Settings,
    np::{Identity, Np, NpFuture, StubNp},
    protocol as w,
    store::{AccountId, Character, MemoryStore, Shared},
    Server,
};
use packet::gamed_link::{Message, PACKET_VERSION};
use std::{
    net::Ipv4Addr,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
    sync::watch,
    task::JoinHandle,
    time::{sleep, timeout},
};

const IPV4: u32 = 123;
const FIRST_PCID: u64 = 77;
/// Time limit of a world kick in `gamed_link.rs` (`TRANSACTION_LIMIT`).
const KICK_LIMIT: Duration = Duration::from_secs(10);

/// NP that counts the released presences.
struct CountNp {
    inner: StubNp,
    logouts: AtomicUsize,
}
impl Np for CountNp {
    fn consume(&self, token: [u8; 16], portal: [u8; 16]) -> NpFuture<'_, Identity> {
        self.inner.consume(token, portal)
    }
    fn world_entry(
        &self,
        account: AccountId,
        pcid: u64,
        slot: [u8; 16],
        ipv4: u32,
    ) -> GameFuture<'_, ChargeMode> {
        self.inner.world_entry(account, pcid, slot, ipv4)
    }
    fn login_presence(&self, id: AccountId, owner: u64) -> NpFuture<'_, ()> {
        self.inner.login_presence(id, owner)
    }
    fn logout_presence(&self, id: AccountId, owner: u64) {
        self.logouts.fetch_add(1, Ordering::SeqCst);
        self.inner.logout_presence(id, owner);
    }
}

/// Stub world data, but the world kick goes over the real link.
struct Both {
    stub: StubGameDaemon,
    link: Arc<LinkGameDaemon>,
}
impl GameDaemon for Both {
    fn kick(&self, world: i16, client: u64, pcid: u64, reason: i8) -> GameFuture<'_, ()> {
        self.link.kick(world, client, pcid, reason)
    }
    fn eligibility(&self, character: &Character) -> Result<Eligibility, i16> {
        self.stub.eligibility(character)
    }
    fn restore(&self, world: i16, pcid: u64) -> GameFuture<'_, ()> {
        self.stub.restore(world, pcid)
    }
    fn admit(
        &self,
        owner: u64,
        world: i16,
        pcid: u64,
        charge: ChargeMode,
    ) -> GameFuture<'_, Admission> {
        self.stub.admit(owner, world, pcid, charge)
    }
}

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
fn world_token_request(key: u64, slot: [u8; 16]) -> Vec<u8> {
    packet("hqo", &[Value::I16(24), Value::U64(key), Value::Id(slot)])
}
fn character(index: u64) -> Character {
    Character {
        pcid: FIRST_PCID + index,
        slot: [9 + index as u8; 16],
        world: 1,
        name: vec![65],
        showcase: vec![],
        deletion_time: 0,
        state: 0,
        jumping_state: 0,
        last_play_end: 0,
    }
}

async fn read_gate(stream: &mut TcpStream) -> Vec<u8> {
    timeout(Duration::from_secs(2), w::read(stream))
        .await
        .unwrap()
        .unwrap()
        .unwrap()
}
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

type Srv = Server<Shared<MemoryStore>, Arc<CountNp>>;

struct Rig {
    server: Arc<Srv>,
    np: Arc<CountNp>,
    store: Shared<MemoryStore>,
    gate: TcpStream,
    link: TcpStream,
    /// Client key and PC id of each session. The first ones are in the world (4.0), the rest
    /// are in the lobby (2.0).
    sessions: Vec<(u64, u64)>,
    stop: watch::Sender<bool>,
    task: JoinHandle<std::io::Result<()>>,
}

impl Rig {
    /// `world` sessions in state 4.0 and `lobby` sessions in state 2.0, all on one LobbyGate
    /// link, with a live GameDaemon link for world 1.
    async fn new(world: usize, lobby: usize) -> Self {
        let count = world + lobby;
        let (mut np, ..) = StubNp::fixture(60);
        let mut store = MemoryStore::default();
        let mut stub = StubGameDaemon::default();
        let mut logins = Vec::new();
        for index in 0..count {
            let (other, t, p, id) = StubNp::fixture(61 + index as u8);
            drop(other);
            np.identities.insert((t, p), Ok(id.clone()));
            let row = character(index as u64);
            np.world_results
                .insert((id.account, row.pcid, row.slot), Ok(ChargeMode::Paid));
            store.restoration_allowed.insert(row.pcid);
            stub.worlds.insert(
                (1, row.pcid),
                WorldFixture {
                    slot: row.slot,
                    eligibility: Ok(Eligibility::Ready),
                    restore: Ok(()),
                    admission: Ok(None),
                },
            );
            store.characters.insert(id.account, vec![row.clone()]);
            logins.push((t, p, row));
        }
        let np = Arc::new(CountNp {
            inner: np,
            logouts: AtomicUsize::new(0),
        });
        let store = Shared::new(store);
        let gamed = Arc::new(LinkGameDaemon::default());
        let server = Arc::new(
            Server::new(
                store.clone(),
                np.clone(),
                Settings {
                    world_token_lifetime: Some(Duration::from_secs(10)),
                    ..Settings::default()
                },
            )
            .unwrap()
            .with_game_daemon(Arc::new(Both {
                stub,
                link: gamed.clone(),
            })),
        );
        gamed.attach(server.clone());
        // LobbyGate link.
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        let (stop, shutdown) = watch::channel(false);
        let task = tokio::spawn(server.clone().serve(listener, shutdown));
        let mut gate = TcpStream::connect(address).await.unwrap();
        gate.write_all(&hello_packet()).await.unwrap();
        read_gate(&mut gate).await;
        // GameDaemon link, live for world 1.
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        let consume: lobbydaemon::gamed_link::Consume =
            Arc::new(|_, _, _, _| Box::pin(async { Err(15) }));
        tokio::spawn(gamed.serve(listener, consume));
        let mut link = TcpStream::connect(address).await.unwrap();
        write_message(
            &mut link,
            Message::Hello {
                cluster: 1,
                world: 1,
                version: PACKET_VERSION,
                port: 0,
            },
        )
        .await;
        assert_eq!(read_message(&mut link).await, Message::HelloOk);
        write_message(
            &mut link,
            Message::WorldLive {
                cluster: 1,
                world: 1,
            },
        )
        .await;
        // Sessions.
        let mut sessions = Vec::new();
        for (index, (t, p, row)) in logins.into_iter().enumerate() {
            gate.write_all(&init_packet(100 + index as u64, t, p))
                .await
                .unwrap();
            let init = read_gate(&mut gate).await;
            let Value::U64(key) = w::fields("hqqoo", &init).unwrap()[2] else {
                panic!("key")
            };
            gate.write_all(&start_packet(key, t, p)).await.unwrap();
            read_gate(&mut gate).await;
            if index < world {
                gate.write_all(&world_token_request(key, row.slot))
                    .await
                    .unwrap();
                // Packets of the start (10 and more) can still wait: take them first.
                let mut reply = read_gate(&mut gate).await;
                while w::command(&reply).unwrap() != 25 {
                    reply = read_gate(&mut gate).await;
                }
                let Value::U64(token) = w::fields("hqqh", &reply).unwrap()[2] else {
                    panic!("token")
                };
                let entered = server.consume_world(key, token, IPV4, 1).await;
                assert_eq!(entered, Ok(row.pcid));
            }
            sessions.push((key, row.pcid));
        }
        assert_eq!(server.session_count().await, count);
        Self {
            server,
            np,
            store,
            gate,
            link,
            sessions,
            stop,
            task,
        }
    }

    async fn wait_sessions(&self, count: usize) {
        for _ in 0..300 {
            if self.server.session_count().await == count {
                return;
            }
            sleep(Duration::from_millis(10)).await;
        }
        panic!("session count is not {count}");
    }

    /// Wait for the last-logout rows (`Store::update_last_logout`) of ended sessions.
    async fn wait_last_logouts(&self, count: usize) {
        for _ in 0..300 {
            if self.store.lock().logout_updates.len() == count {
                return;
            }
            sleep(Duration::from_millis(10)).await;
        }
        panic!("last logout rows are not {count}");
    }

    /// Start a kick of session `index` and read the `Kick` of the link.
    /// Returns the kick task and the transaction.
    async fn start_kick(
        &mut self,
        index: usize,
        reason: i8,
    ) -> (JoinHandle<std::io::Result<()>>, u64) {
        let (key, pcid) = self.sessions[index];
        let server = self.server.clone();
        let task = tokio::spawn(async move { server.kick(key, reason).await });
        let Message::Kick {
            transaction,
            client,
            pcid: kicked,
            reason: sent,
        } = read_message(&mut self.link).await
        else {
            panic!("Kick expected")
        };
        assert_eq!((client, kicked, sent), (key, pcid, reason));
        (task, transaction)
    }

    async fn finish(self) {
        self.stop.send(true).unwrap();
        let _ = timeout(Duration::from_secs(2), self.task).await;
    }
}

/// A LogoutNotify (12) of the right PC: the session ends (6.0), the NP presence is released and
/// the last-logout row is written.
#[tokio::test]
async fn logout_in_world_ends_session() {
    let mut rig = Rig::new(1, 0).await;
    let (key, pcid) = rig.sessions[0];
    write_message(
        &mut rig.link,
        Message::LogoutNotify {
            pc_db_id: pcid,
            cli_key: key,
        },
    )
    .await;
    rig.wait_sessions(0).await;
    rig.wait_last_logouts(1).await;
    assert_eq!(rig.np.logouts.load(Ordering::SeqCst), 1);
    rig.finish().await;
}

/// Another PC id or another client key: no change. The session can still leave the world, so it
/// is in state 4.0 with its playing character.
#[tokio::test]
async fn logout_in_world_wrong_pcid() {
    let mut rig = Rig::new(1, 0).await;
    let (key, pcid) = rig.sessions[0];
    for (pc_db_id, cli_key) in [(pcid + 1, key), (pcid, key + 1000), (0, key)] {
        write_message(&mut rig.link, Message::LogoutNotify { pc_db_id, cli_key }).await;
    }
    // The next Leave comes after the three LogoutNotify messages in the queue of the link task.
    write_message(
        &mut rig.link,
        Message::Leave {
            transaction: 1,
            pc_db_id: pcid,
            cli_key: key,
        },
    )
    .await;
    assert!(matches!(
        read_message(&mut rig.link).await,
        Message::LeaveOk { transaction: 1, .. }
    ));
    assert_eq!(rig.server.session_count().await, 1);
    assert_eq!(rig.np.logouts.load(Ordering::SeqCst), 0);
    assert!(rig.store.lock().logout_updates.is_empty());
    rig.finish().await;
}

/// A LogoutNotify of a session that is in the lobby (2.0) changes nothing.
#[tokio::test]
async fn logout_in_world_lobby_session_unchanged() {
    let mut rig = Rig::new(0, 1).await;
    let (key, pcid) = rig.sessions[0];
    write_message(
        &mut rig.link,
        Message::LogoutNotify {
            pc_db_id: pcid,
            cli_key: key,
        },
    )
    .await;
    // Barrier: a leave of a 2.0 session fails with reason 6.
    write_message(
        &mut rig.link,
        Message::Leave {
            transaction: 1,
            pc_db_id: pcid,
            cli_key: key,
        },
    )
    .await;
    assert_eq!(
        read_message(&mut rig.link).await,
        Message::LeaveFail {
            transaction: 1,
            reason: 6
        }
    );
    assert_eq!(rig.server.session_count().await, 1);
    rig.finish().await;
}

/// The link of the world closes: both sessions in the world end (`07-gamed-link.md` step 11).
/// The session in the lobby stays.
#[tokio::test]
async fn world_link_close_ends_sessions() {
    let rig = Rig::new(2, 1).await;
    let Rig {
        server,
        np,
        store,
        gate,
        link,
        stop,
        task,
        ..
    } = rig;
    drop(link);
    for _ in 0..300 {
        if server.session_count().await == 1 {
            break;
        }
        sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(server.session_count().await, 1);
    for _ in 0..300 {
        if store.lock().logout_updates.len() == 2 {
            break;
        }
        sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(store.lock().logout_updates.len(), 2);
    assert_eq!(np.logouts.load(Ordering::SeqCst), 2);
    drop(gate);
    stop.send(true).unwrap();
    let _ = timeout(Duration::from_secs(2), task).await;
}

/// `Server::kick` of a session in the world: the link gets `Kick`. After `KickOk` the session
/// ends and `kick` returns `Ok(())`. The client gets the kick packet.
#[tokio::test]
async fn kick_in_world_ok() {
    let mut rig = Rig::new(1, 0).await;
    let (key, _) = rig.sessions[0];
    let (task, transaction) = rig.start_kick(0, 7).await;
    write_message(&mut rig.link, Message::KickOk { transaction }).await;
    let done = timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap();
    assert!(done.is_ok());
    rig.wait_sessions(0).await;
    assert_eq!(read_gate(&mut rig.gate).await, w::kick(key, 7).unwrap());
    rig.wait_last_logouts(1).await;
    assert_eq!(rig.np.logouts.load(Ordering::SeqCst), 1);
    rig.finish().await;
}

/// The same, with a check of the wait: `kick` does not return before the reply.
#[tokio::test]
async fn kick_in_world_waits_for_reply() {
    let mut rig = Rig::new(1, 0).await;
    let (mut task, transaction) = rig.start_kick(0, 7).await;
    assert!(timeout(Duration::from_millis(200), &mut task)
        .await
        .is_err());
    assert_eq!(rig.server.session_count().await, 1);
    write_message(&mut rig.link, Message::KickOk { transaction }).await;
    assert!(timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap()
        .is_ok());
    assert_eq!(rig.server.session_count().await, 0);
    rig.finish().await;
}

/// KickFail 24 (kick done already, result 1) and other reasons (result 2): the session ends.
#[tokio::test]
async fn kick_in_world_fail_ends_session() {
    for reason in [24, 2] {
        let mut rig = Rig::new(1, 0).await;
        let (task, transaction) = rig.start_kick(0, 3).await;
        write_message(
            &mut rig.link,
            Message::KickFail {
                transaction,
                reason,
            },
        )
        .await;
        assert!(timeout(Duration::from_secs(2), task)
            .await
            .unwrap()
            .unwrap()
            .is_ok());
        assert_eq!(rig.server.session_count().await, 0);
        assert_eq!(rig.np.logouts.load(Ordering::SeqCst), 1);
        rig.finish().await;
    }
}

/// No reply: the session ends after the limit. The clock is paused after the `Kick` went out.
#[tokio::test]
async fn kick_in_world_timeout() {
    let mut rig = Rig::new(1, 0).await;
    let (key, _) = rig.sessions[0];
    let (mut task, _) = rig.start_kick(0, 7).await;
    tokio::time::pause();
    tokio::time::advance(KICK_LIMIT - Duration::from_secs(1)).await;
    assert!(timeout(Duration::from_millis(1), &mut task).await.is_err());
    assert_eq!(rig.server.session_count().await, 1);
    tokio::time::advance(Duration::from_secs(2)).await;
    assert!(task.await.unwrap().is_ok());
    assert_eq!(rig.server.session_count().await, 0);
    assert_eq!(read_gate(&mut rig.gate).await, w::kick(key, 7).unwrap());
    assert_eq!(rig.np.logouts.load(Ordering::SeqCst), 1);
    rig.finish().await;
}

/// The link of the world closes while a kick waits: the kick ends at once (result 2).
#[tokio::test]
async fn kick_in_world_link_close_ends_kick() {
    let mut rig = Rig::new(1, 0).await;
    let (task, _) = rig.start_kick(0, 7).await;
    let Rig {
        server,
        link,
        stop,
        task: serve,
        ..
    } = rig;
    drop(link);
    assert!(timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap()
        .is_ok());
    assert_eq!(server.session_count().await, 0);
    stop.send(true).unwrap();
    let _ = timeout(Duration::from_secs(2), serve).await;
}

/// A kick that waits for the GameDaemon does not stop the link task: a command of another
/// session on the same LobbyGate link gets its reply (35 → 36) first.
#[tokio::test]
async fn kick_does_not_block_link() {
    let mut rig = Rig::new(1, 1).await;
    let (other, _) = rig.sessions[1];
    let (mut task, transaction) = rig.start_kick(0, 7).await;
    rig.gate
        .write_all(&w::notice(35, other).unwrap())
        .await
        .unwrap();
    let reply = read_gate(&mut rig.gate).await;
    assert_eq!(w::command(&reply).unwrap(), 36);
    // The kick is still waiting.
    assert!(timeout(Duration::from_millis(100), &mut task)
        .await
        .is_err());
    write_message(&mut rig.link, Message::KickOk { transaction }).await;
    assert!(timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap()
        .is_ok());
    assert_eq!(rig.server.session_count().await, 1);
    rig.finish().await;
}
