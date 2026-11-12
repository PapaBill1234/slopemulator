//! Shard tasks (`scale-rules.md` sections 3, 4 and 6).
//!
//! One task owns the sessions of one zone. Shard 0 is the entry shard: it owns a session
//! from Hello to command 42. No lock guards game state. Other tasks send [`Event`] values.
//!
//! Order rules (O5):
//! - A session that waits for a result (token, PC load) keeps its next packets in a queue.
//! - A move to another shard uses a handover: the old shard keeps the packets that come
//!   late and gives them to the new shard before the new shard reads direct packets.
//!
//! Full-queue policy (H6): client queue full → the session ends. Shard inbox full for a
//! message of another shard → the session ends. A connection task waits for the inbox.
use crate::{
    db::{Ordered, Pc},
    link::{Link, KICK_FAIL, KICK_NOT_FOUND},
    play::{self, Effect, Play},
};
use bytes::Bytes;
use std::{
    collections::{HashMap, VecDeque},
    io,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex, Weak,
    },
    time::Duration,
};
use tokio::{
    sync::{mpsc, oneshot},
    time::{interval, MissedTickBehavior},
};

/// Period of the simulation step. ❓ Local value (tier 3). The client does not see it.
pub const TICK: Duration = Duration::from_millis(100);
/// Events that a shard takes in one pass before it looks at the tick again.
const PASS: usize = 1024;
/// Events that can wait for one shard.
const INBOX: usize = 8192;
/// Packets of one session that can wait for a result or a handover.
const SESSION_QUEUE: usize = 64;
/// Time for the search of a PC to kick. LobbyDaemon waits 10 s for the reply. ❓ Local value.
const KICK_LIMIT: Duration = Duration::from_secs(5);

pub type Inbox = mpsc::Sender<Event>;

/// Message from a shard to the connection task of a client.
pub enum ToConn {
    /// One plain packet. The connection task encrypts and writes it.
    Packet(Bytes),
    /// Send the next client packets to this shard. Reply with [`Event::Switched`].
    Route(Inbox),
}

pub enum Event {
    /// A client finished Hello.
    Open {
        conn: u32,
        address: u32,
        out: mpsc::Sender<ToConn>,
    },
    /// One decrypted client packet.
    Packet { conn: u32, packet: Bytes },
    /// The connection ended.
    Closed { conn: u32 },
    /// Result of [`Effect::ConsumeToken`].
    Entered { conn: u32, result: Result<u64, i16> },
    /// Result of [`Effect::LoadPc`].
    Loaded { conn: u32, pc: Option<Box<Pc>> },
    /// Result of [`Effect::LeaveWorld`]: the lobby token key or a reason.
    Left { conn: u32, result: Result<u64, i16> },
    /// Another shard gives a session to this shard.
    Join { conn: u32, session: Box<Session> },
    /// The connection task got [`ToConn::Route`]. No more packets come to the old shard.
    Switched { conn: u32 },
    /// The old shard gives the packets that came after the move.
    HandoverDone { conn: u32, late: Vec<Bytes> },
    /// Link command 14: end the session of this PC. `reply` gets `true` if a shard found and
    /// dropped it. The connection task then closes the socket. No LogoutNotify goes out:
    /// LobbyDaemon ends its session itself after the reply. A shard whose session moved away
    /// forwards the event to the new shard.
    Kick {
        client: u64,
        pcid: u64,
        reply: oneshot::Sender<bool>,
    },
}

#[derive(Debug, PartialEq, Eq)]
enum Wait {
    None,
    /// For [`Event::Entered`], [`Event::Loaded`] or [`Event::Left`].
    Result,
    /// For [`Event::HandoverDone`].
    Handover,
}

pub struct Session {
    play: Play,
    out: mpsc::Sender<ToConn>,
    wait: Wait,
    queue: VecDeque<Bytes>,
}

struct Moved {
    to: Inbox,
    late: Vec<Bytes>,
    /// For a kick that comes while the session is on its way.
    pcid: Option<u64>,
    client_key: Option<u64>,
}

/// Read-only data and the shard directory. Shared by all shards.
pub struct World {
    /// Frames of command 930.
    pub rules: Vec<Bytes>,
    pub link: Option<Arc<Link>>,
    pub db: Option<Ordered>,
    /// Transaction ids of the leave requests (link command 8).
    transactions: AtomicU64,
    /// Directory only (zone → inbox). It holds no game state. A shard reads it at a zone change.
    shards: Mutex<HashMap<u32, Inbox>>,
}

