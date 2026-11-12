//! Return to lobby (client 26 → link 8 → 9 or 10 → client 27 or 28).
//! Spec: Docs/Server/GameDaemon/10-session-handlers.md §26, Docs/Server/LobbyDaemon/23-return-to-lobby.md.
//! A fake LobbyDaemon on loopback answers the real `Link`. The real shard `World` runs the session.
use bytes::Bytes;
use gamedaemon::{
    handlers::{dispatch, Context, Gate, MemoryServices, Pc, Session, World as HandlerWorld},
    ids,
    link::Link,
    shard::{Event, Inbox, ToConn, World},
};
use packet::gamed_link::Message;
use shared::net::FrameReader;
use std::time::Duration;
use tokio::{
    io::AsyncWriteExt,
    net::TcpListener,
    sync::mpsc::{self, UnboundedReceiver},
};

const PCID: u64 = 5;
const CLIENT_KEY: u64 = 7;
const TOKEN_KEY: u64 = 0x1122_3344_5566_7788;

#[derive(Clone, Copy)]
enum Reply {
    Ok(u64),
    Fail(i16),
    Silent,
}

fn request(cmd: u16, body: &[u8]) -> Bytes {
    let mut packet = ((body.len() + 4) as i16).to_le_bytes().to_vec();
    packet.extend(cmd.to_le_bytes());
    packet.extend(body);
    packet.into()
}

/// Fake LobbyDaemon: answers Hello and Enter. A Leave gets `reply`. Every message goes to the
/// returned channel.
async fn fake_lobby(reply: Reply) -> (String, UnboundedReceiver<Message>, Push) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let (seen, messages) = mpsc::unbounded_channel();
    let (push, mut pushed) = mpsc::unbounded_channel::<Message>();
    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let (read, mut write) = stream.into_split();
        let mut reader = FrameReader::new(read, usize::from(u16::MAX));
        // Messages of the test go out in the same order as the answers (one writer).
        let (to_write, mut writes) = mpsc::unbounded_channel::<Message>();
        let extra = to_write.clone();
        tokio::spawn(async move {
            while let Some(message) = pushed.recv().await {
                if extra.send(message).is_err() {
                    break;
                }
            }
        });
        tokio::spawn(async move {
            while let Some(message) = writes.recv().await {
                if write.write_all(&message.encode().unwrap()).await.is_err() {
                    break;
                }
            }
        });
        while let Ok(Some(frame)) = reader.next().await {
            let Ok(Some(message)) = Message::decode(&frame) else {
                continue;
            };
            let answer = match &message {
                Message::Hello { .. } => Some(Message::HelloOk),
                Message::Enter { transaction, .. } => Some(Message::EnterOk {
                    transaction: *transaction,
                    pcid: PCID,
                    account: [0; 16],
                }),
                Message::Leave { transaction, .. } => match reply {
                    Reply::Ok(token) => Some(Message::LeaveOk {
                        transaction: *transaction,
                        token,
                    }),
                    Reply::Fail(reason) => Some(Message::LeaveFail {
                        transaction: *transaction,
                        reason,
                    }),
                    Reply::Silent => None,
                },
                _ => None,
            };
            let _ = seen.send(message);
            if let Some(answer) = answer {
                let _ = to_write.send(answer);
            }
        }
    });
    (address, messages, push)
}

/// Lets a test send a link message from the fake LobbyDaemon to GameDaemon.
type Push = mpsc::UnboundedSender<Message>;

struct Client {
    shard: Inbox,
    packets: mpsc::Receiver<ToConn>,
    messages: UnboundedReceiver<Message>,
    push: Push,
}

