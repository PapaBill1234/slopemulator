//! `PlayStubs --config StServer.ini --route-ini Route.ini [--index N] [--server-id N]`
//! `StServer.ini` needs a `PlayStubs` line.
use std::io;
use sts_core::service;

#[tokio::main]
async fn main() -> io::Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let started = service::start("PlayStubs", &play_stubs::PROTOCOLS, &[], &mut args)?;
    service::serve("PlayStubs", started, |protocol, command, _, _| {
        play_stubs::handle(protocol, command)
    })
    .await;
    Ok(())
}
