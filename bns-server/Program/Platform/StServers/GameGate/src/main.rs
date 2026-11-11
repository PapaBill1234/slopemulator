//! `GameGate --config StServer.ini --route-ini Route.ini [--listen addr:port] [--index N] [--server-id N]`
use gamegate::gate::GameGate;
use std::io;
use sts_core::{route_client::parse_systems, service};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> io::Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut started = service::start("GameGate", &["Game"], &["Auth"], &mut args)?;
    let listen = match &args[..] {
        [flag, addr] if flag == "--listen" => addr.clone(),
        [] => parse_systems(&started.ini)
            .ok()
            .and_then(|e| e.into_iter().find(|e| e.server_name == "GameGate"))
            .map(|e| e.addr.to_string())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no listen address"))?,
        _ => return Err(io::Error::new(io::ErrorKind::InvalidInput, "bad arguments")),
    };
    let listener = TcpListener::bind(&listen).await?;
    eprintln!("GameGate: daemons on {listen}");
    let gate = GameGate::new(started.route.clone(), started.server_id);
    // Bus → daemon requests are not ported: the events are read and dropped.
    tokio::spawn(async move { while started.events.recv().await.is_some() {} });
    gate.serve(listener).await
}