impl Client {
    /// Link up, session open, token auth done (packet 23 → 24).
    async fn start(reply: Reply, leave_limit: Duration) -> Self {
        let (address, mut messages, push) = fake_lobby(reply).await;
        let link = Link::new(1, 1, 10241, None);
        link.set_leave_limit(leave_limit);
        tokio::spawn(link.clone().run(address));
        loop {
            let message = tokio::time::timeout(Duration::from_secs(5), messages.recv())
                .await
                .expect("no WorldLive")
                .expect("lobby closed");
            if matches!(message, Message::WorldLive { .. }) {
                break;
            }
        }
        let world = World::new(vec![], Some(link), None);
        let shard = world.shard(0);
        let (out, packets) = mpsc::channel(16);
        let open = Event::Open {
            conn: 1,
            address: 0,
            out,
        };
        shard.send(open).await.ok().unwrap();
        let mut client = Self {
            shard,
            packets,
            messages,
            push,
        };
        let mut body = 1i16.to_le_bytes().to_vec();
        body.extend(CLIENT_KEY.to_le_bytes());
        body.extend(9u64.to_le_bytes());
        client.send(ids::G_LobbyTokenAuthReqCmd, &body).await;
        let ok = client.next().await;
        assert_eq!(command(&ok), ids::G_LobbyTokenAuthOkCmd);
        client
    }

    async fn send(&self, cmd: u16, body: &[u8]) {
        let packet = request(cmd, body);
        let event = Event::Packet { conn: 1, packet };
        self.shard.send(event).await.ok().unwrap();
    }

    async fn next(&mut self) -> Bytes {
        loop {
            let item = tokio::time::timeout(Duration::from_secs(5), self.packets.recv())
                .await
                .expect("no packet")
                .expect("session closed");
            if let ToConn::Packet(packet) = item {
                return packet;
            }
        }
    }

    /// Wait for the Leave message that the link sent.
    async fn leave_seen(&mut self) -> (u64, u64) {
        loop {
            let message = tokio::time::timeout(Duration::from_secs(5), self.messages.recv())
                .await
                .expect("no Leave")
                .expect("lobby closed");
            if let Message::Leave {
                pc_db_id, cli_key, ..
            } = message
            {
                return (pc_db_id, cli_key);
            }
        }
    }
}

impl Client {
    /// Wait for the next message of this kind, or `None` after `wait`.
    async fn seen<T>(&mut self, wait: Duration, pick: impl Fn(Message) -> Option<T>) -> Option<T> {
        let end = tokio::time::Instant::now() + wait;
        loop {
            let message = tokio::time::timeout_at(end, self.messages.recv())
                .await
                .ok()??;
            if let Some(found) = pick(message) {
                return Some(found);
            }
        }
    }

    /// The link answer to a Kick: `KickOk` → `Ok`, `KickFail` → `Err(reason)`.
    async fn kick_reply(&mut self, transaction: u64) -> Option<Result<(), i16>> {
        self.seen(Duration::from_secs(5), |m| match m {
            Message::KickOk { transaction: t } if t == transaction => Some(Ok(())),
            Message::KickFail {
                transaction: t,
                reason,
            } if t == transaction => Some(Err(reason)),
            _ => None,
        })
        .await
    }

    async fn logout_notify_seen(&mut self, wait: Duration) -> Option<(u64, u64)> {
        self.seen(wait, |m| match m {
            Message::LogoutNotify { pc_db_id, cli_key } => Some((pc_db_id, cli_key)),
            _ => None,
        })
        .await
    }
}

fn command(packet: &[u8]) -> u16 {
    u16::from_le_bytes([packet[2], packet[3]])
}

/// Reason of a 28 packet (`hh`).
fn fail_reason(packet: &[u8]) -> i16 {
    assert_eq!(command(packet), ids::G_ReturnToLobbyFailCmd);
    assert_eq!(packet.len(), 6);
    i16::from_le_bytes([packet[4], packet[5]])
}