impl World {
    pub fn new(rules: Vec<Bytes>, link: Option<Arc<Link>>, db: Option<Ordered>) -> Arc<Self> {
        Arc::new_cyclic(|weak: &Weak<World>| {
            if let Some(link) = &link {
                // The link holds a weak reference: no cycle.
                let weak = weak.clone();
                link.set_kick(Some(Arc::new(move |client, pcid| {
                    let weak = weak.clone();
                    Box::pin(async move {
                        match weak.upgrade() {
                            Some(world) => world.kick(client, pcid).await,
                            None => Err(KICK_FAIL),
                        }
                    })
                })));
            }
            Self {
                rules,
                link,
                db,
                transactions: AtomicU64::new(0),
                shards: Mutex::new(HashMap::new()),
            }
        })
    }

    /// Link command 14: end the session of the PC with this client key and PC id.
    /// `Err(24)`: no such PC in this world. `Err(2)`: a shard did not answer in time.
    /// The PC can be in transit between two shards, so the search has two passes.
    pub async fn kick(&self, client: u64, pcid: u64) -> Result<(), i16> {
        let ask = async {
            for _ in 0..2 {
                let inboxes: Vec<Inbox> = self
                    .shards
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .values()
                    .cloned()
                    .collect();
                for inbox in inboxes {
                    let (reply, found) = oneshot::channel();
                    let event = Event::Kick {
                        client,
                        pcid,
                        reply,
                    };
                    if inbox.send(event).await.is_ok() && found.await == Ok(true) {
                        return Ok(());
                    }
                }
            }
            Err(KICK_NOT_FOUND)
        };
        tokio::time::timeout(KICK_LIMIT, ask)
            .await
            .unwrap_or(Err(KICK_FAIL))
    }

    /// The inbox of the shard of `zone`. The first call for a zone starts its task.
    pub fn shard(self: &Arc<Self>, zone: u32) -> Inbox {
        let mut shards = self.shards.lock().unwrap_or_else(|e| e.into_inner());
        shards
            .entry(zone)
            .or_insert_with(|| {
                let (own, inbox) = mpsc::channel(INBOX);
                let shard = Shard {
                    zone,
                    own: own.clone(),
                    world: self.clone(),
                    sessions: HashMap::new(),
                    moved: HashMap::new(),
                    effects: Vec::new(),
                };
                tokio::spawn(shard.run(inbox));
                own
            })
            .clone()
    }
}

struct Shard {
    zone: u32,
    own: Inbox,
    world: Arc<World>,
    /// ❓ Local: map by connection id. Game objects of the simulation use dense storage (H10).
    sessions: HashMap<u32, Session>,
    /// Sessions that moved away and whose connection task did not confirm yet.
    moved: HashMap<u32, Moved>,
    effects: Vec<Effect>,
}

