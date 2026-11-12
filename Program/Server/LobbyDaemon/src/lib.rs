//! Offline LobbyDaemon. One task for each LobbyGate link owns the job state of that link.
//! Slow commands run in job tasks on a private copy of one session (scale rules §10 step 6).
pub mod character;
pub mod gamed;
pub mod gamed_link;
pub mod ids;
pub mod jobs;
#[cfg(feature = "mssql")]
pub mod mssql;
pub mod np;
pub mod protocol;
pub mod scheduler;
pub mod store;

use jobs::{Jobs, LinkId, Output, Settings, State};
use np::Np;
use std::{
    collections::{HashMap, VecDeque},
    io,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex as SyncMutex,
    },
    time::Duration,
};
use store::{AccountId, Store};
use tokio::{
    io::AsyncWriteExt,
    net::{tcp::OwnedWriteHalf, TcpListener, TcpStream},
    sync::{mpsc, oneshot, watch, Semaphore},
    task::JoinSet,
};

/// Items that can wait for one busy session of a link (rule 2 of step 6).
const MAX_QUEUED: usize = 64;
/// Control messages that can wait for one link task.
const CONTROL_QUEUE: usize = 64;
/// Last-logout batches that can wait for the writer of one link.
const LOGOUT_QUEUE: usize = 64;

/// A Server API call for one session. The link task that owns the session runs it.
enum Control {
    ConsumeWorld {
        key: u64,
        token: u64,
        ipv4: u32,
        world: i16,
        reply: oneshot::Sender<Result<u64, i16>>,
    },
    PromoteWorld {
        key: u64,
        reply: oneshot::Sender<io::Result<()>>,
    },
    CloseAnonymous {
        key: u64,
        reply: oneshot::Sender<io::Result<()>>,
    },
    Kick {
        key: u64,
        reason: i8,
        reply: oneshot::Sender<io::Result<()>>,
    },
    Retain {
        key: u64,
        lifetime: Duration,
        reply: oneshot::Sender<io::Result<u64>>,
    },
    /// LeaveWorldReq of the GameDaemon link. The reply is the token key or the reason.
    LeaveWorld {
        key: u64,
        pcid: u64,
        world: i16,
        reply: oneshot::Sender<Result<u64, i16>>,
    },
    /// LogoutNotify of the GameDaemon link (LogoutInGameWorldJob). No reply.
    LogoutInWorld { key: u64, pcid: u64, world: i16 },
    /// The GameDaemon link of `world` closed. Each link task ends its sessions of the world.
    /// It has no key: `LinkTask::run` expands it into one `LogoutInWorld` for each session.
    WorldClosed { world: i16 },
}
impl Control {
    fn key(&self) -> u64 {
        match self {
            Self::ConsumeWorld { key, .. }
            | Self::PromoteWorld { key, .. }
            | Self::CloseAnonymous { key, .. }
            | Self::Kick { key, .. }
            | Self::Retain { key, .. }
            | Self::LeaveWorld { key, .. }
            | Self::LogoutInWorld { key, .. } => *key,
            Self::WorldClosed { .. } => 0,
        }
    }
}
enum Item {
    Packet(Vec<u8>),
    Control(Control),
}
type Directory = SyncMutex<HashMap<LinkId, mpsc::Sender<Control>>>;
/// A reply that a job holds back until its session copy is merged.
type After = Box<dyn FnOnce() + Send>;
type Job<S, N> = (u64, Jobs<S, Arc<N>>, io::Result<Vec<Output>>, Option<After>);

