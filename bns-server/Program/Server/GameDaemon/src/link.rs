//! GameDaemon link, GameDaemon side. Spec: Docs/Server/LobbyDaemon/07-gamed-link.md.
//! Dial LobbyDaemon; Hello 1 → 2; 4 (world live); serve create character 20 → 21 or 22;
//! send world entry 5 and wait for 6 or 7.
//! Create handler: 381 `HandleLobbyGame_CreateCharacterReq` `0x14013F0F0`,
//! `CreateCharacterTran` init `0x14006F3D0`, reply 21 `0x140149920`.
//! ❓ Local: the dial is repeated each 2 s. The original repeat time is not verified.
//! ❓ Not done: appearance range checks (tables 687, 675), creatable-job and
//! race-sex-job records, jumping characters.
use crate::db::{Db, NewPc};
use packet::gamed_link::{Message, PACKET_VERSION};
use shared::{
    fsd::{compose_new_showcase, decode_make_pc},
    net::{self, FrameReader, Outbox},
};
use std::{
    collections::HashMap,
    future::Future,
    io,
    pin::Pin,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::{io::AsyncWriteExt, net::TcpStream, sync::oneshot};

/// Start zone of each level-1 `PcInitialSetting` record: geo zone 1101.
/// Spawn `1101_SP1` (`zonepcspawndata.1101.xml`): position and yaw. ❓ Units.
pub const START_ZONE: u32 = 1101;
pub const START_POSITION: [i16; 3] = [-3199, 9241, 599];
pub const START_YAW: u16 = 45;
/// ❓ HP of a level-1 PC. Source: local note; the pc data record (table 2007) is not read.
pub const START_HP: u64 = 303;
const LIMIT: Duration = Duration::from_secs(10);
/// ❓ Local choice: wait time for LeaveWorldOk or LeaveWorldFail (`quirk_rtl_no_timeout`, fixed).
const LEAVE_LIMIT_MS: u64 = 10_000;
/// Reason for a link failure or timeout of the leave request (GameDaemon only).
pub const LEAVE_LINK_FAIL: i16 = 1952;

/// Reason of KickFail (16): no PC with this key and PC id is in this world. LobbyDaemon maps it
/// to kick result 1 (`18-jobs-f.md` §5).
pub const KICK_NOT_FOUND: i16 = 24;
/// Reason of KickFail for a failure of GameDaemon (LobbyDaemon result 2). ❓ Local value.
pub const KICK_FAIL: i16 = 2;

type Pending = oneshot::Sender<Result<u64, i16>>;

/// Disconnects the PC with this client key and PC id. `Err` is the KickFail reason.
pub type KickHook =
    Arc<dyn Fn(u64, u64) -> Pin<Box<dyn Future<Output = Result<(), i16>> + Send>> + Send + Sync>;

pub struct Link {
    cluster: i16,
    world: i16,
    port: i16,
    db: Option<Arc<Db>>,
    sender: Mutex<Option<Outbox>>,
    pending: Mutex<HashMap<u64, Pending>>,
    /// Leave requests (8) that wait for 9 or 10, by transaction.
    leaves: Mutex<HashMap<u64, Pending>>,
    leave_limit_ms: AtomicU64,
    next: AtomicU64,
    kick: Mutex<Option<KickHook>>,
}

impl Link {
    pub fn new(cluster: i16, world: i16, port: i16, db: Option<Arc<Db>>) -> Arc<Self> {
        Arc::new(Self {
            cluster,
            world,
            port,
            db,
            sender: Mutex::new(None),
            pending: Mutex::new(HashMap::new()),
            leaves: Mutex::new(HashMap::new()),
            leave_limit_ms: AtomicU64::new(LEAVE_LIMIT_MS),
            next: AtomicU64::new(0),
            kick: Mutex::new(None),
        })
    }

    /// Keep the link to `lobby` up. This future does not end.
    pub async fn run(self: Arc<Self>, lobby: String) {
        loop {
            let result = self.clone().session(&lobby).await;
            *self.sender.lock().unwrap() = None;
            self.pending.lock().unwrap().clear();
            self.leaves.lock().unwrap().clear();
            eprintln!("gamed: lobbyd link closed: {result:?}");
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }

    fn send(&self, message: Message) -> bool {
        let Ok(frame) = message.encode() else {
            return false;
        };
        self.sender
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|sender| sender.try_send(frame).is_ok())
    }

    async fn session(self: Arc<Self>, lobby: &str) -> io::Result<()> {
        let stream = TcpStream::connect(lobby).await?;
        net::tune(&stream)?;
        let (reader, mut writer) = stream.into_split();
        let hello = Message::Hello {
            cluster: self.cluster,
            world: self.world,
            version: PACKET_VERSION,
            port: self.port,
        };
        writer
            .write_all(&hello.encode().map_err(io::Error::other)?)
            .await?;
        let mut reader = FrameReader::new(reader, usize::from(u16::MAX));
        // A full queue fails the send, as a closed link does.
        let (sender, _writer_task) = net::spawn_writer(writer, net::LINK_QUEUE);
        loop {
            let Some(frame) = reader.next().await? else {
                return Err(io::ErrorKind::UnexpectedEof.into());
            };
            let message = Message::decode(&frame)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
            match message {
                Some(Message::HelloOk) => {
                    *self.sender.lock().unwrap() = Some(sender.clone());
                    self.send(Message::WorldLive {
                        cluster: self.cluster,
                        world: self.world,
                    });
                    eprintln!("gamed: lobbyd link up, world {} is live", self.world);
                }
                Some(Message::HelloFail(reason)) => {
                    return Err(io::Error::other(format!("hello refused, reason {reason}")));
                }
                Some(Message::EnterOk {
                    transaction, pcid, ..
                }) => self.finish(transaction, Ok(pcid)),
                Some(Message::EnterFail {
                    transaction,
                    reason,
                }) => self.finish(transaction, Err(reason)),
                Some(Message::LeaveOk { transaction, token }) => {
                    self.finish_leave(transaction, Ok(token));
                }
                Some(Message::LeaveFail {
                    transaction,
                    reason,
                }) => self.finish_leave(transaction, Err(reason)),
                Some(Message::Kick {
                    transaction,
                    client,
                    pcid,
                    ..
                }) => self.start_kick(transaction, client, pcid),
                Some(create @ Message::Create { .. }) => {
                    let link = self.clone();
                    tokio::spawn(async move {
                        let reply = link.create(create).await;
                        eprintln!(
                            "gamed: [lobbyd channel] create character reply {}",
                            match &reply {
                                Message::CreateOk { job, level, .. } =>
                                    format!("21, job {job} level {level}"),
                                Message::CreateFail { reason, .. } =>
                                    format!("22, reason {reason}"),
                                _ => String::new(),
                            }
                        );
                        link.send(reply);
                    });
                }
                _ => {}
            }
        }
    }

    fn finish(&self, transaction: u64, result: Result<u64, i16>) {
        if let Some(reply) = self.pending.lock().unwrap().remove(&transaction) {
            let _ = reply.send(result);
        }
    }

    fn finish_leave(&self, transaction: u64, result: Result<u64, i16>) {
        if let Some(reply) = self.leaves.lock().unwrap().remove(&transaction) {
            let _ = reply.send(result);
        }
    }

    /// Set the wait time for the reply to a leave request.
    pub fn set_leave_limit(&self, limit: Duration) {
        let ms = u64::try_from(limit.as_millis()).unwrap_or(u64::MAX);
        self.leave_limit_ms.store(ms, Ordering::Relaxed);
    }

    /// Command 20. Reasons: 25 invalid race, sex or job; 2 data or store failure.
    async fn create(&self, message: Message) -> Message {
        let Message::Create {
            transaction,
            pcid,
            account,
            name,
            make_pc,
            ..
        } = message
        else {
            unreachable!()
        };
        let fail = |reason| Message::CreateFail {
            transaction,
            reason,
        };
        // ❓ The original sends 22 for a bad record; the reason argument is not verified.
        let Some(pc) = decode_make_pc(&make_pc) else {
            return fail(2);
        };
        if pc.race > 11 || pc.job > 19 || pc.sex > 3 {
            return fail(25);
        }
        let Some(db) = &self.db else {
            eprintln!("gamed: create character, no game DB");
            return fail(2);
        };
        let new = NewPc {
            pcid,
            account,
            world: self.world,
            pc: &pc,
            name: &name,
            zone: START_ZONE,
            position: START_POSITION,
            yaw: START_YAW,
            level: 1,
            hp: START_HP,
        };
        match db.create(&new).await {
            Ok(true) => Message::CreateOk {
                transaction,
                job: pc.job.into(),
                level: 1,
                showcase: compose_new_showcase(&pc, &name, 1),
            },
            Ok(false) => {
                eprintln!("gamed: create character {pcid}, the procedure failed");
                fail(1)
            }
            Err(error) => {
                eprintln!("gamed: create character {pcid}, game DB error: {error}");
                fail(2)
            }
        }
    }

    /// Command 5: consume the world-entry token. `Ok` is the PC id.
    pub async fn enter(&self, client_key: u64, token: u64, address: u32) -> Result<u64, i16> {
        let transaction = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        let (reply, result) = oneshot::channel();
        self.pending.lock().unwrap().insert(transaction, reply);
        let sent = self.send(Message::Enter {
            transaction,
            client_key,
            token,
            address,
        });
        let result = if sent {
            match tokio::time::timeout(LIMIT, result).await {
                Ok(Ok(result)) => result,
                _ => Err(2),
            }
        } else {
            Err(2)
        };
        self.pending.lock().unwrap().remove(&transaction);
        result
    }

    /// Command 8: ask LobbyDaemon to take the PC out of the world and issue a lobby token.
    /// `Ok` is the token key. `Err(r)` is the LobbyDaemon reason (2, 5, 6, 21, 22, 23).
    /// A send failure, a closed link or a timeout give `Err(1952)` (`quirk_rtl_no_timeout`, fixed).
    pub async fn leave(&self, transaction: u64, pc_db_id: u64, cli_key: u64) -> Result<u64, i16> {
        let (reply, result) = oneshot::channel();
        self.leaves.lock().unwrap().insert(transaction, reply);
        let sent = self.send(Message::Leave {
            transaction,
            pc_db_id,
            cli_key,
        });
        let limit = Duration::from_millis(self.leave_limit_ms.load(Ordering::Relaxed));
        let result = if sent {
            match tokio::time::timeout(limit, result).await {
                Ok(Ok(result)) => result,
                _ => Err(LEAVE_LINK_FAIL),
            }
        } else {
            Err(LEAVE_LINK_FAIL)
        };
        self.leaves.lock().unwrap().remove(&transaction);
        result
    }

    /// Command 12: tell LobbyDaemon that the client left the world (socket closed or logout).
    /// Does not wait. A full link queue does not lose the message: a task waits for room.
    /// No link gives `NotConnected` (LobbyDaemon then ends the session at the link close).
    pub fn logout_notify(&self, pc_db_id: u64, cli_key: u64) -> io::Result<()> {
        let frame = Message::LogoutNotify { pc_db_id, cli_key }
            .encode()
            .map_err(io::Error::other)?;
        let Some(sender) = self.sender.lock().unwrap().clone() else {
            return Err(io::ErrorKind::NotConnected.into());
        };
        match sender.try_send(frame.clone()) {
            Ok(()) => Ok(()),
            Err(net::SendError::Closed) => Err(io::ErrorKind::NotConnected.into()),
            Err(net::SendError::Full) => {
                tokio::spawn(async move {
                    let _ = sender.send_wait(frame).await;
                });
                Ok(())
            }
        }
    }

    /// Set the function that disconnects a PC for command 14. `None` removes it.
    pub fn set_kick(&self, hook: Option<KickHook>) {
        *self.kick.lock().unwrap() = hook;
    }

    /// Command 14: disconnect the PC, then reply 15 or 16. A task runs it: the link reader
    /// does not wait.
    fn start_kick(self: &Arc<Self>, transaction: u64, client: u64, pcid: u64) {
        let link = self.clone();
        tokio::spawn(async move {
            let hook = link.kick.lock().unwrap().clone();
            let result = match hook {
                Some(hook) => hook(client, pcid).await,
                None => Err(KICK_FAIL),
            };
            eprintln!("gamed: kick of PC {pcid}, key {client}: {result:?}");
            link.send(match result {
                Ok(()) => Message::KickOk { transaction },
                Err(reason) => Message::KickFail {
                    transaction,
                    reason,
                },
            });
        });
    }
}
