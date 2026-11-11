//! Tokio network layer: public client port (port id 1) and one outbound
//! connector per CacheD (port id 2, reconnect after 5000 ms).
use crate::config::CachedInfo;
use crate::gate::{Flow, Gate, Out, Peer};
use std::collections::HashMap;
use std::io;
use std::net::SocketAddrV4;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::AsyncRead;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Notify;

/// Session::tryReconnect delay.
pub const RECONNECT_DELAY: Duration = Duration::from_millis(5000);

type Sink = shared::net::Outbox;

struct Conn {
    sink: Sink,
    close: Arc<Notify>,
}

pub struct Server {
    pub gate: Mutex<Gate>,
    clients: Mutex<HashMap<u64, Conn>>,
    cached: Mutex<HashMap<u16, Conn>>,
    /// Stop signal for each connector task (StopReconnect).
    connectors: Mutex<HashMap<u16, Arc<Notify>>>,
    next_id: AtomicU64,
    /// max-public-session-count + 1 when the option is > 0.
    max_public: Option<usize>,
}

fn spawn_writer(writer: tokio::net::tcp::OwnedWriteHalf) -> Sink {
    shared::net::spawn_writer(writer, shared::net::LINK_QUEUE).0
}

impl Server {
    pub fn new(own_wid: u16, max_public_session_count: i32) -> Arc<Self> {
        Arc::new(Self {
            gate: Mutex::new(Gate::new(own_wid)),
            clients: Mutex::default(),
            cached: Mutex::default(),
            connectors: Mutex::default(),
            next_id: AtomicU64::new(1),
            max_public: (max_public_session_count > 0)
                .then(|| max_public_session_count as usize + 1),
        })
    }

    fn route(&self, out: Vec<Out>) {
        for o in out {
            match o {
                Out::Send(Peer::Client(id), p) => {
                    if let Some(c) = self.clients.lock().unwrap().get(&id) {
                        let _ = c.sink.try_send(p);
                    }
                }
                Out::Send(Peer::Cached(wid), p) => {
                    if let Some(c) = self.cached.lock().unwrap().get(&wid) {
                        let _ = c.sink.try_send(p);
                    }
                }
                Out::Close(Peer::Client(id)) => {
                    if let Some(c) = self.clients.lock().unwrap().get(&id) {
                        c.close.notify_one();
                    }
                }
                Out::Close(Peer::Cached(wid)) => {
                    if let Some(c) = self.cached.lock().unwrap().get(&wid) {
                        c.close.notify_one();
                    }
                }
            }
        }
    }

    /// Read frames until EOF, error, a handler asks to close, or `close`.
    async fn pump(
        &self,
        peer: Peer,
        mut reader: shared::net::FrameReader<impl AsyncRead + Unpin>,
        close: Arc<Notify>,
    ) -> io::Result<()> {
        loop {
            let frame = tokio::select! {
                f = reader.next() => f?,
                _ = close.notified() => return Ok(()),
            };
            let Some(frame) = frame else { return Ok(()) };
            let (out, flow) = self.gate.lock().unwrap().handle(peer, &frame);
            self.route(out);
            if flow == Flow::Close {
                return Ok(());
            }
        }
    }

    /// Public port (port id 1, CacheGateSessionHandler).
    pub async fn serve_public(self: Arc<Self>, listener: TcpListener) -> io::Result<()> {
        loop {
            let (stream, _) = listener.accept().await?;
            let _ = stream.set_nodelay(true); // rule H5
            if self
                .max_public
                .is_some_and(|max| self.clients.lock().unwrap().len() >= max)
            {
                drop(stream);
                continue;
            }
            let server = Arc::clone(&self);
            tokio::spawn(async move { server.client(stream).await });
        }
    }

    async fn client(&self, stream: TcpStream) {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (reader, writer) = stream.into_split();
        let reader = shared::net::FrameReader::new(reader, i16::MAX as usize);
        let close = Arc::new(Notify::new());
        let conn = Conn {
            sink: spawn_writer(writer),
            close: Arc::clone(&close),
        };
        self.clients.lock().unwrap().insert(id, conn);
        let _ = self.pump(Peer::Client(id), reader, close).await;
        self.gate.lock().unwrap().client_closed(id);
        self.clients.lock().unwrap().remove(&id);
    }

    /// Start (or restart) the connector for every row; `drop` lists rows
    /// whose old connector must stop first.
    pub fn connect(self: &Arc<Self>, rows: &[CachedInfo], drop: &[u16]) {
        for wid in drop {
            if let Some(stop) = self.connectors.lock().unwrap().remove(wid) {
                stop.notify_one();
            }
            if let Some(c) = self.cached.lock().unwrap().get(wid) {
                c.close.notify_one();
            }
        }
        for row in rows {
            let stop = Arc::new(Notify::new());
            if let Some(old) = self
                .connectors
                .lock()
                .unwrap()
                .insert(row.wid, Arc::clone(&stop))
            {
                old.notify_one();
            }
            let server = Arc::clone(self);
            let row = *row;
            tokio::spawn(async move { server.connector(row, stop).await });
        }
    }

    /// Merge a new cached list (reload) and connect new or changed rows.
    pub fn reload(self: &Arc<Self>, rows: &[CachedInfo]) {
        let change = self.gate.lock().unwrap().apply_cached_list(rows);
        self.connect(&change.connect, &change.drop);
    }

    async fn connector(self: Arc<Self>, row: CachedInfo, stop: Arc<Notify>) {
        let addr = SocketAddrV4::new(row.ip, row.port);
        loop {
            let connected = tokio::select! {
                c = TcpStream::connect(addr) => c,
                _ = stop.notified() => return,
            };
            if let Ok(stream) = connected {
                let _ = stream.set_nodelay(true); // rule H5
                let (reader, writer) = stream.into_split();
                let reader = shared::net::FrameReader::new(reader, i16::MAX as usize);
                let close = Arc::new(Notify::new());
                let conn = Conn {
                    sink: spawn_writer(writer),
                    close: Arc::clone(&close),
                };
                self.cached.lock().unwrap().insert(row.wid, conn);
                let out = self
                    .gate
                    .lock()
                    .unwrap()
                    .cached_connected(row.wid, row.ip, row.port);
                self.route(out);
                let stopped = tokio::select! {
                    _ = self.pump(Peer::Cached(row.wid), reader, Arc::clone(&close)) => false,
                    _ = stop.notified() => true,
                };
                let mut cached = self.cached.lock().unwrap();
                if cached
                    .get(&row.wid)
                    .is_some_and(|c| Arc::ptr_eq(&c.close, &close))
                {
                    cached.remove(&row.wid);
                }
                drop(cached);
                if stopped {
                    return;
                }
                self.gate.lock().unwrap().cached_closed(row.wid);
            }
            tokio::select! {
                _ = tokio::time::sleep(RECONNECT_DELAY) => {}
                _ = stop.notified() => return,
            }
        }
    }
}
