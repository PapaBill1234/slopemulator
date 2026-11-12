//! Doc 12c part 1: BoardGacha (1152–1182), badge (1140–1146) and slate
//! (1191–1200) entry functions. Build 381.8867.
//!
//! BoardGacha entries submit a manager task or start a user transaction
//! through `Services::board_gacha`. Badge and slate entries queue an
//! `ItemTask` with a `BadgeSlateTask` kind. Manager runners, transaction
//! runners and task actions are adapter boundaries.
use super::items2::{count, pair_fail, pairs, post, u0_passes, KindCheck, Post};
use super::*;

pub const ITEM4_COMMANDS: &[u16] = &[
    ids::G_BoardGachaRetrieveSeasonReqCmd,
    ids::G_BoardGachaSessionOpenReqCmd,
    ids::G_BoardGachaSessionCloseReqCmd,
    ids::G_BoardGachaBoardInfoReqCmd,
    ids::G_BoardGachaGetParticipationCountPerSeasonReqCmd,
    ids::G_BoardGachaRetrieveParticipationInfoPerPcReqCmd,
    ids::G_BoardGachaRetrieveParticipationInfoAllReqCmd,
    ids::G_BoardGachaRetrieveParticipationRewardStateReqCmd,
    ids::G_ChangeBadgeEquipPageReqCmd,
    ids::G_SynthesisBadgeReqCmd,
    ids::G_SynthesisBadgeRecipeDataReqCmd,
    ids::G_DrawSlateNewStoneReqCmd,
    ids::G_SetSlateNewStoneReqCmd,
    ids::G_GiveUpSlateNewStoneReqCmd,
    ids::G_ChangeSlatePageReqCmd,
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BoardGachaRequest {
    /// 0x80-byte manager task, submitted to daemon +5824 with final argument 0.
    Manager {
        command: u16,
        /// Task kind 56..60.
        kind: u8,
        /// PC +40, stored at task +112.
        object_id: u64,
        /// PC +20744, stored at task +120.
        identity: u64,
    },
    /// 0x60-byte user transaction; qword +88 = 0; ID from daemon +5128.
    Transaction {
        command: u16,
        /// Start event passed to `0x1400D4A80` (null payload).
        start_event: u8,
        /// Initial transaction DWORD +8.
        initial: u32,
        object_id: u64,
    },
    /// Doc 16 1185: 0x68-byte user transaction, DWORD +8 = 0x4F210A,
    /// qword +96 = 0; start event 69 with the count as payload.
    RewardTransaction {
        command: u16,
        count: i16,
        object_id: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BadgeSlateTask {
    /// 1140, kind 325. Page +144.
    ChangeBadgePage { page: u8 },
    /// 1143, kind 324. Triples (selector, short, short) 1..8; pairs 0..16.
    SynthesisBadge {
        first: Vec<(u8, u16, u16)>,
        second: Vec<(u16, u16)>,
    },
    /// 1194, kind 341.
    SetSlateStone { page: u8 },
    /// 1197, kind 342.
    GiveUpSlateStone { page: u8 },
    /// 1200, kind 343.
    ChangeSlatePage { page: u8 },
}
impl From<BadgeSlateTask> for ItemTaskKind {
    fn from(task: BadgeSlateTask) -> Self {
        ItemTaskKind::BadgeSlate(task)
    }
}

pub(super) fn dispatch_item4(ctx: &mut Context<'_>, command: u16, req: &[u8]) -> Outcome {
    match command {
        ids::G_BoardGachaRetrieveSeasonReqCmd => board_gacha_manager(
            ctx,
            ids::G_BoardGachaRetrieveSeasonReqCmd,
            ids::G_BoardGachaRetrieveSeasonFailCmd,
            56,
        ),
        ids::G_BoardGachaSessionOpenReqCmd => board_gacha_manager(
            ctx,
            ids::G_BoardGachaSessionOpenReqCmd,
            ids::G_BoardGachaSessionOpenFailCmd,
            57,
        ),
        ids::G_BoardGachaSessionCloseReqCmd => board_gacha_manager(
            ctx,
            ids::G_BoardGachaSessionCloseReqCmd,
            ids::G_BoardGachaSessionCloseFailCmd,
            58,
        ),
        ids::G_BoardGachaBoardInfoReqCmd => board_gacha_manager(
            ctx,
            ids::G_BoardGachaBoardInfoReqCmd,
            ids::G_BoardGachaBoardInfoFailCmd,
            60,
        ),
        ids::G_BoardGachaRetrieveParticipationInfoAllReqCmd => board_gacha_manager(
            ctx,
            ids::G_BoardGachaRetrieveParticipationInfoAllReqCmd,
            ids::G_BoardGachaRetrieveParticipationInfoAllFailCmd,
            59,
        ),
        ids::G_BoardGachaGetParticipationCountPerSeasonReqCmd => board_gacha_transaction(
            ctx,
            ids::G_BoardGachaGetParticipationCountPerSeasonReqCmd,
            ids::G_BoardGachaGetParticipationCountPerSeasonFailCmd,
            65,
            4923658,
        ),
        ids::G_BoardGachaRetrieveParticipationInfoPerPcReqCmd => board_gacha_transaction(
            ctx,
            ids::G_BoardGachaRetrieveParticipationInfoPerPcReqCmd,
            ids::G_BoardGachaRetrieveParticipationInfoPerPcFailCmd,
            66,
            4989194,
        ),
        ids::G_BoardGachaRetrieveParticipationRewardStateReqCmd => board_gacha_transaction(
            ctx,
            ids::G_BoardGachaRetrieveParticipationRewardStateReqCmd,
            ids::G_BoardGachaRetrieveParticipationRewardStateFailCmd,
            70,
            5251338,
        ),
        ids::G_ChangeBadgeEquipPageReqCmd => handle_change_badge_equip_page(ctx, req),
        ids::G_SynthesisBadgeReqCmd => handle_synthesis_badge(ctx, req),
        ids::G_SynthesisBadgeRecipeDataReqCmd => handle_synthesis_badge_recipe_data(ctx, req),
        ids::G_DrawSlateNewStoneReqCmd => handle_draw_slate_new_stone(ctx, req),
        ids::G_SetSlateNewStoneReqCmd => handle_set_slate_new_stone(ctx, req),
        ids::G_GiveUpSlateNewStoneReqCmd => handle_give_up_slate_new_stone(ctx, req),
        ids::G_ChangeSlatePageReqCmd => handle_change_slate_page(ctx, req),
        _ => unreachable!("membership checked"),
    }
}

// ---- BoardGacha ---------------------------------------------------------------

fn board_gacha_submit(ctx: &mut Context<'_>, failure: u16, request: BoardGachaRequest) -> Outcome {
    match ctx.services.board_gacha(&request) {
        Gate::Pass => {
            let command = match request {
                BoardGachaRequest::Manager { command, .. }
                | BoardGachaRequest::Transaction { command, .. }
                | BoardGachaRequest::RewardTransaction { command, .. } => command,
            };
            Outcome {
                changes: vec![StateChange::Queued(command)],
                ..Outcome::default()
            }
        }
        Gate::Fail => Outcome::reason(failure, 2),
        Gate::Unknown => Outcome::unsupported("❓ BoardGacha manager/transaction adapter"),
    }
}

/// parity: 1152 12c-item-handlers.md#command-1152--g_boardgacharetrieveseasonreqcmd partial
/// parity: 1160 12c-item-handlers.md#command-1160--g_boardgachasessionopenreqcmd partial
/// parity: 1162 12c-item-handlers.md#command-1162--g_boardgachasessionclosereqcmd partial
/// parity: 1165 12c-item-handlers.md#command-1165--g_boardgachaboardinforeqcmd partial
/// parity: 1173 12c-item-handlers.md#command-1173--g_boardgacharetrieveparticipationinfoallreqcmd partial
pub fn board_gacha_manager(ctx: &mut Context<'_>, command: u16, failure: u16, kind: u8) -> Outcome {
    if !ctx.world.board_gacha_enabled {
        return Outcome::reason(failure, 2);
    }
    let pc = ctx.pc();
    let request = BoardGachaRequest::Manager {
        command,
        kind,
        object_id: pc.object_id,
        identity: pc.db_id,
    };
    board_gacha_submit(ctx, failure, request)
}

/// parity: 1167 12c-item-handlers.md#command-1167--g_boardgachagetparticipationcountperseasonreqcmd partial
/// parity: 1170 12c-item-handlers.md#command-1170--g_boardgacharetrieveparticipationinfoperpcreqcmd partial
/// parity: 1182 12c-item-handlers.md#command-1182--g_boardgacharetrieveparticipationrewardstatereqcmd partial
pub fn board_gacha_transaction(
    ctx: &mut Context<'_>,
    command: u16,
    failure: u16,
    start_event: u8,
    initial: u32,
) -> Outcome {
    if !ctx.world.board_gacha_enabled {
        return Outcome::reason(failure, 2);
    }
    // User from PC +20696 only when PC +20752 = 0. Missing user: log only.
    if !ctx.pc().user_present {
        return Outcome::log("board gacha: user is not found");
    }
    let request = BoardGachaRequest::Transaction {
        command,
        start_event,
        initial,
        object_id: ctx.pc().object_id,
    };
    board_gacha_submit(ctx, failure, request)
}

// ---- Badge ----------------------------------------------------------------------

/// parity: 1140 12c-item-handlers.md#command-1140--g_changebadgeequippagereqcmd partial
pub fn handle_change_badge_equip_page(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    if !ctx.world.badge_enabled {
        return Outcome::reason(1142, 2);
    }
    let Some(page) = Reader(req).byte() else {
        return Outcome::unsupported("❓ unchecked badge page decode (short body)");
    };
    let task = BadgeSlateTask::ChangeBadgePage { page };
    post(
        ctx,
        1140,
        Outcome::reason(1142, 2),
        KindCheck::Fail,
        Post::Queue,
        task,
    )
}

/// parity: 1143 12c-item-handlers.md#command-1143--g_synthesisbadgereqcmd partial
pub fn handle_synthesis_badge(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_SynthesisBadgeFailCmd;
    if !ctx.world.badge_enabled {
        return Outcome::reason(F, 1);
    }
    if !u0_passes(ctx) {
        return Outcome::reason(F, 2482);
    }
    if ctx.pc().zone.is_none() {
        return Outcome::reason(F, 2);
    }
    let mut r = Reader(req);
    let Some(n) = count(&mut r, 1, 8) else {
        return Outcome::reason(F, 2469);
    };
    let first = (0..n)
        .map(|_| Some((r.byte()?, r.short()?, r.short()?)))
        .collect::<Option<Vec<_>>>();
    let Some(first) = first else {
        return Outcome::reason(F, 2469);
    };
    let second = match pairs(&mut r, 0, 16) {
        Ok(p) => p,
        Err(e) => return pair_fail(e, F, 2469, 2469),
    };
    let task = BadgeSlateTask::SynthesisBadge { first, second };
    post(
        ctx,
        1143,
        Outcome::reason(F, 2),
        KindCheck::Fail,
        Post::Queue,
        task,
    )
}

/// parity: 1146 12c-item-handlers.md#command-1146--g_synthesisbadgerecipedatareqcmd partial
pub fn handle_synthesis_badge_recipe_data(ctx: &mut Context<'_>, _: &[u8]) -> Outcome {
    if !ctx.world.badge_enabled {
        return Outcome::reason(1148, 1);
    }
    Outcome::unsupported("❓ cached recipe packet at table-manager +82176")
}

// ---- Slate ----------------------------------------------------------------------

/// Shared slate gates: T1 → 5, T2 → 7, global DWORD +2092 > 0.
fn slate_gates(ctx: &Context<'_>, failure: u16, disabled: u16) -> Option<Outcome> {
    if ctx.world.arena || session::arena_blocked(ctx.pc()) {
        return Some(Outcome::reason(failure, 5));
    }
    if ctx.pc().t2_blocked {
        return Some(Outcome::reason(failure, 7));
    }
    if ctx.world.slate_pages <= 0 {
        return Some(Outcome::reason(failure, disabled));
    }
    None
}

/// parity: 1191 12c-item-handlers.md#command-1191--g_drawslatenewstonereqcmd partial
pub fn handle_draw_slate_new_stone(ctx: &mut Context<'_>, _: &[u8]) -> Outcome {
    if let Some(out) = slate_gates(ctx, 1193, 2542) {
        return out;
    }
    Outcome::unsupported("❓ 1191 decoder destinations, blob capacity and initializer 0x1402D5F60")
}

/// Page entries 1194/1197/1200: decode page (zero-initialized; failure →
/// `decode`), zone → 2, page `u8(page-1) <= 1` and `page <= +2092`.
fn slate_page(
    ctx: &mut Context<'_>,
    req: &[u8],
    command: u16,
    disabled: u16,
    decode: u16,
    page_fail: u16,
    build: fn(u8) -> BadgeSlateTask,
) -> Outcome {
    let failure = command + 2;
    if let Some(out) = slate_gates(ctx, failure, disabled) {
        return out;
    }
    let Some(page) = Reader(req).byte() else {
        return Outcome::reason(failure, decode);
    };
    if ctx.pc().zone.is_none() {
        return Outcome::reason(failure, 2);
    }
    if page.wrapping_sub(1) > 1 || i32::from(page) > ctx.world.slate_pages {
        return Outcome::reason(failure, page_fail);
    }
    let fail = Outcome::reason(failure, 2);
    post(
        ctx,
        command,
        fail,
        KindCheck::None,
        Post::Queue,
        build(page),
    )
}

/// parity: 1194 12c-item-handlers.md#command-1194--g_setslatenewstonereqcmd partial
pub fn handle_set_slate_new_stone(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    slate_page(ctx, req, 1194, 2547, 2549, 2, |page| {
        BadgeSlateTask::SetSlateStone { page }
    })
}

/// parity: 1197 12c-item-handlers.md#command-1197--g_giveupslatenewstonereqcmd partial
pub fn handle_give_up_slate_new_stone(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    slate_page(ctx, req, 1197, 2551, 2553, 2, |page| {
        BadgeSlateTask::GiveUpSlateStone { page }
    })
}

/// parity: 1200 12c-item-handlers.md#command-1200--g_changeslatepagereqcmd partial
pub fn handle_change_slate_page(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    slate_page(ctx, req, 1200, 2555, 2557, 2557, |page| {
        BadgeSlateTask::ChangeSlatePage { page }
    })
}
