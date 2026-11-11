//! Fixed replies for `Access` and `VirtualCurrency`. Not a port: AccessSrv and
//! VirtualCurrencySrv are not analysed. See `README.md`.
use sts_core::{Element, StsError};

pub const PROTOCOLS: [&str; 2] = ["Access", "VirtualCurrency"];

pub fn handle(protocol: &str, command: &str) -> Result<Option<Element>, StsError> {
    match (protocol, command) {
        // CliGate needs `<AccessMask>` (11 §10.2). ❓ The real mask values are not known.
        ("Access", "GetAppUserRights") => Ok(Some(
            Element::new("Reply").child(Element::text("AccessMask", "0")),
        )),
        // The client reads `BalanceInfoListCount` (`../Sts/07-client-login-wire.md`).
        ("VirtualCurrency", "GetBalance") => Ok(Some(
            Element::new("Reply").child(Element::int("BalanceInfoListCount", 0)),
        )),
        _ => Err(StsError::new(52, 2, 462)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replies() {
        let r = handle("Access", "GetAppUserRights").unwrap().unwrap();
        assert_eq!(r.get_text("AccessMask"), Some("0"));
        let r = handle("VirtualCurrency", "GetBalance").unwrap().unwrap();
        assert_eq!(r.get_text("BalanceInfoListCount"), Some("0"));
        assert!(handle("Access", "Other").is_err());
    }
}
