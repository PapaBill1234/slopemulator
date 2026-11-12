//! Doc 19: misc entry functions. Build 381.8867.
//!
//! Zone tasks queue a `SkillTask` with a `MiscTask` kind through
//! `Services::queue_skill`. Task bodies and game-log writers are boundaries.
use super::party::string_body;
use super::skills::{counted, queue};
use super::*;

pub const MISC_COMMANDS: &[u16] = &[
    ids::G_StopCinematicReqCmd,
    ids::G_DpGameClientToServerReqCmd,
    ids::G_DpArenaClientToServerReqCmd,
    ids::G_ReloadDaemonReqCmd,
    ids::G_RegisterTraceReqCmd,
    ids::G_UnregisterTraceReqCmd,
    ids::G_WriteClientLogReqCmd,
    ids::G_ReportBotSpammerReqCmd,
    ids::G_TraceRouteIPReqCmd,
    ids::G_ChallengeListsReqCmd,
    ids::G_SecretTreasureExpirationTimeReqCmd,
    ids::G_SetSlateAmpSlotReqCmd,
    ids::G_ChallengeListTakeRewardReqCmd,
];

/// Doc 19 shared fields.
#[derive(Clone, Debug, Default)]
pub struct MiscWorld {
    /// cfg +2744 DWORD +1160: minimum reporter level for 419 (≤ 0 disables the check).
    pub bot_report_min_level: i32,
    /// cfg +2736 byte +537: 820 writes the trace-route game log.
    pub trace_route_log: bool,
    /// `qword_140EE58A0` DWORD index 523: slate amp page count (1188).
    pub slate_amp_pages: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MiscTask {
    /// 165, kind 205, vtable `0x140BC2B48`. i32 +144, byte +148 = 0.
    StopCinematic { value: i32 },
    /// 1130, kind 333, vtable `0x140C41208`.
    ChallengeLists,
    /// 1137, kind 320, vtable `0x140B96730`. Request ID +144.
    SecretTreasureExpiration { request: i16 },
    /// 1188, kind 339, vtable `0x140A81828`. +144 page, +145 position, DWORD +146.
    SetSlateAmpSlot { page: u8, position: u8, order: u32 },
    /// 1423, kind 223, vtable `0x140CFE4B8`. Sub type +144.
    ChallengeListTakeReward { sub_type: u8 },
}
impl From<MiscTask> for SkillTaskKind {
    fn from(task: MiscTask) -> Self {
        SkillTaskKind::Misc(task)
    }
}

pub(super) fn dispatch_misc(ctx: &mut Context<'_>, command: u16, req: &[u8]) -> Outcome {
    match command {
        ids::G_StopCinematicReqCmd => handle_stop_cinematic(ctx, req),
        ids::G_DpGameClientToServerReqCmd
        | ids::G_DpArenaClientToServerReqCmd
        | ids::G_ReloadDaemonReqCmd
        | ids::G_RegisterTraceReqCmd
        | ids::G_UnregisterTraceReqCmd => handle_return_true_stub(ctx, req),
        ids::G_WriteClientLogReqCmd => handle_write_client_log(ctx, req),
        ids::G_ReportBotSpammerReqCmd => handle_report_bot_spammer(ctx, req),
        ids::G_TraceRouteIPReqCmd => handle_trace_route_ip(ctx, req),
        ids::G_ChallengeListsReqCmd => handle_challenge_lists(ctx, req),
        ids::G_SecretTreasureExpirationTimeReqCmd => {
            handle_secret_treasure_expiration_time(ctx, req)
        }
        ids::G_SetSlateAmpSlotReqCmd => handle_set_slate_amp_slot(ctx, req),
        ids::G_ChallengeListTakeRewardReqCmd => handle_challenge_list_take_reward(ctx, req),
        _ => unreachable!("membership checked"),
    }
}

fn task(ctx: &Context<'_>, command: u16, zone: u64, task: MiscTask) -> SkillTask {
    SkillTask {
        command,
        caster: ctx.pc().object_id,
        summon: false,
        zone,
        kind: task.into(),
    }
}

fn logged(mut out: Outcome, text: &'static str) -> Outcome {
    out.logs.push(text);
    out
}

/// parity: 165 19-misc-handlers.md#165--g_stopcinematicreqcmd--partial partial
pub fn handle_stop_cinematic(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_StopCinematicFailCmd;
    // The decode result is ignored: a short body leaves the value at 0.
    let value = Reader(req).int().unwrap_or(0) as i32;
    let Some(zone) = ctx.pc().zone else {
        return logged(Outcome::reason(F, 2), "stop cinematic: zone is null");
    };
    if ctx.pc().native_kind != 1 {
        return logged(Outcome::reason(F, 2), "stop cinematic: caster is not a PC");
    }
    let t = task(ctx, 165, zone, MiscTask::StopCinematic { value });
    queue(ctx, t, Outcome::reason(F, 2))
}

/// parity: 356 19-misc-handlers.md#356--g_dpgameclienttoserverreqcmd--done ported
/// parity: 358 27-duel-ranking-handlers.md#358--g_dparenaclienttoserverreqcmd--done ported
/// parity: 359 19-misc-handlers.md#359--g_reloaddaemonreqcmd--done ported
/// parity: 360 19-misc-handlers.md#360--g_registertracereqcmd--done ported
/// parity: 362 19-misc-handlers.md#362--g_unregistertracereqcmd--done ported
pub fn handle_return_true_stub(_ctx: &mut Context<'_>, _req: &[u8]) -> Outcome {
    Outcome::default()
}

/// parity: 407 19-misc-handlers.md#407--g_writeclientlogreqcmd--partial partial
pub fn handle_write_client_log(_ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    match Reader(req).string(4000) {
        Some(_) => logged(Outcome::reply(408, Vec::new()), "client log"),
        None => logged(
            Outcome::reason(409, 2),
            "write client log: decompose failed",
        ),
    }
}

/// parity: 419 19-misc-handlers.md#419--g_reportbotspammerreqcmd--partial partial
pub fn handle_report_bot_spammer(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_ReportBotSpammerFailCmd;
    let now = ctx.world.tick;
    if ctx.pc().bot_report_tick + 2000 > now {
        return Outcome::log("report bot spammer: too frequent request");
    }
    let mut r = Reader(req);
    let kind = r.byte();
    let fail = |code: u16, kind: u8, text: &'static str| {
        let mut body = code.to_le_bytes().to_vec();
        body.push(kind);
        logged(Outcome::reply(F, body), text)
    };
    let (Some(kind), Some(target), Some(name)) = (kind, r.quad(), r.string(33)) else {
        return fail(
            2224,
            kind.unwrap_or(0),
            "report bot spammer: decompose failed",
        );
    };
    if !(1..=2).contains(&kind) {
        return fail(2224, kind, "report bot spammer: invalid type value");
    }
    let lobby = ctx.pc().arena_state == 3;
    if lobby && (target != 0 || name.is_empty()) {
        return fail(
            2224,
            kind,
            "report bot spammer: target pc id in arena lobby",
        );
    }
    // Native lookups 0x1403F7D20 / 0x1403F7EF0 with rights mask 6.
    let found = if target != 0 {
        ctx.world.mentoring.by_id(target, 6)
    } else {
        ctx.world.mentoring.by_name(&name, 6)
    };
    if found.is_none() && !lobby {
        return fail(2226, kind, "report bot spammer: target pc not found");
    }
    let min = ctx.world.misc.bot_report_min_level;
    if min > 0 && i32::from(ctx.pc().level) < min {
        return fail(2225, kind, "report bot spammer: pc level check failed");
    }
    let target_name = match found {
        Some(pc) if !lobby => pc.name.clone(),
        _ => name,
    };
    let mut body = vec![kind];
    string_body(&mut body, &target_name);
    if let Some(pc) = ctx.session.pc.as_mut() {
        pc.bot_report_tick = now;
    }
    logged(Outcome::reply(420, body), "report bot spammer: game log")
}

