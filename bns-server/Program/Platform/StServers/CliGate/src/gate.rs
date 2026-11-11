//! The gate: client map, bus events (`Cli/Logout`, spec §12.4) and the accept loop.

use crate::conn;
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use sts_core::{
    conn::{header, reply_error, reply_ok, set_header, Incoming},
    error::{module, StsError, ERR_BAD_PARAM, ERR_TIMEOUT},
    route_client::{RouteClient, RouteEvent},
    xml::parse_u32,
};
use tokio::{
    net::TcpListener,
    sync::{mpsc, oneshot},
};

use crate::session::ERR_CLIENT_NOT_FOUND;

/// Most log lines that `log_lines` keeps.
const LOG_KEEP: usize = 1000;

#[derive(Clone)]
pub struct GateConfig {
    pub server_id: u32,
    /// Own datacenter, the center in the `t` header (§9.3).
    pub center: u32,
    pub idle_timeout: Duration,
    /// Timeout of each call to AuthSrv (25 000 ms).
    pub auth_timeout: Duration,
}

/// A command from the gate to one client task.
pub(crate) enum ConnCmd {
    /// Reset the session, close the socket, then send on `done`.
    Close { done: oneshot::Sender<()> },
}

pub(crate) struct ClientHandle {
    pub tx: mpsc::UnboundedSender<ConnCmd>,
    /// UserId of the session, set after LoginStart.
    pub user_id: Arc<Mutex<Option<String>>>,
}

pub struct CliGate {
    pub(crate) cfg: GateConfig,
    pub(crate) route: RouteClient,
    clients: Mutex<HashMap<u32, ClientHandle>>,
    next_id: AtomicU32,
    log: Mutex<VecDeque<String>>,
    started: Instant,
}

pub(crate) struct Registered {
    pub id: u32,
    pub rx: mpsc::UnboundedReceiver<ConnCmd>,
    pub user_id: Arc<Mutex<Option<String>>>,
}

impl CliGate {
    pub fn new(cfg: GateConfig, route: RouteClient) -> Arc<Self> {
        Arc::new(Self {
            cfg,
            route,
            clients: Mutex::new(HashMap::new()),
            next_id: AtomicU32::new(1),
            log: Mutex::new(VecDeque::new()),
            started: Instant::now(),
        })
    }

    /// Accept loop. Each socket gets one task.
    pub async fn serve(self: Arc<Self>, listener: TcpListener) -> std::io::Result<()> {
        loop {
            let (stream, addr) = listener.accept().await?;
            let _ = stream.set_nodelay(true); // rule H5
            tokio::spawn(conn::run(self.clone(), stream, addr));
        }
    }

    /// Handles the bus events until the channel closes.
    pub async fn handle_bus(self: Arc<Self>, mut events: mpsc::UnboundedReceiver<RouteEvent>) {
        while let Some(event) = events.recv().await {
            match event {
                RouteEvent::Request(inc) => {
                    let gate = self.clone();
                    tokio::spawn(async move {
                        if inc.command() == ("Cli", "Logout") {
                            gate.cli_logout(&inc).await;
                        } else {
                            let err = StsError::new(ERR_BAD_PARAM, module::CLI_GATE, 0);
                            bus_reply(&inc, Err(err), gate.cfg.server_id);
                        }
                    });
                }
                RouteEvent::Registered => eprintln!("CliGate: registered on the route bus"),
                // §12.6: AuthSrv lost, nothing more to do. Open calls end with error 42.
                _ => {}
            }
        }
    }

    /// Ids of the open client sockets, sorted.
    pub fn client_ids(&self) -> Vec<u32> {
        let mut ids: Vec<u32> = self.clients.lock().unwrap().keys().copied().collect();
        ids.sort_unstable();
        ids
    }

    /// The last log lines (also written to stderr).
    pub fn log_lines(&self) -> Vec<String> {
        self.log.lock().unwrap().iter().cloned().collect()
    }

    pub(crate) fn log(&self, line: String) {
        eprintln!("{line}");
        let mut log = self.log.lock().unwrap();
        if log.len() >= LOG_KEEP {
            log.pop_front();
        }
        log.push_back(line);
    }

    /// Login time stamp: ms since start, never 0 (0 = not logged in).
    pub(crate) fn clock(&self) -> u32 {
        (self.started.elapsed().as_millis() as u32).max(1)
    }

    pub(crate) fn register(&self) -> Registered {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = mpsc::unbounded_channel();
        let user_id = Arc::new(Mutex::new(None));
        self.clients.lock().unwrap().insert(
            id,
            ClientHandle {
                tx,
                user_id: user_id.clone(),
            },
        );
        Registered { id, rx, user_id }
    }

    pub(crate) fn unregister(&self, id: u32) {
        self.clients.lock().unwrap().remove(&id);
    }

    /// `Cli/Logout` (§12.4). Phase 1: with or without `<Force>` the gate resets
    /// the session and closes the socket at once.
    async fn cli_logout(&self, inc: &Incoming) {
        let result = self.cli_logout_inner(inc).await;
        bus_reply(inc, result, self.cfg.server_id);
    }

    async fn cli_logout_inner(&self, inc: &Incoming) -> Result<(), StsError> {
        let bad = |text: &str| StsError::new(ERR_BAD_PARAM, module::CLI_GATE, 0).with_text(text);
        let body = inc.body().ok_or_else(|| bad("Cli/Logout: missing body"))?;
        let client_id = body
            .get_text("ClientId")
            .and_then(parse_u32)
            .ok_or_else(|| bad("Cli/Logout: missing ClientId"))?;
        let user_id = body.get_text("UserId").unwrap_or("");
        let found = {
            let clients = self.clients.lock().unwrap();
            clients.get(&client_id).and_then(|h| {
                let same = h
                    .user_id
                    .lock()
                    .unwrap()
                    .as_deref()
                    .is_some_and(|u| u.eq_ignore_ascii_case(user_id));
                same.then(|| h.tx.clone())
            })
        };
        let not_found = || StsError::new(ERR_CLIENT_NOT_FOUND, module::CLI_GATE, 0);
        let tx = found.ok_or_else(not_found)?;
        let (done, wait) = oneshot::channel();
        tx.send(ConnCmd::Close { done }).map_err(|_| not_found())?;
        match tokio::time::timeout(self.cfg.auth_timeout, wait).await {
            Ok(_) => Ok(()),
            Err(_) => Err(StsError::new(ERR_TIMEOUT, module::CLI_GATE, 0)),
        }
    }
}

/// Reply on the bus. The router needs `t` = the `p` header of the request.
fn bus_reply(inc: &Incoming, result: Result<(), StsError>, server_id: u32) {
    let id = inc.id();
    if id == 0 {
        return;
    }
    let mut frame = match &result {
        Ok(()) => reply_ok(id, None),
        Err(e) => reply_error(id, e, server_id),
    };
    if let Some(origin) = header(&inc.frame, "p") {
        set_header(&mut frame, "t", origin);
    }
    inc.peer.send(frame);
}
