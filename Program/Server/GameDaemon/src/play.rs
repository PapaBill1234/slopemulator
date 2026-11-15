//! Session logic of the world entry. Pure: no socket, no database, no clock. The shard sets
//! the time fields of [`Play`] before each step.
//! Input: one client packet or one result. Output: a list of [`Effect`] values.
//! The shard (`shard.rs`) runs the effects. Spec: Docs/Server/GameDaemon/05, 06, 10.
use crate::{
    db::Pc,
    entry::{
        bad, enter_world_at, enter_world_fail, enter_zone_fail, enter_zone_ok, ping, prepare_zone,
        rule_list_fail, token_fail, token_ok, ENTER_WORLD_TWICE, ENTER_ZONE_BAD_KEY,
        RULE_LIST_TOO_SOON, SPAWN_STATE_WAIT, TOKEN_BAD_REQUEST,
    },
    ids,
};
use bloodlib::packet::{compose, Value};
use bytes::Bytes;
use std::io;

/// One list per 30 s per session (05 "Request handler").
pub const RULE_LIST_PERIOD_MS: u64 = 30_000;
/// State of a spawn action that took its command 42 (the action is then over).
const SPAWN_STATE_DONE: u8 = 3;
/// Permitted distance between the PC and the summoned object of command 42, in position units.
/// ❓ Local value: the spec gives no range ("units and valid ranges open").
pub const SUMMON_RANGE: i64 = 500;

/// Spawn action of the PC (`PcSpawnAction`): the key of command 41 and its state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spawn {
    pub key: u32,
    pub state: u8,
}

/// Options of the server ping (06 "Ping interval and disconnect checks").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PingConfig {
    /// `enable-server-ping`. Default: off.
    pub enabled: bool,
    /// `server-ping-send-sec`. Default: 60.
    pub send_sec: u32,
    /// `server-ping-disconnect-count`. Default: 3.
    pub disconnect_count: u32,
    /// `server-ping-disconnect-no-packet-enabled`. ❓ The close action is not traced.
    pub close_no_packet: bool,
    /// `server-ping-disconnect-invalid-packet-enabled`. ❓ The close action is not traced.
    pub close_invalid_packet: bool,
}

impl Default for PingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            send_sec: 60,
            disconnect_count: 3,
            close_no_packet: false,
            close_invalid_packet: false,
        }
    }
}

impl PingConfig {
    /// `max(1000, 1000 × server-ping-send-sec)` in milliseconds.
    pub fn interval_ms(&self) -> u64 {
        (u64::from(self.send_sec) * 1000).max(1000)
    }
}

/// Ping state of one session. `sent` and `received` are (sequence, timestamp) pairs.
#[derive(Debug, Default)]
pub struct PingState {
    sent: Option<(u64, u64)>,
    received: (u64, u64),
    due_ms: u64,
}

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
    /// Milliseconds since the server start. The shard sets it before each step.
    pub now_ms: u64,
    /// Current UTC time in seconds. The shard sets it before each step.
    pub utc_secs: u64,
    /// World clock hour 0..23. The shard sets it before each step.
    pub hour: u8,
    /// Spawn key that the shard reserved from the global counter (0: none). The counter skips 0.
    pub spawn_next: u32,
    /// Time of the last 930 that this session got (`None`: never).
    pub rules_sent_ms: Option<u64>,
    /// Command 31 was accepted. A second one gives 33.
    pub entered_world: bool,
    /// The spawn action of the PC.
    pub spawn: Option<Spawn>,
    /// PC transition counter (+3480).
    pub transition: u16,
    /// Position and yaw of the summoned object after the clamp of command 42.
    pub summon: [i16; 3],
    pub summon_yaw: u16,
    pub ping: PingState,
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

fn i16_at(packet: &[u8], offset: usize) -> Option<i16> {
    let bytes = packet.get(offset..offset + 2)?;
    Some(i16::from_le_bytes(bytes.try_into().ok()?))
}

fn send(out: &mut Vec<Effect>, packet: Vec<u8>) {
    out.push(Effect::Send(packet.into()));
}

/// Commands 32, 87, 90, 423, 1122, 1126, 836, then 41 (05 "Ordered path").
fn entry_packets(play: &mut Play, out: &mut Vec<Effect>) -> io::Result<()> {
    let Some(pc) = play.pc.as_ref() else {
        return Ok(());
    };
    let key = if play.spawn_next == 0 {
        1
    } else {
        play.spawn_next
    };
    let mut packets = enter_world_at(pc, play.utc_secs)?;
    packets.push(prepare_zone(pc, key, play.hour, play.transition)?);
    let (position, yaw) = (pc.position, pc.yaw);
    play.spawn_next = 0;
    play.spawn = Some(Spawn {
        key,
        state: SPAWN_STATE_WAIT,
    });
    play.transition = play.transition.wrapping_add(1);
    play.summon = position;
    play.summon_yaw = yaw;
    out.extend(packets.into_iter().map(|p| Effect::Send(p.into())));
    Ok(())
}

