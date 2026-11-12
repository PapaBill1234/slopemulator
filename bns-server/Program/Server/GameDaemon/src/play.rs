//! Session logic of the world entry. Pure: no socket, no database, no clock.
//! Input: one client packet or one result. Output: a list of [`Effect`] values.
//! The shard (`shard.rs`) runs the effects. Spec: Docs/Server/GameDaemon/10-session-handlers.md.
use crate::{
    db::Pc,
    entry::{bad, enter_world, prepare_zone, PC_OBJECT_ID, SPAWN_KEY},
    ids,
};
use bloodlib::packet::{compose, Value};
use bytes::Bytes;
use packet::zone_entry::EnterZoneOk;
use std::io;

/// State of one client session. One shard owns it at a time.
#[derive(Debug, Default)]
pub struct Play {
    /// IPv4 address of the client, in the form that LobbyGate sends to LobbyDaemon.
    pub address: u32,
    pub pcid: Option<u64>,
    pub pc: Option<Pc>,
    /// Client key of the token request. It becomes `client_key` when LobbyDaemon accepts it.
    pub pending_key: Option<u64>,
    /// Client key that LobbyDaemon accepted (`cliKey`, user +248).
    pub client_key: Option<u64>,
    /// The user left the user manager (return to lobby, step 4). A second request gives 28/1954.
    pub logged_out: bool,
}

/// What the shard must do for a session.
#[derive(Debug, PartialEq, Eq)]
pub enum Effect {
    /// Send one packet to the client.
    Send(Bytes),
    /// Ask LobbyDaemon to consume the world token. Result: [`entered`].
    ConsumeToken { client_key: u64, token: u64 },
    /// Load the PC from the game database. Result: [`loaded`].
    LoadPc(u64),
    /// Move the session to the shard of this zone. Result: [`joined`] in that shard.
    EnterZone(u32),
    /// Return to lobby, step 7: ask LobbyDaemon to take the PC out of the world and issue a
    /// lobby token (link command 8). Result: [`left`].
    LeaveWorld { pcid: u64, client_key: u64 },
}

/// Client reason of 28 for a leave result that LobbyDaemon refused with 21, 22 or 23.
const RTL_REASON_PLAY: i16 = 1954;
/// Client reason of 28 for a refusal of the other kinds.
const RTL_REASON_OTHER: i16 = 1;

fn u64_at(packet: &[u8], offset: usize) -> Option<u64> {
    let bytes = packet.get(offset..offset + 8)?;
    Some(u64::from_le_bytes(bytes.try_into().ok()?))
}

fn token_reply(reason: i16, out: &mut Vec<Effect>) -> io::Result<()> {
    let fields = [
        Value::I16(ids::G_LobbyTokenAuthOkCmd as i16),
        Value::I8(0),
        Value::I32(0),
        Value::I8(0),
        Value::I32(0),
        Value::I16(reason),
    ];
    out.push(Effect::Send(
        compose("hQiQih", &fields).map_err(bad)?.into(),
    ));
    Ok(())
}

fn entry_packets(pc: &Pc, out: &mut Vec<Effect>) -> io::Result<()> {
    for fragment in enter_world(pc)? {
        out.push(Effect::Send(fragment.into()));
    }
    out.push(Effect::Send(prepare_zone(pc)?.into()));
    Ok(())
}

/// One client packet (`packet[2..4]` is the command). `rules`: the frames of command 930.
/// An error closes the connection.
pub fn packet(
    play: &mut Play,
    rules: &[Bytes],
    packet: &[u8],
    out: &mut Vec<Effect>,
) -> io::Result<()> {
    let Some(&[low, high]) = packet.get(2..4).and_then(|c| c.first_chunk::<2>()) else {
        return Err(bad("packet without command"));
    };
    match u16::from_le_bytes([low, high]) {
        // G_LobbyTokenAuthReqCmd `hhqq` world, client key, lobby token → G_LobbyTokenAuthOkCmd
        // `hQiQih` (10-session-handlers.md §23).
        // ❓ Failure form of the reply: the reason is in the last field. Not verified.
        ids::G_LobbyTokenAuthReqCmd => match (u64_at(packet, 6), u64_at(packet, 14)) {
            (Some(client_key), Some(token)) => {
                play.pending_key = Some(client_key);
                out.push(Effect::ConsumeToken { client_key, token });
            }
            _ => token_reply(2, out)?,
        },
        // G_ReturnToLobbyReqCmd (no body) → G_ReturnToLobbyOkCmd `hq` or
        // G_ReturnToLobbyFailCmd `hh` (10-session-handlers.md §26.1, §26.3).
        // The arena check (28/5) is in `handlers::session::handle_return_to_lobby`.
        ids::G_ReturnToLobbyReqCmd => return_to_lobby(play, out)?,
        // G_EnterWorldReqCmd `hic` client version, character → G_EnterWorldOkCmd fragments,
        // then G_PrepareEnterZoneCmd.
        ids::G_EnterWorldReqCmd => match (&play.pc, play.pcid) {
            (Some(pc), _) => entry_packets(pc, out)?,
            (None, Some(id)) => out.push(Effect::LoadPc(id)),
            // No PC id: no reply (the client then waits).
            (None, None) => {}
        },
        ids::G_ContentsRuleListReqCmd => out.extend(rules.iter().cloned().map(Effect::Send)),
        // G_EnterZoneReqCmd `hiuuuh` → G_EnterZoneOkCmd `hq`. ❓ The spawn context is the PC
        // object id here.
        ids::G_EnterZoneReqCmd => {
            let key = packet
                .get(4..8)
                .and_then(|k| k.first_chunk::<4>())
                .map(|k| u32::from_le_bytes(*k));
            // A wrong key: no reply.
            if key == Some(SPAWN_KEY) {
                // Zone 0 (no PC row) is the entry shard.
                out.push(Effect::EnterZone(play.pc.as_ref().map_or(0, |pc| pc.zone)));
            }
        }
        _ => {}
    }
    Ok(())
}

