//! Client session state (spec §12.1), precheck rules (§3, §10.1) and reset (§12.2).

use crate::auth_cmd::AuthCmd;
use sts_core::error::{module, StsError, ERR_PERMISSION};

pub const ERR_USER_NOT_ONLINE: u32 = 3007;
pub const ERR_CLIENT_NOT_FOUND: u32 = 3018;
/// Default and minimum idle timeout (§12.5).
pub const MIN_IDLE_TIMEOUT_MS: u64 = 300_000;

/// `[IdleTimeout]` seconds to ms. Missing, zero, negative or small values give 300 000.
pub fn idle_timeout_ms(seconds: Option<i64>) -> u64 {
    match seconds {
        Some(s) if s > 0 => (s as u64).saturating_mul(1000).max(MIN_IDLE_TIMEOUT_MS),
        _ => MIN_IDLE_TIMEOUT_MS,
    }
}

/// `t` header for a user: `<center>$<UserId>` (§9.3).
pub fn user_target(center: u32, user_id: &str) -> String {
    format!("{center}${user_id}")
}

/// Log line of §12.2: `<tag>, <UserId>, <address>, <login name>`.
pub fn reset_log_line(logged_in: bool, user_id: &str, addr: &str, login_name: &str) -> String {
    let tag = if logged_in { "LOGOUT" } else { "DISCONNECT" };
    format!("{tag}, {user_id}, {addr}, {login_name}")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionState {
    Connected,
    LoginStarted,
    KeyDataComplete,
    LoggedIn,
}

#[derive(Default)]
pub struct Session {
    pub session: Option<String>,
    pub user_id: Option<String>,
    pub user_center: u32,
    pub user_name: String,
    pub login_name: String,
    pub key_complete: bool,
    /// Non-zero = logged in (conn+300).
    pub login_time: u32,
    /// Failed LoginStart count (conn+296). Reset does not clear it.
    pub login_failures: u32,
}

impl Session {
    pub fn state(&self) -> SessionState {
        if self.login_time != 0 {
            SessionState::LoggedIn
        } else if self.key_complete {
            SessionState::KeyDataComplete
        } else if self.session.is_some() {
            SessionState::LoginStarted
        } else {
            SessionState::Connected
        }
    }

    /// Precheck of an Auth command in the current state.
    pub fn check(&self, cmd: AuthCmd) -> Result<(), StsError> {
        let err = |code: u32, line: u32| Err(StsError::new(code, module::CLI_GATE, line));
        let state = self.state();
        match cmd {
            // These two reset the session first, so every state is fine.
            AuthCmd::LoginStart | AuthCmd::LoginTokenStart => Ok(()),
            // ❓ The handler of ids 18 and 20 (sub_1400CFA50) needs the login
            // session (conn+328), else ErrPermission line 2331. But the game
            // client sends GetMyUserInfo after LoginFinish and Gate.ini lists
            // it. The port forwards it for a logged-in user (§3 note).
            AuthCmd::GetMyUserInfo => {
                if state == SessionState::LoggedIn {
                    Ok(())
                } else {
                    err(ERR_PERMISSION, 2331)
                }
            }
            AuthCmd::CreateAccount => {
                if state == SessionState::LoggedIn {
                    err(ERR_PERMISSION, 0)
                } else {
                    Ok(())
                }
            }
            // ❓ The spec has no source line for this error.
            AuthCmd::KeyData => {
                if state == SessionState::LoginStarted {
                    Ok(())
                } else {
                    err(ERR_PERMISSION, 0)
                }
            }
            AuthCmd::LoginFinish => {
                if state == SessionState::LoggedIn || !self.key_complete {
                    err(ERR_PERMISSION, 1681)
                } else if self.session.is_none() {
                    err(ERR_PERMISSION, 1686)
                } else {
                    Ok(())
                }
            }
            AuthCmd::LogoutMyClient => {
                if state == SessionState::LoggedIn {
                    Ok(())
                } else {
                    err(ERR_USER_NOT_ONLINE, 1736)
                }
            }
            AuthCmd::RequestToken | AuthCmd::RequestGameToken => {
                if state == SessionState::LoggedIn {
                    Ok(())
                } else {
                    err(ERR_PERMISSION, 2628)
                }
            }
            _ => Ok(()),
        }
    }

    /// Clears the session (§12.2) and returns the log line.
    pub fn reset(&mut self, addr: &str) -> String {
        let line = reset_log_line(
            self.login_time != 0,
            self.user_id.as_deref().unwrap_or(""),
            addr,
            &self.login_name,
        );
        self.session = None;
        self.user_id = None;
        self.user_center = 0;
        self.user_name.clear();
        self.login_name.clear();
        self.key_complete = false;
        self.login_time = 0;
        line
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_timeout_min() {
        assert_eq!(idle_timeout_ms(None), 300_000);
        assert_eq!(idle_timeout_ms(Some(100)), 300_000);
        assert_eq!(idle_timeout_ms(Some(600)), 600_000);
    }

    #[test]
    fn reset_log_tags() {
        assert!(reset_log_line(true, "U", "1.2.3.4", "n").starts_with("LOGOUT, U, 1.2.3.4, n"));
        assert!(reset_log_line(false, "U", "1.2.3.4", "n").starts_with("DISCONNECT, "));
    }

    #[test]
    fn user_target_form() {
        assert_eq!(user_target(1, "0A0B"), "1$0A0B");
    }

    #[test]
    fn state_checks() {
        let s = Session::default();
        assert_eq!(s.check(AuthCmd::KeyData).unwrap_err().code, 49);
        let s = Session {
            session: Some("x".into()),
            ..Session::default()
        };
        assert_eq!(s.state(), SessionState::LoginStarted);
        let e = s.check(AuthCmd::LoginFinish).unwrap_err();
        assert_eq!((e.code, e.line), (49, 1681));
        assert!(s.check(AuthCmd::KeyData).is_ok());
        let s = Session {
            key_complete: true,
            ..Session::default()
        };
        assert_eq!(s.check(AuthCmd::LoginFinish).unwrap_err().line, 1686);
    }

    #[test]
    fn reset_clears_and_logs() {
        let mut s = Session {
            user_id: Some("U".into()),
            login_name: "n".into(),
            login_time: 5,
            key_complete: true,
            login_failures: 2,
            ..Session::default()
        };
        assert!(s.check(AuthCmd::LogoutMyClient).is_ok());
        assert!(s.reset("a").starts_with("LOGOUT, U, a, n"));
        assert_eq!(s.state(), SessionState::Connected);
        assert_eq!(s.login_failures, 2);
        let e = s.check(AuthCmd::LogoutMyClient).unwrap_err();
        assert_eq!((e.code, e.line), (3007, 1736));
    }
}
