//! Card play-1, stage 6: one scripted client on loopback, through the Rust
//! StServers stack and LobbyGate, to the character list.
//!
//! client ─STS─► CliGate ─► RouteSrv ─► AuthSrv / GameAccountSrv / SlotSrv / PlayStubs
//! client ─────► LobbyGate ─► LobbyDaemon ─GateNp─► GameGate ─► AuthSrv (ConsumeGameToken)
#[allow(dead_code, unused_imports)]
#[path = "end_to_end.rs"]
mod helpers;
use authsrv::{
    accounts::{enroll, identity, to_line, Accounts},
    service::{AuthService, Config},
};
use bloodlib::packet::{compose, Value};
use cligate::gate::{CliGate, GateConfig as CliGateConfig};
use gameaccountsrv::GameAccounts;
use gamegate::{gate::GameGate, np_client::GateNp};
use helpers::{config, connect_client, keys, receive, send};
use lobbydaemon::{
    jobs::Settings,
    np::StubNp,
    store::{Account, Character, MemoryStore, Shared},
    Server as Lobby,
};
use lobbygate::{daemon::TcpLobbyDaemon, server::Server as Gate};
use routesrv::{config::RouteConfig, server::RouteServer};
use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};
use sts::{key_data, rc4::Rc4, srp6, Codec, Frame, StartLine};
use sts_core::{
    conn::{request, set_header},
    connect::ConnectInfo,
    guid,
    route::{Location, Topology},
    route_client::{Registration, RouteClient, RouteClientConfig, RouteIni},
    service::{serve, Started},
    Element,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::watch,
    time::timeout,
};

const PASSWORD: &[u8] = b"generated-test-value";
const GAME_ACCOUNT: &str = "11111111-2222-3333-4444-555555555555";
const APP_ID: &str = "B0D42105-0000-0000-0000-000000000001";