/// Result of [`Effect::ConsumeToken`]: the PC id, or the reason of LobbyDaemon.
pub fn entered(play: &mut Play, result: Result<u64, i16>, out: &mut Vec<Effect>) -> io::Result<()> {
    let reason = match result {
        Ok(id) => {
            play.pcid = Some(id);
            play.client_key = play.pending_key;
            0
        }
        Err(reason) => reason,
    };
    play.pending_key = None;
    token_reply(reason, out)
}

fn rtl_fail(reason: i16, out: &mut Vec<Effect>) -> io::Result<()> {
    let fields = [
        Value::I16(ids::G_ReturnToLobbyFailCmd as i16),
        Value::I16(reason),
    ];
    out.push(Effect::Send(compose("hh", &fields).map_err(bad)?.into()));
    Ok(())
}

/// Steps 1–7 of the return to lobby transaction (§26.3).
fn return_to_lobby(play: &mut Play, out: &mut Vec<Effect>) -> io::Result<()> {
    // Step 2 of §26.1: no user or no PC id → log only, no reply.
    let (Some(pcid), Some(client_key)) = (play.pcid, play.client_key) else {
        eprintln!("gamed: return to lobby without a user, ignored");
        return Ok(());
    };
    // Step 4 of §26.1: `LogoutUser` fails when the user already left → 28/1954.
    // ❓ A second request: the spec gives no clear reply. Local choice: 1954.
    if play.logged_out {
        return rtl_fail(RTL_REASON_PLAY, out);
    }
    eprintln!("gamed: return to lobby, pc {pcid}, key {client_key}");
    play.logged_out = true;
    // TODO ❓ step 1: stop charge (`tryStopCharge`; Lobby 33), no charge system in the port.
    // TODO ❓ step 2: zone leave (`tryLeaveZone`), the zone shard does not hold PC objects yet.
    // TODO ❓ step 3: character update (`tryUpdateCharacterInfo`; Lobby 39).
    // TODO ❓ step 4: time-based reward (`AutoTakeTimeBasedReward`).
    // TODO ❓ step 5: account update (`tryUpdateAccountInfo`; Lobby 42).
    // TODO ❓ step 6: unload character (`tryUnloadCharacter`; Cached 35).
    out.push(Effect::LeaveWorld { pcid, client_key });
    Ok(())
}

/// Result of [`Effect::LeaveWorld`]: the lobby token key, or the reason of LobbyDaemon.
/// 1952 is a link failure or a timeout of GameDaemon.
pub fn left(_play: &mut Play, result: Result<u64, i16>, out: &mut Vec<Effect>) -> io::Result<()> {
    match result {
        Ok(token) => {
            let fields = [
                Value::I16(ids::G_ReturnToLobbyOkCmd as i16),
                Value::U64(token),
            ];
            out.push(Effect::Send(compose("hq", &fields).map_err(bad)?.into()));
            Ok(())
        }
        Err(21..=23) => rtl_fail(RTL_REASON_PLAY, out),
        Err(crate::link::LEAVE_LINK_FAIL) => rtl_fail(crate::link::LEAVE_LINK_FAIL, out),
        Err(_) => rtl_fail(RTL_REASON_OTHER, out),
    }
}

/// Result of [`Effect::LoadPc`]. `None`: no row or a database error. Then there is no reply.
pub fn loaded(play: &mut Play, pc: Option<Pc>, out: &mut Vec<Effect>) -> io::Result<()> {
    play.pc = pc;
    match &play.pc {
        Some(pc) => entry_packets(pc, out),
        None => Ok(()),
    }
}