pub struct Server<S, N> {
    store: S,
    np: Arc<N>,
    gamed: Arc<dyn gamed::GameDaemon>,
    settings: Settings,
    links: Directory,
    owners: Arc<SyncMutex<HashMap<u64, LinkId>>>,
    next_link: AtomicU64,
    next_key: Arc<AtomicU64>,
    capacity: Arc<Semaphore>,
}
impl<S: Store, N: Np> Server<S, N> {
    pub fn new(store: S, np: N, settings: Settings) -> io::Result<Self> {
        let np = Arc::new(np);
        // Validate the same settings as a standalone job lane.
        Jobs::new(store.clone(), np.clone(), settings.clone())?;
        Ok(Self {
            store,
            np,
            gamed: Arc::new(gamed::StubGameDaemon::default()),
            capacity: Arc::new(Semaphore::new(settings.max_sessions)),
            settings,
            links: SyncMutex::new(HashMap::new()),
            owners: Arc::default(),
            next_link: AtomicU64::new(1),
            next_key: Arc::new(AtomicU64::new(1)),
        })
    }
    pub fn with_game_daemon(mut self, gamed: Arc<dyn gamed::GameDaemon>) -> Self {
        self.gamed = gamed;
        self
    }
    /// Start the deletion scheduler: one `Interval` task for the whole daemon. It uses the
    /// period and the row count of the settings and stops with `shutdown`.
    pub fn spawn_deletion_scheduler(
        &self,
        shutdown: watch::Receiver<bool>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(scheduler::run(
            self.store.clone(),
            self.settings.deletion_scheduler_period,
            self.settings.deletion_scheduler_batch,
            shutdown,
        ))
    }
    /// Send a control message to the link task that owns `key`. `None`: no session.
    async fn control<T>(
        &self,
        key: u64,
        make: impl FnOnce(oneshot::Sender<T>) -> Control,
    ) -> Option<T> {
        let owner = self.owners.lock().unwrap().get(&key).copied()?;
        let link = self.links.lock().unwrap().get(&owner).cloned()?;
        let (reply, answer) = oneshot::channel();
        link.send(make(reply)).await.ok()?;
        answer.await.ok()
    }
    pub async fn consume_world(
        &self,
        key: u64,
        token: u64,
        ipv4: u32,
        world: i16,
    ) -> Result<u64, i16> {
        self.control(key, |reply| Control::ConsumeWorld {
            key,
            token,
            ipv4,
            world,
            reply,
        })
        .await
        .unwrap_or(Err(16))
    }
    pub async fn promote_world(&self, key: u64) -> io::Result<()> {
        self.control(key, |reply| Control::PromoteWorld { key, reply })
            .await
            .unwrap_or_else(|| Err(io::Error::other("session absent")))
    }
    pub async fn close_anonymous(&self, key: u64) -> io::Result<()> {
        self.control(key, |reply| Control::CloseAnonymous { key, reply })
            .await
            .unwrap_or(Ok(()))
    }
    /// Kick a session. A session in the world (state 4.0) gets the world kick: KickReq to its
    /// GameDaemon, wait for the reply or the time limit, then the session ends (`18-jobs-f.md`
    /// §5). The wait runs in a job task: the link task keeps serving its other sessions.
    /// The result is `Ok` when the session ended, also when the world kick failed (result 2).
    pub async fn kick(&self, key: u64, reason: i8) -> io::Result<()> {
        self.control(key, |reply| Control::Kick { key, reason, reply })
            .await
            .unwrap_or(Ok(()))
    }
    /// Deterministic world-return control. No GameDaemon listener is exposed.
    pub async fn retain(&self, key: u64, lifetime: Duration) -> io::Result<u64> {
        self.control(key, |reply| Control::Retain {
            key,
            lifetime,
            reply,
        })
        .await
        .unwrap_or_else(|| Err(io::Error::other("session absent")))
    }
    /// LeaveWorldReq of the GameDaemon link (`23-return-to-lobby.md` §3). The link task that
    /// owns the session runs the leave job on a copy. `Ok` is the lobby token key.
    /// `Err`: 5 no session for the key, 2 the job cannot start, else the reason of the job.
    /// `world` is the world of the GameDaemon link.
    pub async fn leave_world(&self, key: u64, pcid: u64, world: i16) -> Result<u64, i16> {
        let owner = self.owners.lock().unwrap().get(&key).copied().ok_or(5i16)?;
        let link = self
            .links
            .lock()
            .unwrap()
            .get(&owner)
            .cloned()
            .ok_or(5i16)?;
        let (reply, answer) = oneshot::channel();
        let control = Control::LeaveWorld {
            key,
            pcid,
            world,
            reply,
        };
        // The send fails when the link ends. The answer fails when the job does not run.
        // Both are "job cannot start": LeaveWorldFail (not EnterWorldFail), `quirk_leave_fail_as_enter_fail`.
        link.send(control).await.map_err(|_| 2i16)?;
        answer.await.unwrap_or(Err(2))
    }
    /// LogoutNotify of the GameDaemon link (LogoutInGameWorldJob, `13-jobs-a.md` §3.7). The
    /// session of `key` must be in state 4.0 with the playing character `pcid` in `world`,
    /// else nothing changes. The call ends when the job is queued for the link task of the
    /// session. It does not wait for the job: the GameDaemon reader must stay free for the
    /// reply of a kick that waits for the same session.
    pub async fn logout_in_world(&self, key: u64, pcid: u64, world: i16) {
        let Some(owner) = self.owners.lock().unwrap().get(&key).copied() else {
            return;
        };
        let link = self.links.lock().unwrap().get(&owner).cloned();
        if let Some(link) = link {
            let _ = link.send(Control::LogoutInWorld { key, pcid, world }).await;
        }
    }
    /// The GameDaemon link of `world` closed: end all sessions in state 4.0 of that world
    /// (`07-gamed-link.md` step 11: link close starts the logout jobs). Each link task ends
    /// its own sessions. The call ends when all link tasks have the message.
    pub async fn world_closed(&self, world: i16) {
        let links: Vec<_> = self.links.lock().unwrap().values().cloned().collect();
        for link in links {
            let _ = link.send(Control::WorldClosed { world }).await;
        }
    }
    pub async fn session_count(&self) -> usize {
        self.owners.lock().unwrap().len()
    }
    pub async fn serve(
        self: Arc<Self>,
        listener: TcpListener,
        mut shutdown: watch::Receiver<bool>,
    ) -> io::Result<()> {
        let permits = Arc::new(Semaphore::new(self.settings.max_links));
        let mut tasks = JoinSet::new();
        loop {
            while tasks.try_join_next().is_some() {}
            if *shutdown.borrow() {
                break;
            }
            tokio::select! {
                _ = shutdown.changed() => break,
                accepted = listener.accept() => {
                    let (stream, _) = accepted?;
                    let Ok(permit) = permits.clone().try_acquire_owned() else { drop(stream); continue; };
                    let link = self.next_link.fetch_add(1, Ordering::Relaxed);
                    let server = self.clone();
                    let stop = shutdown.clone();
                    tasks.spawn(async move {
                        let _permit = permit;
                        server.link(link, stream, stop).await
                    });
                }
                Some(_) = tasks.join_next(), if !tasks.is_empty() => (),
            }
        }
        while tasks.join_next().await.is_some() {}
        Ok(())
    }
    /// The task of one LobbyGate link (rules 1, 8, 9 and 10 of step 6).
    async fn link(
        &self,
        link: LinkId,
        stream: TcpStream,
        stop: watch::Receiver<bool>,
    ) -> io::Result<()> {
        let mut jobs = Jobs::new(self.store.clone(), self.np.clone(), self.settings.clone())?
            .with_game_daemon(self.gamed.clone());
        jobs.set_resources(
            self.next_key.clone(),
            self.capacity.clone(),
            self.owners.clone(),
        );
        shared::net::tune(&stream)?;
        let (reader, writer) = stream.into_split();
        let (control, controls) = mpsc::channel(CONTROL_QUEUE);
        self.links.lock().unwrap().insert(link, control);
        let (logouts, pending) = mpsc::channel(LOGOUT_QUEUE);
        let logout_writer = tokio::spawn(write_logouts(self.store.clone(), pending));
        let mut task = LinkTask {
            link,
            jobs,
            busy: HashMap::new(),
            running: JoinSet::new(),
            writer,
            logouts,
            stop: stop.clone(),
        };
        let result = task.run(reader, controls, stop).await;
        // No new control message can reach this link from here.
        self.links.lock().unwrap().remove(&link);
        task.close().await;
        drop(task);
        let _ = logout_writer.await;
        result
    }
}

