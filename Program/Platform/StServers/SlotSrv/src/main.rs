//! `SlotSrv --config StServer.ini --route-ini Route.ini [--index N] [--server-id N]`
use std::io;
use sts_core::{service, StsError};

#[tokio::main]
async fn main() -> io::Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let started = service::start("SlotSrv", &["Slot"], &[], &mut args)?;
    service::serve("SlotSrv", started, |protocol, command, t, body| {
        if protocol == "Slot" {
            slotsrv::handle(command, t, body)
        } else {
            Err(StsError::new(52, 2, 462))
        }
    })
    .await;
    Ok(())
}
