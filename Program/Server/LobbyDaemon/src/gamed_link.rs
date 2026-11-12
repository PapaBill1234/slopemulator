//! GameDaemon link, LobbyDaemon side. Spec: Docs/Server/LobbyDaemon/07-gamed-link.md.
//! The GameDaemon dials this listener. Hello 1 → 2 or 3; 4 sets the world live;
//! create character 20 → 21 or 22; world entry 5 → 6 or 7; return to lobby 8 → 9 or 10;
//! 12 LogoutNotify only logs (❓ lp2-4-rust).
//! Hello must come from a configured GameDaemon (`LinkPeer`): world, cluster, peer address.
//! One channel for each world.
//! ❓ Local: delete, restore, eligibility and admission have no link message here.
//! ❓ Local: the transaction time limit is 10 s. The original value is not verified.
use crate::{
    character::{CreateCharacter, CreatedCharacter},
    gamed::{Admission, Eligibility, GameDaemon},
    store::Character,
};
use packet::gamed_link::{Message, PACKET_VERSION};
use shared::net::{self, FrameReader, Outbox};
use shared::np_channel::{ChargeMode, GameFuture};
use std::{
    collections::HashMap,
    future::Future,
    io,
    net::IpAddr,
    pin::Pin,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::oneshot,
};

const TRANSACTION_LIMIT: Duration = Duration::from_secs(10);

/// World-entry token check: client key, token key, address, world → PC id or reason.
pub type Consume = Arc<
    dyn Fn(u64, u64, u32, i16) -> Pin<Box<dyn Future<Output = Result<u64, i16>> + Send>>
        + Send
        + Sync,
>;

/// Return to lobby: PC id, client key, world → lobby token key or LeaveWorldFail reason.
/// `Server::leave_world` is the usual implementation.
pub type Leave = Arc<
    dyn Fn(u64, u64, i16) -> Pin<Box<dyn Future<Output = Result<u64, i16>> + Send>> + Send + Sync,
>;

/// Create-character transaction: world of the GameDaemon that must answer, reply slot.
type Pending = (i16, oneshot::Sender<Result<CreatedCharacter, i16>>);

/// One configured GameDaemon (`gamed-list.config.xml` row in the original: wid, ip-address).
/// Hello must match the world, the cluster and the peer address (`0x140086D50 -> 0x14003F920`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkPeer {
    pub world: i16,
    pub cluster: i16,
    pub address: IpAddr,
}

/// HelloFail reason for an unknown world, a wrong cluster or a wrong peer address.
/// ❓ The original reason for each identity check is not verified; 1 is the shared check failure.
const HELLO_FAIL_IDENTITY: i16 = 1;
/// HelloFail reason when the world already has a channel (registration failure, `0x140086D50`).
const HELLO_FAIL_REGISTER: i16 = 12;

#[derive(Default)]
pub struct LinkGameDaemon {
    /// Configured GameDaemons. Read-only after `new`. Empty: every Hello fails.
    peers: Vec<LinkPeer>,
    /// Leave-world hook. None: every LeaveWorldReq gets LeaveWorldFail reason 2.
    leave: Mutex<Option<Leave>>,
    /// Worlds with a channel after Hello. One connection for each world.
    channels: Mutex<HashMap<i16, Outbox>>,
    /// Live worlds: world id → frame queue of its GameDaemon.
    worlds: Mutex<HashMap<i16, Outbox>>,
    pending: Mutex<HashMap<u64, Pending>>,
    next: AtomicU64,
    /// PC id → account id, for reply 6.
    accounts: Mutex<HashMap<u64, [u8; 16]>>,
}

impl LinkGameDaemon {
    /// A link that accepts Hello only from the given GameDaemons.
    pub fn new(peers: Vec<LinkPeer>) -> Self {
        Self {
            peers,
            ..Self::default()
        }
    }

    /// Hello identity check: world, cluster and peer address must match one configured row.
    fn known(&self, world: i16, cluster: i16, address: IpAddr) -> bool {
        self.peers.iter().any(|p| {
            p.world == world && p.cluster == cluster && p.address == address.to_canonical()
        })
    }

    pub fn live(&self, world: i16) -> bool {
        self.worlds.lock().unwrap().contains_key(&world)
    }

    /// Set the hook for LeaveWorldReq (message 8). `main.rs` sets `Server::leave_world`.
    pub fn set_leave(&self, leave: Leave) {
        *self.leave.lock().unwrap() = Some(leave);
    }

