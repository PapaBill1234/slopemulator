//! LobbyGate requests for charge, grade and account-inventory item services with the
//! feature off. Each request gets a Fail packet, never silence and never a closed link.
//! The on-paths are not ported: they need NP charge, NP grade and the account-inventory link.
use super::*;
use crate::ids::gate::*;

/// One request shape of the account-inventory item services (`20-handlers.md` §4.1).
struct ItemService {
    /// Request format (`02-links.md`).
    format: &'static str,
    /// Fail command.
    fail: i16,
    /// Field index of the slot id (`o`). `None`: the request has no slot id.
    slot: Option<usize>,
    /// Field index of the user name (`s`). `None`: the request has no user name.
    name: Option<usize>,
    /// Field index of the label id.
    label: usize,
    /// Field index of the item instance id.
    item: usize,
}
const fn service(cmd: i16) -> ItemService {
    match cmd {
        // 82 appearance `hqobqqi`.
        ChangeCharacterAppearanceWithAccountInventoryItemReq => ItemService {
            format: "hqobqqi",
            fail: 84,
            slot: Some(2),
            name: None,
            label: 4,
            item: 5,
        },
        // 85 sex `hqocbqqi`.
        ChangeCharacterSexWithAccountInventoryItemReq => ItemService {
            format: "hqocbqqi",
            fail: 87,
            slot: Some(2),
            name: None,
            label: 5,
            item: 6,
        },
        // 88 race `hqoccbqqi`.
        ChangeCharacterRaceWithAccountInventoryItemReq => ItemService {
            format: "hqoccbqqi",
            fail: 90,
            slot: Some(2),
            name: None,
            label: 6,
            item: 7,
        },
        // 91 rename `hqosqqi`. Bug, not ported (`20-handlers.md` §4.1): the original sends
        // its failure with command 84. The port sends 93.
        RenameCharacterWithAccountInventoryItemReq => ItemService {
            format: "hqosqqi",
            fail: 93,
            slot: Some(2),
            name: None,
            label: 4,
            item: 5,
        },
        // 94 add slot `hqqqi`.
        AddSlotWithAccountInventoryItemReq => ItemService {
            format: "hqqqi",
            fail: 96,
            slot: None,
            name: None,
            label: 2,
            item: 3,
        },
        // 97 user name `hqsqqi`.
        ChangeUserNameWithAccountInventoryItemReq => ItemService {
            format: "hqsqqi",
            fail: 99,
            slot: None,
            name: Some(2),
            label: 3,
            item: 4,
        },
        // 100 jumping quota `hqqqi`.
        _ => ItemService {
            format: "hqqqi",
            fail: 102,
            slot: None,
            name: None,
            label: 2,
            item: 3,
        },
    }
}
fn is_zero(value: Option<&Value>) -> bool {
    match value {
        Some(Value::U64(v)) => *v == 0,
        Some(Value::Id(v)) => v.iter().all(|b| *b == 0),
        Some(Value::String(v)) => v.is_empty(),
        _ => true,
    }
}
impl<S: Store, N: Np> Jobs<S, N> {
    /// The session of this link for the client key, if any.
    fn link_session(&self, link: LinkId, key: u64) -> Option<&Session> {
        self.sessions.get(&key).filter(|s| s.link == link)
    }
    /// D60 GetChargeInfoReq `hq` → D61 / D62 `hqE` (build 307 handler `0x1400c3250`,
    /// `20-handlers.md` §4.1). Decode fail: no reply. No session: code 4. Config byte
    /// +400 off: code 7.
    pub(super) fn charge_info(
        &self,
        link: LinkId,
        packet: &[u8],
        key: u64,
    ) -> io::Result<Vec<Vec<u8>>> {
        if wire::fields("hq", packet).is_err() {
            return Ok(vec![]);
        }
        if self.link_session(link, key).is_none() {
            return Ok(vec![wire::failure(62, key, 4)?]);
        }
        if !self.settings.charge {
            return Ok(vec![wire::failure(62, key, 7)?]);
        }
        // Not ported: the NP charge request. Code 2 is the "job start fail" code.
        eprintln!("lobbyd: charge info on-path is not ported, reply code 2");
        Ok(vec![wire::failure(62, key, 2)?])
    }
    /// D106 ActivateGradeReq `hqQi` → D107 / D108 `hqQiE` (build 307 handler `0x1400c6af0`,
    /// `20-handlers.md` §4.1; job in `15-jobs-c.md` §3). Decode fail: no reply. Config byte
    /// +413 (`useGrade`) off: code 7, before the session lookup. No session: code 4.
    /// The Fail echoes the game grade key and the grade id.
    pub(super) fn activate_grade(
        &self,
        link: LinkId,
        packet: &[u8],
        key: u64,
    ) -> io::Result<Vec<Vec<u8>>> {
        let Ok(values) = wire::fields("hqQi", packet) else {
            return Ok(vec![]);
        };
        let reply = |code: i16| {
            wire::packet(
                "hqQiE",
                &[
                    Value::I16(108),
                    Value::U64(key),
                    values[2].clone(),
                    values[3].clone(),
                    Value::I16(code),
                ],
            )
        };
        if !self.settings.grade {
            return Ok(vec![reply(7)?]);
        }
        if self.link_session(link, key).is_none() {
            return Ok(vec![reply(4)?]);
        }
        // Not ported: the NP grade request. Code 2 is the "job start fail" code.
        eprintln!("lobbyd: activate grade on-path is not ported, reply code 2");
        Ok(vec![reply(2)?])
    }
    /// Account-inventory item services with account inventory off. Build 307 handlers:
    /// D82 `0x1400c44c0`, D85 `0x1400c4ac0`, D88 `0x1400c5100`, D91 `0x1400c5780`,
    /// D94 `0x1400c5d20`, D97 `0x1400c6070`, D100 `0x1400c6430` (`20-handlers.md` §4.1).
    /// Decode fail: no reply. No session: code 4. Then the job init:
    /// - 82, 85, 88, 91 (`17-jobs-e.md` §3): a zero slot id, label id or item instance id
    ///   gives no reply. The account-inventory channel is absent: code 7.
    /// - 94 (`19-jobs-g.md` §1): bad parameter: code 1 ❓; session not 2.0: code 5; code 7.
    /// - 97 (`19-jobs-g.md` §2): empty name or zero id: code 1 ❓; then code 7 (before 5).
    /// - 100 (`19-jobs-g.md` §5): ❓ the init order is not in the spec; the order of 97 is used.
    pub(super) fn account_item_service(
        &self,
        link: LinkId,
        packet: &[u8],
        key: u64,
        cmd: i16,
    ) -> io::Result<Vec<Vec<u8>>> {
        let shape = service(cmd);
        let Ok(values) = wire::fields(shape.format, packet) else {
            return Ok(vec![]);
        };
        let fail = |code: i16| -> io::Result<Vec<Vec<u8>>> {
            let (format, fields) = match shape.slot {
                Some(slot) => (
                    "hqoqqE",
                    vec![
                        Value::I16(shape.fail),
                        Value::U64(key),
                        values[slot].clone(),
                        values[shape.label].clone(),
                        values[shape.item].clone(),
                        Value::I16(code),
                    ],
                ),
                None => (
                    "hqqqE",
                    vec![
                        Value::I16(shape.fail),
                        Value::U64(key),
                        values[shape.label].clone(),
                        values[shape.item].clone(),
                        Value::I16(code),
                    ],
                ),
            };
            Ok(vec![wire::packet(format, &fields)?])
        };
        let Some(session) = self.link_session(link, key) else {
            return fail(4);
        };
        let zero = is_zero(values.get(shape.label))
            || is_zero(values.get(shape.item))
            || shape.slot.is_some_and(|i| is_zero(values.get(i)))
            || shape.name.is_some_and(|i| is_zero(values.get(i)));
        match cmd {
            // Zero input: the job does not start and the client gets no reply.
            ChangeCharacterAppearanceWithAccountInventoryItemReq
            | ChangeCharacterSexWithAccountInventoryItemReq
            | ChangeCharacterRaceWithAccountInventoryItemReq
            | RenameCharacterWithAccountInventoryItemReq
                if zero =>
            {
                Ok(vec![])
            }
            AddSlotWithAccountInventoryItemReq if zero => fail(1),
            AddSlotWithAccountInventoryItemReq
                if !matches!(session.state, State::Lobby | State::InitialName) =>
            {
                fail(5)
            }
            ChangeUserNameWithAccountInventoryItemReq
            | ExtendJumpingCharacterCreationQuotaWithAccountInventoryItemReq
                if zero =>
            {
                fail(1)
            }
            // Account-inventory channel absent (feature off).
            _ => fail(7),
        }
    }
}
