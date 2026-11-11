//! One task per client socket: dispatcher (spec §3), login (§4, §9, §10),
//! logout (§12.3) and idle timeout (§12.5).

use crate::{
    auth_cmd::AuthCmd,
    crypt::CryptStream,
    gate::{CliGate, ConnCmd},
    session::{user_target, Session, SessionState},
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use sts::{Frame, StartLine};
use sts_core::{
    conn::{reply_error, reply_ok, reply_result, request, set_header, transaction, write_frame},
    error::{module, StsError, ERR_BAD_DATA, ERR_BAD_PARAM, ERR_PERMISSION, ERR_TIMEOUT},
    xml::{parse_u32, Element},
};
use tokio::{
    io::{AsyncWriteExt, ReadHalf, WriteHalf},
    net::TcpStream,
    sync::{mpsc, oneshot},
};

type Stream = CryptStream<TcpStream>;

/// Module of the StsGate errors (§9.1).
const MODULE_GATE: u32 = 8000;
/// Commands of `Gate.ini [Commands.CliGate]` that the login path of the game
/// client uses (`../Sts/10-stservers-original.md`). The other 140 are not ported.
const FORWARDED: [(&str, &str); 3] = [
    ("GameAccount", "ListMyAccounts"),
    ("Slot", "ListSlots"),
    ("VirtualCurrency", "GetBalance"),
];

/// Failed LoginStart calls that close the socket (§9.4).
const MAX_LOGIN_FAILURES: u32 = 3;
const MAX_LOGIN_NAME: usize = 64;

enum Flow {
    Continue,
    Close,
    /// Write the reply in the clear, then switch RC4 on with this key.
    EnableCrypt(Vec<u8>),
}

struct Outcome {
    result: Result<Option<Element>, StsError>,
    flow: Flow,
}

fn ok(body: Option<Element>) -> Outcome {
    Outcome {
        result: Ok(body),
        flow: Flow::Continue,
    }
}

fn fail(err: StsError) -> Outcome {
    Outcome {
        result: Err(err),
        flow: Flow::Continue,
    }
}

fn gate_err(code: u32, line: u32, text: &str) -> StsError {
    StsError::new(code, module::CLI_GATE, line).with_text(text)
}

fn unknown_command() -> StsError {
    StsError::new(ERR_BAD_PARAM, MODULE_GATE, 2405)
        .with_text("StsGate: unknown protocol or command")
}

/// `strtoi64(s, _, 0)`: decimal, `0x` hex or leading-0 octal. Garbage gives 0.
fn parse_c_i64(s: &str) -> i64 {
    let s = s.trim();
    let (neg, s) = match s.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, s.strip_prefix('+').unwrap_or(s)),
    };
    let value = if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        i64::from_str_radix(hex, 16)
    } else if s.len() > 1 && s.starts_with('0') {
        i64::from_str_radix(&s[1..], 8)
    } else {
        s.parse()
    }
    .unwrap_or(0);
    if neg {
        -value
    } else {
        value
    }
}

/// Copy of a KeyData-like element without any `SessionKey`.
fn without_session_key(e: &Element) -> Element {
    let mut copy = e.clone();
    copy.children.retain(|c| c.name != "SessionKey");
    copy
}