/// Clamp the summoned-object position of command 42 to the permitted distance from the PC.
/// A client value that is too far gives the position of the server. The client value never moves
/// the PC. Spec: 05 "Loading and command 42", "bug, not ported".
pub fn clamp_summon(own: [i16; 3], client: [i16; 3]) -> [i16; 3] {
    let squared: i64 = (0..3)
        .map(|i| {
            let d = i64::from(own[i]) - i64::from(client[i]);
            d * d
        })
        .sum();
    if squared > SUMMON_RANGE * SUMMON_RANGE {
        own
    } else {
        client
    }
}

/// Command 42 `hiuuuh`: key, summoned x/y/z and yaw. ❓ Units of the values are open.
fn enter_zone(play: &mut Play, packet: &[u8], out: &mut Vec<Effect>) -> io::Result<()> {
    let key = packet
        .get(4..8)
        .and_then(|k| k.first_chunk::<4>())
        .map(|k| u32::from_le_bytes(*k));
    let allowed = match (play.spawn, key) {
        (Some(spawn), Some(key)) => spawn.state == SPAWN_STATE_WAIT && spawn.key == key,
        _ => false,
    };
    let Some(pc) = play.pc.as_ref().filter(|_| allowed) else {
        send(out, enter_zone_fail(ENTER_ZONE_BAD_KEY)?);
        return Ok(());
    };
    // The client value of the summoned object. A missing value is the server position.
    let client = [
        i16_at(packet, 8),
        i16_at(packet, 10),
        i16_at(packet, 12),
        i16_at(packet, 14),
    ];
    let (own, own_yaw, zone) = (pc.position, pc.yaw, pc.zone);
    if let [Some(x), Some(y), Some(z), Some(yaw)] = client {
        let clamped = clamp_summon(own, [x, y, z]);
        play.summon = clamped;
        // The yaw follows the position: a refused position keeps the yaw of the server.
        play.summon_yaw = if clamped == [x, y, z] {
            yaw as u16
        } else {
            own_yaw
        };
    }
    if let Some(spawn) = play.spawn.as_mut() {
        // One command 42 for each spawn action.
        spawn.state = SPAWN_STATE_DONE;
    }
    out.push(Effect::EnterZone(zone));
    Ok(())
}

/// Command 929: 930 or 931 (05 "Request handler").
fn rule_list_request(play: &mut Play, rules: &[Bytes], out: &mut Vec<Effect>) -> io::Result<()> {
    let now = play.now_ms;
    let due = play.rules_sent_ms.is_none_or(|last| {
        now <= RULE_LIST_PERIOD_MS || now.saturating_sub(last) >= RULE_LIST_PERIOD_MS
    });
    if due {
        play.rules_sent_ms = Some(now);
        out.extend(rules.iter().cloned().map(Effect::Send));
    } else {
        send(out, rule_list_fail(RULE_LIST_TOO_SOON)?);
    }
    Ok(())
}

/// Command 18 `hqq`: the echo of command 17. An echo that is short, or that is newer than the
/// last ping that the server sent, is not valid and is ignored.
fn ping_echo(play: &mut Play, packet: &[u8]) {
    let (Some(sequence), Some(timestamp)) = (u64_at(packet, 4), u64_at(packet, 12)) else {
        return;
    };
    let Some((last, _)) = play.ping.sent else {
        return;
    };
    if sequence <= last && sequence >= play.ping.received.0 {
        play.ping.received = (sequence, timestamp);
    }
}

