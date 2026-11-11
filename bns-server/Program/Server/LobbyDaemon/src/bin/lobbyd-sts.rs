//! Local test pairing of sts-local and lobbyd in one process.
//! Usage: lobbyd-sts --lobbyd-port <port> <sts-local options...>
//! Game tokens issued on the STS listener are consumed by lobbyd through
//! `sts::np_adapter::AuthorityNp`. Presence, names and world entry use the
//! empty `StubNp` backend. Loopback only. No account data is printed.
use lobbydaemon::{
    jobs::Settings,
    np::StubNp,
    store::{MemoryStore, Shared},
    Server,
};
use std::{
    io,
    net::{Ipv4Addr, SocketAddrV4},
    sync::Arc,
};
use sts::{listener, np_adapter::AuthorityNp, tokens::TokenAuthority};
use tokio::{net::TcpListener, sync::watch};

fn usage() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "usage: lobbyd-sts --lobbyd-port <port> [--world <id>,<cluster>,<name>,<ip>,<port>]... <sts-local options>",
    )
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 || args[0] != "--lobbyd-port" {
        return Err(usage());
    }
    let port: u16 = args[1].parse().map_err(|_| usage())?;
    args.drain(..2);
    // Optional fixture worlds: --world <id>,<cluster>,<name>,<public-ip>,<port>
    let mut settings = Settings::default();
    while args.first().is_some_and(|a| a == "--world") {
        let spec = args.get(1).ok_or_else(usage)?.clone();
        args.drain(..2);
        let parts: Vec<&str> = spec.split(',').collect();
        let [id, cluster, name, ip, port] = parts[..] else {
            return Err(usage());
        };
        let ip: Ipv4Addr = ip.parse().map_err(|_| usage())?;
        settings.worlds.push(lobbydaemon::protocol::WorldInfo {
            id: id.parse().map_err(|_| usage())?,
            cluster: cluster.parse().map_err(|_| usage())?,
            name: name.into(),
            public_ip: ip,
            internal_ip: ip,
            public_port: port.parse().map_err(|_| usage())?,
            world_type: 0,
            play_type: 0,
        });
        if settings.worlds.len() > 16 {
            return Err(usage());
        }
    }
    let (config, store) =
        listener::load(args).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    let authority = Arc::new(
        TokenAuthority::new(config.token_lifetime, config.token_capacity).map_err(|_| usage())?,
    );
    let np = AuthorityNp::new(authority.clone(), config.binding.clone(), StubNp::default())
        .map_err(|_| usage())?;
    let sts_addr = config.bind;
    std::thread::spawn(move || {
        if let Err(error) = listener::run(&config, &store, authority) {
            eprintln!("sts-local: {error}");
            std::process::exit(1);
        }
    });
    let lobby = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).await?;
    println!(
        "lobbyd-sts: sts on {sts_addr}, lobbyd on {}",
        lobby.local_addr()?
    );
    let server = Arc::new(Server::new(
        Shared::new(MemoryStore::default()),
        np,
        settings,
    )?);
    let (stop, shutdown) = watch::channel(false);
    let mut task = tokio::spawn(server.serve(lobby, shutdown));
    tokio::select! {
        result = &mut task => result.map_err(io::Error::other)?,
        result = tokio::signal::ctrl_c() => {
            result?;
            let _ = stop.send(true);
            task.await.map_err(io::Error::other)?
        }
    }
}
