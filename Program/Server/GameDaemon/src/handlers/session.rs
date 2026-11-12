use super::*;

/// parity: 16 10-session-handlers.md#16--g_pingreqcmd--done ported
pub fn handle_ping(_: &mut Context<'_>, _: &[u8]) -> Outcome {
    Outcome::log("ping")
}
/// parity: 19 10-session-handlers.md#19--g_gamekeyauthreqcmd--done ported
pub fn handle_game_key_auth(_: &mut Context<'_>, _: &[u8]) -> Outcome {
    Outcome::default()
}
/// parity: 22 10-session-handlers.md#22--g_nptokenauthreqcmd--done ported
pub fn handle_np_token_auth(_: &mut Context<'_>, _: &[u8]) -> Outcome {
    Outcome::default()
}
/// parity: 34 10-session-handlers.md#34--g_querywaitingnumberreqcmd--done ported
pub fn handle_query_waiting_number(_: &mut Context<'_>, _: &[u8]) -> Outcome {
    Outcome::default()
}
/// parity: 38 10-session-handlers.md#38--g_leaveworldreqcmd--done ported
pub fn handle_leave_world(_: &mut Context<'_>, _: &[u8]) -> Outcome {
    Outcome::default()
}
/// parity: 307 10-session-handlers.md#307--g_clientconfigreqcmd--done ported
pub fn handle_client_config(_: &mut Context<'_>, _: &[u8]) -> Outcome {
    Outcome::default()
}

/// parity: 414 10-session-handlers.md#414--g_checkroundtriptimereqcmd--done ported
pub fn handle_check_round_trip_time(_: &mut Context<'_>, req: &[u8]) -> Outcome {
    let mut r = Reader(req);
    match (r.quad(), r.quad()) {
        (Some(a), Some(b)) => Outcome::reply(415, [a.to_le_bytes(), b.to_le_bytes()].concat()),
        _ => Outcome::log("RTT decode failure"),
    }
}
/// parity: 416 10-session-handlers.md#416--g_writelatencylogreqcmd--done ported
pub fn handle_write_latency_log(_: &mut Context<'_>, req: &[u8]) -> Outcome {
    let mut r = Reader(req);
    let Some(kind) = r.byte() else {
        return Outcome::log("latency decode failure");
    };
    if (0..5).any(|_| r.int().is_none()) {
        return Outcome::log("latency decode failure");
    }
    // Never retain client-address bits. No identity or address enters this plan.
    Outcome::log(match kind {
        0 => "skill latency (address redacted)",
        1 => "skill latency overflow (address redacted)",
        2 => "RTT latency (address redacted)",
        3 => "RTT latency overflow (address redacted)",
        _ => "unknown latency type (address redacted)",
    })
}
/// parity: 422 10-session-handlers.md#422--g_currentutctimereqcmd--done ported
pub fn handle_current_utc_time(ctx: &mut Context<'_>, _: &[u8]) -> Outcome {
    Outcome::reply(423, ctx.world.utc_seconds.to_le_bytes().to_vec())
}
/// parity: 812 10-session-handlers.md#812--g_upgradejumpingcharacterstatewithcreationquotareqcmd--done ported
pub fn handle_upgrade_jumping_character_state_with_creation_quota(
    _: &mut Context<'_>,
    _: &[u8],
) -> Outcome {
    Outcome::reason(814, 1974)
}

