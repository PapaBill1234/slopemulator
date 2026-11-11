//! Handlers of AuthSrv (spec 15). Ids: 18 GetMyUserInfo, 21 KeyData,
//! 22 LoginStart, 23 LoginFinish, 30 RequestGameToken, 31 ConsumeGameToken,
//! 32 RequestToken.
use crate::{
    accounts::{identity, Account, Accounts},
    guid,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use rand::RngCore;
use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use sts::{key_data, srp6::ServerExchange, Frame};
use sts_core::{
    conn::{header, reply_result, request, set_header, DEFAULT_TIMEOUT},
    error::{ERR_BAD_DATA, ERR_BAD_PARAM, ERR_PERMISSION},
    route_client::{RouteClient, RouteEvent},
    service::bus_reply,
    Element, StsError,
};
use tokio::sync::mpsc;

pub const ERR_BAD_PASSWD: u32 = 11;
pub const ERR_TOKEN_NOT_FOUND: u32 = 3010;
pub const ERR_TOKEN_WRONG_LOCATION: u32 = 3303;
pub const ERR_BAD_SESSION: u32 = 3101;
pub const ERR_NO_SALT_VERIFIER: u32 = 3317;
pub const ERR_NEED_EMAIL_VALIDATED: u32 = 3320;
/// Module ids from the error calls of the binary.
const MOD_MAIN: u32 = 5;
const MOD_LOGIN: u32 = 6;

pub type BusFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Option<Element>, StsError>> + Send + 'a>>;

/// Requests that AuthSrv sends to other services.
pub trait Bus: Send + Sync {
    fn call(&self, frame: Frame) -> BusFuture<'_>;
}

impl Bus for RouteClient {
    fn call(&self, frame: Frame) -> BusFuture<'_> {
        Box::pin(async move { reply_result(&self.request(frame, DEFAULT_TIMEOUT).await?) })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Config {
    /// `[TokenTimeout] App`, default 1800 s (§7.1).
    pub app_token: Duration,
    /// `[TokenTimeout] Game`, default 300 s (§7.1).
    pub game_token: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            app_token: Duration::from_secs(1800),
            game_token: Duration::from_secs(300),
        }
    }
}

struct LoginSession {
    account: Account,
    exchange: Option<ServerExchange>,
    complete: bool,
}

struct AppToken {
    user_id: String,
    app_id: String,
    expiry: Instant,
}

struct GameToken {
    user_id: String,
    game_code: String,
    game_account_id: String,
    expiry: Instant,
}

#[derive(Default)]
struct State {
    sessions: HashMap<String, LoginSession>,
    app_tokens: HashMap<String, AppToken>,
    game_tokens: HashMap<String, GameToken>,
}

pub struct AuthService {
    cfg: Config,
    accounts: Accounts,
    state: Mutex<State>,
    log: Box<dyn Fn(String) + Send + Sync>,
}

fn err(code: u32, line: u32, text: &str) -> StsError {
    StsError::new(code, MOD_LOGIN, line).with_text(text)
}

fn text<'a>(body: Option<&'a Element>, name: &str) -> Option<&'a str> {
    body.and_then(|b| b.get_text(name))
        .filter(|t| !t.is_empty())
}

fn roles(a: &Account) -> Element {
    let mut e = Element::array("Roles");
    for r in &a.roles {
        e.push(Element::int("RoleId", i64::from(*r)));
    }
    e
}

impl AuthService {
    pub fn new(cfg: Config, accounts: Accounts) -> Self {
        Self {
            cfg,
            accounts,
            state: Mutex::new(State::default()),
            log: Box::new(|line| eprintln!("{line}")),
        }
    }

    pub fn with_log(mut self, log: impl Fn(String) + Send + Sync + 'static) -> Self {
        self.log = Box::new(log);
        self
    }

    /// Runs one `Auth` request. `t` is the target header of the request.
    pub async fn handle(
        &self,
        command: &str,
        t: Option<&str>,
        body: Option<&Element>,
        bus: &dyn Bus,
    ) -> Result<Option<Element>, StsError> {
        match command {
            "LoginStart" => self.login_start(t, body),
            "KeyData" => self.key_data(body),
            "LoginFinish" => self.login_finish(body),
            "RequestToken" => self.request_token(self.user(t)?, body),
            "RequestGameToken" => self.request_game_token(self.user(t)?, body, bus).await,
            "ConsumeGameToken" => self.consume_game_token(t, body),
            "GetMyUserInfo" => Ok(Some(self.user_info(self.user(t)?))),
            _ => Err(StsError::new(ERR_BAD_PARAM, MOD_MAIN, 0)
                .with_text("AuMain: invalid message, parser line 0")),
        }
    }

