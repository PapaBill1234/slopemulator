//! `lobbyd [local-port] [--gamegate <addr:port> [--game-code <code>]] [--world <id>,<cluster>,<name>,<ip>,<port>]...`
//! Without `--gamegate` the empty stub rejects all tokens.
use gamegate::np_client::GateNp;
use lobbydaemon::{jobs::Settings, np::StubNp, protocol::WorldInfo, store::MemoryStore, Server};
use shared::np_channel::Np;
use std::{
    io,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    sync::Arc,
};
use tokio::{net::TcpListener, sync::watch};

const USAGE: &str = "usage: lobbyd [local-port] [--gamegate <addr:port> [--game-code <code>]] [--world <id>,<cluster>,<name>,<ip>,<port>]...";

fn usage() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, USAGE)
}

fn world(spec: &str) -> io::Result<WorldInfo> {
    let parts: Vec<&str> = spec.split(',').collect();
    let [id, cluster, name, ip, port] = parts[..] else {
        return Err(usage());
    };
    let ip: Ipv4Addr = ip.parse().map_err(|_| usage())?;
    Ok(WorldInfo {
        id: id.parse().map_err(|_| usage())?,
        cluster: cluster.parse().map_err(|_| usage())?,
        name: name.into(),
        public_ip: ip,
        internal_ip: ip,
        public_port: port.parse().map_err(|_| usage())?,
        world_type: 0,
        play_type: 0,
    })
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let mut port = 0u16;
    let mut gate: Option<SocketAddr> = None;
    let mut game_code = String::from("bns");
    let mut settings = Settings::default();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or_else(usage);
        match arg.as_str() {
            "--gamegate" => gate = Some(value()?.parse().map_err(|_| usage())?),
            "--game-code" => game_code = value()?,
            "--world" => {
                settings.worlds.push(world(&value()?)?);
                if settings.worlds.len() > 16 {
                    return Err(usage());
                }
            }
            _ => port = arg.parse().map_err(|_| usage())?,
        }
    }
    // Loopback only.
    let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).await?;
    match gate {
        Some(gate) => {
            println!(
                "lobbyd listens on {}, tokens through GameGate {gate}",
                listener.local_addr()?
            );
            // `Program` of the Connect body: program id 302 of `bnsserver` in the original Game.ini.
            let mut backend = StubNp::default();
            backend.presence_replace = true;
            let np = GateNp::new(gate, game_code, 302, backend);
            run(np, settings, listener, true).await
        }
        None => {
            println!("lobbyd local stub listens on {}", listener.local_addr()?);
            run(StubNp::default(), settings, listener, false).await
        }
    }
}

async fn run<N: Np>(
    np: N,
    settings: Settings,
    listener: TcpListener,
    local_game: bool,
) -> io::Result<()> {
    #[cfg(feature = "mssql")]
    if std::env::var_os("LOBBYD_MSSQL_URL").is_some() {
        let store = tokio::task::spawn_blocking(lobbydaemon::mssql::MssqlStore::from_env)
            .await
            .map_err(io::Error::other)??;
        return serve(store, np, settings, listener, local_game).await;
    }
    serve(MemoryStore::default(), np, settings, listener, local_game).await
}

async fn serve<S: lobbydaemon::store::Store, N: Np>(
    store: S,
    np: N,
    settings: Settings,
    listener: TcpListener,
    local_game: bool,
) -> io::Result<()> {
    // With `--gamegate` (play stack) characters are made locally: no GameDaemon server exists.
    let mut settings = settings;
    if local_game {
        // ❓ Local policy: names of 2..=16 letters, 7 days deletion wait.
        settings.character_policy = Some(lobbydaemon::character::CharacterPolicy {
            slots: Default::default(),
            worlds: settings.worlds.iter().map(|w| w.id).collect(),
            min_name_units: 2,
            max_name_units: 16,
            deletion_wait_seconds: 7 * 24 * 3600,
        });
    }
    let mut server = Server::new(store, np, settings)?;
    if local_game {
        server = server.with_game_daemon(Arc::new(lobbydaemon::gamed::LocalGameDaemon));
    }
    let server = Arc::new(server);
    let (stop, shutdown) = watch::channel(false);
    let mut task = tokio::spawn(server.serve(listener, shutdown));
    tokio::select! {
        result = &mut task => result.map_err(io::Error::other)?,
        result = tokio::signal::ctrl_c() => {
            result?;
            let _ = stop.send(true);
            task.await.map_err(io::Error::other)?
        }
    }
}