/// parity: 11 10-session-handlers.md#11--g_helloreqcmd--partial partial
pub fn handle_hello(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let mut r = Reader(req);
    let decoded = (|| {
        Some((
            r.short()?,
            r.quad()?,
            r.string(40)?,
            r.quad()?,
            r.blob(4000)?,
        ))
    })();
    let Some((protocol, property, publisher, extra, blob)) = decoded else {
        return native_false();
    };
    let publisher_end = publisher
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(publisher.len());
    let expected = &ctx.world.publisher;
    let expected_end = expected
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(expected.len());
    if protocol != 1278
        || property != 0x12D261D120403
        || publisher[..publisher_end] != expected[..expected_end]
    {
        return native_false();
    }
    match ctx.world.publisher_code {
        1 | 9 | 11..=15 => {}
        2..=8 | 10 if extra == 0x40000000000 => {}
        _ => return native_false(),
    }
    if blob.is_empty() {
        return native_false();
    }
    let reply = match ctx.services.hello(&blob) {
        HelloOperation::Rejected => return native_false(),
        HelloOperation::Unknown => {
            return Outcome::unsupported(
                "TODO ❓ Hello operation / framing acceptance, 0x1406A65D0",
            )
        }
        HelloOperation::Reply(reply) => reply,
    };
    // Size, command and blob count consume six of the native 4000 bytes.
    if reply.len() > 3994 {
        return Outcome::unsupported("TODO ❓ invalid opaque Hello output capacity");
    }
    let mut body = (reply.len() as u16).to_le_bytes().to_vec();
    body.extend(reply);
    let mut out = Outcome::default();
    out.session_effects
        .push(SessionEffect::SendWithCurrentProcessor(Message {
            command: 12,
            body,
        }));
    match ctx.session.hello_mode {
        Some(3) => {
            ctx.session.hello_mode = Some(5);
            out.session_effects
                .push(SessionEffect::ActivateSessionProcessor);
        }
        Some(_) => {}
        None => out.status = Status::Unsupported("TODO ❓ missing Hello session mode"),
    }
    out
}
fn native_false() -> Outcome {
    Outcome {
        status: Status::NativeFalse,
        ..Outcome::default()
    }
}

/// parity: 13 10-session-handlers.md#13--g_mobilehelloreqcmd--done ported
pub fn handle_mobile_hello(_: &mut Context<'_>, req: &[u8]) -> Outcome {
    let mut r = Reader(req);
    let (Some(protocol), Some(_publisher)) = (r.short(), r.string(40)) else {
        return Outcome::reason(15, 2);
    };
    if protocol != 1278 {
        return Outcome::reason(15, 63);
    }
    let Some(count) = r.short().filter(|v| *v <= i16::MAX as u16) else {
        return Outcome::reason(15, 2);
    };
    for _ in 0..count {
        let (Some(id), Some(version)) = (r.short(), r.short()) else {
            return Outcome::reason(15, 2);
        };
        let expected = match id {
            2 => 301,
            3 => 38,
            4 => 29,
            5 => 18,
            7 | 8 | 27 => 9,
            9 | 14 | 16 | 17 => 4,
            10 | 20 => 3,
            11 | 15 => 20,
            12 | 13 => 2,
            21..=25 => 11,
            26 => 7,
            34 | 35 => 1,
            _ => return Outcome::reason(15, 63),
        };
        if version != expected {
            return Outcome::reason(15, 63);
        }
    }
    Outcome::reply(14, vec![])
}

/// parity: 18 10-session-handlers.md#18--g_serverpingreqcmd--partial ported
pub fn handle_server_ping(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let mut r = Reader(req);
    let (Some(a), Some(b)) = (r.quad(), r.quad()) else {
        let mut out = native_false();
        out.logs.push("ServerPing decode failure");
        return out;
    };
    ctx.session.pc.as_mut().expect("PC dispatch").received_ping = [a, b];
    // STOP 0x1403710A5 -> 0x14036DE10. No restricted call.
    Outcome {
        changes: vec![StateChange::PingReceived([a, b])],
        logs: vec!["server ping (metadata redacted)"],
        ..Outcome::default()
    }
}

