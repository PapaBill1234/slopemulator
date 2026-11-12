//! Connection task: one for each client. It owns the socket and the session crypto.
//! It decrypts client frames for the shard and encrypts the packets of the shard.
//! It holds no game state and knows no game rule (`scale-rules.md` section 3, layer Net).
use crate::{
    entry::{bad, hello},
    shard::{Event, Inbox, ToConn},
};
use bloodlib::{crypto::RsaKey, session::SessionPacketCrypt};
use bytes::{Bytes, BytesMut};
use lobbygate::wire::inner_packet;
use shared::net::{tune, ClientFrameReader, CLIENT_QUEUE};
use std::io;
use tokio::{
    io::{AsyncRead, AsyncWrite, AsyncWriteExt},
    net::TcpStream,
    sync::mpsc,
};

/// Bytes of one write (rule H4).
const BATCH_BYTES: usize = 64 * 1024;

fn hex(data: &[u8]) -> String {
    data.iter().map(|b| format!("{b:02x}")).collect()
}

fn command(packet: &[u8]) -> i16 {
    packet
        .get(2..4)
        .map_or(0, |c| i16::from_le_bytes([c[0], c[1]]))
}

/// Serve one client to the end of its connection. `route`: the entry shard.
/// `log`: one line for each packet (for protocol work; off in normal use, rule H12).
pub async fn run(
    stream: TcpStream,
    conn: u32,
    key: &RsaKey,
    mut route: Inbox,
    log: bool,
) -> io::Result<()> {
    tune(&stream)?;
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
    let (reply, session) = hello(key, &frame)?;
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
    log: bool,
) -> io::Result<()> {
    let mut batch = BytesMut::with_capacity(8 * 1024);
    loop {
        tokio::select! {
            frame = reader.next() => {
                let Some(frame) = frame? else { return Ok(()) };
                let (mut plain, _) = session.decrypt(&frame, 4000).map_err(bad)?;
                let size = inner_packet(&plain)?.len();
                plain.truncate(size);
                if log {
                    eprintln!("gamed: client {conn} cmd {} body {}", command(&plain), hex(&plain[4..]));
                }
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
                            if log {
                                eprintln!(
                                    "gamed: client {conn} reply cmd {} ({} bytes)",
                                    command(&packet),
                                    packet.len()
                                );
                            }
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
