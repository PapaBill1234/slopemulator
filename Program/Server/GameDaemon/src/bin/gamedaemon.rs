//! `gamedaemon --listen <addr:port> --handshake-key <file> [--lobby <addr:port>] [--world <cluster>,<world>]`
//!
//! Client listener of the play stack (card world-1). Spec: Docs/Server/GameDaemon/04, 05.
//! Structure: `conn.rs` (one task for each client), `shard.rs` (one task for each zone),
//! `play.rs` (pure session logic), `entry.rs` (packet builders).
//! Done: Hello 11/12 (RSA handshake, then AES-128 session), token 23 → 24, log of each command.
//! Enter world: 31 → 32 (fragments) and 41; 42 → 43. Spec: 10-session-handlers.md.
//! `--lobby`: LobbyDaemon link (07-gamed-link.md). Command 23 consumes the world token there.
//! Game DB: `BlGame`, connection string in the environment variable `GAMED_MSSQL_URL`.
//! Command 31 loads the PC with `usp_LoadCreatureProperty`.
//! 929 → 930: rule list from `--contents-rule <ContentsRule.gamed.config.xml>`.
//! ❓ The sync packets after 32 have no reply yet.
use gamedaemon::{
    conn,
    db::{Db, Ordered},
    entry::{bad, rule_list},
    link::Link,
    shard::World,
};
use std::{io, sync::Arc};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> io::Result<()> {
    let mut args = std::env::args().skip(1);
    let (mut listen, mut key_path, mut rule_path) = (None, None, None);
    let (mut lobby, mut cluster, mut world) = (None, 1i16, 1i16);
    while let Some(arg) = args.next() {
        let value = args.next().ok_or_else(|| bad("missing option value"))?;
        match arg.as_str() {
            "--listen" => listen = Some(value),
            "--handshake-key" => key_path = Some(value),
            "--contents-rule" => rule_path = Some(value),
            "--lobby" => lobby = Some(value),
            "--world" => {
                let ids = value
                    .split_once(',')
                    .ok_or_else(|| bad("--world <cluster>,<world>"))?;
                cluster = ids.0.parse().map_err(bad)?;
                world = ids.1.parse().map_err(bad)?;
            }
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
    // No file: no reply to command 929 (the client then waits).
    let rules: Vec<bytes::Bytes> = match rule_path {
        Some(path) => rule_list(&std::fs::read_to_string(path)?)?,
        None => Vec::new(),
    }
    .into_iter()
    .map(Into::into)
    .collect();
    eprintln!("gamed: rule list, {} fragments", rules.len());
    let db = match Db::from_env()? {
        Some(mut db) => {
            db.prepare().await?;
            eprintln!("gamed: game DB BlGame (SQL Server)");
            Some(Arc::new(db))
        }
        None => {
            eprintln!("gamed: no game DB (GAMED_MSSQL_URL is not set)");
            None
        }
    };
    let listener = TcpListener::bind(&listen).await?;
    let port = listener.local_addr()?.port() as i16;
    let link = lobby.map(|lobby| {
        let link = Link::new(cluster, world, port, db.clone());
        tokio::spawn(link.clone().run(lobby));
        link
    });
    let ordered = db.map(|db| Ordered::new(db.clone(), db.pool_size()));
    let world = World::new(rules, link, ordered);
    let entry = world.shard(0);
    // One line for each packet: only with `GAMED_PACKET_LOG` (rule H12).
    let log = std::env::var_os("GAMED_PACKET_LOG").is_some();
    eprintln!("gamed: listening on {listen}");
    let mut next = 0u32;
    loop {
        let (stream, peer) = listener.accept().await?;
        next = next.wrapping_add(1);
        let conn = next;
        eprintln!("gamed: client {conn} from {peer}");
        let (key, entry) = (key.clone(), entry.clone());
        tokio::spawn(async move {
            let result = conn::run(stream, conn, &key, entry, log).await;
            eprintln!("gamed: client {conn} closed: {result:?}");
        });
    }
}
