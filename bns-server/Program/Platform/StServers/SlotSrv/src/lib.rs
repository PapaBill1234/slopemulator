//! SlotSrv, login path only. RE: `Docs/Platform/StServers/RE/03-gameaccount-slot-spawn-world.md` §2.
use sts_core::{service::target_user, Element, StsError};

/// `Slot/ListSlots` (id 4, `0x1400D6930`). The original loads the user slots
/// from SlotDb (`ListSlots`, schema `v2016041101`). SlotDb has 0 rows, so the
/// reply has no `Slot` item. The client accepts zero items
/// (`../Sts/07-client-login-wire.md`).
pub fn handle(
    command: &str,
    t: Option<&str>,
    _body: Option<&Element>,
) -> Result<Option<Element>, StsError> {
    if target_user(t).is_none() {
        // ❓ Code and module are not decoded. The text is from `0x1400C82C0`.
        return Err(StsError::new(10, 0, 0).with_text("SlotMain: invalid to header"));
    }
    match command {
        "ListSlots" => Ok(Some(Element::new("Reply"))),
        // Default of the user switch `0x1400D88A0`.
        _ => Err(StsError::new(52, 2, 462)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_slots_is_empty() {
        let t = "0$03020100-0504-0706-0809-0A0B0C0D0E0F";
        let reply = handle("ListSlots", Some(t), None).unwrap().unwrap();
        assert!(reply.children.is_empty());
        assert_eq!(handle("SetData", Some(t), None).unwrap_err().code, 52);
        assert_eq!(
            handle("ListSlots", None, None).unwrap_err().text,
            "SlotMain: invalid to header"
        );
    }
}
