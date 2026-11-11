//! AuthSrv handlers with a client model of the SRP exchange (spec 15 §4.4).
use authsrv::{
    accounts::{enroll, identity, to_line, Accounts},
    service::{AuthService, Bus, BusFuture, Config},
};
use base64::{engine::general_purpose::STANDARD, Engine};
use std::sync::{Arc, Mutex};
use sts::{key_data, srp6, Frame};
use sts_core::{conn::header, Element, StsError};

const PASSWORD: &[u8] = b"generated-test-value";
const ACCOUNT_ID: &str = "11111111-2222-3333-4444-555555555555";

/// Fake GameAccountSrv: one game account with alias `bns`.
struct FakeBus;
impl Bus for FakeBus {
    fn call(&self, frame: Frame) -> BusFuture<'_> {
        Box::pin(async move {
            assert!(header(&frame, "t").is_some());
            Ok(Some(
                Element::new("Reply").child(
                    Element::new("GameAccount")
                        .child(Element::text("Alias", "bns"))
                        .child(Element::text("GameAccountId", ACCOUNT_ID)),
                ),
            ))
        })
    }
}

struct Fixture {
    service: AuthService,
    user_id: String,
    log: Arc<Mutex<Vec<String>>>,
}

fn fixture() -> Fixture {
    let account = enroll("Tester@Example", "Tester", PASSWORD);
    assert_eq!(account.salt.len(), 8);
    let user_id = account.user_id.clone();
    // The account file form is the input of the service.
    let accounts = Accounts::parse(&to_line(&account)).unwrap();
    let log = Arc::new(Mutex::new(Vec::new()));
    let log2 = log.clone();
    let service = AuthService::new(Config::default(), accounts)
        .with_log(move |line| log2.lock().unwrap().push(line));
    Fixture {
        service,
        user_id,
        log,
    }
}

async fn call(
    f: &Fixture,
    command: &str,
    t: Option<&str>,
    body: Element,
) -> Result<Element, StsError> {
    f.service
        .handle(command, t, Some(&body), &FakeBus)
        .await
        .map(|b| b.expect("body"))
}

fn req() -> Element {
    Element::new("Request")
}

/// LoginStart + KeyData. Returns the session id.
async fn login(f: &Fixture, login_name: &str, password: &[u8]) -> Result<String, StsError> {
    let start = call(
        f,
        "LoginStart",
        None,
        req().child(Element::text("LoginName", login_name)),
    )
    .await?;
    assert_eq!(start.get_text("UserId"), Some(f.user_id.as_str()));
    let session = start.get_text("Session").unwrap().to_owned();
    let (salt, b) = key_data::decode_challenge(start.get_text("KeyData").unwrap()).unwrap();
    assert_eq!(salt.len(), 8);
    let (a, m1, m2, key) =
        srp6::client_model(&identity(login_name), password, &salt, &b, &[7; 32]).unwrap();
    let reply = call(
        f,
        "KeyData",
        None,
        req()
            .child(Element::text("Session", session.clone()))
            .child(Element::text(
                "KeyData",
                key_data::encode_client(&a, &m1).unwrap(),
            )),
    )
    .await?;
    assert!(reply.has("Complete"));
    assert_eq!(
        key_data::decode_server(reply.get_text("KeyData").unwrap()).unwrap(),
        m2
    );
    assert_eq!(
        STANDARD
            .decode(reply.get_text("SessionKey").unwrap())
            .unwrap(),
        key
    );
    Ok(session)
}

