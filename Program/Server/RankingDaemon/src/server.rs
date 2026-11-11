//! Tokio network layer: game, InfoGate and manage listen ports
//! (RankingDaemon::openListenPort @0x14001DC90 opens manage, infogate, game).
use crate::boards::RankingBoards;
use crate::protocol;
use crate::service::{Daemon, Flow, Out, Peer};
use std::collections::HashMap;
use std::io;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Notify};

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Port {
    Game,
    InfoGate,
    Manage,
}

struct Conn {
    sink: mpsc::UnboundedSender<Vec<u8>>,
    close: Arc<Notify>,
}

pub struct Server {
    pub daemon: Mutex<Daemon>,
    conns: Mutex<HashMap<Peer, Conn>>,
    next_id: AtomicU64,
}

impl Server {
    pub fn new(daemon: Daemon) -> Arc<Self> {
        Arc::new(Self {
            daemon: Mutex::new(daemon),
            conns: Mutex::default(),
            next_id: AtomicU64::new(1),
        })
    }

    fn route(&self, out: Vec<Out>) {
        let conns = self.conns.lock().unwrap();
        for o in out {
            match o {
                Out::Send(peer, p) => {
                    if let Some(c) = conns.get(&peer) {
                        let _ = c.sink.send(p);
                    }
                }
                Out::Close(peer) => {
                    if let Some(c) = conns.get(&peer) {
                        c.close.notify_one();
                    }
                }
            }
        }
    }

    /// RankingListMaker (RankingList.cpp): first run 1000 ms after load
    /// (loadAllDataFromDB @0x14001D910), then every `period_ms` aligned to
    /// the build time (RankingListMaker::HandleEvent @0x1400271B0). The
    /// task ends when the step says not to add the timer again.
    pub async fn run_ranking_list_maker(
        self: Arc<Self>,
        boards: Arc<Mutex<RankingBoards>>,
        period_ms: i32,
    ) {
        let mut delay = crate::service::RANKING_LIST_MAKER_FIRST_DELAY_MS;
        loop {
            tokio::time::sleep(Duration::from_millis(delay.max(0) as u64)).await;
            let started = Instant::now();
            let (out, rearm) = {
                let boards = boards.lock().unwrap();
                let mut daemon = self.daemon.lock().unwrap();
                let activated = daemon.activated_ranking_lists(now());
                daemon.ranking_list_maker_step(Some(&boards), &activated, false)
            };
            self.route(out);
            if !rearm {
                return;
            }
            let duration = started.elapsed().as_millis() as i64;
            delay = crate::service::next_ranking_list_delay(period_ms, duration);
        }
    }

    pub async fn serve(self: Arc<Self>, port: Port, listener: TcpListener) -> io::Result<()> {
        loop {
            let (stream, _) = listener.accept().await?;
            let _ = stream.set_nodelay(true); // rule H5
            let server = Arc::clone(&self);
            tokio::spawn(async move { server.session(port, stream).await });
        }
    }

    async fn session(&self, port: Port, stream: TcpStream) {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let peer = match port {
            Port::Game => Peer::Game(id),
            Port::InfoGate => Peer::InfoGate(id),
            Port::Manage => Peer::Manage(id),
        };
        let ip = match stream.peer_addr().map(|a| a.ip()) {
            Ok(IpAddr::V4(ip)) => ip,
            Ok(IpAddr::V6(ip)) => ip.to_ipv4_mapped().unwrap_or(Ipv4Addr::UNSPECIFIED),
            Err(_) => return,
        };
        let (mut reader, mut writer) = stream.into_split();
        let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let writer_task = tokio::spawn(async move {
            while let Some(p) = rx.recv().await {
                if writer.write_all(&p).await.is_err() {
                    break;
                }
            }
            let _ = writer.shutdown().await;
        });
        let close = Arc::new(Notify::new());
        self.conns.lock().unwrap().insert(
            peer,
            Conn {
                sink: tx,
                close: Arc::clone(&close),
            },
        );
        loop {
            let frame = tokio::select! {
                f = protocol::read(&mut reader) => f,
                _ = close.notified() => break,
            };
            let Ok(Some(frame)) = frame else { break };
            let (out, flow) = self.daemon.lock().unwrap().handle(peer, ip, &frame, now());
            self.route(out);
            if flow == Flow::Close {
                break;
            }
        }
        self.daemon.lock().unwrap().closed(peer);
        self.conns.lock().unwrap().remove(&peer);
        let _ = writer_task.await;
    }
}
