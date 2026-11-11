//! Daemon side of the GameGate link: the `Np` operations of LobbyDaemon over
//! TCP. Message shapes: `Docs/Platform/Sts/02-messages.md` (STS server library).
use shared::np_channel::{AccountId, ChargeMode, GameFuture, Identity, Np, NpError, NpFuture};
use std::{
    net::SocketAddr,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};
use sts_core::{
    conn::{reply_result, request, set_header, Peer, DEFAULT_TIMEOUT},
    guid, Element, StsError,
};
use tokio::net::TcpStream;

/// Token rejections of AuthSrv (spec 15 §7.7).
const TOKEN_ERRORS: [u32; 2] = [3010, 3303];

/// `consume` goes to GameGate. The other operations use `backend`.
pub struct GateNp<N> {
    gate: SocketAddr,
    game_code: String,
    /// `Program` of the Connect body (the portal program id of `Game.ini`).
    program: u32,
    peer: Mutex<Option<Peer>>,
    backend: N,
}

impl<N: Np> GateNp<N> {
    pub fn new(gate: SocketAddr, game_code: impl Into<String>, program: u32, backend: N) -> Self {
        Self {
            gate,
            game_code: game_code.into(),
            program,
            peer: Mutex::new(None),
            backend,
        }
    }

    /// The open link, or a new one with `Sts/Connect` done.
    async fn peer(&self) -> Result<Peer, StsError> {
        let open = self.peer.lock().unwrap().clone().filter(|p| !p.is_closed());
        if let Some(peer) = open {
            return Ok(peer);
        }
        let unavailable = || StsError::new(1, 0, 0);
        let stream = TcpStream::connect(self.gate)
            .await
            .map_err(|_| unavailable())?;
        let _ = stream.set_nodelay(true);
        let local = stream.local_addr().map_err(|_| unavailable())?;
        let (peer, _incoming) = Peer::spawn(stream);
        // Seconds since 2001-01-01 UTC.
        let epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs().saturating_sub(978_307_200));
        let body = Element::new("Connect")
            .child(Element::int("ConnType", 401))
            .child(Element::text("Address", local.to_string()))
            .child(Element::int("ProductType", 0))
            .child(Element::int("AppIndex", 1))
            .child(Element::int("Epoch", epoch as i64))
            .child(Element::int("Program", i64::from(self.program)))
            .child(Element::int("Build", 4000))
            .child(Element::int("Process", i64::from(std::process::id())));
        peer.call(request("Sts", "Connect", Some(&body)), DEFAULT_TIMEOUT)
            .await
            .and_then(|f| reply_result(&f))?;
        *self.peer.lock().unwrap() = Some(peer.clone());
        Ok(peer)
    }

    async fn consume_token(&self, token: [u8; 16], portal: [u8; 16]) -> Result<Identity, NpError> {
        let peer = self.peer().await.map_err(|_| NpError::Unavailable)?;
        let body = Element::new("Request")
            .child(Element::text("Token", guid::format(&token)))
            .child(Element::text("GameCode", self.game_code.clone()))
            // ❓ LobbyDaemon does not give the client address to this interface.
            .child(Element::text("ClientNetAddress", ""));
        let mut frame = request("Auth", "ConsumeGameToken", Some(&body));
        set_header(&mut frame, "t", guid::format(&portal));
        let reply = peer
            .call(frame, DEFAULT_TIMEOUT)
            .await
            .and_then(|f| reply_result(&f))
            .map_err(|e| {
                if TOKEN_ERRORS.contains(&e.code) || e.code == 10 {
                    NpError::RejectedToken
                } else {
                    NpError::Unavailable
                }
            })?
            .ok_or(NpError::MissingData)?;
        let account = reply
            .get_text("GameAccountId")
            .and_then(guid::parse)
            .ok_or(NpError::MissingData)?;
        let utf16 = |name: &str| reply.get_text(name).unwrap_or("").encode_utf16().collect();
        Ok(Identity {
            account,
            login_name: utf16("LoginName"),
            user_name: utf16("UserName"),
            // ❓ The slot comes from SlotSrv in the original. One fixed slot here.
            slot_id: 1,
            profile: vec![],
            grades: vec![],
        })
    }
}

impl<N: Np> Np for GateNp<N> {
    fn consume(&self, token: [u8; 16], portal: [u8; 16]) -> NpFuture<'_, Identity> {
        Box::pin(self.consume_token(token, portal))
    }
    fn create_initial_user_name(&self, account: AccountId, name: Vec<u16>) -> GameFuture<'_, ()> {
        self.backend.create_initial_user_name(account, name)
    }
    fn login_presence(&self, account: AccountId, owner: u64) -> NpFuture<'_, ()> {
        self.backend.login_presence(account, owner)
    }
    fn logout_presence(&self, account: AccountId, owner: u64) {
        self.backend.logout_presence(account, owner);
    }
    fn world_entry(
        &self,
        account: AccountId,
        pcid: u64,
        slot: [u8; 16],
        ipv4: u32,
    ) -> GameFuture<'_, ChargeMode> {
        self.backend.world_entry(account, pcid, slot, ipv4)
    }
    fn expire(&self) {
        self.backend.expire();
    }
}