/// parity: 23 10-session-handlers.md#23--g_lobbytokenauthreqcmd--partial partial
pub fn handle_lobby_token_auth(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    if ctx.world.arena {
        return Outcome::reason(25, 5);
    }
    let mut r = Reader(req);
    let (Some(world), Some(key), Some(token)) = (r.short(), r.quad(), r.quad()) else {
        // TODO ❓ Native decoder return is unchecked. Do not invent a reason.
        return Outcome::unsupported("TODO ❓ unchecked token-auth decode");
    };
    if (world as i16) < 1 || !ctx.world.worlds.contains(&world) {
        return Outcome::reason(25, 1937);
    }
    if key == 0 {
        return Outcome::reason(25, 1939);
    }
    let mut out = ctx.queue(
        23,
        25,
        2,
        "TODO ❓ token-auth initialization and later recovery/charge/DB callbacks",
    );
    if out.changes.contains(&StateChange::Queued(23)) {
        out.changes
            .push(StateChange::LobbyTokenAuthQueued(TokenAuthRequest {
                world,
                client_key: key,
                lobby_token: token,
            }));
    }
    out
}

/// parity: 26 10-session-handlers.md#26--g_returntolobbyreqcmd ported
///
/// The gates (arena 28/5, logout 28/1954) run here. Steps 1–7 of the transaction run in
/// `play::packet` (`Effect::LeaveWorld`, link command 8) in the live path.
pub fn handle_return_to_lobby(ctx: &mut Context<'_>, _: &[u8]) -> Outcome {
    if ctx.world.arena {
        return Outcome::reason(28, 5);
    }
    // TODO ❓ Logout helper can mutate manager state before registration.
    match ctx.services.logout() {
        Gate::Fail => Outcome::reason(28, 1954),
        Gate::Unknown => Outcome::unsupported("TODO ❓ LogoutUser, 0x1403F0340"),
        // TODO ❓ steps 1–6 (stop charge, zone leave, character update, time reward,
        // account update, unload): stubs, see `play::return_to_lobby`.
        // TODO ❓ step 7: a `DaemonRequest::LobbyLeaveWorld` needs `handlers/mod.rs`, which
        // is outside the files of slice lp2-3-rust. The queue marker stays.
        Gate::Pass => ctx.queue(26, 28, 2, "step 7 runs as play::Effect::LeaveWorld"),
    }
}

/// parity: 29 10-session-handlers.md#29--g_cleanuserconnectioninfoincligatereqcmd--done ported
pub fn handle_clean_user_connection_info_in_cligate(ctx: &mut Context<'_>, _: &[u8]) -> Outcome {
    if ctx.world.arena {
        return Outcome::log("arena clean connection ignored");
    }
    Outcome {
        daemon_requests: vec![DaemonRequest::LobbyClean {
            world: ctx.pc().world_id,
            db_id: ctx.pc().db_id,
        }],
        ..Outcome::default()
    }
}

/// parity: 31 10-session-handlers.md#31--g_enterworldreqcmd--partial partial
pub fn handle_enter_world(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    if ctx.world.arena || arena_blocked(ctx.pc()) {
        return Outcome::reason(33, 5);
    }
    let mut r = Reader(req);
    let (Some(version), Some(_character)) = (r.int(), r.byte()) else {
        return Outcome::reason(33, 2);
    };
    if ctx.pc().received_enter_world {
        return Outcome::reason(33, 2);
    }
    let pc = ctx.session.pc.as_mut().expect("PC dispatch");
    pc.received_enter_world = true;
    pc.client_version = version;
    // STOP optional 0x140371AC8. Ordinary composite precedes destination checks.
    let mut out = Outcome::unsupported("TODO ❓ server-owned initial PC snapshot for command 32");
    out.changes.push(StateChange::EnterWorldReceived {
        client_version: version,
    });
    let Some(snapshot) = ctx.services.initial_world(pc) else {
        return out;
    };
    // Adapter consistency checks, not new native failure reasons. Never send
    // another PC's snapshot or convert an adapter error into a native 33 reply.
    if snapshot.header.object_id != pc.object_id || snapshot.pc_db_id != pc.db_id {
        out.status = Status::Unsupported("TODO ❓ initial PC snapshot identity mismatch");
        return out;
    }
    let fragments = match snapshot.encode_fragments() {
        Ok(fragments) => fragments,
        Err(_) => {
            out.status = Status::Unsupported("TODO ❓ invalid initial PC snapshot encoding");
            return out;
        }
    };
    out.session_effects.extend(
        fragments
            .into_iter()
            .map(SessionEffect::SendFragmentWithCurrentProcessor),
    );
    // The native builder continues with summon/quest/other sync before travel.
    // A complete 32 alone must not approve or queue a destination or spawn.
    out.status = Status::Unsupported("TODO ❓ post-32 sync, destination and World::Enter");
    out
}
pub(super) fn arena_blocked(pc: &Pc) -> bool {
    matches!(pc.arena_state, 2 | 3 | 5 | 6)
}

