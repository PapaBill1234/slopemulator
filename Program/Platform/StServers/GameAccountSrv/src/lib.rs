//! GameAccountSrv, login path only. RE: `Docs/Platform/StServers/RE/03-gameaccount-slot-spawn-world.md` §1.
use std::collections::HashMap;
use sts_core::{error::ERR_BAD_PARAM, service::target_user, Element, StsError};

/// ❓ The module id of GameAccountSrv errors is not decoded.
const MODULE: u32 = 0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameAccount {
    pub game_code: String,
    pub alias: String,
    pub game_account_id: String,
}

/// The original reads the user games from GameDb through DbSrv and keeps them
/// in a cache (`0x1400CC8E0`). This phase reads a TSV file.
#[derive(Default)]
pub struct GameAccounts {
    by_user: HashMap<String, Vec<GameAccount>>,
}

impl GameAccounts {
    /// TSV, four fields: UserId, GameCode, Alias, GameAccountId. `#` starts a comment.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut out = Self::default();
        for (n, line) in text.lines().enumerate() {
            if line.trim().is_empty() || line.starts_with('#') {
                continue;
            }
            let f: Vec<&str> = line.split('\t').collect();
            let user = (f.len() == 4)
                .then(|| target_user(Some(f[0])))
                .flatten()
                .ok_or_else(|| format!("game accounts line {}: bad record", n + 1))?;
            out.by_user.entry(user).or_default().push(GameAccount {
                game_code: f[1].into(),
                alias: f[2].into(),
                game_account_id: f[3].to_ascii_uppercase(),
            });
        }
        Ok(out)
    }

    pub fn handle(
        &self,
        command: &str,
        t: Option<&str>,
        body: Option<&Element>,
    ) -> Result<Option<Element>, StsError> {
        let err =
            |line: u32, text: &str| StsError::new(ERR_BAD_PARAM, MODULE, line).with_text(text);
        match command {
            // Ids 1 and 3 use the same handler (0x1400C70D0).
            "ListAccounts" | "ListMyAccounts" => {
                // ❓ The source of the "target code" is not decoded. Here: body `GameCode`.
                let code = body
                    .and_then(|b| b.get_text("GameCode"))
                    .filter(|c| !c.is_empty())
                    .ok_or_else(|| err(134, "missing parameters - no target code"))?;
                let user = target_user(t).ok_or_else(|| err(0, "invalid userid"))?;
                let mut reply = Element::new("Reply");
                for g in self.by_user.get(&user).into_iter().flatten() {
                    if g.game_code.eq_ignore_ascii_case(code) {
                        reply.push(
                            Element::new("GameAccount")
                                .child(Element::text("Alias", g.alias.clone()))
                                .child(Element::text("GameAccountId", g.game_account_id.clone())),
                        );
                    }
                }
                Ok(Some(reply))
            }
            _ => Err(StsError::new(52, 2, 462)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const USER: &str = "03020100-0504-0706-0809-0A0B0C0D0E0F";

    fn store() -> GameAccounts {
        GameAccounts::parse(&format!(
            "# comment\n{USER}\tbns\tmain\t11111111-2222-3333-4444-555555555555\n{USER}\tother\tx\t11111111-2222-3333-4444-555555555556\n"
        ))
        .unwrap()
    }

    #[test]
    fn lists_the_accounts_of_the_game() {
        let body = Element::new("Request").child(Element::text("GameCode", "bns"));
        let t = format!("0${USER}");
        for command in ["ListAccounts", "ListMyAccounts"] {
            let reply = store()
                .handle(command, Some(&t), Some(&body))
                .unwrap()
                .unwrap();
            let all: Vec<_> = reply.all("GameAccount").collect();
            assert_eq!(all.len(), 1);
            assert_eq!(all[0].get_text("Alias"), Some("main"));
            assert_eq!(
                all[0].get_text("GameAccountId"),
                Some("11111111-2222-3333-4444-555555555555")
            );
        }
    }

    #[test]
    fn errors() {
        let body = Element::new("Request").child(Element::text("GameCode", "bns"));
        let s = store();
        let e = s.handle("ListAccounts", Some(USER), None).unwrap_err();
        assert_eq!((e.code, e.line), (10, 134));
        let e = s
            .handle("ListAccounts", Some("x"), Some(&body))
            .unwrap_err();
        assert_eq!(e.text, "invalid userid");
        assert_eq!(
            s.handle("Nope", Some(USER), Some(&body)).unwrap_err().code,
            52
        );
        // A user without accounts gets an empty list.
        let other = "13020100-0504-0706-0809-0A0B0C0D0E0F";
        let reply = s
            .handle("ListAccounts", Some(other), Some(&body))
            .unwrap()
            .unwrap();
        assert_eq!(reply.all("GameAccount").count(), 0);
        assert!(GameAccounts::parse("a\tb").is_err());
    }
}
