//! Tokio listeners: daemon port (GameDaemons) and manage port (manager).
use crate::np::Np;
use crate::protocol;
use crate::service::{Daemon, DaemonConn, Handled, ManagerConn, Work};
use crate::store::Store;
use std::io;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

type Sink = mpsc::UnboundedSender<Vec<u8>>;

/// Shared runtime state around the protocol core.
pub struct Server<S, N> {
    pub daemon: Arc<Daemon<S, N>>,
    work: mpsc::UnboundedSender<Work>,
    manager: Arc<Mutex<Option<(u64, Sink)>>>,
    next_id: AtomicU64,
}

impl<S: Store, N: Np> Server<S, N> {
    /// Start the game-event worker. Start-up work runs first.
    pub fn start(daemon: Arc<Daemon<S, N>>, startup: Vec<Work>) -> Arc<Self> {
        let (work, mut rx) = mpsc::unbounded_channel::<Work>();
        let manager: Arc<Mutex<Option<(u64, Sink)>>> = Arc::default();
        let server = Arc::new(Self {
            daemon: daemon.clone(),
            work: work.clone(),
            manager: manager.clone(),
            next_id: AtomicU64::new(1),
        });
        for job in startup {
            let _ = work.send(job);
        }
        tokio::spawn(async move {
            while let Some(job) = rx.recv().await {
                for packet in daemon.process(job).await {
                    let sink = manager.lock().unwrap_or_else(|e| e.into_inner()).clone();
                    if let Some((_, sink)) = sink {
                        let _ = sink.send(packet);
                    }
                }
            }
        });
        server
    }

    fn dispatch(&self, handled: Handled, out: &Sink) -> bool {
        for packet in handled.replies {
            let _ = out.send(packet);
        }
        for job in handled.work {
            let _ = self.work.send(job);
        }
        !handled.close
    }

    pub async fn serve_daemons(self: Arc<Self>, listener: TcpListener) -> io::Result<()> {
        loop {
            let (stream, addr) = listener.accept().await?;
            let _ = stream.set_nodelay(true); // rule H5
            let ip = match addr.ip() {
                IpAddr::V4(ip) => ip,
                IpAddr::V6(ip) => ip.to_ipv4_mapped().unwrap_or(Ipv4Addr::UNSPECIFIED),
            };
            let id = self.next_id.fetch_add(1, Ordering::Relaxed);
            tokio::spawn(
                self.clone()
                    .daemon_connection(stream, DaemonConn::new(id, ip)),
            );
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

    async fn daemon_connection(self: Arc<Self>, stream: TcpStream, mut conn: DaemonConn) {
        let (mut reader, writer) = stream.into_split();
        let (out, writer_task) = spawn_writer(writer);
        let closer = conn.closer.clone();
        loop {
            tokio::select! {
                frame = protocol::read(&mut reader) => {
                    let Ok(Some(packet)) = frame else { break };
                    let handled = self.daemon.daemon_packet(&mut conn, &packet);
                    if !self.dispatch(handled, &out) {
                        break;
                    }
                }
                _ = closer.notified() => break,
            }
        }
        self.daemon.disconnect(&conn);
        drop(out);
        let _ = writer_task.await;
    }

    async fn manager_connection(self: Arc<Self>, stream: TcpStream, id: u64) {
        let (mut reader, writer) = stream.into_split();
        let (out, writer_task) = spawn_writer(writer);
        let mut conn = ManagerConn::default();
        while let Ok(Some(packet)) = protocol::read(&mut reader).await {
            let was_hello = conn.hello;
            let handled = self.daemon.manager_packet(&mut conn, &packet);
            if !was_hello && conn.hello {
                *self.manager.lock().unwrap_or_else(|e| e.into_inner()) = Some((id, out.clone()));
            }
            if !self.dispatch(handled, &out) {
                break;
            }
        }
        {
            let mut manager = self.manager.lock().unwrap_or_else(|e| e.into_inner());
            if manager.as_ref().is_some_and(|(owner, _)| *owner == id) {
                *manager = None;
            }
        }
        drop(out);
        let _ = writer_task.await;
    }
}

fn spawn_writer(
    mut writer: tokio::net::tcp::OwnedWriteHalf,
) -> (Sink, tokio::task::JoinHandle<()>) {
    let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let task = tokio::spawn(async move {
        while let Some(packet) = rx.recv().await {
            if writer.write_all(&packet).await.is_err() {
                break;
            }
        }
        let _ = writer.shutdown().await;
    });
    (tx, task)
}
