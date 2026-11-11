//! `gamedaemon --listen <addr:port> --handshake-key <file> [--lobby <addr:port>]`
//!
//! Client listener of the play stack (card world-1). Spec: Docs/Server/GameDaemon/04, 05.
//! Done: Hello 11/12 (RSA handshake, then AES-128 session), token 23 → 24, log of each command.
//! ❓ Not parity: the token is not consumed through LobbyDaemon (no link). `--lobby` is not used yet.
//! ❓ Commands 929, 31 and later have no reply yet.
use bloodlib::{
    crypto::{DhParameters, RsaKey},
    frames::{decode_client, encode_client, ClientDecrypt, ClientEncrypt},
    packet::{compose, decompose, DecodeLimits, Value},
    session::SessionPacketCrypt,
};
use lobbygate::wire::{inner_packet, read_frame, write_frame};
use rand::rngs::OsRng;
use std::{io, sync::Arc};
use tokio::net::{TcpListener, TcpStream};

const PROTOCOL: i16 = 1278;

fn bad(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}

fn hex(data: &[u8]) -> String {
    data.iter().map(|b| format!("{b:02x}")).collect()
}

/// Command 11 `hhqsqb` → command 12 `hb`. The blob layout is the LobbyGate layout.
fn hello(key: &RsaKey, frame: &[u8]) -> io::Result<(Vec<u8>, SessionPacketCrypt)> {
    let decoded = decode_client(
        frame,
        ClientDecrypt::Handshake {
            key,
            expected_code: 4,
        },
    )
    .map_err(bad)?
    .ok_or_else(|| bad("incomplete Hello"))?;
    eprintln!("gamed: hello packet {}", hex(&decoded.packet));
    let limits = DecodeLimits {
        blob_bytes: 4000,
        ..DecodeLimits::default()
    };
    let fields = decompose("hhqsqb", &decoded.packet, limits).map_err(bad)?;
    let [Value::I16(11), Value::I16(PROTOCOL), _, _, _, Value::Bytes(blob)] = fields.as_slice()
    else {
        return Err(bad("invalid Hello command or version"));
    };
    if blob.len() < 68 || u32::from_le_bytes(blob[..4].try_into().unwrap()) != 64 {
        return Err(bad("client RSA key must be 512 bits"));
    }
    let client = RsaKey::from_public_blob(&blob[..68]).map_err(bad)?;
    let dh = DhParameters::from_blob(&blob[68..])
        .map_err(bad)?
        .exchange(&mut OsRng);
    let reply = compose("hb", &[Value::I16(12), Value::Bytes(dh.reply)]).map_err(bad)?;
    let reply = encode_client(
        &reply,
        ClientEncrypt::Handshake {
            key: &client,
            code: 5,
        },
        &mut OsRng,
    )
    .map_err(bad)?;
    Ok((reply, SessionPacketCrypt::new(dh.session_key)))
}

async fn connection(key: Arc<RsaKey>, stream: TcpStream) -> io::Result<()> {
    stream.set_nodelay(true)?;
    let (mut reader, mut writer) = stream.into_split();
    let Some(frame) = read_frame(&mut reader).await? else {
        return Ok(());
    };
    let (reply, mut session) = hello(&key, &frame)?;
    write_frame(&mut writer, &reply).await?;
    eprintln!("gamed: hello ok");
    while let Some(frame) = read_frame(&mut reader).await? {
        let (plain, _) = session.decrypt(&frame, 4000).map_err(bad)?;
        let packet = inner_packet(&plain)?;
        let cmd = i16::from_le_bytes([packet[2], packet[3]]);
        eprintln!("gamed: client cmd {cmd} body {}", hex(&packet[4..]));
        let reply = match cmd {
            // 23 `hhqq` world, client key, lobby token → 24 `hQiQih` (10-session-handlers.md §23).
            23 => Some(
                compose(
                    "hQiQih",
                    &[
                        Value::I16(24),
                        Value::I8(0),
                        Value::I32(0),
                        Value::I8(0),
                        Value::I32(0),
                        Value::I16(0),
                    ],
                )
                .map_err(bad)?,
            ),
            _ => None,
        };
        if let Some(reply) = reply {
            eprintln!(
                "gamed: reply cmd {}",
                i16::from_le_bytes([reply[2], reply[3]])
            );
            let frame = session.encrypt(&reply, 4002, false).map_err(bad)?;
            write_frame(&mut writer, &frame).await?;
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let mut args = std::env::args().skip(1);
    let (mut listen, mut key_path) = (None, None);
    while let Some(arg) = args.next() {
        let value = args.next().ok_or_else(|| bad("missing option value"))?;
        match arg.as_str() {
            "--listen" => listen = Some(value),
            "--handshake-key" => key_path = Some(value),
            "--lobby" => {}
            _ => {
                return Err(bad(
                    "usage: gamedaemon --listen <addr:port> --handshake-key <file>",
                ))
            }
        }
    }
    let listen = listen.ok_or_else(|| bad("--listen is necessary"))?;
    let key_path = key_path.ok_or_else(|| bad("--handshake-key is necessary"))?;
    let key = Arc::new(lobbygate::server::load_handshake_key(key_path.as_ref())?);
    let listener = TcpListener::bind(&listen).await?;
    eprintln!("gamed: listening on {listen}");
    loop {
        let (stream, peer) = listener.accept().await?;
        eprintln!("gamed: client {peer}");
        let key = key.clone();
        tokio::spawn(async move {
            let result = connection(key, stream).await;
            eprintln!("gamed: client {peer} closed: {result:?}");
        });
    }
}
