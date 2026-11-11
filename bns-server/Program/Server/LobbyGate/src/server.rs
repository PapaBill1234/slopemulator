//! TCP sessions, handshake, and login start. Specs 02, 03, and 05 §1.2.
use crate::{
    config::{read_bounded, GateConfig},
    daemon::{InitSession, LobbyDaemon, LoginToken},
    lists::Lists,
    login::{Delivery, LoginHub},
    wire::{inner_packet, invalid, read_frame, write_frame, write_frames},
};
use bloodlib::{
    crypto::{DhParameters, RsaKey},
    frames::{decode_client, encode_client, ClientDecrypt, ClientEncrypt},
    packet::{compose, decompose, DecodeLimits, Value},
    session::SessionPacketCrypt,
};
use packet_ids::LobbyCmd;
use rand::{rngs::OsRng, RngCore};
use std::{
    io,
    net::Ipv4Addr,
    path::Path,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{mpsc, watch, Semaphore},
    task::JoinSet,
};

fn protocol(error: impl std::fmt::Display) -> io::Error {
    invalid(&error.to_string())
}
/// Load only the operator file. There is no default key and no key extraction.
pub fn load_handshake_key(path: &Path) -> io::Result<RsaKey> {
    let blob = read_bounded(path, 264).map_err(protocol)?;
    let key = RsaKey::from_private_blob(&blob).map_err(protocol)?;
    if blob.len() != 264 || key.block_len() != 128 {
        return Err(invalid(
            "handshake key must use the 1024-bit private blob layout",
        ));
    }
    Ok(key)
}