#[tokio::test]
async fn rtl_ok_sends_27_token() {
    let mut client = Client::start(Reply::Ok(TOKEN_KEY), Duration::from_secs(5)).await;
    client.send(ids::G_ReturnToLobbyReqCmd, &[]).await;
    let ok = client.next().await;
    assert_eq!(command(&ok), ids::G_ReturnToLobbyOkCmd);
    assert_eq!(ok.len(), 12);
    assert_eq!(ok[4..12], TOKEN_KEY.to_le_bytes());
    // The link sent the PC id and the client key of the token request.
    assert_eq!(client.leave_seen().await, (PCID, CLIENT_KEY));
}

#[tokio::test]
async fn rtl_fail_21_to_1954() {
    for reason in [21, 22, 23] {
        let mut client = Client::start(Reply::Fail(reason), Duration::from_secs(5)).await;
        client.send(ids::G_ReturnToLobbyReqCmd, &[]).await;
        assert_eq!(fail_reason(&client.next().await), 1954);
    }
}

#[tokio::test]
async fn rtl_fail_other_to_1() {
    for reason in [5, 6, 2] {
        let mut client = Client::start(Reply::Fail(reason), Duration::from_secs(5)).await;
        client.send(ids::G_ReturnToLobbyReqCmd, &[]).await;
        assert_eq!(fail_reason(&client.next().await), 1);
    }
}

/// `quirk_rtl_no_timeout` (fixed): no reply of LobbyDaemon gives 28 with reason 1952.
#[tokio::test]
async fn rtl_timeout_1952() {
    let mut client = Client::start(Reply::Silent, Duration::from_millis(200)).await;
    client.send(ids::G_ReturnToLobbyReqCmd, &[]).await;
    assert_eq!(fail_reason(&client.next().await), 1952);
}

/// A second request: the user already left the user manager (§26.1 step 4) → 28/1954.
/// The link sends only one Leave.
#[tokio::test]
async fn rtl_second_request_1954() {
    let mut client = Client::start(Reply::Ok(TOKEN_KEY), Duration::from_secs(5)).await;
    client.send(ids::G_ReturnToLobbyReqCmd, &[]).await;
    assert_eq!(command(&client.next().await), ids::G_ReturnToLobbyOkCmd);
    client.send(ids::G_ReturnToLobbyReqCmd, &[]).await;
    assert_eq!(fail_reason(&client.next().await), 1954);
}

/// Link command 14: the PC connection closes (the shard drops the session) and 15 goes back.
/// The kick of another PC id gives 16 with reason 24 and keeps the session.
#[tokio::test]
async fn gamed_kick_disconnects_pc() {
    let mut client = Client::start(Reply::Silent, Duration::from_secs(5)).await;
    let kick = |transaction, pcid| Message::Kick {
        transaction,
        client: CLIENT_KEY,
        pcid,
        reason: 8,
    };
    client.push.send(kick(10, PCID + 1)).unwrap();
    assert_eq!(client.kick_reply(10).await, Some(Err(24)));
    // The session is still open.
    assert!(matches!(
        client.packets.try_recv(),
        Err(mpsc::error::TryRecvError::Empty)
    ));
    client.push.send(kick(11, PCID)).unwrap();
    assert_eq!(client.kick_reply(11).await, Some(Ok(())));
    // The connection task sees that its queue closed.
    let closed = tokio::time::timeout(Duration::from_secs(5), client.packets.recv()).await;
    assert!(matches!(closed, Ok(None)), "the session was not dropped");
    // The kick sends no LogoutNotify: LobbyDaemon ends its session itself.
    assert_eq!(
        client.logout_notify_seen(Duration::from_millis(300)).await,
        None
    );
    // A second kick finds no PC.
    client.push.send(kick(12, PCID)).unwrap();
    assert_eq!(client.kick_reply(12).await, Some(Err(24)));
}

/// A socket close in the world sends LogoutNotify (link command 12) with the PC id and the
/// client key.
#[tokio::test]
async fn close_in_world_sends_logout_notify() {
    let mut client = Client::start(Reply::Silent, Duration::from_secs(5)).await;
    let event = Event::Closed { conn: 1 };
    client.shard.send(event).await.ok().unwrap();
    let seen = client.logout_notify_seen(Duration::from_secs(5)).await;
    assert_eq!(seen, Some((PCID, CLIENT_KEY)));
}