/// parity: 42 10-session-handlers.md#42--g_enterzonereqcmd--partial partial
pub fn handle_enter_zone(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    if req.len() < 12 {
        return Outcome::unsupported("TODO ❓ unchecked EnterZone decode");
    }
    if ctx.pc().zone.is_none() {
        return Outcome::reason(44, 2);
    }
    if !ctx.pc().user_available {
        return Outcome::log("missing zone-entry user");
    }
    let mut out = ctx.queue(
        42,
        44,
        2,
        "TODO ❓ EnterZoneTask continuation / appearance / summon correction",
    );
    if out.changes.contains(&StateChange::Queued(42)) {
        if ctx.pc().native_kind != 1 {
            out = Outcome::log("EnterZone task caster is not a PC");
            out.changes.push(StateChange::Queued(42));
            return out;
        }
        let correlation = u32::from_le_bytes(req[..4].try_into().unwrap());
        let reason = match ctx.pc().spawn_action_present {
            None => return out,
            Some(false) => Some(2),
            Some(true) if ctx.pc().spawn_state != 2 => Some(1149),
            Some(true) if ctx.pc().spawn_correlation != correlation => Some(1150),
            Some(true) => None,
        };
        if let Some(reason) = reason {
            out = Outcome::reason(44, reason);
            out.changes.push(StateChange::Queued(42));
        }
    }
    out
}

/// parity: 929 10-session-handlers.md#929--g_contentsrulelistreqcmd--partial partial
pub fn handle_contents_rule_list(ctx: &mut Context<'_>, _: &[u8]) -> Outcome {
    // Preserve the native signed comparisons over raw 64-bit tick storage.
    let last = ctx.session.contents_last_tick.unwrap_or(0) as i64;
    let now = ctx.world.tick;
    let signed_now = now as i64;
    if !(last <= 0
        || last <= ctx.world.config_reload_tick as i64
        || signed_now <= 30000
        || signed_now.wrapping_sub(last) >= 30000)
    {
        return Outcome::reason(931, 2167);
    }
    // 0x140370250 writes the tick before encryption/send, even with a null cache.
    let mut out =
        Outcome::unsupported("TODO ❓ cached contents fragments and null-cache send contract");
    if let Some(last) = ctx.session.contents_last_tick.as_mut() {
        *last = now;
        out.changes.push(StateChange::ContentsTick(now));
    }
    out
}

/// parity: 174 14-party-handlers.md#174--g_clearlogoutpartymemberreqcmd--partial partial
pub fn handle_clear_logout_party_member(ctx: &mut Context<'_>, _: &[u8]) -> Outcome {
    clear_members(ctx, 174, 175)
}
/// parity: 262 14-party-handlers.md#262--g_clearlogoutteammemberreqcmd--partial partial
pub fn handle_clear_logout_team_member(ctx: &mut Context<'_>, _: &[u8]) -> Outcome {
    if !ctx.world.team_enabled {
        return Outcome::reason(263, 2);
    }
    clear_members(ctx, 262, 263)
}
fn clear_members(ctx: &mut Context<'_>, command: u16, failure: u16) -> Outcome {
    // ❓ Expiry/removal/leader/disband and downstream effects remain unread.
    if ctx.pc().zone.is_none() || ctx.pc().native_kind != 1 {
        return Outcome::reason(failure, 2);
    }
    let mut out = ctx.queue(
        command,
        failure,
        2,
        "TODO ❓ ClearExpiredPartyMember membership, expiration, mutations and notifications",
    );
    if out.changes.contains(&StateChange::Queued(command)) {
        // Retain the existing session-plan task precheck for callers of doc 10.
        if ctx.pc().party_id == 0 {
            out = Outcome::reason(175, 2); // Shared task uses 175 even for command 262.
            out.changes.push(StateChange::Queued(command));
        }
        out.changes.push(StateChange::PartyTaskQueued(PartyTask {
            command,
            caster: ctx.pc().object_id,
            zone: ctx.pc().zone.unwrap(),
            kind: PartyTaskKind::ClearMembers,
        }));
    }
    out
}