fn bus(router: SocketAddr, port: u16, providers: &[&str]) -> RouteClientConfig {
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

/// A service with a handler that does not wait.
async fn simple(
    router: SocketAddr,
    port: u16,
    name: &'static str,
    protocols: &[&str],
    handler: impl Fn(
            &str,
            &str,
            Option<&str>,
            Option<&Element>,
        ) -> Result<Option<Element>, sts_core::StsError>
        + Send
        + 'static,
) {
    let (route, events) = RouteClient::start(bus(router, port, protocols));
    registered(&route).await;
    let started = Started {
        route,
        events,
        server_id: u32::from(port),
        ini: Default::default(),
    };
    tokio::spawn(serve(name, started, handler));
}

/// STS client of the game: plain frames, then RC4 after KeyData.
struct StsClient {
    stream: TcpStream,
    codec: Codec,
    buf: Vec<u8>,
    enc: Option<Rc4>,
    dec: Option<Rc4>,
    next_id: u32,
}

impl StsClient {
    async fn call(&mut self, protocol: &str, command: &str, body: Element) -> Element {
        let mut frame = request(protocol, command, Some(&body));
        set_header(&mut frame, "s", self.next_id.to_string());
        self.next_id += 1;
        let mut bytes = Codec::default().encode(&frame).unwrap();
        if let Some(enc) = &mut self.enc {
            enc.apply(&mut bytes);
        }
        self.stream.write_all(&bytes).await.unwrap();
        let reply: Frame = loop {
            if let Some((frame, used)) = self.codec.decode(&self.buf).unwrap() {
                self.buf.drain(..used);
                break frame;
            }
            let mut chunk = [0u8; 4096];
            let n = timeout(Duration::from_secs(10), self.stream.read(&mut chunk))
                .await
                .expect("reply in time")
                .unwrap();
            assert_ne!(n, 0, "socket closed in {protocol}/{command}");
            if let Some(dec) = &mut self.dec {
                dec.apply(&mut chunk[..n]);
            }
            self.buf.extend_from_slice(&chunk[..n]);
        };
        let text = String::from_utf8_lossy(&reply.body).into_owned();
        match reply.start {
            StartLine::Response { status: 200, .. } => {}
            _ => panic!("{protocol}/{command} failed: {text}"),
        }
        if reply.body.is_empty() {
            Element::new("Reply")
        } else {
            Element::parse(&reply.body).unwrap()
        }
    }
}

fn req() -> Element {
    Element::new("Request")
}

#[tokio::test]
async fn client_logs_in_through_stservers_to_the_character_list() {
    timeout(Duration::from_secs(30), async {
        // ---- route bus and services
        let router_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let router = router_listener.local_addr().unwrap();
        let server = Arc::new(RouteServer::new(RouteConfig::default(), 9));
        tokio::spawn(server.serve(router_listener, std::future::pending::<()>()));

        let account = enroll("Tester@Example", "Tester", PASSWORD);
        let user_id = account.user_id.clone();
        let (auth_route, auth_events) = RouteClient::start(bus(router, 7201, &["Auth"]));
        let auth = Arc::new(
            AuthService::new(
                Config::default(),
                Accounts::parse(&to_line(&account)).unwrap(),
            )
            .with_log(|_| {}),
        );
        tokio::spawn(authsrv::service::run(
            auth,
            auth_route.clone(),
            auth_events,
            2,
        ));
        registered(&auth_route).await;

        let games =
            GameAccounts::parse(&format!("{user_id}\tbns\tmain\t{GAME_ACCOUNT}\n")).unwrap();
        simple(
            router,
            7202,
            "GameAccountSrv",
            &["GameAccount"],
            move |_, c, t, b| games.handle(c, t, b),
        )
        .await;
        simple(router, 7203, "SlotSrv", &["Slot"], |_, c, t, b| {
            slotsrv::handle(c, t, b)
        })
        .await;
        simple(
            router,
            7204,
            "PlayStubs",
            &play_stubs::PROTOCOLS,
            |p, c, _, _| play_stubs::handle(p, c),
        )
        .await;

        let (cli_route, cli_events) = RouteClient::start(bus(router, 7205, &["Cli"]));
        let cli_gate = CliGate::new(
            CliGateConfig {
                server_id: 6,
                center: 0,
                idle_timeout: Duration::from_secs(60),
                auth_timeout: Duration::from_secs(5),
            },
            cli_route.clone(),
        );
        tokio::spawn(cli_gate.clone().handle_bus(cli_events));
        let cli_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let cli_addr = cli_listener.local_addr().unwrap();
        tokio::spawn(cli_gate.serve(cli_listener));
        registered(&cli_route).await;

        let (gg_route, _gg_events) = RouteClient::start(bus(router, 7206, &["Game"]));
        registered(&gg_route).await;
        let gg_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let gg_addr = gg_listener.local_addr().unwrap();
        tokio::spawn(GameGate::new(gg_route, 7).serve(gg_listener));

        // ---- LobbyDaemon (tokens through GameGate) and LobbyGate
        let game_account = guid::parse(GAME_ACCOUNT).unwrap();
        let mut store = MemoryStore::default();
        store.accounts.insert(
            game_account,
            Account {
                id: game_account,
                name: "Tester@Example".encode_utf16().collect(),
                right: 1,
                representative_pcid: 77,
                representative_world: 1,
                bot_score: 0,
            },
        );
        let row = Character {
            pcid: 77,
            slot: [45; 16],
            world: 1,
            name: "generated-character".encode_utf16().collect(),
            showcase: vec![7; 512],
            deletion_time: None,
            state: lobbydaemon::character::InternalState::Normal,
            jumping_state: 0,
            last_play_end: 0,
        };
        store.characters.insert(game_account, vec![row.clone()]);
        let lobby_listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let cfg = config(lobby_listener.local_addr().unwrap().port());
        let np = GateNp::new(gg_addr, "bns", 0, StubNp::default());
        let lobby = Arc::new(Lobby::new(Shared::new(store), np, Settings::default()).unwrap());
        let (_stop, shutdown) = watch::channel(false);
        tokio::spawn(lobby.clone().serve(lobby_listener, shutdown.clone()));
        let daemon = TcpLobbyDaemon::start(&cfg, shutdown.clone()).await.unwrap();
        let (public, private, _) = keys(1024);
        let gate = Arc::new(Gate::new(&cfg, private, daemon).unwrap());
        let gate_listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let gate_addr = gate_listener.local_addr().unwrap();
        tokio::spawn(gate.serve(gate_listener, shutdown));

        // ---- the client: SRP login on CliGate
        let mut sts = StsClient {
            stream: TcpStream::connect(cli_addr).await.unwrap(),
            codec: Codec::default(),
            buf: Vec::new(),
            enc: None,
            dec: None,
            next_id: 1,
        };
        // The game sends `Sts/Connect` first (11 §7.1).
        sts.call("Sts", "Connect", Element::new("Connect")).await;
        let login_name = "tester@example";
        let start = sts
            .call(
                "Auth",
                "LoginStart",
                req().child(Element::text("LoginName", login_name)),
            )
            .await;
        let (salt, b) = key_data::decode_challenge(start.get_text("KeyData").unwrap()).unwrap();
        let (a, m1, m2, key) =
            srp6::client_model(&identity(login_name), PASSWORD, &salt, &b, &[9; 32]).unwrap();
        let proof = sts
            .call(
                "Auth",
                "KeyData",
                req().child(Element::text(
                    "KeyData",
                    key_data::encode_client(&a, &m1).unwrap(),
                )),
            )
            .await;
        assert_eq!(
            key_data::decode_server(proof.get_text("KeyData").unwrap()).unwrap(),
            m2
        );
        assert!(proof.get("SessionKey").is_none());
        // RC4 with the SRP key K, both directions, after the KeyData reply.
        sts.enc = Some(Rc4::new(&key).unwrap());
        sts.dec = Some(Rc4::new(&key).unwrap());
        let finish = sts
            .call(
                "Auth",
                "LoginFinish",
                req().child(Element::int("Language", 1)),
            )
            .await;
        assert_eq!(finish.get_text("UserId"), Some(user_id.as_str()));

        // The requests that the game sends after LoginFinish (07 "After LoginFinish").
        let game_code = || req().child(Element::text("GameCode", "bns"));
        let accounts = sts.call("GameAccount", "ListMyAccounts", game_code()).await;
        let alias = accounts
            .get("GameAccount")
            .and_then(|g| g.get_text("Alias"))
            .unwrap()
            .to_owned();
        assert_eq!(alias, "main");
        let info = sts.call("Auth", "GetMyUserInfo", req()).await;
        assert_eq!(info.get_text("LoginName"), Some("Tester@Example"));
        let app = sts
            .call(
                "Auth",
                "RequestToken",
                req().child(Element::text("AppId", APP_ID)),
            )
            .await;
        assert!(app.get_text("AuthnToken").unwrap().starts_with(&user_id));
        let balance = sts.call("VirtualCurrency", "GetBalance", req()).await;
        assert_eq!(balance.get_text("BalanceInfoListCount"), Some("0"));
        let slots = sts.call("Slot", "ListSlots", req()).await;
        assert_eq!(slots.all("Slot").count(), 4);
        let token = sts
            .call(
                "Auth",
                "RequestGameToken",
                game_code().child(Element::text("AccountAlias", alias)),
            )
            .await;
        let token = guid::parse(token.get_text("Token").unwrap()).unwrap();
        let portal = guid::parse(&user_id).unwrap();

        // ---- the client: LobbyGate login with the game token, then the character list
        let (mut client, mut crypt) = connect_client(gate_addr, &public).await;
        send(
            &mut client,
            &mut crypt,
            compose("hoo", &[Value::I16(3), Value::Id(token), Value::Id(portal)]).unwrap(),
        )
        .await;
        let login = receive(&mut client, &mut crypt, 4, "qQoihCiiiii;CiiQiqq;QqiqiqK").await;
        assert_eq!(login[2], Value::Id(game_account));
        assert_eq!(lobby.session_count().await, 1);
        send(
            &mut client,
            &mut crypt,
            compose("h", &[Value::I16(27)]).unwrap(),
        )
        .await;
        let list = receive(&mut client, &mut crypt, 28, "qhCqohsbqQQi;").await;
        assert_eq!(list[0], Value::U64(77));
        let Value::Repeat(rows) = &list[2] else {
            panic!("character rows")
        };
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0][3], Value::String(row.name));
        // Enter world (stage 7) is not in this test: GameDaemon has no listener.
    })
    .await
    .expect("play path in time");
}
