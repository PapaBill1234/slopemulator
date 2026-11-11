//! Offline LobbyDaemon. Each link has an ordered job lane.
pub mod character;
pub mod gamed;
pub mod gamed_link;
pub mod jobs;
#[cfg(feature = "mssql")]
pub mod mssql;
pub mod np;
pub mod protocol;
pub mod store;

use jobs::{Jobs, LinkId, Output, Settings};
use np::Np;
use std::{
    collections::HashMap,
    io,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex as SyncMutex,
    },
    time::Duration,
};
use store::Store;
use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
    sync::{mpsc, watch, Mutex, Semaphore},
    task::JoinSet,
};

type Lane<S, N> = Arc<Mutex<Jobs<Arc<SyncMutex<S>>, Arc<N>>>>;
type Link<S, N> = (mpsc::Sender<Vec<Vec<u8>>>, Lane<S, N>);
pub struct Server<S, N> {
    store: Arc<SyncMutex<S>>,
    np: Arc<N>,
    gamed: Arc<dyn gamed::GameDaemon>,
    settings: Settings,
    links: Mutex<HashMap<LinkId, Link<S, N>>>,
    owners: Arc<SyncMutex<HashMap<u64, LinkId>>>,
    next_link: AtomicU64,
    next_key: Arc<AtomicU64>,
    capacity: Arc<Semaphore>,
}
impl<S: Store, N: Np> Server<S, N> {
    pub fn new(store: S, np: N, settings: Settings) -> io::Result<Self> {
        let store = Arc::new(SyncMutex::new(store));
        let np = Arc::new(np);
        // Validate the same settings as a standalone job lane.
        Jobs::new(store.clone(), np.clone(), settings.clone())?;
        Ok(Self {
            store,
            np,
            gamed: Arc::new(gamed::StubGameDaemon::default()),
            capacity: Arc::new(Semaphore::new(settings.max_sessions)),
            settings,
            links: Mutex::new(HashMap::new()),
            owners: Arc::default(),
            next_link: AtomicU64::new(1),
            next_key: Arc::new(AtomicU64::new(1)),
        })
    }
    pub fn with_game_daemon(mut self, gamed: Arc<dyn gamed::GameDaemon>) -> Self {
        self.gamed = gamed;
        self
    }
    pub async fn consume_world(
        &self,
        key: u64,
        token: u64,
        ipv4: u32,
        world: i16,
    ) -> Result<u64, i16> {
        let lane = self.lane(key).await.ok_or(16i16)?;
        let result = lane.lock().await.consume_world(key, token, ipv4, world);
        result
    }
    pub async fn promote_world(&self, key: u64) -> io::Result<()> {
        let lane = self
            .lane(key)
            .await
            .ok_or_else(|| io::Error::other("session absent"))?;
        let output = lane.lock().await.promote_world(key)?;
        self.deliver(output).await
    }
    async fn deliver(&self, outputs: Vec<Output>) -> io::Result<()> {
        let links = self.links.lock().await;
        for output in outputs {
            if let Some((tx, _)) = links.get(&output.link) {
                tx.try_send(output.packets)
                    .map_err(|_| io::Error::other("link output queue unavailable"))?;
            }
        }
        Ok(())
    }
    async fn lane(&self, key: u64) -> Option<Lane<S, N>> {
        let owner = self.owners.lock().unwrap().get(&key).copied()?;
        self.links
            .lock()
            .await
            .get(&owner)
            .map(|(_, lane)| lane.clone())
    }
    pub async fn close_anonymous(&self, key: u64) -> io::Result<()> {
        if let Some(lane) = self.lane(key).await {
            let mut jobs = lane.lock().await;
            self.deliver(jobs.close_anonymous(key)?).await?;
        }
        Ok(())
    }
    pub async fn kick(&self, key: u64, reason: i8) -> io::Result<()> {
        if let Some(lane) = self.lane(key).await {
            let mut jobs = lane.lock().await;
            self.deliver(jobs.kick(key, reason)?).await?;
        }
        Ok(())
    }
    /// Deterministic world-return control. No GameDaemon listener is exposed.
    pub async fn retain(&self, key: u64, lifetime: Duration) -> io::Result<u64> {
        let lane = self
            .lane(key)
            .await
            .ok_or_else(|| io::Error::other("session absent"))?;
        let result = lane.lock().await.retain(key, lifetime);
        result
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
                    let mut stop = shutdown.clone();
                    tasks.spawn(async move {
                        let _permit = permit;
                        let mut jobs = Jobs::new(server.store.clone(), server.np.clone(), server.settings.clone())?.with_game_daemon(server.gamed.clone());
                        jobs.set_resources(server.next_key.clone(), server.capacity.clone(), server.owners.clone());
                        let lane = Arc::new(Mutex::new(jobs));
                        let result = tokio::select! {
                            _ = stop.changed() => Ok(()),
                            result = server.connection(link, stream, lane.clone()) => result,
                        };
                        server.links.lock().await.remove(&link);
                        lane.lock().await.disconnect(link);
                        result
                    });
                }
                Some(_) = tasks.join_next(), if !tasks.is_empty() => (),
            }
        }
        while tasks.join_next().await.is_some() {}
        Ok(())
    }
    async fn connection(
        &self,
        link: LinkId,
        stream: TcpStream,
        lane: Lane<S, N>,
    ) -> io::Result<()> {
        let (mut reader, mut writer) = stream.into_split();
        let (tx, mut rx) = mpsc::channel::<Vec<Vec<u8>>>(32);
        self.links.lock().await.insert(link, (tx, lane.clone()));
        let receive = async {
            while let Some(packet) = protocol::read(&mut reader).await? {
                let mut jobs = lane.lock().await;
                // Expire before dispatch too, so traffic cannot extend a deadline.
                self.deliver(jobs.expire()?).await?;
                let output = jobs.handle(link, &packet).await?;
                self.deliver(output).await?;
            }
            Ok(())
        };
        let expire = async {
            // Local scheduler resolution, not an original lifetime.
            let mut tick = tokio::time::interval(Duration::from_millis(10));
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tick.tick().await;
                let mut jobs = lane.lock().await;
                self.deliver(jobs.expire()?).await?;
            }
            #[allow(unreachable_code)]
            Ok::<(), io::Error>(())
        };
        let send = async {
            while let Some(packets) = rx.recv().await {
                for packet in packets {
                    writer.write_all(&packet).await?;
                }
            }
            Ok(())
        };
        // Neither expiry nor output cancels a partial input read.
        tokio::select! { result = receive => result, result = send => result, result = expire => result }
    }
}