/// parity: 820 19-misc-handlers.md#820--g_tracerouteipreqcmd--partial partial
pub fn handle_trace_route_ip(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let mut r = Reader(req);
    let Some(n) = r.short().map(|n| n as i16) else {
        return native_false("trace route ip: packet decompose fail");
    };
    if !(0..=5).contains(&n) {
        return native_false("trace route ip: too many ip list");
    }
    let mut r2 = Reader(req);
    if counted(&mut r2, 5, |r| r.int()).is_none() {
        return native_false("trace route ip: packet decompose fail");
    }
    if ctx.world.misc.trace_route_log && ctx.pc().native_kind == 1 {
        return Outcome::log("trace route ip: game log");
    }
    Outcome::default()
}

fn native_false(text: &'static str) -> Outcome {
    Outcome {
        status: Status::NativeFalse,
        logs: vec![text],
        ..Outcome::default()
    }
}

/// parity: 1130 19-misc-handlers.md#1130--g_challengelistsreqcmd--partial partial
pub fn handle_challenge_lists(ctx: &mut Context<'_>, _req: &[u8]) -> Outcome {
    const F: u16 = ids::G_ChallengeListsFailCmd;
    let Some(zone) = ctx.pc().zone else {
        return logged(Outcome::reason(F, 2), "challenge lists: failed to get zone");
    };
    let t = task(ctx, 1130, zone, MiscTask::ChallengeLists);
    let fail = logged(
        Outcome::reason(F, 2),
        "challenge lists: failed to initialize task",
    );
    queue(ctx, t, fail)
}