    /// §3.3: each request needs a GUID `t`.
    fn user(&self, t: Option<&str>) -> Result<&Account, StsError> {
        let id = t.and_then(guid::from_target).ok_or_else(|| {
            StsError::new(ERR_BAD_PARAM, MOD_MAIN, 170).with_text("AuMain: missing userId")
        })?;
        // ❓ The original finds the user record before the handler. The error is not decoded.
        self.accounts.by_user_id(&id).ok_or_else(|| {
            StsError::new(ERR_BAD_PARAM, MOD_MAIN, 170).with_text("AuMain: missing userId")
        })
    }

    // ---------------------------------------------------------- LoginStart §3

    fn login_start(
        &self,
        t: Option<&str>,
        body: Option<&Element>,
    ) -> Result<Option<Element>, StsError> {
        let login =
            text(body, "LoginName").ok_or_else(|| err(ERR_BAD_PARAM, 0, "missing login name"))?;
        // §3.3: the router side resolves the login name to a UserId in the
        // original ❓. Here the account file is the resolver. A GUID `t` wins.
        let account = match t.and_then(guid::from_target) {
            Some(id) => self.accounts.by_user_id(&id),
            None => self.accounts.by_login(login),
        }
        .ok_or_else(|| err(ERR_BAD_SESSION, 0, "login start reply, missing row"))?;
        if account.salt.is_empty() || account.verifier.is_empty() {
            return Err(err(ERR_NO_SALT_VERIFIER, 563, "no salt verifier"));
        }
        let mut random = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut random);
        let fail = || err(ERR_BAD_DATA, 571, "start key exchange fail");
        if account.salt.len() != 8 {
            return Err(fail());
        }
        let exchange =
            ServerExchange::start(&identity(login), &account.salt, &account.verifier, &random)
                .map_err(|_| fail())?;
        let challenge =
            key_data::encode_challenge(&account.salt, exchange.public()).map_err(|_| fail())?;
        let session = guid::random();
        let reply = Element::new("Reply")
            .child(Element::text("Session", session.clone()))
            .child(Element::text("UserId", account.user_id.clone()))
            .child(Element::text("UserName", account.user_name.clone()))
            .child(Element::int("UserCenter", i64::from(account.user_center)))
            .child(Element::text("KeyData", challenge));
        self.state.lock().unwrap().sessions.insert(
            session,
            LoginSession {
                account: account.clone(),
                exchange: Some(exchange),
                complete: false,
            },
        );
        Ok(Some(reply))
    }

    // ------------------------------------------------------------- KeyData §5

    fn key_data(&self, body: Option<&Element>) -> Result<Option<Element>, StsError> {
        let mut state = self.state.lock().unwrap();
        let id = text(body, "Session").unwrap_or("").to_owned();
        if !state.sessions.contains_key(&id) {
            return Err(err(ERR_BAD_SESSION, 0, "login session not found."));
        }
        let data =
            text(body, "KeyData").ok_or_else(|| err(ERR_BAD_PARAM, 0, "missing keydata."))?;
        let is_b64 = |c: u8| c.is_ascii_alphanumeric() || matches!(c, b'+' | b'/' | b'=');
        if !data.bytes().all(is_b64) {
            state.sessions.remove(&id);
            return Err(err(ERR_BAD_PARAM, 0, "invalid keydata."));
        }
        if data.len() / 4 == 0 {
            state.sessions.remove(&id);
            return Err(err(ERR_BAD_PARAM, 0, "invalid keydatalen."));
        }
        let session = state.sessions.get_mut(&id).expect("checked");
        let login = session.account.login_name.clone();
        let verified = session.exchange.take().and_then(|x| {
            let (a, m1) = key_data::decode_client(data).ok()?;
            x.verify(&a, &m1).ok()
        });
        let Some(verified) = verified else {
            state.sessions.remove(&id);
            (self.log)(format!("LOGIN-FAILED, {login}"));
            return Err(StsError::new(ERR_BAD_PASSWD, MOD_LOGIN, 7483));
        };
        if !session.account.validated {
            state.sessions.remove(&id);
            (self.log)(format!("NOT VALIDATED, {login}"));
            return Err(StsError::new(ERR_NEED_EMAIL_VALIDATED, MOD_LOGIN, 7487));
        }
        session.complete = true;
        (self.log)(format!("LOGIN, {login}"));
        let m2 = key_data::encode_server(&verified.proof)
            .map_err(|_| err(ERR_BAD_DATA, 0, "key data encode"))?;
        Ok(Some(
            Element::new("Reply")
                .child(Element::text("KeyData", m2))
                .child(Element::new("Complete"))
                .child(Element::text("SessionKey", STANDARD.encode(verified.key))),
        ))
    }

    // --------------------------------------------------------- LoginFinish §6

    fn login_finish(&self, body: Option<&Element>) -> Result<Option<Element>, StsError> {
        let id = text(body, "Session").unwrap_or("");
        // ❓ The error code of this handler is not verified. KeyData uses 3101.
        // ❓ A session that is not complete is refused here. The original
        // relies on the CliGate pre-check (11 §10.1).
        let session = {
            let mut state = self.state.lock().unwrap();
            match state.sessions.remove(id) {
                Some(s) if s.complete => s,
                _ => return Err(err(ERR_BAD_SESSION, 0, "login session not found.")),
            }
        };
        let a = &session.account;
        Ok(Some(
            Element::new("Reply")
                .child(Element::text("LoginName", a.login_name.clone()))
                .child(Element::text("UserId", a.user_id.clone()))
                .child(Element::text("UserName", a.user_name.clone()))
                .child(Element::int("UserCenter", i64::from(a.user_center)))
                .child(Element::int("UserStatus", i64::from(a.user_status)))
                .child(roles(a))
                .child(Element::array("ExternalAccounts")),
        ))
    }

    /// ❓ GetMyUserInfo (id 18) is not decoded in AuthSrv. The reply has the
    /// elements that the client parser reads (`../Sts/07-client-login-wire.md`).
    fn user_info(&self, a: &Account) -> Element {
        Element::new("Reply")
            .child(Element::text("UserId", a.user_id.clone()))
            .child(Element::int("UserCenter", i64::from(a.user_center)))
            .child(Element::text("UserName", a.user_name.clone()))
            .child(Element::text("LoginName", a.login_name.clone()))
    }

    // ------------------------------------------------------- RequestToken §7.2

    fn request_token(
        &self,
        user: &Account,
        body: Option<&Element>,
    ) -> Result<Option<Element>, StsError> {
        if user.user_status != 1 {
            return Err(err(ERR_PERMISSION, 0, "Invalid UserStatus"));
        }
        // ❓ The parser copies the issuer AppId when the element is absent. No issuer here.
        let app_id = text(body, "AppId")
            .and_then(guid::normalize)
            .filter(|a| a.bytes().any(|c| c != b'0' && c != b'-'))
            .ok_or_else(|| err(ERR_BAD_PARAM, 0, "missing appId"))?;
        let token = guid::random();
        let mut state = self.state.lock().unwrap();
        // One token for each (user, AppId): the old one is deleted (§7.3 step 2).
        let old = state
            .app_tokens
            .iter()
            .find(|(_, t)| t.user_id == user.user_id && t.app_id == app_id)
            .map(|(k, _)| k.clone());
        if let Some(old) = old {
            state.app_tokens.remove(&old);
            (self.log)(format!(
                "APPTOKEN,OVERWRITTEN,UserId,{},AppId,{app_id}",
                user.user_id
            ));
        }
        let now = Instant::now();
        state.app_tokens.retain(|_, t| t.expiry > now);
        state.app_tokens.insert(
            token.clone(),
            AppToken {
                user_id: user.user_id.clone(),
                app_id: app_id.clone(),
                expiry: now + self.cfg.app_token,
            },
        );
        (self.log)(format!(
            "APPTOKEN,CREATE,UserId,{},AppId,{app_id}",
            user.user_id
        ));
        Ok(Some(Element::new("Reply").child(Element::text(
            "AuthnToken",
            format!("{}:{token}", user.user_id),
        ))))
    }

    // --------------------------------------------------- RequestGameToken §7.4

    async fn request_game_token(
        &self,
        user: &Account,
        body: Option<&Element>,
        bus: &dyn Bus,
    ) -> Result<Option<Element>, StsError> {
        if user.user_status != 1 {
            return Err(err(ERR_PERMISSION, 0, "invalid user status"));
        }
        let (Some(game_code), Some(alias)) = (text(body, "GameCode"), text(body, "AccountAlias"))
        else {
            return Err(err(ERR_BAD_PARAM, 0, "missing parameter"));
        };
        let req = Element::new("Request").child(Element::text("GameCode", game_code));
        let mut frame = request("GameAccount", "ListAccounts", Some(&req));
        // Router target form `<center>$<key>`, center 0 = local (13 §5).
        set_header(&mut frame, "t", format!("0${}", user.user_id));
        let reply = bus
            .call(frame)
            .await?
            .ok_or_else(|| err(ERR_BAD_DATA, 0, "request game token reply, missing data"))?;
        let account = reply
            .all("GameAccount")
            .find(|g| g.get_text("Alias") == Some(alias))
            .and_then(|g| g.get("GameAccountId"))
            .ok_or_else(|| err(ERR_BAD_DATA, 0, "request game token, missing game account"))?;
        if account.text.is_empty() {
            return Err(err(ERR_BAD_DATA, 0, "request game token, invalid data"));
        }
        let token = guid::random();
        let mut state = self.state.lock().unwrap();
        let now = Instant::now();
        state.game_tokens.retain(|_, t| t.expiry > now);
        // No "replace the old token" step (§7.5 step 3).
        state.game_tokens.insert(
            token.clone(),
            GameToken {
                user_id: user.user_id.clone(),
                game_code: game_code.chars().take(16).collect(),
                game_account_id: account.text.clone(),
                expiry: now + self.cfg.game_token,
            },
        );
        Ok(Some(
            Element::new("Reply").child(Element::text("Token", token)),
        ))
    }

    // --------------------------------------------------- ConsumeGameToken §7.7

    /// §7.7. The user comes from `t`; the token list belongs to that user.
    fn consume_game_token(
        &self,
        t: Option<&str>,
        body: Option<&Element>,
    ) -> Result<Option<Element>, StsError> {
        let user = self.user(t)?;
        if body.is_none() {
            return Err(err(ERR_BAD_PARAM, 7933, "missing body"));
        }
        let token = text(body, "Token")
            .and_then(guid::normalize)
            .filter(|g| g.bytes().any(|c| c != b'0' && c != b'-'))
            .ok_or_else(|| err(ERR_BAD_PARAM, 7940, "missing token"))?;
        let game_code =
            text(body, "GameCode").ok_or_else(|| err(ERR_BAD_PARAM, 7947, "missing game code"))?;
        let from = text(body, "ClientNetAddress").unwrap_or("");
        let found = {
            let mut state = self.state.lock().unwrap();
            let now = Instant::now();
            state.game_tokens.retain(|_, t| t.expiry > now);
            let hit = state
                .game_tokens
                .get(&token)
                .is_some_and(|t| t.user_id == user.user_id && t.game_code == game_code);
            // The token is deleted after the reply: one use only.
            hit.then(|| state.game_tokens.remove(&token)).flatten()
        };
        let Some(found) = found else {
            (self.log)(format!(
                "failed to authenticate game token - user:{}, from:{from}, error:{ERR_TOKEN_NOT_FOUND}(1), game:{game_code}, token:{token}",
                user.user_id
            ));
            return Err(StsError::new(ERR_TOKEN_NOT_FOUND, MOD_LOGIN, 8038));
        };
        // ❓ The address check (error 3303) passes when GeoIP has no data. No GeoIP here.
        (self.log)(format!(
            "game token is authenticated - user:{}, from:{from}, game:{game_code}, token:{token}",
            user.user_id
        ));
        Ok(Some(
            Element::new("Reply")
                .child(Element::text("GameAccountId", found.game_account_id))
                .child(Element::text("LoginName", user.login_name.clone()))
                .child(Element::text("UserId", user.user_id.clone()))
                .child(Element::text("UserName", user.user_name.clone()))
                .child(Element::int("UserStatus", i64::from(user.user_status)))
                .child(Element::int("UserCenter", i64::from(user.user_center)))
                .child(roles(user)),
        ))
    }
}

/// Request loop of the service on the route bus.
pub async fn run(
    service: Arc<AuthService>,
    route: RouteClient,
    mut events: mpsc::UnboundedReceiver<RouteEvent>,
    server_id: u32,
) {
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
                            .handle(command, header(&inc.frame, "t"), body.as_ref(), &route)
                            .await
                    } else {
                        Err(StsError::new(ERR_BAD_PARAM, MOD_MAIN, 0))
                    };
                    eprintln!(
                        "AuthSrv: {protocol}/{command} -> {}",
                        result.as_ref().map_or_else(|e| e.code, |_| 0)
                    );
                    bus_reply(&inc, &result, server_id);
                });
            }
            _ => {}
        }
    }
}
