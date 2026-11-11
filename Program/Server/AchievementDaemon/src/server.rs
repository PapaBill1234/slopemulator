//! Tokio listeners and the independent update/Ping timers.
use crate::service::{Daemon, GameConn, ManagerConn, Out};
use crate::store::Store;
use std::collections::HashMap;
use std::io;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;

type Sink = shared::net::Outbox;
/// CheckPingTimer::HandleEvent rearms after each check (0x140002650).
pub const PING_CHECK_INTERVAL: Duration = Duration::from_millis(3000);

struct ChannelSink {
    token: u64,
    sink: Sink,
    close: watch::Sender<bool>,
}

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}
fn clock_ms() -> u64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as u64
}

pub struct Server<S> {
    pub daemon: Arc<Daemon<S>>,
    clusters: Mutex<HashMap<i16, ChannelSink>>,
    managers: Mutex<HashMap<u64, Sink>>,
    next_id: AtomicU64,
}

impl<S: Store> Server<S> {
    pub fn start(daemon: Arc<Daemon<S>>) -> Arc<Self> {
        let server = Arc::new(Self {
            daemon,
            clusters: Mutex::default(),
            managers: Mutex::default(),
            next_id: AtomicU64::new(1),
        });
        let weak = Arc::downgrade(&server);
        let period = Duration::from_millis(server.daemon.config.update_cycle_ms);
        tokio::spawn(async move {
            let mut timer = tokio::time::interval(period);
            timer.tick().await;
            loop {
                timer.tick().await;
                let Some(server) = weak.upgrade() else {
                    break;
                };
                server.route(server.daemon.tick(), None);
            }
        });
        if server.daemon.config.ping_enable {
            let weak = Arc::downgrade(&server);
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(PING_CHECK_INTERVAL).await;
                    let Some(server) = weak.upgrade() else {
                        break;
                    };
                    server.route(server.daemon.check_ping(clock_ms()), None);
                }
            });
        }
        server
    }

    fn route(&self, out: Vec<Out>, reply: Option<&Sink>) {
        for o in out {
            match o {
                Out::Reply(p) => {
                    if let Some(sink) = reply {
                        let _ = sink.try_send(p);
                    }
                }
                Out::Cluster(c, p) => {
                    let clusters = self.clusters.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(channel) = clusters.get(&c) {
                        let _ = channel.sink.try_send(p);
                    }
                }
                Out::Managers(p) => {
                    let managers = self.managers.lock().unwrap_or_else(|e| e.into_inner());
                    for sink in managers.values() {
                        let _ = sink.try_send(p.clone());
                    }
                }
                Out::Expired {
                    cluster,
                    token,
                    close,
                } => {
                    let mut clusters = self.clusters.lock().unwrap_or_else(|e| e.into_inner());
                    if clusters.get(&cluster).is_some_and(|c| c.token == token) {
                        if let Some(channel) = clusters.remove(&cluster) {
                            if close {
                                let _ = channel.close.send(true);
                            }
                        }
                    }
                }
            }
        }
    }

    pub async fn serve_games(self: Arc<Self>, listener: TcpListener) -> io::Result<()> {
        loop {
            let (stream, addr) = listener.accept().await?;
            let _ = stream.set_nodelay(true); // rule H5
            let ip = match addr.ip() {
                IpAddr::V4(ip) => ip,
                IpAddr::V6(ip) => ip.to_ipv4_mapped().unwrap_or(Ipv4Addr::UNSPECIFIED),
            };
            tokio::spawn(self.clone().game_connection(stream, GameConn::new(ip)));
        }
    }
    pub async fn serve_managers(self: Arc<Self>, listener: TcpListener) -> io::Result<()> {
        loop {
            let (stream, _) = listener.accept().await?;
            let _ = stream.set_nodelay(true); // rule H5
            let id = self.next_id.fetch_add(1, Ordering::Relaxed);
            tokio::spawn(self.clone().manager_connection(stream, id));
        }
    }

    async fn game_connection(self: Arc<Self>, stream: TcpStream, mut conn: GameConn) {
        let (reader, writer) = stream.into_split();
        let mut reader = shared::net::FrameReader::new(reader, shared::net::MAX_TRUSTED_FRAME);
        let (out, writer_task) = spawn_writer(writer);
        let (close, mut closing) = watch::channel(false);
        // Keep a local sender so removal without socket closure does not wake this branch.
        loop {
            let packet = tokio::select! {
                _ = closing.changed() => break,
                packet = reader.next() => packet,
            };
            let Ok(Some(packet)) = packet else {
                break;
            };
            let had_cluster = conn.cluster.is_some();
            // Match the lock order used by the Ping timer. Register the output sink
            // atomically with Hello so a timeout cannot leave a stale sink behind.
            let result = {
                let mut clusters = self.clusters.lock().unwrap_or_else(|e| e.into_inner());
                let result = self
                    .daemon
                    .game_packet_at(&mut conn, &packet, now(), clock_ms());
                if !had_cluster {
                    if let Some(c) = conn.cluster {
                        clusters.insert(
                            c,
                            ChannelSink {
                                token: conn.token,
                                sink: out.clone(),
                                close: close.clone(),
                            },
                        );
                    }
                }
                result
            };
            self.route(result, Some(&out));
        }
        if let Some(c) = conn.cluster {
            let mut clusters = self.clusters.lock().unwrap_or_else(|e| e.into_inner());
            if clusters.get(&c).is_some_and(|s| s.token == conn.token) {
                clusters.remove(&c);
            }
        }
        self.route(self.daemon.game_disconnect(&conn, now()), None);
        drop(out);
        let _ = writer_task.await;
    }

    async fn manager_connection(self: Arc<Self>, stream: TcpStream, id: u64) {
        let (reader, writer) = stream.into_split();
        let mut reader = shared::net::FrameReader::new(reader, shared::net::MAX_TRUSTED_FRAME);
        let (out, writer_task) = spawn_writer(writer);
        let mut conn = ManagerConn::default();
        while let Ok(Some(packet)) = reader.next().await {
            let was_hello = conn.hello;
            let result = self.daemon.manager_packet(&mut conn, &packet);
            if !was_hello && conn.hello {
                self.managers
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .insert(id, out.clone());
            }
            self.route(result, Some(&out));
        }
        self.managers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&id);
        drop(out);
        let _ = writer_task.await;
    }
}

fn spawn_writer(
    writer: tokio::net::tcp::OwnedWriteHalf,
) -> (Sink, tokio::task::JoinHandle<std::io::Result<()>>) {
    shared::net::spawn_writer(writer, shared::net::LINK_QUEUE)
}