/// parity: 1137 19-misc-handlers.md#1137--g_secrettreasureexpirationtimereqcmd--partial partial
pub fn handle_secret_treasure_expiration_time(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_SecretTreasureExpirationTimeFailCmd;
    let Some(request) = Reader(req).short().map(|v| v as i16) else {
        return logged(Outcome::reason(F, 2), "secret treasure: decompose failed");
    };
    let Some(zone) = ctx.pc().zone else {
        return logged(Outcome::reason(F, 2), "secret treasure: zone is null");
    };
    let t = task(
        ctx,
        ids::G_SecretTreasureExpirationTimeReqCmd,
        zone,
        MiscTask::SecretTreasureExpiration { request },
    );
    // Native quirk: the queue failure replies 1028, not 1139.
    let fail = logged(
        Outcome::reason(1028, 2),
        "secret treasure: task execute failed",
    );
    queue(ctx, t, fail)
}

/// parity: 1188 19-misc-handlers.md#1188--g_setslateampslotreqcmd--partial partial
pub fn handle_set_slate_amp_slot(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_SetSlateAmpSlotFailCmd;
    if ctx.world.arena || session::arena_blocked(ctx.pc()) {
        return logged(
            Outcome::reason(F, 5),
            "set slate amp slot: cannot process in arena",
        );
    }
    if ctx.pc().t2_blocked {
        return logged(Outcome::reason(F, 7), "set slate amp slot: trial play");
    }
    let pages = ctx.world.misc.slate_amp_pages;
    if pages <= 0 {
        return logged(Outcome::reason(F, 2537), "set slate amp slot: not enable");
    }
    let mut r = Reader(req);
    let (Some(page), Some(position), Some(order)) = (r.byte(), r.byte(), r.short()) else {
        return logged(
            Outcome::reason(F, 2539),
            "set slate amp slot: decompose failed",
        );
    };
    let Some(zone) = ctx.pc().zone else {
        return logged(Outcome::reason(F, 2), "set slate amp slot: zone is null");
    };
    let page_ok = (1..=2).contains(&page) && i32::from(page as i8) <= pages;
    if !page_ok || !(1..=6).contains(&position) || order == 0 {
        return logged(
            Outcome::reason(F, 2539),
            "set slate amp slot: task initialize fail",
        );
    }
    let order = u32::from(order) | 0x1_0000;
    let t = task(
        ctx,
        ids::G_SetSlateAmpSlotReqCmd,
        zone,
        MiscTask::SetSlateAmpSlot {
            page,
            position,
            order,
        },
    );
    let fail = logged(Outcome::reason(F, 2), "set slate amp slot: task post fail");
    queue(ctx, t, fail)
}

/// parity: 1423 19-misc-handlers.md#1423--g_challengelisttakerewardreqcmd--partial partial
pub fn handle_challenge_list_take_reward(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_ChallengeListTakeRewardFailCmd;
    // The decode result is ignored: the default sub type is −1.
    let sub_type = Reader(req).byte().unwrap_or(0xFF);
    let Some(zone) = ctx.pc().zone else {
        return logged(
            Outcome::reason(F, 2),
            "challenge list reward: cannot find zone",
        );
    };
    if ctx.pc().native_kind != 1 {
        return logged(
            Outcome::reason(F, 2),
            "challenge list reward: caster is not PC",
        );
    }
    if sub_type > 1 {
        return logged(
            Outcome::reason(F, 2),
            "challenge list reward: invalid sub type",
        );
    }
    let t = task(
        ctx,
        ids::G_ChallengeListTakeRewardReqCmd,
        zone,
        MiscTask::ChallengeListTakeReward { sub_type },
    );
    let fail = logged(
        Outcome::reason(F, 2),
        "challenge list reward: execute failed",
    );
    queue(ctx, t, fail)
}
