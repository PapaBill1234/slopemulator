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
use shared::fsd::{compose_new_showcase, decode_make_pc};
use std::{
    collections::HashMap,
    io,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::{mpsc, oneshot},
};

/// Start zone of each level-1 `PcInitialSetting` record: geo zone 1101.
/// Spawn `1101_SP1` (`zonepcspawndata.1101.xml`): position and yaw. ❓ Units.
pub const START_ZONE: u32 = 1101;
pub const START_POSITION: [i16; 3] = [-3199, 9241, 599];
pub const START_YAW: u16 = 45;
/// ❓ HP of a level-1 PC. Source: local note; the pc data record (table 2007) is not read.
pub const START_HP: u64 = 303;
const LIMIT: Duration = Duration::from_secs(10);

type Pending = oneshot::Sender<Result<u64, i16>>;

pub struct Link {
    cluster: i16,
    world: i16,
    port: i16,
    db: Option<Arc<Db>>,
    sender: Mutex<Option<mpsc::UnboundedSender<Vec<u8>>>>,
    pending: Mutex<HashMap<u64, Pending>>,
    next: AtomicU64,
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
            next: AtomicU64::new(0),
        })
    }

    /// Keep the link to `lobby` up. This future does not end.
    pub async fn run(self: Arc<Self>, lobby: String) {
        loop {
            let result = self.clone().session(&lobby).await;
            *self.sender.lock().unwrap() = None;
            self.pending.lock().unwrap().clear();
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
            .is_some_and(|sender| sender.send(frame).is_ok())
    }

    async fn session(self: Arc<Self>, lobby: &str) -> io::Result<()> {
        let stream = TcpStream::connect(lobby).await?;
        stream.set_nodelay(true)?;
        let (mut reader, mut writer) = stream.into_split();
        let hello = Message::Hello {
            cluster: self.cluster,
            world: self.world,
            version: PACKET_VERSION,
            port: self.port,
        };
        writer
            .write_all(&hello.encode().map_err(io::Error::other)?)
            .await?;
        let (sender, mut queue) = mpsc::unbounded_channel::<Vec<u8>>();
        tokio::spawn(async move {
            while let Some(frame) = queue.recv().await {
                if writer.write_all(&frame).await.is_err() {
                    break;
                }
            }
        });
        loop {
            let mut head = [0u8; 2];
            reader.read_exact(&mut head).await?;
            let size = usize::from(u16::from_le_bytes(head));
            if size < 4 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "link frame size",
                ));
            }
            let mut frame = vec![0u8; size];
            frame[..2].copy_from_slice(&head);
            reader.read_exact(&mut frame[2..]).await?;
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
}
