//! SlotSrv, login path only. RE: `Docs/Platform/StServers/RE/03-gameaccount-slot-spawn-world.md` §2.
use sts_core::{guid, service::target_user, Element, StsError};

/// Slot type of a character slot (client string at `Client.exe` 0x15D9888).
pub const CHAR: &str = "char";
/// ❓ Number of character slots of a new user. The original takes it from the
/// tag policy of SlotDb (`CreateSlots`). Local choice.
pub const CHAR_SLOTS: u8 = 4;

/// Slot ids of a user. ❓ The original ids are new GUIDs in SlotDb. Here: the
/// UserId with the last byte changed, so the ids are stable without a DB.
pub fn char_slot_ids(user: &[u8; 16]) -> Vec<[u8; 16]> {
    (1..=CHAR_SLOTS)
        .map(|n| {
            let mut id = *user;
            id[15] = id[15].wrapping_add(n);
            id
        })
        .collect()
}

/// `Slot/ListSlots` (id 4, `0x1400D6930`). The game client needs one slot or
/// more of type `char`: with zero slots `NPSession::ReceiveCharacterListSuccess`
/// (0xC1DF90) returns before it tells the lobby session, and the client does
/// not ask LobbyGate for the character list. Types `customize` and `activity`
/// have no data here.
pub fn handle(
    command: &str,
    t: Option<&str>,
    body: Option<&Element>,
) -> Result<Option<Element>, StsError> {
    let Some(user) = target_user(t).and_then(|u| guid::parse(&u)) else {
        // ❓ Code and module are not decoded. The text is from `0x1400C82C0`.
        return Err(StsError::new(10, 0, 0).with_text("SlotMain: invalid to header"));
    };
    match command {
        "ListSlots" => {
            let wanted = body.and_then(|b| b.get_text("SlotType")).unwrap_or("");
            let mut reply = Element::new("Reply");
            if wanted.is_empty() || wanted == CHAR {
                for id in char_slot_ids(&user) {
                    reply.push(
                        Element::new("Slot")
                            .child(Element::text("SlotId", guid::format(&id)))
                            .child(Element::text("SlotType", CHAR))
                            .child(Element::int("SystemSlot", 0)),
                    );
                }
            }
            Ok(Some(reply))
        }
        // Default of the user switch `0x1400D88A0`.
        _ => Err(StsError::new(52, 2, 462)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_slots() {
        let t = "0$03020100-0504-0706-0809-0A0B0C0D0E0F";
        let of = |ty: Option<&str>| {
            let mut body = Element::new("Request");
            if let Some(ty) = ty {
                body.push(Element::text("SlotType", ty));
            }
            handle("ListSlots", Some(t), Some(&body)).unwrap().unwrap()
        };
        for reply in [of(None), of(Some("char"))] {
            let slots: Vec<_> = reply.all("Slot").collect();
            assert_eq!(slots.len(), 4);
            assert_eq!(slots[0].get_text("SlotType"), Some("char"));
            assert_eq!(
                slots[0].get_text("SlotId"),
                Some("03020100-0504-0706-0809-0A0B0C0D0E10")
            );
        }
        assert_eq!(of(Some("customize")).all("Slot").count(), 0);
        assert_eq!(handle("SetData", Some(t), None).unwrap_err().code, 52);
        assert_eq!(
            handle("ListSlots", None, None).unwrap_err().text,
            "SlotMain: invalid to header"
        );
    }
}