/// Server ping step (06 "Ping interval and disconnect checks"). The shard calls it on its tick
/// for each session. An `Err` closes the session.
pub fn ping_tick(play: &mut Play, config: &PingConfig, out: &mut Vec<Effect>) -> io::Result<()> {
    if !config.enabled || play.now_ms < play.ping.due_ms {
        return Ok(());
    }
    play.ping.due_ms = play.now_ms + config.interval_ms();
    let received = play.ping.received;
    let mut next = 1;
    if let Some(sent) = play.ping.sent.filter(|sent| *sent != received) {
        let d_seq = i128::from(sent.0) - i128::from(received.0);
        let d_time = i128::from(sent.1) - i128::from(received.1);
        if d_seq > i128::from(config.disconnect_count) {
            // Category `NoPacket`. ❓ The close action is not traced: close only when the
            // option is on; else send no new ping.
            return if config.close_no_packet {
                Err(bad("no ping echo"))
            } else {
                Ok(())
            };
        }
        if d_seq < 0 || d_time < 0 {
            // Category `InvalidPacket`.
            return if config.close_invalid_packet {
                Err(bad("invalid ping echo"))
            } else {
                Ok(())
            };
        }
    }
    if let Some((sequence, _)) = play.ping.sent {
        next = sequence + 1;
    }
    let timestamp = play.now_ms;
    play.ping.sent = Some((next, timestamp));
    send(out, ping(next, timestamp)?);
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
        // `hQiQih` or G_LobbyTokenAuthFailCmd `hh` (05 "Command 24").
        ids::G_LobbyTokenAuthReqCmd => match (u64_at(packet, 6), u64_at(packet, 14)) {
            (Some(client_key), Some(token)) => {
                play.pending_key = Some(client_key);
                out.push(Effect::ConsumeToken { client_key, token });
            }
            _ => send(out, token_fail(TOKEN_BAD_REQUEST)?),
        },
        // G_ReturnToLobbyReqCmd (no body) → G_ReturnToLobbyOkCmd `hq` or
        // G_ReturnToLobbyFailCmd `hh` (10-session-handlers.md §26.1, §26.3).
        // The arena check (28/5) is in `handlers::session::handle_return_to_lobby`.
        ids::G_ReturnToLobbyReqCmd => return_to_lobby(play, out)?,
        // G_EnterWorldReqCmd `hic` client version, character → G_EnterWorldOkCmd fragments and
        // the packets after it, then G_PrepareEnterZoneCmd. A second request gives 33.
        ids::G_EnterWorldReqCmd => match (&play.pc, play.pcid) {
            _ if play.entered_world => send(out, enter_world_fail(ENTER_WORLD_TWICE)?),
            (Some(_), _) => {
                play.entered_world = true;
                entry_packets(play, out)?;
            }
            (None, Some(id)) => {
                play.entered_world = true;
                out.push(Effect::LoadPc(id));
            }
            // No PC id: no reply (the client then waits).
            (None, None) => {}
        },
        ids::G_ContentsRuleListReqCmd => rule_list_request(play, rules, out)?,
        // G_EnterZoneReqCmd `hiuuuh` → G_EnterZoneOkCmd `hq` or G_EnterZoneFailCmd.
        ids::G_EnterZoneReqCmd => enter_zone(play, packet, out)?,
        ids::G_ServerPingReqCmd => ping_echo(play, packet),
        _ => {}
    }
    Ok(())
}

