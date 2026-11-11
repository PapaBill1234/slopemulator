//! Game token path on a real bus: RouteSrv, AuthSrv, GameAccountSrv, GameGate
//! and the daemon-side client `GateNp`.
use authsrv::{
    accounts::{enroll, to_line, Accounts},
    service::{AuthService, Config},
};
use gameaccountsrv::GameAccounts;
use gamegate::{gate::GameGate, np_client::GateNp};
use routesrv::{config::RouteConfig, server::RouteServer};
use shared::np_channel::{Np, NpError, StubNp};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use sts_core::{
    conn::{reply_result, request, set_header},
    connect::ConnectInfo,
    guid,
    route::{Location, Topology},
    route_client::{Registration, RouteClient, RouteClientConfig, RouteIni},
    service::{serve, Started},
    Element,
};
use tokio::net::TcpListener;

const ACCOUNT: &str = "11111111-2222-3333-4444-555555555555";

fn client(router: SocketAddr, port: u16, providers: &[&str]) -> RouteClientConfig {
    let own = Location {
        addr: Some(format!("127.0.0.1:{port}").parse().unwrap()),
        product: 1,
        index: 1,
        deployment: 0,
        epoch: 5,
    };
    RouteClientConfig {
        router,
        identity: ConnectInfo {
            conn_type: 404,
            location: own,
            process: 77,
            program: 101,
            build: 381,
            ..ConnectInfo::default()
        },
        own,
        route_ini: RouteIni::default(),
        providers: providers
            .iter()
            .map(|p| Registration {
                name: (*p).into(),
                build: 1,
                topology: Topology::Grid,
                block_bits: 0,
            })
            .collect(),
        subscriptions: vec![],
        reconnect_delay: Duration::from_millis(3000),
        watchdog: Duration::from_millis(10_000),
    }
}

async fn registered(c: &RouteClient) {
    for _ in 0..500 {
        if c.is_registered() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("route client not registered");
}

#[tokio::test]
async fn game_token_through_the_gate() {
    let router_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let router = router_listener.local_addr().unwrap();
    let server = Arc::new(RouteServer::new(RouteConfig::default(), 9));
    tokio::spawn(server.serve(router_listener, std::future::pending::<()>()));

    let account = enroll("tester@example", "Tester", b"generated-test-value");
    let user = account.user_id.clone();
    let accounts = Accounts::parse(&to_line(&account)).unwrap();
    let (auth_route, auth_events) = RouteClient::start(client(router, 7101, &["Auth"]));
    let auth = Arc::new(AuthService::new(Config::default(), accounts).with_log(|_| {}));
    tokio::spawn(authsrv::service::run(
        auth,
        auth_route.clone(),
        auth_events,
        3,
    ));
    registered(&auth_route).await;

    let games = GameAccounts::parse(&format!("{user}\tbns\tmain\t{ACCOUNT}\n")).unwrap();
    let (ga_route, ga_events) = RouteClient::start(client(router, 7102, &["GameAccount"]));
    registered(&ga_route).await;
    let started = Started {
        route: ga_route,
        events: ga_events,
        server_id: 4,
        ini: Default::default(),
    };
    tokio::spawn(serve(
        "GameAccountSrv",
        started,
        move |_, command, t, body| games.handle(command, t, body),
    ));

    let (gate_route, _gate_events) = RouteClient::start(client(router, 7103, &["Game"]));
    registered(&gate_route).await;
    let gate_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let gate_addr = gate_listener.local_addr().unwrap();
    tokio::spawn(GameGate::new(gate_route.clone(), 5).serve(gate_listener));

    // A client (through CliGate in the real path) asks for a game token.
    let body = Element::new("Request")
        .child(Element::text("GameCode", "bns"))
        .child(Element::text("AccountAlias", "main"));
    let mut frame = request("Auth", "RequestGameToken", Some(&body));
    set_header(&mut frame, "t", format!("0${user}"));
    let reply = gate_route
        .request(frame, Duration::from_secs(5))
        .await
        .unwrap();
    let reply = reply_result(&reply).unwrap().unwrap();
    let token = guid::parse(reply.get_text("Token").unwrap()).unwrap();
    let portal = guid::parse(&user).unwrap();

    // The daemon consumes it through GameGate. One use only.
    let np = GateNp::new(gate_addr, "bns", 1, StubNp::default());
    let wrong_user = np.consume(token, [9; 16]).await;
    assert_eq!(wrong_user.unwrap_err(), NpError::RejectedToken);
    let identity = np.consume(token, portal).await.unwrap();
    assert_eq!(identity.account, guid::parse(ACCOUNT).unwrap());
    assert_eq!(
        String::from_utf16(&identity.login_name).unwrap(),
        "tester@example"
    );
    assert_eq!(String::from_utf16(&identity.user_name).unwrap(), "Tester");
    assert_eq!(
        np.consume(token, portal).await.unwrap_err(),
        NpError::RejectedToken
    );
}