/// parity: 300 10-session-handlers.md#300--g_channellistreqcmd--partial partial
pub fn handle_channel_list(ctx: &mut Context<'_>, _: &[u8]) -> Outcome {
    // STOP restriction query 0x140394181 -> 0x14060E280. Ordinary path only.
    if ctx.pc().zone.is_none() || ctx.pc().native_kind != 1 {
        return Outcome::reason(302, 2);
    }
    let mut out = ctx.queue(300, 302, 2, "TODO ❓ channel-list population");
    if out.changes.contains(&StateChange::Queued(300)) {
        let list = ctx
            .pc()
            .zone
            .and_then(|id| ctx.world.zones.get(&id))
            .and_then(|z| z.channel_list);
        out = match list {
            Some((max, flags)) => {
                let mut body = max.to_le_bytes().to_vec();
                body.extend(51i16.to_le_bytes());
                body.extend(flags);
                Outcome::reply(301, body)
            }
            None => Outcome::reason(302, 1189),
        };
        out.changes.push(StateChange::Queued(300));
    }
    out
}

/// parity: 303 10-session-handlers.md#303--g_changechannelreqcmd--partial partial
pub fn handle_change_channel(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    if ctx.pc().zone.is_none() {
        return Outcome::reason(304, 2);
    }
    // STOP 0x140394427 -> 0x14060E280. Only ordinary path below.
    let Some(channel) = Reader(req).short().map(|v| v as i16) else {
        return Outcome::unsupported("TODO ❓ unchecked ChangeChannel decode");
    };
    if ctx.pc().native_kind != 1 {
        return Outcome::reason(304, 2);
    }
    let out = ctx.queue(303, 304, 2, "TODO ❓ ChangeChannelTask");
    if !out.changes.contains(&StateChange::Queued(303)) {
        return out;
    }
    let mut result = change_channel_task(ctx, channel);
    result.changes.insert(0, StateChange::Queued(303));
    result
}
fn change_channel_task(ctx: &mut Context<'_>, channel: i16) -> Outcome {
    let pc = ctx.pc();
    let reason = if pc.achievement_reward || pc.jumping_ticket {
        Some(1194)
    } else if pc.combat {
        Some(2)
    } else if pc.instant_payment {
        Some(1195)
    } else if !ctx.world.arena && arena_blocked(pc) {
        Some(5)
    } else {
        None
    };
    if let Some(reason) = reason {
        return Outcome::reason(304, reason);
    }
    let Some(zone) = pc.zone.and_then(|id| ctx.world.zones.get(&id)) else {
        return Outcome::unsupported("TODO ❓ missing server zone record");
    };
    if channel == zone.channel {
        return Outcome::reason(304, 1190);
    }
    if channel > 0 {
        match ctx.services.available_channel(channel) {
            Gate::Fail => return Outcome::reason(304, 1192),
            Gate::Unknown => return Outcome::unsupported("TODO ❓ World::AvailableChannel"),
            Gate::Pass => {}
        }
    }
    let pc = ctx.pc();
    if !pc.builder
        && !pc.ignore_channel_cooldown
        && pc.last_channel_change != 0
        && ctx.world.tick.wrapping_sub(pc.last_channel_change) < 160000
    {
        return Outcome::reason(304, 1193);
    }
    if !pc.builder
        && (zone.forbidden_channel_destination
            || zone.zone_type == 40
            || (zone.zone_type == 87 && ctx.world.channel_type_87_restricted))
    {
        return Outcome::reason(304, 1191);
    }
    match ctx.services.attraction_channel(channel) {
        Some(Err(Some(reason))) => return Outcome::reason(304, reason),
        Some(Err(None)) => return Outcome::unsupported("TODO ❓ attraction virtual channel check"),
        None if channel == 0 => return Outcome::reason(304, 1191),
        _ => {}
    }
    // This second gate represents LeaveAction creation/initialization, not travel.
    let mut out = ctx.queue(
        3,
        304,
        2,
        "TODO ❓ LeaveAction routing, rollback and channel success",
    );
    if out.changes.contains(&StateChange::Queued(3)) {
        let tick = ctx.world.tick;
        ctx.session
            .pc
            .as_mut()
            .expect("PC dispatch")
            .last_channel_change = tick;
        out.changes.push(StateChange::LastChannelChange(tick));
    }
    out
}

