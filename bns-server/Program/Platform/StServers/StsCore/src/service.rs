//! Start-up of a simple route bus service: arguments, registration, request loop.
//! Each small `*Srv` binary of the port uses it.
use crate::{
    conn::{header, reply_error, reply_ok, set_header, Incoming},
    connect::ConnectInfo,
    ini::Ini,
    route::{Location, Topology},
    route_client::{
        parse_systems, select_router, Registration, RouteClient, RouteClientConfig, RouteEvent,
        RouteIni,
    },
    Element, StsError,
};
use std::{
    io,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::mpsc;

/// ❓ ConnType, program id and build of a server socket are not traced (same as CliGate).
const CONN_TYPE_SERVER: u32 = 404;
const PROGRAM_ID: u32 = 101;
const BUILD: u32 = 381;

pub struct Started {
    pub route: RouteClient,
    pub events: mpsc::UnboundedReceiver<RouteEvent>,
    pub server_id: u32,
    /// `StServer.ini`.
    pub ini: Ini,
}

fn invalid(text: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, text.into())
}

/// Takes `--config`, `--route-ini`, `--index` and `--server-id` out of `args`
/// and registers `protocols` on the router of `server_name`. Other arguments stay.
/// Needs a tokio runtime.
pub fn start(
    server_name: &str,
    protocols: &[&str],
    subscriptions: &[&str],
    args: &mut Vec<String>,
) -> io::Result<Started> {
    let mut take = |name: &str| -> Option<String> {
        let pos = args.iter().position(|a| a == name)?;
        (pos + 1 < args.len()).then(|| {
            args.remove(pos);
            args.remove(pos)
        })
    };
    let config = take("--config").ok_or_else(|| invalid("--config StServer.ini is necessary"))?;
    let route_ini =
        take("--route-ini").ok_or_else(|| invalid("--route-ini Route.ini is necessary"))?;
    let number = |v: Option<String>, default: u32| match v {
        Some(v) => v.parse().map_err(|_| invalid("bad number")),
        None => Ok(default),
    };
    let index = number(take("--index"), 1)?;
    let server_id = number(take("--server-id"), 0)?;
    let ini = Ini::load(config)?;
    let entries = parse_systems(&ini).map_err(|e| invalid(e.text))?;
    let router = select_router(&entries, server_name, index).map_err(invalid)?;
    let own_entry = entries
        .iter()
        .find(|e| e.server_name == server_name && e.app_index == index)
        .ok_or_else(|| invalid(format!("{server_name} is not in stserver.ini")))?;
    let own = Location {
        addr: Some(own_entry.addr),
        product: 1,
        index,
        deployment: 0,
        epoch: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(1, |d| d.as_secs() as u32),
    };
    let (route, events) = RouteClient::start(RouteClientConfig {
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
        route_ini: RouteIni::from_ini(&Ini::load(route_ini)?),
        providers: protocols
            .iter()
            .map(|p| Registration {
                name: (*p).into(),
                build: 1,
                topology: Topology::Grid,
                block_bits: 0,
            })
            .collect(),
        subscriptions: subscriptions.iter().map(|s| (*s).into()).collect(),
        reconnect_delay: Duration::from_millis(3000),
        watchdog: Duration::from_millis(10_000),
    });
    eprintln!("{server_name}: router {router}");
    Ok(Started {
        route,
        events,
        server_id,
        ini,
    })
}

/// Reply of a provider. The router needs `t` = the `p` header of the request.
pub fn bus_reply(inc: &Incoming, result: &Result<Option<Element>, StsError>, server_id: u32) {
    if inc.id() == 0 {
        return;
    }
    let mut frame = match result {
        Ok(body) => reply_ok(inc.id(), body.as_ref()),
        Err(e) => reply_error(inc.id(), e, server_id),
    };
    if let Some(origin) = header(&inc.frame, "p") {
        set_header(&mut frame, "t", origin);
    }
    inc.peer.send(frame);
}

/// Request loop for handlers that do not wait.
/// Handler input: protocol, command, `t` header, body.
pub async fn serve(
    name: &str,
    mut started: Started,
    handler: impl Fn(&str, &str, Option<&str>, Option<&Element>) -> Result<Option<Element>, StsError>,
) {
    while let Some(event) = started.events.recv().await {
        match event {
            RouteEvent::Registered => eprintln!("{name}: registered on the route bus"),
            RouteEvent::Request(inc) => {
                let (protocol, command) = inc.command();
                let body = inc.body();
                let result = handler(protocol, command, header(&inc.frame, "t"), body.as_ref());
                eprintln!(
                    "{name}: {protocol}/{command} -> {}",
                    result.as_ref().map_or_else(|e| e.code, |_| 0)
                );
                bus_reply(&inc, &result, started.server_id);
            }
            _ => {}
        }
    }
}

/// UserId text of a `t` header: `<GUID>` or `<dc>$<GUID>`, optional `;params`. Upper case.
pub fn target_user(t: Option<&str>) -> Option<String> {
    let t = t?.split(';').next()?.rsplit('$').next()?.trim();
    let ok = t.len() == 36
        && t.bytes().enumerate().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => c == b'-',
            _ => c.is_ascii_hexdigit(),
        });
    ok.then(|| t.to_ascii_uppercase())
}
