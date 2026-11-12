//! Doc 27: duel, party battle, ranking and season entry functions.
//! Build 381.8867.
//!
//! Part 1 covers the duel and party-battle zone tasks. Task constructors
//! and initialisers (`0x1402B8010`, `0x1402B84F0`) are opaque: the adapter
//! runs them inside `skills::queue`. An initialiser failure is a queue Fail.

use super::*;

pub const DUEL_RANKING_COMMANDS: &[u16] = &[
    ids::G_DuelChooseDuelOrderReqCmd,
    ids::G_DuelReadyRoundReqCmd,
    ids::G_DuelTakeTagTokenReqCmd,
    ids::G_PartyBattleReadyReqCmd,
    ids::G_PartyBattleStartReqCmd,
    ids::G_DuelBotChallengeCombatStartReqCmd,
    ids::G_DuelBotChallengePickStrategicToolReqCmd,
    ids::G_DuelBotChallengeUseStrategicToolReqCmd,
    ids::G_PartyBattleVoteCastReqCmd,
    ids::G_DuelBotTrainingRoomPickDuelBotReqCmd,
    ids::G_DuelBotTrainingRoomCombatStartReqCmd,
    ids::G_DuelBotTrainingRoomRewardReqCmd,
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DuelTask {
    /// 277, kind 227, 0x98 bytes, vtable 0x140C3B318. Order byte +144.
    ChooseDuelOrder { order: u8 },
    /// 280 / 283: DuelTask 0x14036D860, init 0x1402B8010(task, zone, pc, mode).
    /// Mode 1 = ready round, 3 = take tag token (732 uses mode 2).
    Duel { mode: u8 },
    /// 289 / 292: PartyBattleTask 0x14036D8D0, init 0x1402B84F0(task, zone, pc, mode).
    /// Mode 0 = ready, 1 = start.
    PartyBattle { mode: u8 },
    /// 1022, kind 260, 0x98 bytes, vtable 0x140C3A928. Target PC ID qword +144.
    /// ❓ The adapter must also check zone +16536 virtual +8 == 6
    /// (party-battle zone); Fail replies 1024 [2], the same as a queue Fail.
    PartyBattleVote { target: u64 },
    /// 968: task 0x140098E90, init 0x1402BD380(task, zone, pc, bot).
    DuelBotChallengeStart { bot: u64 },
    /// 973 / 977: task 0x14036D930 (0xA8), init 0x1402BD990(task, zone, pc, op, tool, slot).
    /// Op 0 = pick (slot), op 1 = use (tool ID).
    StrategicTool { op: u8, tool: u64, slot: u8 },
    /// 1587, kind 249, vtable 0x140BAA8F0. Bot DWORD +144, level byte +148.
    TrainingRoomPick { bot: u32, level: u8 },
    /// 1590, kind 250, vtable 0x140BC1578. Bot object ID qword +144.
    TrainingRoomStart { bot: u64 },
    /// 1594, kind 262, vtable 0x140B1CD90. Bytes +144/+145/+146.
    /// Queued with 0x140603BD0 on the zone owner, not 0x140254FD0.
    TrainingRoomReward { a: u8, b: u8, c: u8 },
}
impl From<DuelTask> for SkillTaskKind {
    fn from(task: DuelTask) -> Self {
        SkillTaskKind::Duel(task)
    }
}

pub(super) fn dispatch_duel_ranking(ctx: &mut Context<'_>, command: u16, req: &[u8]) -> Outcome {
    match command {
        ids::G_DuelChooseDuelOrderReqCmd => handle_duel_choose_duel_order(ctx, req),
        ids::G_DuelReadyRoundReqCmd => handle_duel_ready_round(ctx, req),
        ids::G_DuelTakeTagTokenReqCmd => handle_duel_take_tag_token(ctx, req),
        ids::G_PartyBattleReadyReqCmd => handle_party_battle_ready(ctx, req),
        ids::G_PartyBattleStartReqCmd => handle_party_battle_start(ctx, req),
        ids::G_PartyBattleVoteCastReqCmd => handle_party_battle_vote_cast(ctx, req),
        ids::G_DuelBotChallengeCombatStartReqCmd => {
            handle_duel_bot_challenge_combat_start(ctx, req)
        }
        ids::G_DuelBotChallengePickStrategicToolReqCmd => {
            handle_duel_bot_challenge_pick_strategic_tool(ctx, req)
        }
        ids::G_DuelBotChallengeUseStrategicToolReqCmd => {
            handle_duel_bot_challenge_use_strategic_tool(ctx, req)
        }
        ids::G_DuelBotTrainingRoomPickDuelBotReqCmd => {
            handle_duel_bot_training_room_pick_duel_bot(ctx, req)
        }
        ids::G_DuelBotTrainingRoomCombatStartReqCmd => {
            handle_duel_bot_training_room_combat_start(ctx, req)
        }
        ids::G_DuelBotTrainingRoomRewardReqCmd => handle_duel_bot_training_room_reward(ctx, req),
        _ => unreachable!("membership checked"),
    }
}

fn logged(mut out: Outcome, text: &'static str) -> Outcome {
    out.logs.push(text);
    out
}
fn zone_task(ctx: &mut Context<'_>, command: u16, fail: u16, kind: DuelTask) -> Outcome {
    let Some(zone) = ctx.pc().zone else {
        return logged(Outcome::reason(fail, 2), "duel: zone is null");
    };
    let t = SkillTask {
        command,
        caster: ctx.pc().object_id,
        summon: false,
        zone,
        kind: kind.into(),
    };
    queue(ctx, t, Outcome::reason(fail, 2))
}

/// parity: 277 27-duel-ranking-handlers.md#277--g_duelchooseduelorderreqcmd--partial partial
pub fn handle_duel_choose_duel_order(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let Some(order) = Reader(req).byte() else {
        return logged(Outcome::reason(279, 2), "duel order: decompose failed");
    };
    if order as i8 <= 0 {
        return logged(Outcome::reason(279, 898), "duel order: invalid order");
    }
    zone_task(ctx, 277, 279, DuelTask::ChooseDuelOrder { order })
}

/// parity: 280 27-duel-ranking-handlers.md#280--g_duelreadyroundreqcmd--partial partial
pub fn handle_duel_ready_round(ctx: &mut Context<'_>, _req: &[u8]) -> Outcome {
    // Decode "h" only: a body-only request cannot fail here.
    zone_task(ctx, 280, 282, DuelTask::Duel { mode: 1 })
}

/// parity: 283 27-duel-ranking-handlers.md#283--g_dueltaketagtokenreqcmd--partial partial
pub fn handle_duel_take_tag_token(ctx: &mut Context<'_>, _req: &[u8]) -> Outcome {
    zone_task(ctx, 283, 285, DuelTask::Duel { mode: 3 })
}

/// parity: 289 27-duel-ranking-handlers.md#289--g_partybattlereadyreqcmd--partial partial
pub fn handle_party_battle_ready(ctx: &mut Context<'_>, _req: &[u8]) -> Outcome {
    zone_task(ctx, 289, 291, DuelTask::PartyBattle { mode: 0 })
}

/// parity: 292 27-duel-ranking-handlers.md#292--g_partybattlestartreqcmd--partial partial
pub fn handle_party_battle_start(ctx: &mut Context<'_>, _req: &[u8]) -> Outcome {
    zone_task(ctx, 292, 294, DuelTask::PartyBattle { mode: 1 })
}

/// parity: 1022 27-duel-ranking-handlers.md#1022--g_partybattlevotecastreqcmd--partial partial
pub fn handle_party_battle_vote_cast(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_PartyBattleVoteCastFailCmd;
    let Some(target) = Reader(req).quad() else {
        return logged(Outcome::reason(F, 2), "party battle vote: decompose failed");
    };
    if ctx.pc().zone.is_none() {
        return logged(Outcome::reason(F, 2), "party battle vote: zone is null");
    }
    // PC word +46 (caller ID type) and target ID type must both be 1 (PC).
    if ctx.pc().object_id >> 48 != 1 || target >> 48 != 1 {
        return logged(Outcome::reason(F, 2), "party battle vote: invalid vote");
    }
    zone_task(ctx, 1022, F, DuelTask::PartyBattleVote { target })
}

/// PC signed byte +532 > 0 (any GM grade) blocks duel-bot combat.
fn is_gm(pc: &Pc) -> bool {
    pc.gm_grade as i8 > 0
}

/// parity: 968 27-duel-ranking-handlers.md#968--g_duelbotchallengecombatstartreqcmd--partial partial
pub fn handle_duel_bot_challenge_combat_start(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    if is_gm(ctx.pc()) {
        return logged(Outcome::reason(970, 2), "duel bot challenge: gm");
    }
    let Some(bot) = Reader(req).quad() else {
        return logged(
            Outcome::reason(970, 2),
            "duel bot challenge: decompose failed",
        );
    };
    zone_task(ctx, 968, 970, DuelTask::DuelBotChallengeStart { bot })
}

/// parity: 973 27-duel-ranking-handlers.md#973--g_duelbotchallengepickstrategictoolreqcmd--partial partial
pub fn handle_duel_bot_challenge_pick_strategic_tool(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let Some(slot) = Reader(req).byte() else {
        return logged(Outcome::reason(975, 2), "strategic tool: decompose failed");
    };
    let kind = DuelTask::StrategicTool {
        op: 0,
        tool: 0,
        slot,
    };
    zone_task(ctx, 973, 975, kind)
}

/// parity: 977 27-duel-ranking-handlers.md#977--g_duelbotchallengeusestrategictoolreqcmd--partial partial
pub fn handle_duel_bot_challenge_use_strategic_tool(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let Some(tool) = Reader(req).quad() else {
        return logged(Outcome::reason(979, 2), "strategic tool: decompose failed");
    };
    let kind = DuelTask::StrategicTool {
        op: 1,
        tool,
        slot: 0,
    };
    zone_task(ctx, 977, 979, kind)
}

/// parity: 1587 27-duel-ranking-handlers.md#1587--g_duelbottrainingroompickduelbotreqcmd--partial partial
pub fn handle_duel_bot_training_room_pick_duel_bot(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_DuelBotTrainingRoomPickDuelBotFailCmd;
    let mut r = Reader(req);
    let (Some(bot), Some(level)) = (r.int(), r.byte()) else {
        return logged(
            Outcome::reason(F, 2251),
            "training room pick: decompose failed",
        );
    };
    if ctx.pc().zone.is_none() {
        return logged(Outcome::reason(F, 2), "training room pick: zone is null");
    }
    if ctx.pc().object_id >> 48 != 1 || !(1..=14).contains(&bot) || !(1..=10).contains(&level) {
        return logged(Outcome::reason(F, 2), "training room pick: invalid request");
    }
    zone_task(ctx, 1587, F, DuelTask::TrainingRoomPick { bot, level })
}

/// parity: 1590 27-duel-ranking-handlers.md#1590--g_duelbottrainingroomcombatstartreqcmd--partial partial
pub fn handle_duel_bot_training_room_combat_start(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_DuelBotTrainingRoomCombatStartFailCmd;
    if is_gm(ctx.pc()) {
        return logged(Outcome::reason(F, 2), "training room start: gm");
    }
    let Some(bot) = Reader(req).quad() else {
        return logged(
            Outcome::reason(F, 2),
            "training room start: decompose failed",
        );
    };
    if ctx.pc().zone.is_none() {
        return logged(Outcome::reason(F, 2), "training room start: zone is null");
    }
    // Caller ID type 1 (PC); target ID type 240 (duel bot).
    if ctx.pc().object_id >> 48 != 1 || bot >> 48 != 240 {
        return logged(Outcome::reason(F, 2), "training room start: invalid bot");
    }
    zone_task(ctx, 1590, F, DuelTask::TrainingRoomStart { bot })
}

/// parity: 1594 27-duel-ranking-handlers.md#1594--g_duelbottrainingroomrewardreqcmd--partial partial
pub fn handle_duel_bot_training_room_reward(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let mut r = Reader(req);
    let (Some(a), Some(b), Some(c)) = (r.byte(), r.byte(), r.byte()) else {
        return logged(
            Outcome::reason(1596, 2),
            "training room reward: decompose failed",
        );
    };
    zone_task(ctx, 1594, 1596, DuelTask::TrainingRoomReward { a, b, c })
}