/// parity: 305 10-session-handlers.md#305--g_saveclientoptionreqcmd--partial partial
pub fn handle_save_client_option(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let mut r = Reader(req);
    let (Some(kind), Some(data)) = (r.byte(), r.blob(4000)) else {
        return Outcome::reason(306, 2);
    };
    if data.len() > 3300 || !matches!(kind, 1 | 2) {
        return Outcome::reason(306, 2);
    }
    if ctx.pc().db_id == 0 {
        return Outcome::log("zero PC DB ID; option not sent");
    }
    let mut out =
        Outcome::unsupported("TODO ❓ cached option response, persistence failures and success");
    out.daemon_requests.push(DaemonRequest::SaveClientOption {
        db_id: ctx.pc().db_id,
        option_type: kind,
        data,
    });
    out
}

/// parity: 411 10-session-handlers.md#411--g_autologoutcheckreqcmd--done partial
pub fn handle_auto_logout_check(ctx: &mut Context<'_>, _: &[u8]) -> Outcome {
    if ctx.pc().zone.is_none() || ctx.pc().native_kind != 1 {
        return Outcome::reason(413, 2);
    }
    let mut out = ctx.queue(411, 413, 2, "TODO ❓ AutoLogoutCheck task queue adapter");
    if out.changes.contains(&StateChange::Queued(411)) {
        out.status = Status::Handled;
        if ctx.world.game_log_enabled {
            let detail = match ctx.pc().life_state {
                0 => 1,
                1 => 2,
                2 => 3,
                _ => 0,
            };
            out.reports.push(Report::AutoLogoutCheck { detail });
        }
    }
    out
}

/// parity: 809 10-session-handlers.md#809--g_extendjumpingcharactercreationquotawithwarehouseitemreqcmd--partial partial
pub fn handle_extend_jumping_character_creation_quota_with_warehouse_item(
    _: &mut Context<'_>,
    req: &[u8],
) -> Outcome {
    let mut r = Reader(req);
    let (Some(label), Some(item), Some(_data)) = (r.quad(), r.quad(), r.int()) else {
        return Outcome::unsupported("TODO ❓ unchecked quota request echo bits");
    };
    let mut body = 1967u16.to_le_bytes().to_vec();
    body.extend(label.to_le_bytes());
    body.extend(item.to_le_bytes());
    Outcome::reply(811, body)
}

/// parity: 887 10-session-handlers.md#887--g_qqlogoutreqcmd--partial partial
pub fn handle_qq_logout(ctx: &mut Context<'_>, _: &[u8]) -> Outcome {
    if !ctx.world.qq_interface_present {
        return native_false();
    }
    if !ctx.pc().user_available {
        return Outcome::default();
    }
    Outcome::unsupported("TODO ❓ QQ interface identity and virtual +8 effects, 0x140371430")
}

