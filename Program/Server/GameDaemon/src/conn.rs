//! Connection task: one for each client. It owns the socket and the session crypto.
//! It decrypts client frames for the shard and encrypts the packets of the shard.
//! It holds no game state and knows no game rule (`scale-rules.md` section 3, layer Net).
//! It writes the packet log (spec 06 "Packet log contract").
use crate::{
    entry::{bad, hello_sizes},
    ids,
    shard::{Event, Inbox, ToConn},
};
use bloodlib::{crypto::RsaKey, session::SessionPacketCrypt};
use bytes::{Bytes, BytesMut};
use lobbygate::wire::inner_packet;
use packet_ids::GameCmd;
use shared::net::{tune, ClientFrameReader, CLIENT_QUEUE};
use std::{io, time::Instant};
use tokio::{
    io::{AsyncRead, AsyncWrite, AsyncWriteExt},
    net::TcpStream,
    sync::mpsc,
};

/// Bytes of one write (rule H4).
const BATCH_BYTES: usize = 64 * 1024;
/// Bytes of the body on one line of a hex dump.
const DUMP_WIDTH: usize = 32;
/// Environment variable of the packet log.
pub const LOG_VARIABLE: &str = "GAMED_PACKET_LOG";
/// Written in a dump in place of key bytes.
const KEY_TEXT: &str = "keyhere";

/// Direction of a logged packet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    /// Client → server.
    Recv,
    /// Server → client.
    Send,
}

impl Dir {
    fn letter(self) -> char {
        match self {
            Dir::Recv => 'R',
            Dir::Send => 'S',
        }
    }
}

/// What the packet log writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogMode {
    Off,
    /// One line for each packet.
    Line,
    /// One line and a hex dump of the body.
    Hex,
}

impl LogMode {
    /// Not set or empty: off. `hex`: line and dump. Any other value: line.
    pub fn parse(value: Option<&str>) -> Self {
        match value {
            None | Some("") => LogMode::Off,
            Some("hex") => LogMode::Hex,
            Some(_) => LogMode::Line,
        }
    }

    pub fn from_env() -> Self {
        let value = std::env::var_os(LOG_VARIABLE);
        Self::parse(value.as_ref().map(|v| v.to_string_lossy()).as_deref())
    }
}

/// One log line: `time dir id name size`. `name` is `?` when the table has no name.
pub fn log_line(time_ms: u64, dir: Dir, id: u16, size: usize) -> String {
    let name = i16::try_from(id)
        .ok()
        .and_then(GameCmd::from_id)
        .map_or("?", GameCmd::name);
    format!("{time_ms} {} {id} {name} {size}", dir.letter())
}

/// The text of the body in a dump: two-digit hex for each byte. Key material is not written:
/// the whole body of 11 and 12, and the key bytes of 41, become `keyhere`.
fn dump_tokens(id: u16, body: &[u8]) -> Vec<String> {
    fn hex(bytes: &[u8]) -> impl Iterator<Item = String> + '_ {
        bytes.iter().map(|b| format!("{b:02x}"))
    }
    if id == ids::G_HelloReqCmd || id == ids::G_HelloOkCmd {
        return if body.is_empty() {
            Vec::new()
        } else {
            vec![KEY_TEXT.to_owned()]
        };
    }
    if id == ids::G_PrepareEnterZoneCmd {
        // Body: q h i i c c, then K (u16 length and the key bytes).
        const KEY_AT: usize = 20;
        if let Some(prefix) = body.get(KEY_AT..KEY_AT + 2) {
            let length = usize::from(u16::from_le_bytes([prefix[0], prefix[1]]));
            let end = (KEY_AT + 2 + length).min(body.len());
            let mut tokens: Vec<String> = hex(&body[..KEY_AT + 2]).collect();
            tokens.push(KEY_TEXT.to_owned());
            tokens.extend(hex(&body[end..]));
            return tokens;
        }
    }
    hex(body).collect()
}

/// The lines of one packet: the line and, in `hex` mode, the dump lines of the body. Empty when
/// the log is off. `packet`: size field, command and body.
pub fn log_lines(mode: LogMode, time_ms: u64, dir: Dir, packet: &[u8]) -> Vec<String> {
    if mode == LogMode::Off {
        return Vec::new();
    }
    let id = packet
        .get(2..4)
        .map_or(0, |c| u16::from_le_bytes([c[0], c[1]]));
    let mut lines = vec![log_line(time_ms, dir, id, packet.len())];
    if mode == LogMode::Hex {
        let body = packet.get(4..).unwrap_or(&[]);
        for line in dump_tokens(id, body).chunks(DUMP_WIDTH) {
            lines.push(format!("  {}", line.concat()));
        }
    }
    lines
}