fn truncated(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// What a good LoginStart reply carries (§9.3, §9.4).
struct Started {
    session: String,
    user_id: String,
    user_center: u32,
    user_name: String,
    key_data: Element,
}

fn parse_login_start(body: Option<Element>) -> Result<Started, StsError> {
    let bad = |line, text| gate_err(ERR_BAD_PARAM, line, text);
    let body = body.ok_or_else(|| bad(1293, "CliConn: login start reply, missing body"))?;
    let session = body
        .get_text("Session")
        .filter(|s| !s.is_empty())
        .ok_or_else(|| bad(1297, "CliConn: login start reply, missing session"))?;
    let user_id = body.get_text("UserId").filter(|s| !s.is_empty());
    let user_center = body.get_text("UserCenter").and_then(parse_u32);
    let (Some(user_id), Some(user_center)) = (user_id, user_center) else {
        return Err(bad(1302, "CliConn: login start, invalid userref"));
    };
    let key_data = body
        .get("KeyData")
        .ok_or_else(|| bad(1306, "CliConn: login start reply, missing key data"))?;
    Ok(Started {
        session: session.to_owned(),
        user_id: user_id.to_owned(),
        user_center,
        user_name: truncated(body.get_text("UserName").unwrap_or(""), 64),
        key_data: key_data.clone(),
    })
}

/// Entry of the socket task. Registers the client, runs, then unregisters.
pub(crate) async fn run(gate: Arc<CliGate>, stream: TcpStream, addr: SocketAddr) {
    let reg = gate.register();
    let id = reg.id;
    let mut conn = Conn {
        gate: gate.clone(),
        id,
        addr: addr.to_string(),
        ip: addr.ip().to_string(),
        session: Session::default(),
        shared_user: reg.user_id,
        writer: None,
        licenses: None,
        roles: Vec::new(),
        password_until: String::new(),
        user_status: 0,
        access_mask: 0,
    };
    conn.serve(stream, reg.rx).await;
    gate.unregister(id);
}

struct Conn {
    gate: Arc<CliGate>,
    id: u32,
    /// `ip:port`, for the log.
    addr: String,
    ip: String,
    session: Session,
    shared_user: Arc<Mutex<Option<String>>>,
    writer: Option<WriteHalf<Stream>>,
    // Data of the last good LoginFinish (§10.1, §10.2).
    licenses: Option<Element>,
    roles: Vec<u32>,
    password_until: String,
    #[allow(dead_code)]
    user_status: u32,
    #[allow(dead_code)]
    access_mask: i64,
}

impl Conn {
    async fn serve(&mut self, stream: TcpStream, mut cmd_rx: mpsc::UnboundedReceiver<ConnCmd>) {
        let (rh, wh): (ReadHalf<Stream>, WriteHalf<Stream>) =
            tokio::io::split(CryptStream::new(stream));
        self.writer = Some(wh);
        let mut reader = sts_core::conn::FrameReader::new(rh);
        let idle = self.gate.cfg.idle_timeout;
        let sleep = tokio::time::sleep(idle);
        tokio::pin!(sleep);
        let mut done: Option<oneshot::Sender<()>> = None;
        loop {
            let flow = tokio::select! {
                frame = reader.next() => match frame {
                    Ok(Some(frame)) => {
                        let flow = self.on_frame(frame).await;
                        sleep.as_mut().reset(tokio::time::Instant::now() + idle);
                        flow
                    }
                    _ => break,
                },
                cmd = cmd_rx.recv() => {
                    if let Some(ConnCmd::Close { done: d }) = cmd {
                        done = Some(d);
                    }
                    break;
                }
                () = &mut sleep => {
                    self.gate.log(format!(
                        "StsSocket::{}, Disconnect - idle timeout for ({})",
                        self.id, self.addr
                    ));
                    break;
                }
            };
            match flow {
                Flow::Continue => {}
                Flow::Close => break,
                Flow::EnableCrypt(key) => {
                    let (rh, leftover) = reader.into_parts();
                    let Some(wh) = self.writer.take() else { break };
                    let mut stream = rh.unsplit(wh);
                    stream.restore_unread(leftover);
                    stream.enable(&key);
                    let (rh, wh) = tokio::io::split(stream);
                    self.writer = Some(wh);
                    reader = sts_core::conn::FrameReader::new(rh);
                }
            }
        }
        // Close: Reset logs LOGOUT or DISCONNECT (§12.2, §12.5).
        self.reset_session();
        if let Some(w) = self.writer.as_mut() {
            let _ = w.shutdown().await;
        }
        if let Some(d) = done {
            let _ = d.send(());
        }
    }

    /// Writes a frame. `false` = the socket is broken.
    async fn send(&mut self, frame: &Frame) -> bool {
        let Some(w) = self.writer.as_mut() else {
            return false;
        };
        write_frame(w, frame).await.is_ok() && w.flush().await.is_ok()
    }

    fn reset_session(&mut self) {
        let line = self.session.reset(&self.addr);
        self.gate.log(line);
        *self.shared_user.lock().unwrap() = None;
        self.licenses = None;
        self.roles.clear();
        self.password_until.clear();
    }

    fn user(&self) -> &str {
        self.session.user_id.as_deref().unwrap_or("")
    }

    /// Call on the bus. A timeout also writes the report line of §9.4.
    async fn call(&self, frame: Frame, reply_name: &str) -> Result<Option<Element>, StsError> {
        let result = async {
            let reply = self
                .gate
                .route
                .request(frame, self.gate.cfg.auth_timeout)
                .await?;
            reply_result(&reply)
        }
        .await;
        if let Err(e) = &result {
            if e.code == ERR_TIMEOUT {
                self.gate.log(format!(
                    "CliConn: timeout, {}, {}, Cli, {reply_name}",
                    self.user(),
                    self.addr
                ));
            }
        }
        result
    }

    async fn on_frame(&mut self, frame: Frame) -> Flow {
        let StartLine::Request { protocol, command } = &frame.start else {
            return Flow::Continue;
        };
        let rid = transaction(&frame).map_or(0, |t| t.id);
        let body = (!frame.body.is_empty())
            .then(|| Element::parse(&frame.body).ok())
            .flatten();
        if protocol == "Sts" {
            // §7.1: the game client sends `Sts/Connect` first and `Sts/Ping` later.
            return match command.as_str() {
                "Connect" => {
                    let reply = if rid != 0 {
                        reply_ok(rid, Some(&Element::new("Reply")))
                    } else {
                        Frame {
                            start: StartLine::Response {
                                status: 200,
                                reason: "OK".into(),
                            },
                            headers: Vec::new(),
                            body: b"<Reply/>".to_vec(),
                        }
                    };
                    if self.send(&reply).await {
                        Flow::Continue
                    } else {
                        Flow::Close
                    }
                }
                // One-way keepalive: no reply.
                "Ping" => Flow::Continue,
                _ => self.finish(rid, fail(unknown_command())).await,
            };
        }
        let outcome = if protocol == "Auth" {
            match AuthCmd::from_name(command) {
                Some(cmd) => match self.session.check(cmd) {
                    Ok(()) => self.auth(cmd, body).await,
                    Err(e) => fail(e),
                },
                None => fail(unknown_command()),
            }
        } else if self.session.state() != SessionState::LoggedIn {
            fail(
                StsError::new(ERR_PERMISSION, MODULE_GATE, 2603).with_text(format!(
                    "permission denied '{protocol}/{command}', not logged in"
                )),
            )
        } else if FORWARDED.contains(&(protocol.as_str(), command.as_str())) {
            // Generic gate path (§11.1), part: whitelist check and forward.
            // ❓ The rate limit and the `f` header are not ported.
            let mut out = request(protocol, command, body.as_ref());
            set_header(&mut out, "t", user_target(0, self.user()));
            match self.call(out, "CCliConn::ClientReply").await {
                Ok(reply) => ok(reply),
                Err(e) => fail(e),
            }
        } else {
            fail(unknown_command())
        };
        self.finish(rid, outcome).await
    }

    /// Writes the reply of an outcome and returns the flow.
    async fn finish(&mut self, rid: u32, outcome: Outcome) -> Flow {
        if rid != 0 {
            let frame = match &outcome.result {
                Ok(body) => reply_ok(rid, body.as_ref()),
                Err(e) => reply_error(rid, e, self.gate.cfg.server_id),
            };
            if !self.send(&frame).await {
                return Flow::Close;
            }
        }
        outcome.flow
    }

    async fn auth(&mut self, cmd: AuthCmd, body: Option<Element>) -> Outcome {
        match cmd {
            AuthCmd::LoginStart => self.login_start(body).await,
            AuthCmd::KeyData => self.key_data(body).await,
            AuthCmd::LoginFinish => self.login_finish(body).await,
            AuthCmd::LogoutMyClient => Outcome {
                // The close path runs Reset and logs LOGOUT (§12.3).
                result: Ok(None),
                flow: Flow::Close,
            },
            AuthCmd::LoginTokenStart => self.login_token_start(body),
            AuthCmd::RequestToken | AuthCmd::RequestGameToken | AuthCmd::GetMyUserInfo => {
                // ❓ The `f` header of §11.1 is not set in phase 1.
                let mut frame = request("Auth", cmd.name(), body.as_ref());
                set_header(
                    &mut frame,
                    "t",
                    user_target(self.gate.cfg.center, self.user()),
                );
                match self.call(frame, "CCliConn::ClientReply").await {
                    Ok(reply) => ok(reply),
                    Err(e) => fail(e),
                }
            }
            _ => fail(unknown_command()),
        }
    }

    // ------------------------------------------------------------ LoginStart

    async fn login_start(&mut self, body: Option<Element>) -> Outcome {
        self.reset_session();
        let Some(login_name) = body
            .as_ref()
            .and_then(|b| b.get_text("LoginName"))
            .filter(|n| !n.is_empty() && n.chars().count() <= MAX_LOGIN_NAME)
        else {
            return fail(gate_err(
                ERR_BAD_PARAM,
                0,
                "CliConn: login start, bad LoginName",
            ));
        };
        let login_name = login_name.to_owned();
        let req = Element::new("Request")
            .child(Element::text("LoginName", login_name.clone()))
            .child(Element::text("NetAddress", self.ip.clone()));
        // No `t`: the protocol `Auth` selects the AuthSrv provider (§9.3).
        let frame = request("Auth", "LoginStart", Some(&req));
        let result = self
            .call(frame, "CCliConn::LoginStartReply")
            .await
            .and_then(parse_login_start);
        match result {
            Ok(started) => {
                self.session.session = Some(started.session);
                self.session.user_id = Some(started.user_id.clone());
                self.session.user_center = started.user_center;
                self.session.user_name = started.user_name;
                self.session.login_name = login_name;
                *self.shared_user.lock().unwrap() = Some(started.user_id);
                ok(Some(Element::new("Reply").child(started.key_data)))
            }
            Err(e) => {
                self.session.session = None;
                self.session.login_failures += 1;
                let flow = if self.session.login_failures >= MAX_LOGIN_FAILURES {
                    Flow::Close
                } else {
                    Flow::Continue
                };
                Outcome {
                    result: Err(e),
                    flow,
                }
            }
        }
    }

    // --------------------------------------------------------------- KeyData

    fn log_login(&self, tag: &str) {
        self.gate.log(format!(
            "{tag}, {}, {}, {}",
            self.user(),
            self.addr,
            self.session.login_name
        ));
    }

    async fn key_data(&mut self, body: Option<Element>) -> Outcome {
        let Some(key_data) = body.as_ref().and_then(|b| b.get("KeyData")) else {
            return fail(gate_err(
                ERR_BAD_PARAM,
                0,
                "CliConn: key data, missing body",
            ));
        };
        let Some(session) = self.session.session.clone() else {
            return fail(gate_err(ERR_PERMISSION, 0, "CliConn: key data, no session"));
        };
        let req = Element::new("Request")
            .child(Element::text("Session", session))
            .child(key_data.clone());
        let mut frame = request("Auth", "KeyData", Some(&req));
        set_header(
            &mut frame,
            "t",
            user_target(self.gate.cfg.center, self.user()),
        );
        let reply = match self.call(frame, "CCliConn::KeyDataReply").await {
            Ok(Some(reply)) => reply,
            Ok(None) => {
                return self.key_data_failed(gate_err(
                    ERR_BAD_PARAM,
                    0,
                    "CliConn: key data reply, missing body",
                ))
            }
            Err(e) => return self.key_data_failed(e),
        };
        // Everything but the session key goes to the client.
        let mut out = Element::new("Reply");
        for child in reply.children.iter().filter(|c| c.name != "SessionKey") {
            out.push(without_session_key(child));
        }
        let kd = reply.get("KeyData");
        let complete = reply.has("Complete") || kd.is_some_and(|k| k.has("Complete"));
        if !complete {
            if !out.has("KeyData") {
                return self.key_data_failed(gate_err(
                    ERR_BAD_PARAM,
                    0,
                    "CliConn: key data reply, missing key data",
                ));
            }
            return ok(Some(out));
        }
        let key_text = reply
            .get_text("SessionKey")
            .or_else(|| kd.and_then(|k| k.get_text("SessionKey")))
            .unwrap_or("");
        let key = match STANDARD.decode(key_text.trim()) {
            Ok(k) if !k.is_empty() && k.len() <= 256 => k,
            _ => {
                return self.key_data_failed(gate_err(
                    ERR_BAD_DATA,
                    0,
                    "CliConn: key data reply, bad session key",
                ))
            }
        };
        self.session.key_complete = true;
        self.log_login("LOGIN");
        Outcome {
            result: Ok(Some(out)),
            flow: Flow::EnableCrypt(key),
        }
    }

    fn key_data_failed(&mut self, err: StsError) -> Outcome {
        self.log_login("LOGIN-FAILED");
        self.session.session = None;
        fail(err)
    }

    // ----------------------------------------------------------- LoginFinish

    async fn login_finish(&mut self, body: Option<Element>) -> Outcome {
        let Some(mut req) = body else {
            return fail(gate_err(
                ERR_BAD_PARAM,
                0,
                "CliConn: login finish, missing body",
            ));
        };
        let Some(session) = self.session.session.clone() else {
            return fail(gate_err(ERR_PERMISSION, 1686, ""));
        };
        let user_id = self.user().to_owned();
        // Step 1: Auth/LoginFinish. The client headers `t`, `f`, `p` are not copied.
        req.children.retain(|c| c.name != "Session");
        req.push(Element::text("Session", session));
        let mut frame = request("Auth", "LoginFinish", Some(&req));
        set_header(&mut frame, "t", user_target(self.gate.cfg.center, &user_id));
        let reply = match self.call(frame, "CCliConn::LoginFinishReply").await {
            Ok(Some(reply)) => reply,
            Ok(None) => {
                self.session.session = None;
                return fail(gate_err(
                    ERR_BAD_PARAM,
                    1763,
                    "CliConn: login finish reply auth, missing body",
                ));
            }
            Err(e) => {
                self.session.session = None;
                return fail(e);
            }
        };
        if let Some(auth_type) = reply.get("AuthType") {
            // Secondary auth needed: the login stops here (§10.1).
            let mut out = Element::new("Reply").child(auth_type.clone());
            if let Some(status) = reply.get("Status") {
                out.push(status.clone());
            }
            return ok(Some(out));
        }
        let licenses = reply.get("Licenses").cloned();
        let roles: Vec<u32> = reply
            .get("Roles")
            .map(|r| r.all("RoleId").filter_map(|e| parse_u32(&e.text)).collect())
            .unwrap_or_default();
        let password_until = truncated(reply.get_text("PasswordUntil").unwrap_or(""), 32);
        let user_status = reply.get_u32("UserStatus", 0);

        // Step 2: Access/GetAppUserRights, center 0 = local.
        // ❓ AppId comes from Game.ini in the original. Phase 1 sends it empty.
        let rights = Element::new("Request")
            .child(Element::text("AppId", ""))
            .child(Element::text("AppSecret", ""))
            .child(Element::text("RedirectUri", ""));
        let mut frame = request("Access", "GetAppUserRights", Some(&rights));
        set_header(&mut frame, "t", user_target(0, &user_id));
        let rights_reply = self.call(frame, "CCliConn::GetAppUserRightsReply").await;
        // The session string is freed in all cases (§10.4).
        self.session.session = None;
        let rights_reply = match rights_reply {
            Ok(Some(r)) => r,
            Ok(None) => return fail(gate_err(ERR_BAD_PARAM, 2200, "CliConn: missing body")),
            Err(e) => return fail(e),
        };
        let Some(mask) = rights_reply.get_text("AccessMask") else {
            return fail(gate_err(ERR_BAD_PARAM, 2204, "CliConn: missing accessMask"));
        };
        self.access_mask = parse_c_i64(mask);

        // Step 3: Presence/Login is local in phase 1 (§10.3).
        self.session.login_time = self.gate.clock();
        self.licenses = licenses;
        self.roles = roles;
        self.password_until = password_until;
        self.user_status = user_status;
        ok(Some(self.finish_body()))
    }

    /// Client reply body of §10.4.
    fn finish_body(&self) -> Element {
        let mut out = Element::new("Reply")
            .child(Element::text("UserId", self.user()))
            .child(Element::int(
                "UserCenter",
                i64::from(self.session.user_center),
            ));
        let mut roles = Element::array("Roles");
        for role in &self.roles {
            roles.push(Element::int("RoleId", i64::from(*role)));
        }
        out.push(roles);
        out.push_text_nonempty("PasswordUntil", &self.password_until);
        if let Some(licenses) = &self.licenses {
            out.push(licenses.clone());
        }
        out
    }

    // -------------------------------------------------------- LoginTokenStart

    fn login_token_start(&mut self, body: Option<Element>) -> Outcome {
        self.reset_session();
        let rand = body
            .as_ref()
            .and_then(|b| b.get_text("ClientRand"))
            .and_then(|t| STANDARD.decode(t.trim()).ok())
            .filter(|r| r.len() == 32);
        if rand.is_none() {
            return fail(gate_err(ERR_BAD_PARAM, 1456, "CliConn: bad ClientRand"));
        }
        // Phase 1 has no server certificate (§5).
        fail(gate_err(
            ERR_BAD_DATA,
            1466,
            "CliConn: no server certificate",
        ))
    }
}