/// parity: 981 10-session-handlers.md#981--g_enterzonedirectreqcmd--partial partial
pub fn handle_enter_zone_direct(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    if ctx.world.arena || arena_blocked(ctx.pc()) {
        return Outcome::reason(983, 5);
    }
    let mut r = Reader(req);
    let (Some(kind), Some(_q), Some(_flag)) = (r.byte(), r.quad(), r.byte()) else {
        return Outcome::reason(983, 2);
    };
    let Some(count) = r.short().filter(|c| *c <= 16) else {
        return Outcome::reason(983, 2399);
    };
    for _ in 0..count {
        if r.short().is_none() || r.short().is_none() {
            return Outcome::reason(983, 2);
        }
    }
    if ctx.pc().zone.is_none() || ctx.pc().native_kind != 1 || !(1..=3).contains(&kind) {
        return Outcome::reason(983, 2);
    }
    let mut out = ctx.queue(
        981,
        983,
        2,
        "TODO ❓ direct zone entry authorization and deferred effects",
    );
    if out.changes.contains(&StateChange::Queued(981)) {
        let Some(zone) = ctx.pc().zone.and_then(|id| ctx.world.zones.get(&id)) else {
            return out;
        };
        let reason = match zone.direct_entry_geo {
            DirectEntryGeo::Unknown => return out,
            DirectEntryGeo::Missing => Some(2),
            DirectEntryGeo::MissingRecord => Some(2388),
            DirectEntryGeo::Category(category) if category != 1 => Some(2388),
            DirectEntryGeo::Category(_) if ctx.pc().life_state != 0 => Some(2390),
            DirectEntryGeo::Category(_) if ctx.pc().combat => Some(2391),
            DirectEntryGeo::Category(_)
                if ctx.pc().achievement_reward || ctx.pc().jumping_ticket =>
            {
                Some(2392)
            }
            DirectEntryGeo::Category(_) if ctx.pc().instant_payment => Some(2393),
            DirectEntryGeo::Category(_) if kind == 2 => Some(3),
            DirectEntryGeo::Category(_) => None,
        };
        if let Some(reason) = reason {
            out = Outcome::reason(983, reason);
            out.changes.push(StateChange::Queued(981));
        }
    }
    out
}

/// parity: 984 10-session-handlers.md#984--g_leavezonedirectreqcmd--partial partial
pub fn handle_leave_zone_direct(ctx: &mut Context<'_>, _: &[u8]) -> Outcome {
    if ctx.pc().zone.is_none() {
        return Outcome::log("leave-zone missing zone");
    }
    if ctx.pc().native_kind != 1 {
        return Outcome::log("leave-zone task caster is not a PC");
    }
    match ctx.services.queue(984) {
        Gate::Fail => Outcome::log("leave-zone queue failure"),
        Gate::Unknown => Outcome::unsupported("TODO ❓ LeaveZoneDirect task queue adapter"),
        Gate::Pass => Outcome {
            changes: vec![StateChange::Queued(984)],
            ..Outcome::unsupported("TODO ❓ LeaveZoneDirect task execution")
        },
    }
}

/// parity: 1149 10-session-handlers.md#1149--g_usejumpingcharacterticketreqcmd--partial partial
pub fn handle_use_jumping_character_ticket(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let mut r = Reader(req);
    let decoded = (|| Some((r.quad()?, r.quad()?, r.quad()?, r.byte()?)))();
    let reason = match decoded {
        None => Some(2483),
        Some(_) if !ctx.world.account_inventory_enabled => Some(2),
        Some(_) if ctx.world.arena => Some(5),
        Some(_) if !ctx.pc().user_available || ctx.pc().zone.is_none() => Some(2),
        Some((_, _, raw, _))
            if !ctx.world.jumping_ticket_ids.contains(&(raw as u32))
                || !ctx
                    .world
                    .additional_service_ids
                    .contains(&(u64::from(raw as u32) | 0x100000000)) =>
        {
            Some(2499)
        }
        _ => None,
    };
    let mut out = if let Some(reason) = reason {
        Outcome::reason(1151, reason)
    } else {
        ctx.queue(
            1149,
            1151,
            2,
            "TODO ❓ ticket parameter layout / transaction / ownership / consumption / completion",
        )
    };
    if let Some(reply) = out.replies.first_mut() {
        reply.body.extend(0u64.to_le_bytes());
    }
    out
}