#[tokio::test]
async fn login_finish_and_tokens() {
    let f = fixture();
    // The login name is not case-sensitive (§4.1 I).
    let session = login(&f, "tester@example", PASSWORD).await.unwrap();
    assert!(f
        .log
        .lock()
        .unwrap()
        .iter()
        .any(|l| l.starts_with("LOGIN, ")));
    let finish = call(
        &f,
        "LoginFinish",
        None,
        req().child(Element::text("Session", session.clone())),
    )
    .await
    .unwrap();
    let names: Vec<&str> = finish.children.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "LoginName",
            "UserId",
            "UserName",
            "UserCenter",
            "UserStatus",
            "Roles",
            "ExternalAccounts"
        ]
    );
    // The session is deleted by LoginFinish.
    let again = call(
        &f,
        "LoginFinish",
        None,
        req().child(Element::text("Session", session)),
    )
    .await;
    assert_eq!(again.unwrap_err().code, 3101);

    let t = format!("1${}", f.user_id);
    let info = call(&f, "GetMyUserInfo", Some(&t), req()).await.unwrap();
    assert_eq!(info.get_text("LoginName"), Some("Tester@Example"));

    // App token: `<UserId>:<token>`, one for each (user, AppId).
    let app = req().child(Element::text(
        "AppId",
        "B0D42105-0000-0000-0000-000000000001",
    ));
    let first = call(&f, "RequestToken", Some(&t), app.clone())
        .await
        .unwrap();
    let text = first.get_text("AuthnToken").unwrap();
    assert!(text.starts_with(&format!("{}:", f.user_id)) && text.len() == 73);
    call(&f, "RequestToken", Some(&t), app).await.unwrap();
    assert!(f
        .log
        .lock()
        .unwrap()
        .iter()
        .any(|l| l.contains("OVERWRITTEN")));
    let no_app = call(&f, "RequestToken", Some(&t), req()).await.unwrap_err();
    assert_eq!((no_app.code, no_app.text.as_str()), (10, "missing appId"));

    // Game token: made from the ListAccounts reply, consumed one time.
    let game = |alias: &str| {
        req()
            .child(Element::text("GameCode", "bns"))
            .child(Element::text("AccountAlias", alias))
    };
    let miss = call(&f, "RequestGameToken", Some(&t), game("other"))
        .await
        .unwrap_err();
    assert_eq!(miss.text, "request game token, missing game account");
    let token = call(&f, "RequestGameToken", Some(&t), game("bns"))
        .await
        .unwrap();
    let token = token.get_text("Token").unwrap().to_owned();
    let consume = |code: &str| {
        req()
            .child(Element::text("Token", token.clone()))
            .child(Element::text("GameCode", code))
            .child(Element::text("ClientNetAddress", "127.0.0.1"))
    };
    let wrong = call(&f, "ConsumeGameToken", Some(&t), consume("other"))
        .await
        .unwrap_err();
    assert_eq!((wrong.code, wrong.line), (3010, 8038));
    let ok = call(&f, "ConsumeGameToken", Some(&t), consume("bns"))
        .await
        .unwrap();
    assert_eq!(ok.get_text("GameAccountId"), Some(ACCOUNT_ID));
    assert_eq!(ok.get_text("UserId"), Some(f.user_id.as_str()));
    let replay = call(&f, "ConsumeGameToken", Some(&t), consume("bns"))
        .await
        .unwrap_err();
    assert_eq!(replay.code, 3010);
    let no_user = call(&f, "ConsumeGameToken", None, consume("bns"))
        .await
        .unwrap_err();
    assert_eq!(no_user.text, "AuMain: missing userId");
}

#[tokio::test]
async fn wrong_password_deletes_the_session() {
    let f = fixture();
    let e = login(&f, "Tester@Example", b"not-the-value")
        .await
        .unwrap_err();
    assert_eq!((e.code, e.module, e.line), (11, 6, 7483));
    assert!(f
        .log
        .lock()
        .unwrap()
        .iter()
        .any(|l| l.starts_with("LOGIN-FAILED, ")));
}

#[tokio::test]
async fn request_errors() {
    let f = fixture();
    let e = call(&f, "LoginStart", None, req()).await.unwrap_err();
    assert_eq!((e.code, e.text.as_str()), (10, "missing login name"));
    let e = call(
        &f,
        "LoginStart",
        None,
        req().child(Element::text("LoginName", "nobody")),
    )
    .await
    .unwrap_err();
    assert_eq!(e.code, 3101);
    let e = call(
        &f,
        "KeyData",
        None,
        req().child(Element::text("Session", "x")),
    )
    .await
    .unwrap_err();
    assert_eq!(
        (e.code, e.text.as_str()),
        (3101, "login session not found.")
    );
    // Bad Base64 text deletes the session.
    let start = call(
        &f,
        "LoginStart",
        None,
        req().child(Element::text("LoginName", "tester@example")),
    )
    .await
    .unwrap();
    let session = start.get_text("Session").unwrap();
    let bad = req()
        .child(Element::text("Session", session))
        .child(Element::text("KeyData", "!!!!"));
    let e = call(&f, "KeyData", None, bad.clone()).await.unwrap_err();
    assert_eq!(e.text, "invalid keydata.");
    assert_eq!(call(&f, "KeyData", None, bad).await.unwrap_err().code, 3101);
    // LoginFinish before KeyData is refused.
    let start = call(
        &f,
        "LoginStart",
        None,
        req().child(Element::text("LoginName", "tester@example")),
    )
    .await
    .unwrap();
    let early = req().child(Element::text("Session", start.get_text("Session").unwrap()));
    assert_eq!(
        call(&f, "LoginFinish", None, early).await.unwrap_err().code,
        3101
    );
    let e = call(&f, "RequestGameToken", Some("1$bad"), req())
        .await
        .unwrap_err();
    assert_eq!(e.text, "AuMain: missing userId");
}
