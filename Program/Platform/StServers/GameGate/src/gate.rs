//! Daemon request dispatch (RE/04 §1.2), part: Connect, the two rejected
//! Auth commands and the forward to the route bus.
use std::{io, sync::Arc};
use sts::Frame;
use sts_core::{
    conn::{header, reply_result, set_header, Incoming, Peer, DEFAULT_TIMEOUT},
    error::{ERR_BAD_CONFIG, ERR_PERMISSION},
    guid,
    route_client::RouteClient,
    Element, StsError,
};
use tokio::net::TcpListener;

/// Module of `GgCli.cpp` errors ❓ (RE/04 §1.3).
const MODULE: u32 = 1;

pub struct GameGate {
    route: RouteClient,
    server_id: u32,
}

impl GameGate {
    pub fn new(route: RouteClient, server_id: u32) -> Arc<Self> {
        Arc::new(Self { route, server_id })
    }

    pub async fn serve(self: Arc<Self>, listener: TcpListener) -> io::Result<()> {
        loop {
            let (stream, addr) = listener.accept().await?;
            let _ = stream.set_nodelay(true);
            let gate = self.clone();
            tokio::spawn(async move {
                eprintln!("GameGate: daemon connected, {addr}");
                let (_peer, mut incoming): (Peer, _) = Peer::spawn(stream);
                let mut connected = false;
                while let Some(inc) = incoming.recv().await {
                    if inc.command() == ("Sts", "Connect") {
                        // ❓ The Connect reply of the gate is not decoded. Empty OK reply.
                        connected = true;
                        inc.reply(None);
                    } else if inc.command() == ("Sts", "Ping") {
                        // No reply (`../Sts/02-messages.md`).
                    } else if !connected {
                        let e = StsError::new(ERR_BAD_CONFIG, MODULE, 0)
                            .with_text("missing client info");
                        inc.reply_error(&e, gate.server_id);
                    } else {
                        let gate = gate.clone();
                        tokio::spawn(async move { gate.request(inc).await });
                    }
                }
                eprintln!("GameGate: daemon closed, {addr}");
            });
        }
    }

    async fn request(&self, inc: Incoming) {
        let (protocol, command) = inc.command();
        let result = match (protocol, command) {
            // 0x1400D24B0: always an error.
            ("Auth", "VerifyMobiOtp" | "VerifySecondPwd") => {
                Err(StsError::new(ERR_PERMISSION, MODULE, 1742).with_text("invalid cmd"))
            }
            // Default forward (step 7) and `Auth/ConsumeGameToken` (0x1400CF690).
            // ❓ Not ported: `TokenInfo`, the `f` header, client ids, the whitelist.
            _ => self.forward(&inc.frame).await,
        };
        let code = result.as_ref().map_or_else(|e| e.code, |_| 0);
        eprintln!("GameGate: {protocol}/{command} -> {code}");
        match result {
            Ok(body) => inc.reply(body.as_ref()),
            Err(e) => {
                eprintln!(
                    "Error({}): /{protocol}/{command}: server={} module={} line={}",
                    e.code, e.server, e.module, e.line
                );
                inc.reply_error(&e, self.server_id);
            }
        }
    }

    async fn forward(&self, frame: &Frame) -> Result<Option<Element>, StsError> {
        let mut out = Frame {
            start: frame.start.clone(),
            headers: Vec::new(),
            body: frame.body.clone(),
        };
        if let Some(t) = header(frame, "t") {
            // The daemon library sends the user GUID. The router needs
            // `<center>$<key>` (13 §5). ❓ The original conversion is not decoded.
            match guid::parse(t) {
                Some(user) => set_header(&mut out, "t", format!("0${}", guid::format(&user))),
                None => set_header(&mut out, "t", t),
            }
        }
        reply_result(&self.route.request(out, DEFAULT_TIMEOUT).await?)
    }
}