pub struct Server {
    key: RsaKey,
    daemon: Arc<dyn LobbyDaemon>,
    next_gate_key: AtomicU64,
    next_session: AtomicU64,
    hub: Arc<LoginHub>,
    broadcast: Vec<u8>,
    lists: Mutex<Lists>,
    contents: Option<crate::contents::ContentsRules>,
    started: tokio::time::Instant,
    close_timeout: i32,
    timer_interval: std::time::Duration,
    idle: Option<(i32, bool)>,
    close_jobs: Mutex<std::collections::BinaryHeap<std::cmp::Reverse<(tokio::time::Instant, u64)>>>,
    max_clients: usize,
    log_ip: bool,
    tables: packet::dispatch::Tables,
}
impl Server {
    pub fn new(config: &GateConfig, key: RsaKey, daemon: Arc<dyn LobbyDaemon>) -> io::Result<Self> {
        if key.block_len() != 128 {
            return Err(invalid("server RSA key must be 1024 bits"));
        }
        if config.timer_interval <= 0 {
            return Err(invalid("timer-interval must be positive"));
        }
        if config.max_client <= 0 {
            return Err(invalid("max-client must be positive"));
        }
        let mut broadcast = vec![0; 17];
        OsRng.fill_bytes(&mut broadcast[..16]);
        broadcast[16] = 1;
        let mut lists = Lists::default();
        if let Some(data) = daemon.hello_lists() {
            if let Err(error) = lists.update(&data, true, &broadcast) {
                tracing::warn!(%error, "invalid Hello lists");
            }
        }
        Ok(Self {
            key,
            daemon,
            next_gate_key: AtomicU64::new(1),
            next_session: AtomicU64::new(1),
            hub: Arc::new(LoginHub::default()),
            broadcast,
            lists: Mutex::new(lists),
            contents: None,
            started: tokio::time::Instant::now(),
            close_timeout: config.client_close_timeout,
            timer_interval: std::time::Duration::from_millis(config.timer_interval as u64),
            idle: config
                .max_idle_time_enable
                .then_some((config.max_idle_time, config.disconnect_after_idle_time)),
            close_jobs: Mutex::new(std::collections::BinaryHeap::new()),
            max_clients: config.max_client as usize,
            log_ip: config.allow_client_ip_address_in_logs,
            tables: packet::dispatch::Tables::parse(packet::dispatch::DEFAULT_TABLES)
                .map_err(protocol)?,
        })
    }
    pub fn with_contents(mut self, contents: &crate::config::ContentsConfig) -> io::Result<Self> {
        self.contents = Some(crate::contents::ContentsRules::new(contents)?);
        Ok(self)
    }
    /// A single ordered task owns each connection. Shutdown drops all sockets.
    pub async fn serve(
        self: Arc<Self>,
        listener: TcpListener,
        mut shutdown: watch::Receiver<bool>,
    ) -> io::Result<()> {
        let permits = Arc::new(Semaphore::new(self.max_clients));
        let mut tasks = JoinSet::new();
        let mut events = self.daemon.take_events();
        let mut fragments = packet::fragments::FragmentReader::default();
        let mut cadence = crate::idle::cadence(tokio::time::Instant::now(), self.timer_interval);
        loop {
            // Reap completed tasks before each accept. Their retained results
            // must not accumulate when short connections arrive continuously.
            while let Some(result) = tasks.try_join_next() {
                if let Err(error) = result {
                    tracing::warn!(%error, "session task failed");
                }
            }
            if *shutdown.borrow() {
                break;
            }
            tokio::select! {
                _ = cadence.tick() => {
                    loop {
                        let job = {
                            let mut jobs = self.close_jobs.lock().unwrap();
                            if jobs.peek().is_some_and(|job| job.0.0 <= tokio::time::Instant::now()) { jobs.pop() } else { None }
                        };
                        let Some(std::cmp::Reverse((_, gate))) = job else { break; };
                        self.hub.close_gate(gate).await;
                    }
                }
                changed = shutdown.changed() => { if changed.is_err() || *shutdown.borrow() { break; } }
                packet = async { events.as_mut().unwrap().recv().await }, if events.is_some() => {
                    match packet {
                        None => events = None,
                        Some(packet) if packet.is_empty() => fragments.clear(),
                        Some(packet) => {
                            let cmd = i16::from_le_bytes([packet[2], packet[3]]);
                            let result = if matches!(cmd, 4 | 10 | 16 | 36 | 68 | 71 | 74 | 80) {
                                if packet.len() >= 8 && i32::from_le_bytes(packet[4..8].try_into().unwrap()) > 1024 * 1024 {
                                    fragments.clear();
                                    tracing::warn!(cmd, "fragment exceeds Rust safety limit");
                                    continue;
                                }
                                match fragments.add(&packet) {
                                    Ok(Some(joined)) => self.daemon_packet(cmd, &joined.data).await,
                                    Ok(None) => Ok(()),
                                    Err(error) => { fragments.clear(); Err(protocol(error)) }
                                }
                            } else {
                                self.daemon_packet(cmd, &packet).await
                            };
                            if let Err(error) = result { tracing::warn!(%error, cmd, "LobbyDaemon handler dropped packet"); }
                        }
                    }
                }
                accepted = listener.accept() => {
                    let (stream,addr) = accepted?;
                    let Ok(permit) = permits.clone().try_acquire_owned() else {
                        tracing::warn!("client limit reached");
                        drop(stream); continue;
                    };
                    let server = self.clone();
                    tasks.spawn(async move {
                        let _permit = permit;
                        if server.log_ip { tracing::info!(peer = %addr,"client accepted"); }
                        if let Err(error) = server.connection(stream).await { tracing::debug!(%error,"client closed"); }
                    });
                }
                Some(result) = tasks.join_next(), if !tasks.is_empty() => {
                    if let Err(error) = result { tracing::warn!(%error,"session task failed"); }
                }
            }
        }
        tasks.abort_all();
        while tasks.join_next().await.is_some() {}
        Ok(())
    }
    async fn connection(&self, stream: TcpStream) -> io::Result<()> {
        let state = Arc::new(Mutex::new(crate::idle::IdleState::new(
            self.idle.map_or(0, |v| v.0),
        )));
        let connection = self.connection_io(stream, state.clone());
        tokio::pin!(connection);
        // Join the global scan phase. Do not run a stale startup tick when a
        // connection arrives after the watcher has already scanned.
        let period = crate::idle::SCAN;
        let phase = self.started.elapsed().as_nanos() % period.as_nanos();
        let next =
            tokio::time::Instant::now() + period - std::time::Duration::from_nanos(phase as u64);
        let mut scan = tokio::time::interval_at(next, period);
        scan.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                result = &mut connection => return result,
                _ = scan.tick(), if self.idle.is_some() => {
                    if state.lock().unwrap().scan() {
                        tracing::warn!("client idle timeout");
                        if self.idle.is_some_and(|(_, disconnect)| disconnect) { return Ok(()); }
                    }
                }
            }
        }
    }
    async fn connection_io(
        &self,
        stream: TcpStream,
        state: Arc<Mutex<crate::idle::IdleState>>,
    ) -> io::Result<()> {
        stream.set_nodelay(true)?;
        let ip = match stream.peer_addr()?.ip() {
            std::net::IpAddr::V4(ip) => ip,
            _ => return Err(invalid("IPv4 client required")),
        };
        let (reader, mut writer) = stream.into_split();
        let mut reader = crate::idle::ReceiveWatch { reader, state };
        let Some(frame) = read_frame(&mut reader).await? else {
            return Ok(());
        };
        let (reply, mut session) = self.hello(&frame)?;
        // Mode 3: reply uses client RSA. Switch to mode 5 only after write.
        write_frame(&mut writer, &reply).await?;
        tracing::debug!("client handshake complete");
        let id = self.next_session.fetch_add(1, Ordering::Relaxed);
        let _guard = self.hub.guard(id);
        let (tx, mut rx) = mpsc::channel::<Delivery>(crate::login::DELIVERY_QUEUE_CAPACITY);
        let mut client_key = 0;
        let mut last_list_tick = 0;
        loop {
            let read = read_frame(&mut reader);
            tokio::pin!(read);
            let frame = loop {
                tokio::select! {
                    incoming = &mut read => match incoming? { Some(frame) => break frame, None => return Ok(()) },
                    Some(delivery) = rx.recv() => {
                        if delivery.close { return Ok(()); }
                        if let Some(key) = delivery.authed { client_key = key; }
                        let mut frames = Vec::with_capacity(delivery.packets.len());
                        for packet in delivery.packets {
                            frames.push(if delivery.wire { packet } else { session.encrypt(&packet, 4002, false).map_err(protocol)? });
                        }
                        write_frames(&mut writer, &frames).await?;
                        if let Some(key) = delivery.authed { self.hub.enable_broadcast(key); }
                        // Broadcast eligibility starts after the login reply.
                    }
                }
            };
            let (plain, _) = session.decrypt(&frame, 4000).map_err(protocol)?;
            let packet = inner_packet(&plain)?;
            let cmd = i16::from_le_bytes([packet[2], packet[3]]);
            tracing::debug!(
                cmd,
                len = packet.len(),
                authed = client_key != 0,
                "client packet"
            );
            match LobbyCmd::from_id(cmd) {
                Some(LobbyCmd::L_LoginWithNPTokenReq | LobbyCmd::L_LoginWithLobbyTokenReq)
                    if client_key == 0 =>
                {
                    let login = decode_login(packet, cmd)?;
                    let key = self
                        .next_gate_key
                        .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
                        .unwrap_or(0);
                    // Raw in_addr bytes. 381 compose writes: 0x1400099E2 / 0x14000A211.
                    let request = InitSession {
                        gate_key: key,
                        ip: u32::from_le_bytes(ip.octets()),
                        login,
                    };
                    if key != 0 {
                        self.hub.insert(key, id, tx.clone());
                    }
                    let sent = key != 0 && self.daemon.init_session(request).await.is_ok();
                    if !sent {
                        let fail = compose(
                            "hh",
                            &[
                                Value::I16(if cmd == LobbyCmd::L_LoginWithNPTokenReq.id() {
                                    LobbyCmd::L_LoginWithNPTokenFail.id()
                                } else {
                                    LobbyCmd::L_LoginWithLobbyTokenFail.id()
                                }),
                                Value::I16(2),
                            ],
                        )
                        .map_err(protocol)?;
                        let frame = session.encrypt(&fail, 4002, false).map_err(protocol)?;
                        write_frame(&mut writer, &frame).await?;
                    }
                    // Keep the gate key after a failed D6/D12 send (§1.2).
                }
                Some(LobbyCmd::L_PingReq) => (),
                Some(
                    LobbyCmd::L_GetWorldInfoListReq
                    | LobbyCmd::L_GetPcCafeMileageFeverTimeScheduleListReq,
                ) if client_key != 0 => {
                    let cached = {
                        let lists = self.lists.lock().unwrap();
                        if cmd == LobbyCmd::L_GetWorldInfoListReq.id() {
                            lists.worlds.clone()
                        } else {
                            lists.schedule.clone()
                        }
                    };
                    let reason = if !self.daemon.service_available() {
                        Some(7)
                    } else if cached.is_none() {
                        Some(2)
                    } else {
                        None
                    };
                    if let Some(reason) = reason {
                        let packet = compose(
                            "hh",
                            &[
                                // TSV names id 19 L_GetWaitInfoFail (world list fail is 26).
                                Value::I16(if cmd == LobbyCmd::L_GetWorldInfoListReq.id() {
                                    LobbyCmd::L_GetWaitInfoFail.id()
                                } else {
                                    LobbyCmd::L_GetPcCafeMileageFeverTimeScheduleListFail.id()
                                }),
                                Value::I16(reason),
                            ],
                        )
                        .map_err(protocol)?;
                        write_frame(
                            &mut writer,
                            &session.encrypt(&packet, 4002, false).map_err(protocol)?,
                        )
                        .await?;
                    } else {
                        for frame in cached.unwrap() {
                            write_frame(&mut writer, &frame).await?;
                        }
                    }
                }
                Some(LobbyCmd::L_ContentsRuleListReq) => {
                    let now = self.started.elapsed().as_millis().min(u64::MAX as u128) as u64;
                    if let Some(rules) = &self.contents {
                        for packet in rules.reply(now, &mut last_list_tick) {
                            write_frame(
                                &mut writer,
                                &session.encrypt(&packet, 4002, false).map_err(protocol)?,
                            )
                            .await?;
                        }
                    } else {
                        last_list_tick = now;
                    }
                }
                _ if matches!(
                    self.tables.client_route(cmd, client_key != 0),
                    packet::dispatch::Route::Relay
                ) =>
                {
                    let packet = self
                        .tables
                        .relay_to_lobbyd(packet, client_key, self.daemon.connected())
                        .map_err(protocol)?;
                    self.daemon.send_packet(packet).await?;
                }
                _ => return Err(invalid("unsupported client command")),
            }
        }
    }
    async fn daemon_packet(&self, cmd: i16, data: &[u8]) -> io::Result<()> {
        let tables = &self.tables;
        tracing::debug!(cmd, len = data.len(), "lobbyd packet");
        if matches!(tables.lobbyd_route(cmd), packet::dispatch::Route::Relay) {
            let (key, packet) = tables.relay_to_client(data).map_err(protocol)?;
            self.hub.send_client(key, vec![packet]).await;
            return Ok(());
        }
        match cmd {
            2 => {
                if let Some(body) = self.daemon.take_hello_snapshot() {
                    self.lists
                        .lock()
                        .unwrap()
                        .update(&body, true, &self.broadcast)?;
                }
            }
            4 => self
                .lists
                .lock()
                .unwrap()
                .update(data, false, &self.broadcast)?,
            5 => self
                .hub
                .broadcast(Lists::announcement(data, &self.broadcast)?),
            20..=22 => {
                let values = decompose(
                    if cmd == 22 { "hqQ" } else { "hq" },
                    data,
                    Default::default(),
                )
                .map_err(protocol)?;
                let Value::U64(key) = values[1] else {
                    unreachable!()
                };
                if cmd != 20 {
                    if let Some(gate) = self.hub.gate_for_client(key) {
                        if cmd == 22 {
                            if !self.hub.has_session(key) {
                                return Ok(());
                            }
                            let kick = compose(
                                "hQ",
                                &[Value::I16(LobbyCmd::L_KickNotify.id()), values[2].clone()],
                            )
                            .map_err(protocol)?;
                            self.hub.send_client(key, vec![kick]).await;
                        }
                        if self.close_timeout > 0 {
                            self.close_jobs.lock().unwrap().push(std::cmp::Reverse((
                                tokio::time::Instant::now()
                                    + std::time::Duration::from_millis(self.close_timeout as u64),
                                gate,
                            )));
                        }
                    }
                }
            }
            68 | 71 | 74 | 80 => {
                let (key, packets) = match cmd {
                    68 => crate::goods::list_goods(data)?,
                    71 => crate::goods::take_goods(data)?,
                    74 => crate::goods::service_items(data)?,
                    _ => crate::goods::service_entries(data)?,
                };
                self.hub.send_client(key, packets).await;
            }
            25 | 36 => {
                let (key, packets) = if cmd == 25 {
                    crate::replies::token(data)?
                } else {
                    crate::replies::characters(data)?
                };
                self.hub.send_client(key, packets).await;
            }
            _ => {
                self.hub
                    .handle(cmd, data, self.daemon.as_ref(), &self.broadcast)
                    .await?
            }
        }
        Ok(())
    }
    // The const for the pattern keeps the official name.
    #[allow(non_upper_case_globals)]
    fn hello(&self, frame: &[u8]) -> io::Result<(Vec<u8>, SessionPacketCrypt)> {
        let decoded = decode_client(
            frame,
            ClientDecrypt::Handshake {
                key: &self.key,
                expected_code: 4,
            },
        )
        .map_err(protocol)?
        .ok_or_else(|| invalid("incomplete HelloReq"))?;
        let fields = decompose(
            "hhb",
            &decoded.packet,
            DecodeLimits {
                blob_bytes: 4000,
                ..DecodeLimits::default()
            },
        )
        .map_err(protocol)?;
        const L_HelloReq: i16 = LobbyCmd::L_HelloReq.id();
        let [Value::I16(L_HelloReq), Value::I16(58), Value::Bytes(blob)] = fields.as_slice() else {
            return Err(invalid("invalid HelloReq command or version"));
        };
        if blob.len() < 4 {
            return Err(invalid("empty handshake data"));
        }
        let rsa_len = u32::from_le_bytes(blob[..4].try_into().unwrap()) as usize;
        if rsa_len != 64 || blob.len() < 68 {
            return Err(invalid("client RSA key must be 512 bits"));
        }
        let client = RsaKey::from_public_blob(&blob[..68]).map_err(protocol)?;
        let dh = DhParameters::from_blob(&blob[68..])
            .map_err(protocol)?
            .exchange(&mut OsRng);
        let reply = compose(
            "hb",
            &[Value::I16(LobbyCmd::L_HelloOk.id()), Value::Bytes(dh.reply)],
        )
        .map_err(protocol)?;
        let reply = encode_client(
            &reply,
            ClientEncrypt::Handshake {
                key: &client,
                code: 5,
            },
            &mut OsRng,
        )
        .map_err(protocol)?;
        Ok((reply, SessionPacketCrypt::new(dh.session_key)))
    }
}
// The consts for the patterns keep the official names.
#[allow(non_upper_case_globals)]
pub fn decode_login(packet: &[u8], cmd: i16) -> io::Result<LoginToken> {
    const L_LoginWithNPTokenReq: i16 = LobbyCmd::L_LoginWithNPTokenReq.id();
    const L_LoginWithLobbyTokenReq: i16 = LobbyCmd::L_LoginWithLobbyTokenReq.id();
    let fields = decompose(
        if cmd == L_LoginWithNPTokenReq {
            "hoo"
        } else {
            "hqq"
        },
        packet,
        DecodeLimits::default(),
    )
    .map_err(protocol)?;
    match fields.as_slice() {
        [Value::I16(L_LoginWithNPTokenReq), Value::Id(token), Value::Id(portal)] => {
            Ok(LoginToken::Np {
                token: *token,
                portal: *portal,
            })
        }
        [Value::I16(L_LoginWithLobbyTokenReq), Value::U64(client_key), Value::U64(token)] => {
            Ok(LoginToken::Lobby {
                client_key: *client_key,
                token: *token,
            })
        }
        _ => Err(invalid("invalid login request")),
    }
}

/// Only the client port listens. Spec 03 §1 excludes internal-port.
pub async fn bind(config: &GateConfig) -> io::Result<TcpListener> {
    TcpListener::bind((Ipv4Addr::UNSPECIFIED, config.client_port)).await
}