impl Shard {
    async fn run(mut self, mut inbox: mpsc::Receiver<Event>) {
        let mut tick = interval(TICK);
        tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                event = inbox.recv() => {
                    let Some(event) = event else { return };
                    self.event(event);
                    // Take what waits (rule G2). The replies of one pass go out in one write
                    // of each connection task.
                    for _ in 1..PASS {
                        let Ok(event) = inbox.try_recv() else { break };
                        self.event(event);
                    }
                }
                _ = tick.tick() => self.step(),
            }
        }
    }

    /// Simulation step. Empty: movement, skills and timers come with the handler port.
    fn step(&mut self) {}

    fn event(&mut self, event: Event) {
        match event {
            Event::Open { conn, address, out } => {
                let play = Play {
                    address,
                    ..Play::default()
                };
                let session = Session {
                    play,
                    out,
                    wait: Wait::None,
                    queue: VecDeque::new(),
                };
                self.sessions.insert(conn, session);
            }
            Event::Packet { conn, packet } => {
                if let Some(moved) = self.moved.get_mut(&conn) {
                    if moved.late.len() < SESSION_QUEUE {
                        moved.late.push(packet);
                    }
                    return;
                }
                let Some(session) = self.sessions.get_mut(&conn) else {
                    return;
                };
                if session.wait == Wait::None && session.queue.is_empty() {
                    self.run_step(conn, |play, rules, out| {
                        play::packet(play, rules, &packet, out)
                    });
                } else if session.queue.len() < SESSION_QUEUE {
                    session.queue.push_back(packet);
                } else {
                    eprintln!("gamed: client {conn}: too many packets wait, closed");
                    self.end(conn);
                }
            }
            Event::Closed { conn } => {
                if let Some(moved) = self.moved.remove(&conn) {
                    let late = moved.late;
                    let _ = moved.to.try_send(Event::HandoverDone { conn, late });
                    let _ = moved.to.try_send(Event::Closed { conn });
                }
                self.end(conn);
            }
            Event::Kick {
                client,
                pcid,
                reply,
            } => self.kick(client, pcid, reply),
            Event::Entered { conn, result } => {
                self.result(conn, |play, _, out| play::entered(play, result, out));
            }
            Event::Loaded { conn, pc } => {
                self.result(conn, |play, _, out| {
                    play::loaded(play, pc.map(|pc| *pc), out)
                });
            }
            Event::Left { conn, result } => {
                self.result(conn, |play, _, out| play::left(play, result, out));
            }
            Event::Join { conn, mut session } => {
                session.wait = Wait::Handover;
                self.sessions.insert(conn, *session);
                self.run_step(conn, |play, _, out| play::joined(play, out));
            }
            Event::Switched { conn } => {
                if let Some(moved) = self.moved.remove(&conn) {
                    let late = moved.late;
                    let _ = moved.to.try_send(Event::HandoverDone { conn, late });
                }
            }
            Event::HandoverDone { conn, late } => {
                let Some(session) = self.sessions.get_mut(&conn) else {
                    return;
                };
                if session.wait != Wait::Handover {
                    return;
                }
                for packet in late.into_iter().rev() {
                    session.queue.push_front(packet);
                }
                session.wait = Wait::None;
                self.drain(conn);
            }
        }
    }

    /// Drop the session of `conn`. The connection task then closes the socket. A PC that is in
    /// the world and did not leave by the return to lobby gets a LogoutNotify (link command 12).
    /// TODO ❓ The PC save is not in the port yet (`play.rs` step 3): LogoutNotify then goes
    /// out without a save.
    fn end(&mut self, conn: u32) {
        let Some(session) = self.sessions.remove(&conn) else {
            return;
        };
        let play = &session.play;
        let (Some(pcid), Some(client_key), false) = (play.pcid, play.client_key, play.logged_out)
        else {
            return;
        };
        if let Some(link) = &self.world.link {
            if let Err(error) = link.logout_notify(pcid, client_key) {
                eprintln!("gamed: client {conn}: LogoutNotify not sent: {error}");
            }
        }
    }

    /// Event 14 of the link: find the session, drop it, tell the caller.
    fn kick(&mut self, client: u64, pcid: u64, reply: oneshot::Sender<bool>) {
        let found = self
            .sessions
            .iter()
            .find(|(_, s)| s.play.pcid == Some(pcid) && s.play.client_key == Some(client))
            .map(|(conn, _)| *conn);
        if let Some(conn) = found {
            // LobbyDaemon ends its session after the reply: no LogoutNotify.
            if let Some(session) = self.sessions.get_mut(&conn) {
                session.play.logged_out = true;
            }
            self.end(conn);
            let _ = reply.send(true);
            return;
        }
        let moved = self
            .moved
            .values()
            .find(|m| m.pcid == Some(pcid) && m.client_key == Some(client));
        match moved {
            // A full inbox drops the event. The caller then sees "not found".
            Some(moved) => {
                let _ = moved.to.try_send(Event::Kick {
                    client,
                    pcid,
                    reply,
                });
            }
            None => {
                let _ = reply.send(false);
            }
        }
    }

    /// A result came: run it, then the packets that waited.
    fn result(
        &mut self,
        conn: u32,
        step: impl FnOnce(&mut Play, &[Bytes], &mut Vec<Effect>) -> io::Result<()>,
    ) {
        match self.sessions.get_mut(&conn) {
            Some(session) if session.wait == Wait::Result => session.wait = Wait::None,
            _ => return,
        }
        self.run_step(conn, step);
        self.drain(conn);
    }

    fn drain(&mut self, conn: u32) {
        loop {
            let Some(session) = self.sessions.get_mut(&conn) else {
                return;
            };
            if session.wait != Wait::None {
                return;
            }
            let Some(packet) = session.queue.pop_front() else {
                return;
            };
            self.run_step(conn, |play, rules, out| {
                play::packet(play, rules, &packet, out)
            });
        }
    }

    /// Run one pure step of a session, then its effects. An error ends the session.
    fn run_step(
        &mut self,
        conn: u32,
        step: impl FnOnce(&mut Play, &[Bytes], &mut Vec<Effect>) -> io::Result<()>,
    ) {
        let mut effects = std::mem::take(&mut self.effects);
        let Some(session) = self.sessions.get_mut(&conn) else {
            return;
        };
        let mut result = step(&mut session.play, &self.world.rules, &mut effects);
        let mut zone = None;
        // An effect without a link or a database gives its result at once. The list can grow.
        let mut index = 0;
        while result.is_ok() && index < effects.len() {
            let effect = std::mem::replace(&mut effects[index], Effect::EnterZone(0));
            index += 1;
            result = match effect {
                Effect::Send(packet) => session
                    .out
                    .try_send(ToConn::Packet(packet))
                    .map_err(|_| io::Error::other("client queue full or closed")),
                Effect::ConsumeToken { client_key, token } => match &self.world.link {
                    Some(link) => {
                        session.wait = Wait::Result;
                        let (link, own) = (link.clone(), self.own.clone());
                        let address = session.play.address;
                        // Login path, one time for each session. The link has the timeout.
                        tokio::spawn(async move {
                            let result = link.enter(client_key, token, address).await;
                            eprintln!("gamed: client {conn}: key {client_key}, result {result:?}");
                            let _ = own.send(Event::Entered { conn, result }).await;
                        });
                        Ok(())
                    }
                    None => play::entered(&mut session.play, Err(2), &mut effects),
                },
                Effect::LoadPc(id) => {
                    let own = self.own.clone();
                    let queued = self.world.db.as_ref().is_some_and(|db| {
                        db.submit(id, move |db| async move {
                            let pc = match db.load(id).await {
                                Ok(pc) => pc.map(Box::new),
                                Err(error) => {
                                    eprintln!("gamed: load of PC {id} failed: {}", error.kind());
                                    None
                                }
                            };
                            let _ = own.send(Event::Loaded { conn, pc }).await;
                        })
                    });
                    if queued {
                        session.wait = Wait::Result;
                        Ok(())
                    } else {
                        play::loaded(&mut session.play, None, &mut effects)
                    }
                }
                Effect::LeaveWorld { pcid, client_key } => match &self.world.link {
                    Some(link) => {
                        session.wait = Wait::Result;
                        let transaction =
                            self.world.transactions.fetch_add(1, Ordering::Relaxed) + 1;
                        let (link, own) = (link.clone(), self.own.clone());
                        // The link has the timeout (`quirk_rtl_no_timeout`, fixed). The shard
                        // does not wait.
                        tokio::spawn(async move {
                            let result = link.leave(transaction, pcid, client_key).await;
                            eprintln!("gamed: client {conn}: leave world, result {result:?}");
                            let _ = own.send(Event::Left { conn, result }).await;
                        });
                        Ok(())
                    }
                    None => play::left(
                        &mut session.play,
                        Err(crate::link::LEAVE_LINK_FAIL),
                        &mut effects,
                    ),
                },
                Effect::EnterZone(to) if to == self.zone => {
                    play::joined(&mut session.play, &mut effects)
                }
                Effect::EnterZone(to) => {
                    zone = Some(to);
                    Ok(())
                }
            };
        }
        effects.clear();
        self.effects = effects;
        if let Err(error) = result {
            eprintln!("gamed: client {conn}: {error}, closed");
            self.end(conn);
        } else if let Some(zone) = zone {
            self.leave(conn, zone);
        }
    }

    /// Give the session to the shard of `zone`. Order: `Join` first, then the new route.
    fn leave(&mut self, conn: u32, zone: u32) {
        let Some(mut session) = self.sessions.remove(&conn) else {
            return;
        };
        let to = self.world.shard(zone);
        let late = session.queue.drain(..).collect();
        let out = session.out.clone();
        let (pcid, client_key) = (session.play.pcid, session.play.client_key);
        let session = Box::new(session);
        match to.try_send(Event::Join { conn, session }) {
            Ok(()) => {}
            Err(
                mpsc::error::TrySendError::Full(Event::Join { session, .. })
                | mpsc::error::TrySendError::Closed(Event::Join { session, .. }),
            ) => {
                // The session ends here. The connection task then closes the socket.
                eprintln!("gamed: client {conn}: move to zone {zone} failed, closed");
                self.sessions.insert(conn, *session);
                self.end(conn);
                return;
            }
            Err(_) => return,
        }
        if out.try_send(ToConn::Route(to.clone())).is_err() {
            // The new shard has the session: its `Closed` event ends it there. The `moved`
            // entry stays, so the `Closed` of the connection task goes on to the new shard.
            eprintln!("gamed: client {conn}: move to zone {zone} failed, closed");
            let _ = to.try_send(Event::Closed { conn });
        }
        self.moved.insert(
            conn,
            Moved {
                to,
                late,
                pcid,
                client_key,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::fsd::{MakePc, APPEARANCE_BYTES};

    fn request(cmd: i16, body: &[u8]) -> Bytes {
        let mut packet = ((body.len() + 4) as i16).to_le_bytes().to_vec();
        packet.extend(cmd.to_le_bytes());
        packet.extend(body);
        packet.into()
    }

    async fn command(inbox: &mut mpsc::Receiver<ToConn>) -> i16 {
        match inbox.recv().await {
            Some(ToConn::Packet(packet)) => i16::from_le_bytes([packet[2], packet[3]]),
            _ => panic!("not a packet"),
        }
    }

    /// No link and no database: results come at once, in the order of the requests.
    #[tokio::test]
    async fn entry_shard_keeps_order() {
        let world = World::new(vec![request(930, &[])], None, None);
        let shard = world.shard(0);
        let (out, mut inbox) = mpsc::channel(16);
        let conn = 1;
        let open = Event::Open {
            conn,
            address: 0,
            out,
        };
        shard.send(open).await.ok().unwrap();
        let mut spawn = crate::entry::SPAWN_KEY.to_le_bytes().to_vec();
        spawn.extend([0; 14]);
        for packet in [
            request(23, &[0; 18]),
            request(929, &[]),
            request(31, &[0; 5]),
            request(42, &spawn),
        ] {
            shard
                .send(Event::Packet { conn, packet })
                .await
                .ok()
                .unwrap();
        }
        assert_eq!(command(&mut inbox).await, 24);
        assert_eq!(command(&mut inbox).await, 930);
        // 31 without a PC: no reply. 42: zone 0 is this shard.
        assert_eq!(command(&mut inbox).await, 43);
    }

    /// A zone change: packets that reach the old shard late run before direct packets.
    #[tokio::test]
    async fn handover_keeps_order() {
        let world = World::new(vec![request(930, &[])], None, None);
        let old = world.shard(0);
        let (out, mut inbox) = mpsc::channel(16);
        let conn = 7;
        let pc = Pc {
            zone: 1101,
            position: [0; 3],
            yaw: 0,
            level: 1,
            hp: 1,
            pcid: 5,
            pc: MakePc {
                race: 1,
                sex: 1,
                job: 1,
                appearance: [0; APPEARANCE_BYTES],
            },
            name: Vec::new(),
        };
        let play = Play {
            pc: Some(pc),
            ..Play::default()
        };
        let session = Box::new(Session {
            play,
            out,
            wait: Wait::None,
            queue: VecDeque::new(),
        });
        old.send(Event::Join { conn, session }).await.ok().unwrap();
        let mut spawn = crate::entry::SPAWN_KEY.to_le_bytes().to_vec();
        spawn.extend([0; 14]);
        let late = vec![request(42, &spawn)];
        old.send(Event::HandoverDone { conn, late })
            .await
            .ok()
            .unwrap();
        assert_eq!(command(&mut inbox).await, 43);
        let Some(ToConn::Route(new)) = inbox.recv().await else {
            panic!("no route")
        };
        assert!(!new.same_channel(&old));
        // The direct packet reaches the new shard before the late packet does.
        new.send(Event::Packet {
            conn,
            packet: request(929, &[]),
        })
        .await
        .ok()
        .unwrap();
        old.send(Event::Packet {
            conn,
            packet: request(23, &[]),
        })
        .await
        .ok()
        .unwrap();
        old.send(Event::Switched { conn }).await.ok().unwrap();
        assert_eq!(command(&mut inbox).await, 43);
        assert_eq!(command(&mut inbox).await, 24);
        assert_eq!(command(&mut inbox).await, 930);
    }
}