    pub async fn serve(self: Arc<Self>, listener: TcpListener, consume: Consume) -> io::Result<()> {
        loop {
            let (stream, peer) = listener.accept().await?;
            let (link, consume) = (self.clone(), consume.clone());
            tokio::spawn(async move {
                let result = link.connection(stream, peer.ip(), consume).await;
                eprintln!("lobbyd: gamed link {peer} closed: {result:?}");
            });
        }
    }

    async fn connection(
        self: Arc<Self>,
        stream: TcpStream,
        peer: IpAddr,
        consume: Consume,
    ) -> io::Result<()> {
        net::tune(&stream)?;
        let (reader, writer) = stream.into_split();
        // Link frames have a 16-bit size and no 4000-byte limit (the showcase blob is large).
        let mut reader = FrameReader::new(reader, usize::from(u16::MAX));
        // A full queue fails the request, as a closed link does.
        let (sender, writer_task) = net::spawn_writer(writer, net::LINK_QUEUE);
        let mut hello: Option<i16> = None;
        let mut live: Option<i16> = None;
        let result = async {
            while let Some(frame) = reader.next().await? {
                let message = Message::decode(&frame)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
                let send = |message: Message| {
                    if let Ok(frame) = message.encode() {
                        let _ = sender.try_send(frame);
                    }
                };
                match message {
                    Some(Message::Hello {
                        cluster,
                        world,
                        version,
                        port,
                    }) => {
                        eprintln!("lobbyd: gamed link hello from {peer}, cluster {cluster} world {world} version {version} port {port}");
                        // ❓ The Hello internal port is not checked: the port has no
                        // configured internal port for each GameDaemon.
                        if hello.is_some() {
                            // A second Hello on one channel: close the channel.
                            return Err(io::Error::other("second hello on one channel"));
                        }
                        if version != PACKET_VERSION {
                            send(Message::HelloFail(12));
                            continue;
                        }
                        // `exploit_gamed_link_trusted`: only a configured GameDaemon can
                        // open a channel. The original checks this too (`0x140086D50`).
                        if !self.known(world, cluster, peer) {
                            eprintln!("lobbyd: gamed link hello from {peer} refused: world {world} cluster {cluster} not configured for this address");
                            send(Message::HelloFail(HELLO_FAIL_IDENTITY));
                            return Err(io::Error::other("hello identity mismatch"));
                        }
                        // One channel for each world. A second GameDaemon cannot take over
                        // the world of the first one.
                        let registered = {
                            let mut channels = self.channels.lock().unwrap();
                            match channels.get(&world) {
                                Some(_) => false,
                                None => {
                                    channels.insert(world, sender.clone());
                                    true
                                }
                            }
                        };
                        if !registered {
                            eprintln!("lobbyd: gamed link hello from {peer} refused: world {world} has a channel");
                            send(Message::HelloFail(HELLO_FAIL_REGISTER));
                            return Err(io::Error::other("world already has a channel"));
                        }
                        hello = Some(world);
                        send(Message::HelloOk);
                    }
                    Some(Message::WorldLive { world, cluster }) if hello == Some(world) => {
                        if live.is_some() || !self.known(world, cluster, peer) {
                            // Duplicate service open or a changed cluster: close the channel
                            // (`0x1400877B0`).
                            return Err(io::Error::other("bad service open"));
                        }
                        self.worlds.lock().unwrap().insert(world, sender.clone());
                        live = Some(world);
                        eprintln!("lobbyd: gamed link, world {world} is live");
                    }
                    Some(Message::Enter {
                        transaction,
                        client_key,
                        token,
                        address,
                    }) => {
                        let Some(world) = live else { continue };
                        let reply = match consume(client_key, token, address, world).await {
                            Ok(pcid) => Message::EnterOk {
                                transaction,
                                pcid,
                                account: self
                                    .accounts
                                    .lock()
                                    .unwrap()
                                    .get(&pcid)
                                    .copied()
                                    .unwrap_or_default(),
                            },
                            Err(reason) => Message::EnterFail {
                                transaction,
                                reason,
                            },
                        };
                        eprintln!("lobbyd: gamed link, world entry key {client_key}: {reply:?}");
                        send(reply);
                    }
                    // 8 LeaveWorldReq (`23-return-to-lobby.md` §3). The reply is 9 or 10.
                    // A job start failure is a 10 too, not a 7 (`quirk_leave_fail_as_enter_fail`).
                    Some(Message::Leave {
                        transaction,
                        pc_db_id,
                        cli_key,
                    }) => {
                        let leave = self.leave.lock().unwrap().clone();
                        let (sender, world) = (sender.clone(), live);
                        // The reader goes on while the job runs.
                        tokio::spawn(async move {
                            let result = match (leave, world) {
                                (Some(leave), Some(world)) => leave(pc_db_id, cli_key, world).await,
                                _ => Err(2),
                            };
                            let reply = match result {
                                Ok(token) => Message::LeaveOk { transaction, token },
                                Err(reason) => Message::LeaveFail {
                                    transaction,
                                    reason,
                                },
                            };
                            eprintln!("lobbyd: gamed link, leave world key {cli_key}: {reply:?}");
                            if let Ok(frame) = reply.encode() {
                                let _ = sender.try_send(frame);
                            }
                        });
                    }
                    // 12 LogoutNotify. ❓ The logout job for a session in the game world
                    // (task 118) is not ported: `Jobs::logout` ignores state 4.0.
                    // TODO ❓ lp2-4-rust: run the logout path for `cli_key`.
                    Some(Message::LogoutNotify { pc_db_id, cli_key }) => {
                        eprintln!(
                            "lobbyd: gamed link, logout notify pc {pc_db_id} key {cli_key}: not ported yet"
                        );
                    }
                    Some(Message::CreateOk {
                        transaction,
                        job,
                        level,
                        showcase,
                    }) => self.finish(
                        live,
                        transaction,
                        Ok(CreatedCharacter {
                            job,
                            level,
                            showcase,
                        }),
                    ),
                    Some(Message::CreateFail {
                        transaction,
                        reason,
                    }) => self.finish(live, transaction, Err(reason)),
                    _ => {}
                }
            }
            Ok(())
        }
        .await;
        if let Some(world) = live {
            let mut worlds = self.worlds.lock().unwrap();
            if worlds.get(&world).is_some_and(|s| s.same_channel(&sender)) {
                worlds.remove(&world);
            }
        }
        if let Some(world) = hello {
            let mut channels = self.channels.lock().unwrap();
            if channels
                .get(&world)
                .is_some_and(|s| s.same_channel(&sender))
            {
                channels.remove(&world);
            }
        }
        drop(sender);
        let _ = writer_task.await;
        result
    }

