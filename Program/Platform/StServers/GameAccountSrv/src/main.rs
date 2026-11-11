//! `GameAccountSrv --config StServer.ini --route-ini Route.ini --accounts game-accounts.tsv [--index N] [--server-id N]`
use gameaccountsrv::GameAccounts;
use std::io;
use sts_core::{service, StsError};

#[tokio::main]
async fn main() -> io::Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let started = service::start("GameAccountSrv", &["GameAccount"], &[], &mut args)?;
    let [flag, path] = &args[..] else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "--accounts file is necessary",
        ));
    };
    if flag != "--accounts" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "--accounts file is necessary",
        ));
    }
    let store = GameAccounts::parse(&std::fs::read_to_string(path)?)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    service::serve("GameAccountSrv", started, |protocol, command, t, body| {
        if protocol == "GameAccount" {
            store.handle(command, t, body)
        } else {
            Err(StsError::new(52, 2, 462))
        }
    })
    .await;
    Ok(())
}