/// Writer of the packet log of one connection.
#[derive(Clone, Copy)]
struct PacketLog {
    mode: LogMode,
    start: Instant,
}

impl PacketLog {
    fn write(&self, dir: Dir, packet: &[u8]) {
        if self.mode == LogMode::Off {
            return;
        }
        let time = self.start.elapsed().as_millis() as u64;
        for line in log_lines(self.mode, time, dir, packet) {
            eprintln!("{line}");
        }
    }

    /// A packet that is known only by its command and size (the plain Hello packets).
    fn write_size(&self, dir: Dir, id: u16, size: usize) {
        if self.mode != LogMode::Off {
            let time = self.start.elapsed().as_millis() as u64;
            eprintln!("{}", log_line(time, dir, id, size));
        }
    }
}

/// Serve one client to the end of its connection. `route`: the entry shard.
/// `log`: switch for the packet log (off in normal use, rule H12). The variable
/// `GAMED_PACKET_LOG` selects the form: `hex` adds a dump of each body.
pub async fn run(
    stream: TcpStream,
    conn: u32,
    key: &RsaKey,
    mut route: Inbox,
    log: bool,
) -> io::Result<()> {
    tune(&stream)?;
    let log = PacketLog {
        mode: if log {
            LogMode::from_env()
        } else {
            LogMode::Off
        },
        start: Instant::now(),
    };
    // Same form as LobbyGate sends it to LobbyDaemon (`LobbyGate/src/server.rs`).
    let address = match stream.peer_addr()?.ip() {
        std::net::IpAddr::V4(ip) => u32::from_le_bytes(ip.octets()),
        std::net::IpAddr::V6(_) => 0,
    };
    let (reader, mut writer) = stream.into_split();
    let mut reader = ClientFrameReader::new(reader);
    let Some(frame) = reader.next().await? else {
        return Ok(());
    };
    let (reply, session, sizes) = hello_sizes(key, &frame)?;
    log.write_size(Dir::Recv, ids::G_HelloReqCmd, sizes[0]);
    log.write_size(Dir::Send, ids::G_HelloOkCmd, sizes[1]);
    writer.write_all(&reply).await?;
    writer.flush().await?;
    let (out, inbox) = mpsc::channel(CLIENT_QUEUE);
    let open = Event::Open { conn, address, out };
    if route.send(open).await.is_err() {
        return Ok(());
    }
    let result = serve(
        conn,
        &mut reader,
        &mut writer,
        session,
        inbox,
        &mut route,
        log,
    )
    .await;
    let _ = route.send(Event::Closed { conn }).await;
    result
}

async fn serve(
    conn: u32,
    reader: &mut ClientFrameReader<impl AsyncRead + Unpin>,
    writer: &mut (impl AsyncWrite + Unpin),
    mut session: SessionPacketCrypt,
    mut inbox: mpsc::Receiver<ToConn>,
    route: &mut Inbox,
    log: PacketLog,
) -> io::Result<()> {
    let mut batch = BytesMut::with_capacity(8 * 1024);
    loop {
        tokio::select! {
            frame = reader.next() => {
                let Some(frame) = frame? else { return Ok(()) };
                let (mut plain, _) = session.decrypt(&frame, 4000).map_err(bad)?;
                let size = inner_packet(&plain)?.len();
                plain.truncate(size);
                log.write(Dir::Recv, &plain);
                let packet = Bytes::from(plain);
                // A full shard inbox makes this client wait (rule H6).
                if route.send(Event::Packet { conn, packet }).await.is_err() {
                    return Ok(());
                }
            }
            message = inbox.recv() => {
                // `None`: the shard ended the session.
                let Some(mut message) = message else { return Ok(()) };
                // Take all that waits, in order, then write one time (rule H4).
                loop {
                    match message {
                        ToConn::Packet(packet) => {
                            log.write(Dir::Send, &packet);
                            let frame = session.encrypt(&packet, 4002, false).map_err(bad)?;
                            batch.extend_from_slice(&frame);
                        }
                        ToConn::Route(next) => {
                            let _ = route.send(Event::Switched { conn }).await;
                            *route = next;
                        }
                    }
                    if batch.len() >= BATCH_BYTES {
                        break;
                    }
                    match inbox.try_recv() {
                        Ok(next) => message = next,
                        Err(_) => break,
                    }
                }
                if !batch.is_empty() {
                    writer.write_all(&batch).await?;
                    writer.flush().await?;
                    batch.clear();
                }
            }
        }
    }
}