/// The session is in the shard of its zone: command 43.
pub fn joined(_play: &mut Play, out: &mut Vec<Effect>) -> io::Result<()> {
    let ok = EnterZoneOk {
        context: PC_OBJECT_ID,
    };
    out.push(Effect::Send(ok.encode().map_err(bad)?.into()));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::fsd::{MakePc, APPEARANCE_BYTES};

    fn pc() -> Pc {
        Pc {
            zone: 1101,
            position: [-3199, 9241, 599],
            yaw: 45,
            level: 1,
            hp: 303,
            pcid: 5,
            pc: MakePc {
                race: 1,
                sex: 2,
                job: 3,
                appearance: [7; APPEARANCE_BYTES],
            },
            name: vec![0x41],
        }
    }

    fn request(cmd: u16, body: &[u8]) -> Vec<u8> {
        let mut packet = ((body.len() + 4) as i16).to_le_bytes().to_vec();
        packet.extend(cmd.to_le_bytes());
        packet.extend(body);
        packet
    }

    fn command(effect: &Effect) -> u16 {
        match effect {
            Effect::Send(packet) => u16::from_le_bytes([packet[2], packet[3]]),
            other => panic!("not a packet: {other:?}"),
        }
    }

    #[test]
    fn entry_sequence() {
        let (mut play, mut out) = (Play::default(), Vec::new());
        // Token auth request: world, client key 7, token 9.
        let mut body = 1i16.to_le_bytes().to_vec();
        body.extend(7u64.to_le_bytes());
        body.extend(9u64.to_le_bytes());
        packet(
            &mut play,
            &[],
            &request(ids::G_LobbyTokenAuthReqCmd, &body),
            &mut out,
        )
        .unwrap();
        assert_eq!(
            out,
            [Effect::ConsumeToken {
                client_key: 7,
                token: 9
            }]
        );
        out.clear();
        entered(&mut play, Ok(5), &mut out).unwrap();
        assert_eq!(command(&out[0]), ids::G_LobbyTokenAuthOkCmd);
        assert_eq!(play.pcid, Some(5));
        out.clear();
        // The enter world request loads the PC one time.
        let enter_world = request(ids::G_EnterWorldReqCmd, &[0; 5]);
        packet(&mut play, &[], &enter_world, &mut out).unwrap();
        assert_eq!(out, [Effect::LoadPc(5)]);
        out.clear();
        loaded(&mut play, Some(pc()), &mut out).unwrap();
        assert!(out.len() >= 2);
        assert!(out[..out.len() - 1]
            .iter()
            .all(|e| command(e) == ids::G_EnterWorldOkCmd));
        assert_eq!(command(out.last().unwrap()), ids::G_PrepareEnterZoneCmd);
        out.clear();
        // The enter zone request with the spawn key moves the session to its zone; a wrong key
        // gives nothing.
        let mut body = SPAWN_KEY.to_le_bytes().to_vec();
        body.extend([0; 14]);
        let enter_zone = request(ids::G_EnterZoneReqCmd, &body);
        packet(&mut play, &[], &enter_zone, &mut out).unwrap();
        assert_eq!(out, [Effect::EnterZone(1101)]);
        out.clear();
        let wrong_key = request(ids::G_EnterZoneReqCmd, &[9; 18]);
        packet(&mut play, &[], &wrong_key, &mut out).unwrap();
        assert!(out.is_empty());
        joined(&mut play, &mut out).unwrap();
        assert_eq!(command(&out[0]), ids::G_EnterZoneOkCmd);
    }

    #[test]
    fn short_and_refused_requests() {
        let (mut play, mut out) = (Play::default(), Vec::new());
        // A token auth request that is too short: reason 2 at once.
        let short = request(ids::G_LobbyTokenAuthReqCmd, &[0; 4]);
        packet(&mut play, &[], &short, &mut out).unwrap();
        assert_eq!(command(&out[0]), ids::G_LobbyTokenAuthOkCmd);
        out.clear();
        entered(&mut play, Err(3), &mut out).unwrap();
        let Effect::Send(reply) = &out[0] else {
            panic!()
        };
        assert_eq!(reply[reply.len() - 2..], 3i16.to_le_bytes());
        assert_eq!(play.pcid, None);
        out.clear();
        // An enter world request without a PC id: no reply.
        let enter_world = request(ids::G_EnterWorldReqCmd, &[0; 5]);
        packet(&mut play, &[], &enter_world, &mut out).unwrap();
        assert!(out.is_empty());
        assert!(packet(&mut play, &[], &[1], &mut out).is_err());
    }
}