/// Result of [`Effect::ConsumeToken`]: the PC id, or the reason of LobbyDaemon.
pub fn entered(play: &mut Play, result: Result<u64, i16>, out: &mut Vec<Effect>) -> io::Result<()> {
    let packet = match result {
        Ok(id) => {
            play.pcid = Some(id);
            play.client_key = play.pending_key;
            token_ok(0)
        }
        Err(reason) => token_fail(reason)?,
    };
    play.pending_key = None;
    send(out, packet);
    Ok(())
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

/// Result of [`Effect::LoadPc`]. `None`: no row or a database error. Then there is no reply and
/// the client can send command 31 again.
pub fn loaded(play: &mut Play, pc: Option<Pc>, out: &mut Vec<Effect>) -> io::Result<()> {
    play.pc = pc;
    if play.pc.is_none() {
        play.entered_world = false;
    }
    entry_packets(play, out)
}

/// The session is in the shard of its zone: command 43 with q = 0 (05 "Command 43").
pub fn joined(_play: &mut Play, out: &mut Vec<Effect>) -> io::Result<()> {
    send(out, enter_zone_ok(0));
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

    fn enter_zone_request(key: u32, x: i16) -> Vec<u8> {
        let mut body = key.to_le_bytes().to_vec();
        body.extend(x.to_le_bytes());
        body.extend([0; 6]);
        request(ids::G_EnterZoneReqCmd, &body)
    }

    #[test]
    fn entry_sequence() {
        let (mut play, mut out) = (Play::default(), Vec::new());
        // Token auth request: world, client key 7, token 9.
        let mut body = 1i16.to_le_bytes().to_vec();
        body.extend(7u64.to_le_bytes());
        body.extend(9u64.to_le_bytes());
        let token = request(ids::G_LobbyTokenAuthReqCmd, &body);
        packet(&mut play, &[], &token, &mut out).unwrap();
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
        play.spawn_next = 9;
        packet(&mut play, &[], &enter_world, &mut out).unwrap();
        assert_eq!(out, [Effect::LoadPc(5)]);
        out.clear();
        loaded(&mut play, Some(pc()), &mut out).unwrap();
        let commands: Vec<u16> = out.iter().map(command).collect();
        assert_eq!(commands.last(), Some(&ids::G_PrepareEnterZoneCmd));
        assert_eq!(
            play.spawn,
            Some(Spawn {
                key: 9,
                state: SPAWN_STATE_WAIT
            })
        );
        out.clear();
        // A second 31 gives 33.
        packet(&mut play, &[], &enter_world, &mut out).unwrap();
        assert_eq!(command(&out[0]), ids::G_EnterWorldFailCmd);
        out.clear();
        // A wrong key gives 44 and no move.
        packet(&mut play, &[], &enter_zone_request(8, 0), &mut out).unwrap();
        assert_eq!(command(&out[0]), ids::G_EnterZoneFailCmd);
        out.clear();
        // The right key moves the session to its zone, one time.
        packet(&mut play, &[], &enter_zone_request(9, 0), &mut out).unwrap();
        assert_eq!(out, [Effect::EnterZone(1101)]);
        out.clear();
        packet(&mut play, &[], &enter_zone_request(9, 0), &mut out).unwrap();
        assert_eq!(command(&out[0]), ids::G_EnterZoneFailCmd);
        out.clear();
        joined(&mut play, &mut out).unwrap();
        assert_eq!(command(&out[0]), ids::G_EnterZoneOkCmd);
    }

    #[test]
    fn summon_is_clamped() {
        let own = [-3199, 9241, 599];
        assert_eq!(clamp_summon(own, [-3100, 9241, 599]), [-3100, 9241, 599]);
        assert_eq!(clamp_summon(own, [i16::MAX, i16::MIN, 0]), own);
    }

    #[test]
    fn short_and_refused_requests() {
        let (mut play, mut out) = (Play::default(), Vec::new());
        // A token auth request that is too short: 25 at once.
        let short = request(ids::G_LobbyTokenAuthReqCmd, &[0; 4]);
        packet(&mut play, &[], &short, &mut out).unwrap();
        assert_eq!(command(&out[0]), ids::G_LobbyTokenAuthFailCmd);
        out.clear();
        entered(&mut play, Err(3), &mut out).unwrap();
        let Effect::Send(reply) = &out[0] else {
            panic!()
        };
        assert_eq!(command(&out[0]), ids::G_LobbyTokenAuthFailCmd);
        assert_eq!(reply[reply.len() - 2..], 3i16.to_le_bytes());
        assert_eq!(play.pcid, None);
        out.clear();
        // An enter world request without a PC id: no reply.
        let enter_world = request(ids::G_EnterWorldReqCmd, &[0; 5]);
        packet(&mut play, &[], &enter_world, &mut out).unwrap();
        assert!(out.is_empty());
        assert!(packet(&mut play, &[], &[1], &mut out).is_err());
        // An enter zone request without a spawn action gives 44.
        packet(&mut play, &[], &enter_zone_request(1, 0), &mut out).unwrap();
        assert_eq!(command(&out[0]), ids::G_EnterZoneFailCmd);
    }

    #[test]
    fn ping_follows_the_table() {
        let config = PingConfig {
            enabled: true,
            send_sec: 1,
            disconnect_count: 1,
            close_no_packet: true,
            close_invalid_packet: false,
        };
        let (mut play, mut out) = (Play::default(), Vec::new());
        ping_tick(&mut play, &config, &mut out).unwrap();
        assert_eq!(command(&out[0]), ids::G_ServerPingNotifyCmd);
        // Before the interval ends: nothing.
        play.now_ms = 999;
        ping_tick(&mut play, &config, &mut out).unwrap();
        assert_eq!(out.len(), 1);
        // The echo makes the pairs equal: the next ping goes out.
        let mut body = 1u64.to_le_bytes().to_vec();
        body.extend(0u64.to_le_bytes());
        packet(
            &mut play,
            &[],
            &request(ids::G_ServerPingReqCmd, &body),
            &mut out,
        )
        .unwrap();
        play.now_ms = 1000;
        ping_tick(&mut play, &config, &mut out).unwrap();
        assert_eq!(out.len(), 2);
        // No echo: pending 1 is inside the count, then 2 is over it.
        play.now_ms = 2000;
        ping_tick(&mut play, &config, &mut out).unwrap();
        assert_eq!(out.len(), 3);
        play.now_ms = 3000;
        assert!(ping_tick(&mut play, &config, &mut out).is_err());
    }
}
