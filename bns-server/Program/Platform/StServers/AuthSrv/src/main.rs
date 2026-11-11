//! `AuthSrv --config StServer.ini --route-ini Route.ini --accounts accounts.tsv [--index N]`
//! `AuthSrv enroll <LoginName> <UserName>` reads the password from stdin and
//! prints one account line. The password is not stored or printed.
use authsrv::{
    accounts::{enroll, to_line, Accounts},
    service::{target, AuthService, Config},
};
use std::{
    io::{self, BufRead},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use sts_core::{
    conn::{header, reply_error, reply_ok, set_header},
    connect::ConnectInfo,
    ini::Ini,
    route::{Location, Topology},
    route_client::{
        parse_systems, select_router, Registration, RouteClient, RouteClientConfig, RouteEvent,
        RouteIni,
    },
};

/// ❓ ConnType, program id and build of a server socket are not traced (same as CliGate).
const CONN_TYPE_SERVER: u32 = 404;
const PROGRAM_ID: u32 = 101;
const BUILD: u32 = 381;

fn usage() -> ! {
    eprintln!("usage: AuthSrv --config StServer.ini --route-ini Route.ini --accounts file [--index N] [--server-id N]\n       AuthSrv enroll <LoginName> <UserName>   (password on stdin)");
    std::process::exit(2)
}

fn invalid(text: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, text.into())
}

fn token_seconds(ini: &Ini, key: &str, default: u64) -> Duration {
    let value = ini
        .sections
        .iter()
        .filter(|s| s.name.eq_ignore_ascii_case("TokenTimeout"))
        .flat_map(|s| s.entries.iter())
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .and_then(|(_, v)| v.trim().parse().ok());
    Duration::from_secs(value.unwrap_or(default))
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "enroll") {
        let [_, login, user_name] = &args[..] else {
            usage()
        };
        let mut password = String::new();
        io::stdin().lock().read_line(&mut password)?;
        let password = password.trim_end_matches(['\r', '\n']);
        if password.is_empty() {
            return Err(invalid("empty password"));
        }
        println!(
            "{}",
            to_line(&enroll(login, user_name, password.as_bytes()))
        );
        return Ok(());
    }
    let (mut config, mut route_ini_path, mut accounts) = (None, None, None);
    let (mut index, mut server_id) = (1u32, 0u32);
    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        let mut value = || it.next().unwrap_or_else(|| usage());
        match arg.as_str() {
            "--config" => config = Some(value()),
            "--route-ini" => route_ini_path = Some(value()),
            "--accounts" => accounts = Some(value()),
            "--index" => index = value().parse().unwrap_or_else(|_| usage()),
            "--server-id" => server_id = value().parse().unwrap_or_else(|_| usage()),
            _ => usage(),
        }
    }
    let (Some(config), Some(route_ini_path), Some(accounts)) = (config, route_ini_path, accounts)
    else {
        usage()
    };
    let ini = Ini::load(config)?;
    let entries = parse_systems(&ini).map_err(|e| invalid(e.text))?;
    let router = select_router(&entries, "AuthSrv", index).map_err(invalid)?;
    let own_entry = entries
        .iter()
        .find(|e| e.server_name == "AuthSrv" && e.app_index == index)
        .ok_or_else(|| invalid("AuthSrv is not in stserver.ini"))?;
    let route_ini = RouteIni::from_ini(&Ini::load(route_ini_path)?);
    let accounts = Accounts::parse(&std::fs::read_to_string(accounts)?).map_err(invalid)?;
    eprintln!("AuthSrv: {} accounts", accounts.len());
    let cfg = Config {
        app_token: token_seconds(&ini, "App", 1800),
        game_token: token_seconds(&ini, "Game", 300),
    };
    let own = Location {
        addr: Some(own_entry.addr),
        product: 1,
        index,
        deployment: 0,
        epoch: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(1, |d| d.as_secs() as u32),
    };
    let (route, mut events) = RouteClient::start(RouteClientConfig {
        router,
        identity: ConnectInfo {
            conn_type: CONN_TYPE_SERVER,
            location: own,
            process: std::process::id(),
            program: PROGRAM_ID,
            build: BUILD,
            ..ConnectInfo::default()
        },
        own,
        route_ini,
        providers: vec![Registration {
            name: "Auth".into(),
            build: 1,
            topology: Topology::Grid,
            block_bits: 0,
        }],
        subscriptions: vec!["GameAccount".into()],
        reconnect_delay: Duration::from_millis(3000),
        watchdog: Duration::from_millis(10_000),
    });
    let service = Arc::new(AuthService::new(cfg, accounts));
    eprintln!("AuthSrv: router {router}");
    while let Some(event) = events.recv().await {
        match event {
            RouteEvent::Registered => eprintln!("AuthSrv: registered on the route bus"),
            RouteEvent::Request(inc) => {
                let (service, route) = (service.clone(), route.clone());
                tokio::spawn(async move {
                    let (protocol, command) = inc.command();
                    let body = inc.body();
                    let result = if protocol == "Auth" {
                        service
                            .handle(command, target(&inc.frame), body.as_ref(), &route)
                            .await
                    } else {
                        Err(sts_core::StsError::new(10, 5, 0))
                    };
                    if inc.id() == 0 {
                        return;
                    }
                    let mut frame = match &result {
                        Ok(body) => reply_ok(inc.id(), body.as_ref()),
                        Err(e) => reply_error(inc.id(), e, server_id),
                    };
                    // The router needs `t` = the `p` header of the request.
                    if let Some(origin) = header(&inc.frame, "p") {
                        set_header(&mut frame, "t", origin);
                    }
                    inc.peer.send(frame);
                });
            }
            _ => {}
        }
    }
    Ok(())
}