/// Write the last-logout rows of one link in order (rule 8).
/// Known limit: a last-logout write and a new login of the same account at the same moment
/// can change order. ❓ Not seen as a problem.
async fn write_logouts<S: Store>(store: S, mut pending: mpsc::Receiver<Vec<(AccountId, i32)>>) {
    while let Some(batch) = pending.recv().await {
        for (id, bot_score) in batch {
            let _ = store.update_last_logout(id, bot_score).await;
        }
    }
}

struct LinkTask<S, N> {
    link: LinkId,
    jobs: Jobs<S, Arc<N>>,
    /// Sessions that a job task holds, with the items that wait for them.
    busy: HashMap<u64, VecDeque<Item>>,
    running: JoinSet<Job<S, N>>,
    writer: OwnedWriteHalf,
    logouts: mpsc::Sender<Vec<(AccountId, i32)>>,
    stop: watch::Receiver<bool>,
}
impl<S: Store, N: Np> LinkTask<S, N> {
    /// One `select!` over the socket, control messages, job results and a tick (rule 5).
    async fn run(
        &mut self,
        reader: tokio::net::tcp::OwnedReadHalf,
        mut controls: mpsc::Receiver<Control>,
        mut stop: watch::Receiver<bool>,
    ) -> io::Result<()> {
        let mut reader = shared::net::FrameReader::new(reader, shared::net::MAX_TRUSTED_FRAME);
        // Local scheduler resolution, not an original lifetime.
        let mut tick = tokio::time::interval(Duration::from_millis(10));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            let mut out = Vec::new();
            // The frame reader keeps a partial frame when another branch wins.
            tokio::select! {
                _ = stop.changed() => return Ok(()),
                frame = reader.next() => {
                    let Some(packet) = frame? else { return Ok(()) };
                    // Expire before dispatch too, so traffic cannot extend a deadline.
                    self.expire(&mut out)?;
                    let packet = packet.to_vec();
                    let key = self.jobs.target(self.link, &packet);
                    self.submit(key, Item::Packet(packet), &mut out)?;
                }
                Some(control) = controls.recv() => {
                    if let Control::WorldClosed { world } = control {
                        self.world_closed(world, &mut out)?;
                    } else {
                        self.submit(control.key(), Item::Control(control), &mut out)?;
                    }
                }
                Some(done) = self.running.join_next(), if !self.running.is_empty() => {
                    let (key, job, result, after) = done.map_err(io::Error::other)?;
                    self.done(key, job, result, after, &mut out)?;
                }
                _ = tick.tick() => self.expire(&mut out)?,
            }
            self.write(out).await?;
            self.send_logouts().await;
        }
    }
    /// A GameDaemon link closed: one logout job for each session of this link in the world.
    /// A session that a job holds gets its logout in its queue (rule 2).
    fn world_closed(&mut self, world: i16, out: &mut Vec<Output>) -> io::Result<()> {
        for (key, pcid) in self.jobs.world_players(world) {
            let logout = Control::LogoutInWorld { key, pcid, world };
            self.submit(key, Item::Control(logout), out)?;
        }
        Ok(())
    }
    fn expire(&mut self, out: &mut Vec<Output>) -> io::Result<()> {
        let busy = &self.busy;
        out.extend(self.jobs.expire_now(|key| busy.contains_key(&key))?);
        Ok(())
    }
    /// Run an item now, or queue it when its session is busy (rule 2).
    fn submit(&mut self, key: u64, item: Item, out: &mut Vec<Output>) -> io::Result<()> {
        if let Some(queue) = self.busy.get_mut(&key) {
            if queue.len() >= MAX_QUEUED {
                return Err(io::Error::other("session queue full"));
            }
            queue.push_back(item);
            return Ok(());
        }
        self.start(key, item, out)
    }
    fn start(&mut self, key: u64, item: Item, out: &mut Vec<Output>) -> io::Result<()> {
        let link = self.link;
        match item {
            Item::Packet(packet) => {
                if Jobs::<S, Arc<N>>::slow(protocol::command(&packet)?) {
                    // Rule 3: the job runs on a private copy of the session.
                    self.busy.insert(key, VecDeque::new());
                    let mut job = self.jobs.checkout(link, key);
                    let mut stop = self.stop.clone();
                    self.running.spawn(async move {
                        // Rule 10: a shutdown stops the job at its next wait. The NP calls
                        // are safe to cancel. The stored session holds a taken presence,
                        // so `close` releases it after the merge.
                        let shutdown = async {
                            if stop.wait_for(|stop| *stop).await.is_err() {
                                std::future::pending::<()>().await;
                            }
                        };
                        let result = tokio::select! {
                            result = job.handle(link, &packet) => result,
                            _ = shutdown => Err(io::Error::other("shutdown")),
                        };
                        (key, job, result, None)
                    });
                } else {
                    out.extend(self.jobs.handle_fast(link, &packet)?);
                }
            }
            Item::Control(Control::LeaveWorld {
                key,
                pcid,
                world,
                reply,
            }) => {
                // The leave job runs on a private copy, as a slow command does (rule 3).
                // The reply goes out after the merge: the state 5.1 is then visible to the
                // LobbyGate packets that the client sends after it got the token.
                self.busy.insert(key, VecDeque::new());
                let mut job = self.jobs.checkout(link, key);
                self.running.spawn(async move {
                    let result = job.leave_world(key, pcid, world);
                    let after: After = Box::new(move || {
                        let _ = reply.send(result);
                    });
                    (key, job, Ok(vec![]), Some(after))
                });
            }
            Item::Control(Control::Kick { key, reason, reply })
                if self.jobs.state(key) == Some(State::World) =>
            {
                // KickJob state 2 (world kick): the wait for KickOk or KickFail runs on a
                // private copy in a job task, so this task keeps serving the other sessions
                // of the link (rule 3). The reply goes out after the merge.
                self.busy.insert(key, VecDeque::new());
                let mut job = self.jobs.checkout(link, key);
                let mut stop = self.stop.clone();
                self.running.spawn(async move {
                    let shutdown = async {
                        if stop.wait_for(|stop| *stop).await.is_err() {
                            std::future::pending::<()>().await;
                        }
                    };
                    // A shutdown stops the wait. The session stays in the copy; `close` then
                    // ends it with the other sessions of the link.
                    let mut outputs = Ok(vec![]);
                    tokio::select! {
                        done = job.kick_world(key, reason) => outputs = done,
                        _ = shutdown => (),
                    }
                    let after: After = Box::new(move || {
                        let _ = reply.send(Ok(()));
                    });
                    (key, job, outputs, Some(after))
                });
            }
            Item::Control(control) => self.control(control, out),
        }
        Ok(())
    }
    /// The result of a control message goes to the caller, not to the link.
    fn control(&mut self, control: Control, out: &mut Vec<Output>) {
        let jobs = &mut self.jobs;
        let mut deliver =
            |result: io::Result<Vec<Output>>| result.map(|outputs| out.extend(outputs));
        match control {
            Control::ConsumeWorld {
                key,
                token,
                ipv4,
                world,
                reply,
            } => {
                let _ = reply.send(jobs.consume_world(key, token, ipv4, world));
            }
            Control::PromoteWorld { key, reply } => {
                let _ = reply.send(deliver(jobs.promote_world(key)));
            }
            Control::CloseAnonymous { key, reply } => {
                let _ = reply.send(deliver(jobs.close_anonymous_now(key)));
            }
            Control::Kick { key, reason, reply } => {
                let _ = reply.send(deliver(jobs.kick_now(key, reason)));
            }
            Control::Retain {
                key,
                lifetime,
                reply,
            } => {
                let _ = reply.send(jobs.retain(key, lifetime));
            }
            // `start` runs this control message as a job task. Not reached here.
            Control::LeaveWorld { reply, .. } => {
                let _ = reply.send(Err(2));
            }
            Control::LogoutInWorld { key, pcid, world } => {
                jobs.logout_in_world(key, pcid, world);
            }
            // `run` expands this control message. Not reached here.
            Control::WorldClosed { .. } => (),
        }
    }
    /// Rule 4: merge, write, then run the queue of the key until a new job starts.
    fn done(
        &mut self,
        key: u64,
        job: Jobs<S, Arc<N>>,
        result: io::Result<Vec<Output>>,
        after: Option<After>,
        out: &mut Vec<Output>,
    ) -> io::Result<()> {
        self.jobs.merge(key, job);
        if let Some(after) = after {
            after();
        }
        let mut queue = self.busy.remove(&key).unwrap_or_default();
        out.extend(result?);
        while let Some(item) = queue.pop_front() {
            self.start(key, item, out)?;
            if let Some(next) = self.busy.get_mut(&key) {
                next.append(&mut queue);
                break;
            }
        }
        Ok(())
    }
    /// Rule 6: one write for all packets of one event. The order stays the same.
    async fn write(&mut self, out: Vec<Output>) -> io::Result<()> {
        let packets: Vec<u8> = out
            .into_iter()
            .inspect(|output| debug_assert_eq!(output.link, self.link))
            .flat_map(|output| output.packets)
            .flatten()
            .collect();
        if !packets.is_empty() {
            self.writer.write_all(&packets).await?;
        }
        Ok(())
    }
    async fn send_logouts(&mut self) {
        let batch = self.jobs.take_logouts();
        if !batch.is_empty() {
            let _ = self.logouts.send(batch).await;
        }
    }
    /// Rule 9: wait for all jobs and merge them. Never abort a job: a `start` job can hold
    /// an NP presence that only its own end releases.
    async fn close(&mut self) {
        while let Some(done) = self.running.join_next().await {
            if let Ok((key, job, _, after)) = done {
                self.jobs.merge(key, job);
                if let Some(after) = after {
                    after();
                }
            }
        }
        self.busy.clear();
        self.jobs.disconnect_now(self.link);
        self.send_logouts().await;
    }
}