    /// Complete a create-character transaction. Only the channel of the world that got
    /// the request can complete it; a reply from another world is ignored.
    fn finish(&self, world: Option<i16>, transaction: u64, result: Result<CreatedCharacter, i16>) {
        let mut pending = self.pending.lock().unwrap();
        if pending
            .get(&transaction)
            .is_some_and(|(w, _)| Some(*w) == world)
        {
            if let Some((_, reply)) = pending.remove(&transaction) {
                let _ = reply.send(result);
            }
        }
    }
}

impl GameDaemon for LinkGameDaemon {
    fn create_character(
        &self,
        request: CreateCharacter,
        pcid: u64,
        appearance: Vec<u8>,
    ) -> GameFuture<'_, CreatedCharacter> {
        Box::pin(async move {
            // No live GameDaemon for the world: reason 2.
            let sender = self
                .worlds
                .lock()
                .unwrap()
                .get(&request.world)
                .cloned()
                .ok_or(2i16)?;
            let transaction = self.next.fetch_add(1, Ordering::Relaxed) + 1;
            let frame = Message::Create {
                transaction,
                pcid,
                account: request.account,
                external: Vec::new(),
                address: 0,
                name: request.name,
                make_pc: appearance,
                jumping: false,
                top_level: 0,
            }
            .encode()
            .map_err(|_| 2i16)?;
            let (reply, result) = oneshot::channel();
            self.pending
                .lock()
                .unwrap()
                .insert(transaction, (request.world, reply));
            let result = if sender.try_send(frame).is_err() {
                Err(2)
            } else {
                match tokio::time::timeout(TRANSACTION_LIMIT, result).await {
                    Ok(Ok(result)) => result,
                    _ => Err(2),
                }
            };
            self.pending.lock().unwrap().remove(&transaction);
            if result.is_ok() {
                self.accounts.lock().unwrap().insert(pcid, request.account);
            }
            result
        })
    }
    fn delete_character(&self, _world: i16, _pcid: u64) -> GameFuture<'_, i16> {
        Box::pin(async { Ok(1) })
    }
    fn restore_character(&self, _world: i16, _pcid: u64) -> GameFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
    fn eligibility(&self, _character: &Character) -> Result<Eligibility, i16> {
        Ok(Eligibility::Ready)
    }
    fn restore(&self, _world: i16, _pcid: u64) -> GameFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
    fn admit(
        &self,
        _owner: u64,
        world: i16,
        _pcid: u64,
        _charge: ChargeMode,
    ) -> GameFuture<'_, Admission> {
        // The world must be live. ❓ Local: no wait queue.
        let live = self.live(world);
        Box::pin(async move {
            if live {
                Ok(Admission::new(None, || {}))
            } else {
                Err(2)
            }
        })
    }
}
