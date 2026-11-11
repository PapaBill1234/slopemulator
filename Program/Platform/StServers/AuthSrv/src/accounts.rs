//! Account rows. The original reads them with `AcctLoginInfoById` through
//! DbSrv (spec 15 §8.1). This phase reads the same columns from a TSV file.
use crate::guid;
use base64::{engine::general_purpose::STANDARD, Engine};
use rand::RngCore;
use std::collections::HashMap;
use sts::srp6;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    pub login_name: String,
    pub user_id: String,
    pub user_name: String,
    pub user_center: u32,
    pub user_status: u32,
    pub validated: bool,
    pub salt: Vec<u8>,
    pub verifier: Vec<u8>,
    pub roles: Vec<u32>,
}

/// SRP identity I (§4.1): ASCII upper case → lower case, max 64 chars.
pub fn identity(login: &str) -> String {
    login
        .chars()
        .take(64)
        .collect::<String>()
        .to_ascii_lowercase()
}

#[derive(Default)]
pub struct Accounts {
    by_login: HashMap<String, Account>,
    login_of: HashMap<String, String>,
}

impl Accounts {
    pub fn insert(&mut self, a: Account) {
        self.login_of
            .insert(a.user_id.clone(), identity(&a.login_name));
        self.by_login.insert(identity(&a.login_name), a);
    }
    pub fn by_login(&self, login: &str) -> Option<&Account> {
        self.by_login.get(&identity(login))
    }
    pub fn by_user_id(&self, user_id: &str) -> Option<&Account> {
        self.by_login.get(self.login_of.get(user_id)?)
    }
    pub fn len(&self) -> usize {
        self.by_login.len()
    }
    pub fn is_empty(&self) -> bool {
        self.by_login.is_empty()
    }

    /// TSV, six fields: LoginName, UserId, UserName, UserCenter, Salt (Base64),
    /// Verifier (Base64). Lines that start with `#` are comments.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut out = Self::default();
        for (n, line) in text.lines().enumerate() {
            if line.trim().is_empty() || line.starts_with('#') {
                continue;
            }
            let f: Vec<&str> = line.split('\t').collect();
            let bad = |what: &str| format!("accounts line {}: {what}", n + 1);
            if f.len() != 6 {
                return Err(bad("6 fields are necessary"));
            }
            out.insert(Account {
                login_name: f[0].into(),
                user_id: guid::normalize(f[1]).ok_or_else(|| bad("bad UserId"))?,
                user_name: f[2].into(),
                user_center: f[3].parse().map_err(|_| bad("bad UserCenter"))?,
                user_status: 1,
                validated: true,
                salt: STANDARD.decode(f[4]).map_err(|_| bad("bad Salt"))?,
                verifier: STANDARD.decode(f[5]).map_err(|_| bad("bad Verifier"))?,
                roles: Vec::new(),
            });
        }
        Ok(out)
    }
}

/// Makes a new account row with an 8-byte salt (the size of `Users.Salt`).
pub fn enroll(login: &str, user_name: &str, password: &[u8]) -> Account {
    let mut rng = rand::rngs::OsRng;
    let mut salt = [0u8; 8];
    // The salt is a little-endian number of 8 bytes: the top byte must not be zero.
    while salt[7] == 0 {
        rng.fill_bytes(&mut salt);
    }
    let mut id = [0u8; 16];
    rng.fill_bytes(&mut id);
    Account {
        login_name: login.into(),
        user_id: guid::format(&id),
        user_name: user_name.into(),
        user_center: 1,
        user_status: 1,
        validated: true,
        verifier: srp6::verifier(&identity(login), password, &salt),
        salt: salt.to_vec(),
        roles: Vec::new(),
    }
}

pub fn to_line(a: &Account) -> String {
    format!(
        "{}\t{}\t{}\t{}\t{}\t{}",
        a.login_name,
        a.user_id,
        a.user_name,
        a.user_center,
        STANDARD.encode(&a.salt),
        STANDARD.encode(&a.verifier)
    )
}
