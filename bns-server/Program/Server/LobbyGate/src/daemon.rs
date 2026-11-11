//! Outbound LobbyDaemon transport. Spec 05 §§1.2, 2.
use crate::{config::GateConfig, wire::invalid};
use bloodlib::packet::{compose, PacketError, Value};
use std::{
    future::Future,
    io,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::{mpsc, oneshot, watch},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoginToken {
    Np { token: [u8; 16], portal: [u8; 16] },
    Lobby { client_key: u64, token: u64 },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InitSession {
    pub gate_key: u64,
    pub ip: u32,
    pub login: LoginToken,
}
impl InitSession {
    pub fn packet(&self) -> Result<Vec<u8>, PacketError> {
        match self.login {
            LoginToken::Np { token, portal } => compose(
                "hqooi",
                &[
                    Value::I16(6),
                    Value::U64(self.gate_key),
                    Value::Id(token),
                    Value::Id(portal),
                    Value::U32(self.ip),
                ],
            ),
            LoginToken::Lobby { client_key, token } => compose(
                "hqqqi",
                &[
                    Value::I16(12),
                    Value::U64(self.gate_key),
                    Value::U64(client_key),
                    Value::U64(token),
                    Value::U32(self.ip),
                ],
            ),
        }
    }
}
pub type SendFuture<'a> = Pin<Box<dyn Future<Output = io::Result<()>> + Send + 'a>>;
pub trait LobbyDaemon: Send + Sync + 'static {
    fn init_session(&self, request: InitSession) -> SendFuture<'_>;
    fn send_packet(&self, _packet: Vec<u8>) -> SendFuture<'_> {
        Box::pin(async {
            Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "no daemon transport",
            ))
        })
    }
    fn hello_lists(&self) -> Option<Vec<u8>> {
        None
    }
    /// Consume the body paired with the next queued D2 event.
    fn take_hello_snapshot(&self) -> Option<Vec<u8>> {
        self.hello_lists()
    }
    fn connected(&self) -> bool {
        false
    }
    fn service_available(&self) -> bool {
        false
    }
    fn take_events(&self) -> Option<mpsc::Receiver<Vec<u8>>> {
        None
    }
}
pub struct UnavailableDaemon;
impl LobbyDaemon for UnavailableDaemon {
    fn init_session(&self, _request: InitSession) -> SendFuture<'_> {
        Box::pin(async {
            Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "LobbyDaemon is not configured",
            ))
        })
    }
}
struct Outbound {
    generation: u64,
    packet: Vec<u8>,
    ack: oneshot::Sender<io::Result<()>>,
}
/// One ordered TCP stream. No internal listening port is opened.
pub struct TcpLobbyDaemon {
    tx: mpsc::Sender<Outbound>,
    events: Mutex<Option<mpsc::Receiver<Vec<u8>>>>,
    connected: AtomicBool,
    generation: AtomicU64,
    service: AtomicBool,
    hello: Mutex<Option<Vec<u8>>>,
    hello_snapshots: Mutex<std::collections::VecDeque<Vec<u8>>>,
}
impl TcpLobbyDaemon {
    /// Wait without a timeout for HelloOk. Bind the client port after this call.
    pub async fn start(
        config: &GateConfig,
        shutdown: watch::Receiver<bool>,
    ) -> io::Result<Arc<Self>> {
        let (tx, rx) = mpsc::channel(64);
        let (event_tx, events) = mpsc::channel(64);
        let daemon = Arc::new(Self {
            tx,
            events: Mutex::new(Some(events)),
            connected: AtomicBool::new(false),
            generation: AtomicU64::new(0),
            service: AtomicBool::new(false),
            hello: Mutex::new(None),
            hello_snapshots: Mutex::new(std::collections::VecDeque::new()),
        });
        let (ready_tx, ready_rx) = oneshot::channel();
        let worker = daemon.clone();
        let config = config.clone();
        tokio::spawn(async move {
            worker.run(config, rx, event_tx, ready_tx, shutdown).await;
        });
        ready_rx
            .await
            .map_err(|_| io::Error::other("LobbyDaemon start cancelled"))??;
        Ok(daemon)
    }
    async fn run(
        self: Arc<Self>,
        config: GateConfig,
        mut rx: mpsc::Receiver<Outbound>,
        events: mpsc::Sender<Vec<u8>>,
        ready: oneshot::Sender<io::Result<()>>,
        mut shutdown: watch::Receiver<bool>,
    ) {
        let mut ready = Some(ready);
        let mut first = true;
        let tables = packet::dispatch::Tables::parse(packet::dispatch::DEFAULT_TABLES)
            .expect("built-in dispatch data");
        loop {
            if *shutdown.borrow() {
                break;
            }
            if !first {
                tokio::select! {
                    result = events.send(Vec::new()) => { if result.is_err() { break; } },
                    _ = shutdown.changed() => break,
                }
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_millis(5000)) => (),
                    _ = shutdown.changed() => break,
                }
            }
            first = false;
            let connected = tokio::select! {
                result = TcpStream::connect((config.lobby_daemon_addr, config.lobby_daemon_port)) => result,
                _ = shutdown.changed() => break,
            };
            let mut stream = match connected {
                Ok(stream) => stream,
                Err(error) => {
                    if let Some(ready) = ready.take() {
                        let _ = ready.send(Err(error));
                        break;
                    }
                    tracing::warn!(%error, "LobbyDaemon reconnect failed");
                    continue;
                }
            };
            let _ = stream.set_nodelay(true); // rule H5
            let hello = compose(
                "hhhh",
                &[
                    Value::I16(1),
                    Value::I16(config.predefined_world_id as i16),
                    Value::I16(58),
                    Value::U16(config.internal_port),
                ],
            )
            .expect("fixed hello");
            if let Err(error) = stream.write_all(&hello).await {
                if let Some(ready) = ready.take() {
                    let _ = ready.send(Err(error));
                    break;
                }
                continue;
            }
            self.connected.store(true, Ordering::Release);
            let (mut reader, mut writer) = stream.into_split();
            let mut fragments = packet::fragments::FragmentReader::default();
            'link: loop {
                // Retain a partial read across outgoing writes.
                let read = read_trusted(&mut reader);
                tokio::pin!(read);
                let packet = loop {
                    tokio::select! {
                        _ = shutdown.changed() => break 'link,
                        incoming = &mut read => match incoming { Ok(Some(packet)) => break packet, _ => break 'link },
                        Some(out) = rx.recv() => {
                            if out.generation != self.generation.load(Ordering::Acquire) {
                                let _ = out.ack.send(Err(io::Error::new(io::ErrorKind::NotConnected, "stale daemon connection")));
                                continue;
                            }
                            let result = tokio::select! {
                                result = writer.write_all(&out.packet) => result,
                                _ = shutdown.changed() => break 'link,
                            };
                            let failed = result.is_err();
                            let _ = out.ack.send(result);
                            if failed { break 'link; }
                        }
                    }
                };
                let cmd = i16::from_le_bytes([packet[2], packet[3]]);
                if matches!(tables.lobbyd_route(cmd), packet::dispatch::Route::Reject) {
                    tracing::warn!(cmd, "unknown LobbyDaemon command");
                    break 'link;
                }
                if cmd == 2
                    && packet.len() >= 8
                    && i32::from_le_bytes(packet[4..8].try_into().unwrap()) > 1024 * 1024
                {
                    tracing::warn!("HelloOk fragment exceeds Rust safety limit");
                    continue;
                }
                if cmd == 2 {
                    match fragments.add(&packet) {
                        Ok(Some(joined)) => {
                            let body = joined.data.into_owned();
                            *self.hello.lock().unwrap() = Some(body.clone());
                            self.hello_snapshots.lock().unwrap().push_back(body);
                            self.service.store(true, Ordering::Release);
                            if let Some(ready) = ready.take() {
                                let _ = ready.send(Ok(()));
                            }
                        }
                        // Cache the complete body before startup wakes.
                        // Do not fill the event queue before startup can finish.
                        Ok(None) => continue,
                        Err(error) => {
                            tracing::warn!(%error, "bad HelloOk fragment");
                            continue;
                        }
                    }
                } else if cmd == 3
                    && bloodlib::packet::decompose("hh", &packet, Default::default()).is_ok()
                {
                    if let Some(ready) = ready.take() {
                        let _ = ready.send(Err(io::Error::other("LobbyDaemon HelloFail")));
                    }
                    self.service.store(false, Ordering::Release);
                }
                // Never block a write acknowledgement behind a full inbound queue.
                // Reconnect on overload. This is an explicit Rust safety policy.
                if events.try_send(packet).is_err() {
                    if cmd == 2 {
                        self.hello_snapshots.lock().unwrap().pop_back();
                    }
                    tracing::warn!("LobbyDaemon event queue unavailable");
                    break 'link;
                }
            }
            self.connected.store(false, Ordering::Release);
            self.generation.fetch_add(1, Ordering::AcqRel);
            self.service.store(false, Ordering::Release);
            while let Ok(out) = rx.try_recv() {
                let _ = out.ack.send(Err(io::Error::new(
                    io::ErrorKind::NotConnected,
                    "LobbyDaemon disconnected",
                )));
            }
            if let Some(ready) = ready.take() {
                let _ = ready.send(Err(io::Error::other("LobbyDaemon closed before HelloOk")));
                break;
            }
        }
        self.connected.store(false, Ordering::Release);
        self.service.store(false, Ordering::Release);
    }
}
impl LobbyDaemon for TcpLobbyDaemon {
    fn init_session(&self, request: InitSession) -> SendFuture<'_> {
        Box::pin(async move {
            self.send_packet(request.packet().map_err(|e| invalid(&e.to_string()))?)
                .await
        })
    }
    fn hello_lists(&self) -> Option<Vec<u8>> {
        self.hello.lock().unwrap().clone()
    }
    fn take_hello_snapshot(&self) -> Option<Vec<u8>> {
        self.hello_snapshots.lock().unwrap().pop_front()
    }
    fn connected(&self) -> bool {
        self.connected.load(Ordering::Acquire)
    }
    fn service_available(&self) -> bool {
        self.service.load(Ordering::Acquire)
    }
    fn take_events(&self) -> Option<mpsc::Receiver<Vec<u8>>> {
        self.events.lock().unwrap().take()
    }
    fn send_packet(&self, packet: Vec<u8>) -> SendFuture<'_> {
        Box::pin(async move {
            bloodlib::frames::encode_trusted(&packet).map_err(|e| invalid(&e.to_string()))?;
            let generation = self.generation.load(Ordering::Acquire);
            if !self.connected() {
                return Err(io::Error::new(
                    io::ErrorKind::NotConnected,
                    "LobbyDaemon unavailable",
                ));
            }
            let (ack, result) = oneshot::channel();
            self.tx
                .send(Outbound {
                    generation,
                    packet,
                    ack,
                })
                .await
                .map_err(|_| io::Error::other("daemon task stopped"))?;
            result
                .await
                .map_err(|_| io::Error::other("daemon write cancelled"))?
        })
    }
}
pub async fn read_trusted(
    reader: &mut (impl tokio::io::AsyncRead + Unpin),
) -> io::Result<Option<Vec<u8>>> {
    let mut header = [0; 2];
    if reader.read(&mut header[..1]).await? == 0 {
        return Ok(None);
    }
    reader.read_exact(&mut header[1..]).await?;
    let size = i16::from_le_bytes(header);
    if !(4..=4000).contains(&size) {
        return Err(invalid("invalid LobbyDaemon frame size"));
    }
    let mut packet = vec![0; size as usize];
    packet[..2].copy_from_slice(&header);
    reader.read_exact(&mut packet[2..]).await?;
    Ok(Some(packet))
}
