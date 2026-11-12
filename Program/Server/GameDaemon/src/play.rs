//! Session logic of the world entry. Pure: no socket, no database, no clock.
//! Input: one client packet or one result. Output: a list of [`Effect`] values.
//! The shard (`shard.rs`) runs the effects. Spec: Docs/Server/GameDaemon/10-session-handlers.md.
use crate::{
    db::Pc,
    entry::{bad, enter_world, prepare_zone, PC_OBJECT_ID, SPAWN_KEY},
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
}

fn u64_at(packet: &[u8], offset: usize) -> Option<u64> {
    let bytes = packet.get(offset..offset + 8)?;
    Some(u64::from_le_bytes(bytes.try_into().ok()?))
}

fn token_reply(reason: i16, out: &mut Vec<Effect>) -> io::Result<()> {
    let fields = [
        Value::I16(24),
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
    match i16::from_le_bytes([low, high]) {
        // 23 `hhqq` world, client key, lobby token → 24 `hQiQih` (10-session-handlers.md §23).
        // ❓ Failure form of 24: the reason is in the last field. Not verified.
        23 => match (u64_at(packet, 6), u64_at(packet, 14)) {
            (Some(client_key), Some(token)) => out.push(Effect::ConsumeToken { client_key, token }),
            _ => token_reply(2, out)?,
        },
        // 31 `hic` client version, character → 32 fragments, then 41.
        31 => match (&play.pc, play.pcid) {
            (Some(pc), _) => entry_packets(pc, out)?,
            (None, Some(id)) => out.push(Effect::LoadPc(id)),
            // No PC id: no reply (the client then waits).
            (None, None) => {}
        },
        929 => out.extend(rules.iter().cloned().map(Effect::Send)),
        // 42 `hiuuuh` → 43 `hq`. ❓ The spawn context is the PC object id here.
        42 => {
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
            0
        }
        Err(reason) => reason,
    };
    token_reply(reason, out)
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

    fn request(cmd: i16, body: &[u8]) -> Vec<u8> {
        let mut packet = ((body.len() + 4) as i16).to_le_bytes().to_vec();
        packet.extend(cmd.to_le_bytes());
        packet.extend(body);
        packet
    }

    fn command(effect: &Effect) -> i16 {
        match effect {
            Effect::Send(packet) => i16::from_le_bytes([packet[2], packet[3]]),
            other => panic!("not a packet: {other:?}"),
        }
    }

    #[test]
    fn entry_sequence() {
        let (mut play, mut out) = (Play::default(), Vec::new());
        // 23: world, client key 7, token 9.
        let mut body = 1i16.to_le_bytes().to_vec();
        body.extend(7u64.to_le_bytes());
        body.extend(9u64.to_le_bytes());
        packet(&mut play, &[], &request(23, &body), &mut out).unwrap();
        assert_eq!(
            out,
            [Effect::ConsumeToken {
                client_key: 7,
                token: 9
            }]
        );
        out.clear();
        entered(&mut play, Ok(5), &mut out).unwrap();
        assert_eq!(command(&out[0]), 24);
        assert_eq!(play.pcid, Some(5));
        out.clear();
        // 31 loads the PC one time.
        packet(&mut play, &[], &request(31, &[0; 5]), &mut out).unwrap();
        assert_eq!(out, [Effect::LoadPc(5)]);
        out.clear();
        loaded(&mut play, Some(pc()), &mut out).unwrap();
        assert!(out.len() >= 2);
        assert!(out[..out.len() - 1].iter().all(|e| command(e) == 32));
        assert_eq!(command(out.last().unwrap()), 41);
        out.clear();
        // 42 with the spawn key moves the session to its zone; a wrong key gives nothing.
        let mut body = SPAWN_KEY.to_le_bytes().to_vec();
        body.extend([0; 14]);
        packet(&mut play, &[], &request(42, &body), &mut out).unwrap();
        assert_eq!(out, [Effect::EnterZone(1101)]);
        out.clear();
        packet(&mut play, &[], &request(42, &[9; 18]), &mut out).unwrap();
        assert!(out.is_empty());
        joined(&mut play, &mut out).unwrap();
        assert_eq!(command(&out[0]), 43);
    }

    #[test]
    fn short_and_refused_requests() {
        let (mut play, mut out) = (Play::default(), Vec::new());
        // 23 that is too short: reason 2 at once.
        packet(&mut play, &[], &request(23, &[0; 4]), &mut out).unwrap();
        assert_eq!(command(&out[0]), 24);
        out.clear();
        entered(&mut play, Err(3), &mut out).unwrap();
        let Effect::Send(reply) = &out[0] else {
            panic!()
        };
        assert_eq!(reply[reply.len() - 2..], 3i16.to_le_bytes());
        assert_eq!(play.pcid, None);
        out.clear();
        // 31 without a PC id: no reply.
        packet(&mut play, &[], &request(31, &[0; 5]), &mut out).unwrap();
        assert!(out.is_empty());
        assert!(packet(&mut play, &[], &[1], &mut out).is_err());
    }
}