/// A close before the token auth sends nothing: the user is not in the world.
#[tokio::test]
async fn close_without_user_sends_no_logout_notify() {
    let (address, mut messages, _push) = fake_lobby(Reply::Silent).await;
    let link = Link::new(1, 1, 10241, None);
    tokio::spawn(link.clone().run(address));
    let live = tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(message) = messages.recv().await {
            if matches!(message, Message::WorldLive { .. }) {
                return true;
            }
        }
        false
    })
    .await;
    assert_eq!(live, Ok(true));
    let world = World::new(vec![], Some(link), None);
    let shard = world.shard(0);
    let (out, _packets) = mpsc::channel(16);
    let open = Event::Open {
        conn: 1,
        address: 0,
        out,
    };
    shard.send(open).await.ok().unwrap();
    shard.send(Event::Closed { conn: 1 }).await.ok().unwrap();
    let seen = tokio::time::timeout(Duration::from_millis(300), messages.recv()).await;
    assert!(seen.is_err(), "unexpected message {seen:?}");
}

/// After a good return to lobby the close sends no LogoutNotify (spec 10 §26.5, start 1).
#[tokio::test]
async fn close_after_leave_sends_no_logout_notify() {
    let mut client = Client::start(Reply::Ok(TOKEN_KEY), Duration::from_secs(5)).await;
    client.send(ids::G_ReturnToLobbyReqCmd, &[]).await;
    assert_eq!(command(&client.next().await), ids::G_ReturnToLobbyOkCmd);
    let event = Event::Closed { conn: 1 };
    client.shard.send(event).await.ok().unwrap();
    assert_eq!(
        client.logout_notify_seen(Duration::from_millis(300)).await,
        None
    );
}

/// A client without a token auth has no user: log only, no reply.
#[tokio::test]
async fn rtl_without_user_no_reply() {
    let rules = request(ids::G_ContentsRuleListOkCmd, &[]);
    let world = World::new(vec![rules], None, None);
    let shard = world.shard(0);
    let (out, mut packets) = mpsc::channel(16);
    let open = Event::Open {
        conn: 1,
        address: 0,
        out,
    };
    shard.send(open).await.ok().unwrap();
    let packet = request(ids::G_ReturnToLobbyReqCmd, &[]);
    shard
        .send(Event::Packet { conn: 1, packet })
        .await
        .ok()
        .unwrap();
    // A later packet with a reply proves that the first one gave nothing.
    let packet = request(ids::G_ContentsRuleListReqCmd, &[]);
    shard
        .send(Event::Packet { conn: 1, packet })
        .await
        .ok()
        .unwrap();
    let first = tokio::time::timeout(Duration::from_secs(5), packets.recv()).await;
    let Ok(Some(ToConn::Packet(first))) = first else {
        panic!("no packet")
    };
    assert_eq!(command(&first), ids::G_ContentsRuleListOkCmd);
}

#[test]
fn rtl_arena_5() {
    let world = HandlerWorld {
        arena: true,
        ..HandlerWorld::default()
    };
    let mut services = MemoryServices {
        queue_result: Gate::Pass,
        ..MemoryServices::default()
    };
    let mut session = Session {
        authenticated: true,
        pc: Some(Pc {
            zone: Some(1),
            db_id: 7,
            world_id: 9,
            ..Pc::default()
        }),
        dispatch_permission: Some(true),
        ..Session::default()
    };
    let out = dispatch(
        &mut Context {
            session: &mut session,
            world: &world,
            services: &mut services,
        },
        ids::G_ReturnToLobbyReqCmd,
        &[],
    );
    let reply = &out.replies[0];
    assert_eq!(reply.command, ids::G_ReturnToLobbyFailCmd);
    assert_eq!(reply.body, 5u16.to_le_bytes());
    assert_eq!(services.logout_calls, 0);
}
