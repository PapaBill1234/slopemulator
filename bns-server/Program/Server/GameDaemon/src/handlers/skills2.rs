//! Doc 13 part 2: faction, manipulate/refiner/fish, summon management, skill
//! score, energy, pet food, training room, combat reset, skill skin and auto
//! combat entry functions. Build 381.8867.
//!
//! Entry functions decode, gate and queue a `SkillTask` with a `CombatTask`
//! kind through `Services::queue_skill`. Command 83 has an explicit task
//! rejection prefix. Its accepted mutations and the other task bodies remain
//! adapter boundaries.
use super::items2::u0_passes;
use super::skills::{pc_task, queue, UNCHECKED_DECODE};
use super::*;

pub const SKILL2_COMMANDS: &[u16] = &[
    ids::G_ExchangeFactionScoreReqCmd,
    ids::G_JoinFactionReqCmd,
    ids::G_LeaveFactionReqCmd,
    ids::G_TransferFactionReqCmd,
    ids::G_RetrieveFactionStatisticsReqCmd,
    ids::G_ManipulateReqCmd,
    ids::G_GiveFactionScoreToRefinerReqCmd,
    ids::G_TakeRefinerRewardReqCmd,
    ids::G_NpcManipulateReqCmd,
    ids::G_FishManipulateReqCmd,
    ids::G_ChangeSummonedReqCmd,
    ids::G_ChangeSummonedNameReqCmd,
    ids::G_KillSummonedReqCmd,
    ids::G_SkillScoreReqCmd,
    ids::G_SetEnergyMultipleValueReqCmd,
    ids::G_OpenEnergyPointRewardReqCmd,
    ids::G_FeedPetFoodReqCmd,
    ids::G_SkillTrainingRoomCombatStartReqCmd,
    ids::G_SkillTrainingRoomCombatStopReqCmd,
    ids::G_SkillTrainingRoomCombatRestartReqCmd,
    ids::G_SkillTrainingRoomInitCreatureStateReqCmd,
    ids::G_EnterSkillTrainingRoomReqCmd,
    ids::G_ResetCombatModeReqCmd,
    ids::G_SkillSkinAcquiredListReqCmd,
    ids::G_ApplySkillSkinReqCmd,
    ids::G_AutoCombatStartReqCmd,
    ids::G_AutoCombatStopReqCmd,
];

/// One `(container:Q, position:h, count:h)` record (native stride 6).
pub type ManipulateRecord = (u8, u16, u16);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CombatTask {
    /// 83, kind 60.
    ExchangeFactionScore { npc: u64, amount: i32 },
    /// 125, kind 20.
    JoinFaction { faction: u8, npc: u64 },
    /// 127, kind 21.
    LeaveFaction { npc: u64 },
    /// 129, kind 22. ❓ Option meaning.
    TransferFaction { faction: u8, npc: u64, option: u8 },
    /// 1253, action class 19. Target object kind 32.
    Manipulate {
        target: u64,
        operation: u16,
        option: u8,
        records: Vec<ManipulateRecord>,
    },
    /// 1255, action class 30.
    GiveFactionScoreToRefiner { target: u64 },
    /// 1258, kind 154.
    TakeRefinerReward { target: u64 },
    /// 1264, action class 20. Target object kind 4.
    NpcManipulate {
        target: u64,
        operation: u16,
        records: Vec<ManipulateRecord>,
    },
    /// 1266 action 0, action class 31.
    FishAction {
        target: u64,
        operation: u16,
        position: [u16; 3],
    },
    /// 1266 actions 1/2, kind 319. Option is the narrowed final byte.
    FishTask { action: u8, option: u8 },
    /// 630, kind 97.
    ChangeSummoned { key: u64, changes: Vec<u8> },
    /// 636, kind 157.
    KillSummoned,
    /// 788, kind 155.
    SkillScore,
    /// 837, kind 354.
    SetEnergyMultiple { multiple: u8 },
    /// 840, kind 355.
    OpenEnergyPointReward { reward: i32, option: u8, value: i16 },
    /// 952, kind 234, scheduled by `0x140603BD0`.
    FeedPetFood {
        selector: u8,
        shorts: [u16; 2],
        option: u8,
    },
    /// 1026, kind 265.
    TrainingRoomCombatStart { id: i32, option: u8 },
    /// 1029, kind 266.
    TrainingRoomCombatStop { option: u8 },
    /// 1032, kind 267.
    TrainingRoomCombatRestart,
    /// 1035, kind 273.
    TrainingRoomInitCreatureState,
    /// 1038, kind 271.
    EnterSkillTrainingRoom,
    /// 1053, kind 288.
    ResetCombatMode,
    /// 1062, kind 290.
    SkillSkinAcquiredList,
    /// 1065, kind 291.
    ApplySkillSkin { skin: i16 },
    /// 1208, kind 365, scheduled by `0x140603BD0`.
    AutoCombatStart { option: u8 },
    /// 1209, kind 366, scheduled by `0x140603BD0`.
    AutoCombatStop { option: u8, reason: u8 },
}
/// Live attached-PC state for command 83. The NPC lookup uses the task's zone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExchangeFactionState {
    pub position: [i16; 3],
    /// Signed PC DWORD +552, not an amount supplied by the client.
    pub score: i32,
    pub npc: SkillLookup<ExchangeFactionNpc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExchangeFactionNpc {
    pub native_kind: u16,
    pub position: [i16; 3],
    pub record: Option<ExchangeFactionRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExchangeFactionRecord {
    /// NPC record byte +228.
    pub enabled: bool,
    /// NPC record signed short +230 and signed byte +232.
    pub maximum_level: i16,
    pub multiplier: i8,
}

impl From<CombatTask> for SkillTaskKind {
    fn from(task: CombatTask) -> Self {
        SkillTaskKind::Combat(task)
    }
}

/// parity: 636 13-skill-handlers.md#636--g_killsummonedreqcmd--partial partial
/// parity: 788 13-skill-handlers.md#788--g_skillscorereqcmd--partial partial
/// parity: 1032 13-skill-handlers.md#1032--g_skilltrainingroomcombatrestartreqcmd--partial partial
/// parity: 1035 13-skill-handlers.md#1035--g_skilltrainingroominitcreaturestatereqcmd--partial partial
/// parity: 1038 13-skill-handlers.md#1038--g_enterskilltrainingroomreqcmd--partial partial
/// parity: 1053 13-skill-handlers.md#1053--g_resetcombatmodereqcmd--partial partial
/// These six use `pc_task` (zone, PC kind 1 with log, queue; reason 2). The
/// command-only decoders of 1032/1035/1038/1053 cannot fail here.
pub(super) fn dispatch_skill2(ctx: &mut Context<'_>, command: u16, req: &[u8]) -> Outcome {
    use CombatTask as T;
    match command {
        ids::G_ExchangeFactionScoreReqCmd => handle_exchange_faction_score(ctx, req),
        ids::G_JoinFactionReqCmd => handle_join_faction(ctx, req),
        ids::G_LeaveFactionReqCmd => handle_leave_faction(ctx, req),
        ids::G_TransferFactionReqCmd => handle_transfer_faction(ctx, req),
        ids::G_RetrieveFactionStatisticsReqCmd => handle_retrieve_faction_statistics(ctx),
        ids::G_ManipulateReqCmd => handle_manipulate(ctx, req),
        ids::G_GiveFactionScoreToRefinerReqCmd => handle_refiner(
            ctx,
            req,
            ids::G_GiveFactionScoreToRefinerReqCmd,
            ids::G_GiveFactionScoreToRefinerFailCmd,
        ),
        ids::G_TakeRefinerRewardReqCmd => handle_refiner(
            ctx,
            req,
            ids::G_TakeRefinerRewardReqCmd,
            ids::G_TakeRefinerRewardFailCmd,
        ),
        ids::G_NpcManipulateReqCmd => handle_npc_manipulate(ctx, req),
        ids::G_FishManipulateReqCmd => handle_fish_manipulate(ctx, req),
        ids::G_ChangeSummonedReqCmd => handle_change_summoned(ctx, req),
        ids::G_ChangeSummonedNameReqCmd => handle_change_summoned_name(ctx, req),
        ids::G_KillSummonedReqCmd => pc_task(
            ctx,
            ids::G_KillSummonedReqCmd,
            ids::G_KillSummonedFailCmd,
            2,
            T::KillSummoned.into(),
        ),
        ids::G_SkillScoreReqCmd => pc_task(
            ctx,
            ids::G_SkillScoreReqCmd,
            ids::G_SkillScoreFailCmd,
            2,
            T::SkillScore.into(),
        ),
        ids::G_SetEnergyMultipleValueReqCmd => handle_set_energy_multiple(ctx, req),
        ids::G_OpenEnergyPointRewardReqCmd => handle_open_energy_point_reward(ctx, req),
        ids::G_FeedPetFoodReqCmd => handle_feed_pet_food(ctx, req),
        ids::G_SkillTrainingRoomCombatStartReqCmd => handle_training_combat_start(ctx, req),
        ids::G_SkillTrainingRoomCombatStopReqCmd => handle_training_combat_stop(ctx, req),
        ids::G_SkillTrainingRoomCombatRestartReqCmd => pc_task(
            ctx,
            ids::G_SkillTrainingRoomCombatRestartReqCmd,
            ids::G_SkillTrainingRoomCombatRestartFailCmd,
            2,
            T::TrainingRoomCombatRestart.into(),
        ),
        ids::G_SkillTrainingRoomInitCreatureStateReqCmd => pc_task(
            ctx,
            ids::G_SkillTrainingRoomInitCreatureStateReqCmd,
            ids::G_SkillTrainingRoomInitCreatureStateFailCmd,
            2,
            T::TrainingRoomInitCreatureState.into(),
        ),
        ids::G_EnterSkillTrainingRoomReqCmd => pc_task(
            ctx,
            ids::G_EnterSkillTrainingRoomReqCmd,
            ids::G_EnterSkillTrainingRoomFailCmd,
            2,
            T::EnterSkillTrainingRoom.into(),
        ),
        ids::G_ResetCombatModeReqCmd => pc_task(
            ctx,
            ids::G_ResetCombatModeReqCmd,
            ids::G_ResetCombatModeFailCmd,
            2,
            T::ResetCombatMode.into(),
        ),
        ids::G_SkillSkinAcquiredListReqCmd => handle_skill_skin_acquired_list(ctx),
        ids::G_ApplySkillSkinReqCmd => handle_apply_skill_skin(ctx, req),
        ids::G_AutoCombatStartReqCmd => handle_auto_combat_start(ctx, req),
        ids::G_AutoCombatStopReqCmd => handle_auto_combat_stop(ctx, req),
        _ => unreachable!("membership checked"),
    }
}

fn task(ctx: &Context<'_>, command: u16, zone: u64, kind: CombatTask) -> SkillTask {
    SkillTask {
        command,
        caster: ctx.pc().object_id,
        summon: false,
        zone,
        kind: kind.into(),
    }
}

/// Reply plus the native assert/log.
fn reason_log(command: u16, reason: u16, log: &'static str) -> Outcome {
    let mut out = Outcome::reason(command, reason);
    out.logs.push(log);
    out
}

/// Zone (reply + log), optional PC kind 1 (reply + log), then queue.
/// Every failure uses `reason`.
fn zone_kind_queue(
    ctx: &mut Context<'_>,
    command: u16,
    failure: u16,
    reason: u16,
    kind_check: bool,
    kind: CombatTask,
) -> Outcome {
    let Some(zone) = ctx.pc().zone else {
        return reason_log(failure, reason, "skill task: zone is not found");
    };
    if kind_check && ctx.pc().native_kind != 1 {
        return reason_log(failure, reason, "skill task: caster is not a PC");
    }
    let t = task(ctx, command, zone, kind);
    queue(ctx, t, Outcome::reason(failure, reason))
}

/// Object kind: ID bits 48..63.
fn object_kind(id: u64) -> u64 {
    id >> 48
}

// ---- Faction ------------------------------------------------------------------------

/// Common faction T1 (reason 5), then config +2105 when `config` is set.
fn faction_gates(ctx: &Context<'_>, failure: u16, config: bool) -> Option<Outcome> {
    if ctx.world.arena || session::arena_blocked(ctx.pc()) {
        return Some(Outcome::reason(failure, 5));
    }
    if config && !ctx.world.faction_enabled {
        return Some(Outcome::reason(failure, 2));
    }
    None
}

/// parity: 83 13-skill-handlers.md#83--g_exchangefactionscorereqcmd--partial partial
pub fn handle_exchange_faction_score(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_ExchangeFactionScoreFailCmd;
    if let Some(out) = faction_gates(ctx, F, false) {
        return out;
    }
    if !u0_passes(ctx) {
        return Outcome::reason(F, 336);
    }
    let mut r = Reader(req);
    let (Some(npc), Some(amount)) = (r.quad(), r.int()) else {
        return Outcome::unsupported(UNCHECKED_DECODE);
    };
    let kind = CombatTask::ExchangeFactionScore {
        npc,
        amount: amount as i32,
    };
    zone_kind_queue(ctx, 83, F, 1, true, kind)
}

/// parity: 85 13-skill-handlers.md#85--g_exchangefactionscorefailcmd--done done
fn exchange_failure(reason: u16) -> Outcome {
    Outcome::reason(85, reason)
}

/// parity: 83 13-skill-handlers.md#83--g_exchangefactionscorereqcmd--partial partial
pub(super) fn execute_exchange_faction_score(
    ctx: &mut Context<'_>,
    task: &SkillTask,
    amount: i32,
) -> Outcome {
    let Some(state) = ctx.services.skill_completion_state() else {
        return Outcome::unsupported("❓ exchange live attached-PC state");
    };
    let exchange = match &state.exchange {
        SkillLookup::Found(exchange) => exchange,
        SkillLookup::Unknown | SkillLookup::Missing => {
            return Outcome::unsupported(
                "❓ exchange attached-PC pointer is dereferenced natively",
            );
        }
    };
    let npc = match &exchange.npc {
        SkillLookup::Unknown => return Outcome::unsupported("❓ exchange live zone NPC lookup"),
        SkillLookup::Missing => return exchange_failure(333),
        SkillLookup::Found(npc) => npc,
    };
    if npc.native_kind != 4 {
        return exchange_failure(333);
    }
    let Some(record) = &npc.record else {
        return exchange_failure(333);
    };
    if !record.enabled {
        return exchange_failure(333);
    }
    // Copy the remaining live fields before the separate opaque level query.
    let (position, score, npc_position, maximum_level, multiplier) = (
        exchange.position,
        exchange.score,
        npc.position,
        record.maximum_level,
        record.multiplier,
    );
    let Some(level) = ctx.services.skill_faction_level(task) else {
        return Outcome::unsupported("❓ exchange faction-level iterator 0x140260E70");
    };
    if maximum_level > 0 && i32::from(maximum_level) < level {
        return exchange_failure(333);
    }
    let distance_squared: i64 = position
        .into_iter()
        .zip(npc_position)
        .map(|(pc, npc)| {
            let delta = i64::from(pc) - i64::from(npc);
            delta * delta
        })
        .sum();
    if distance_squared > 10_000 {
        return exchange_failure(334);
    }
    if amount <= 0 || amount > score {
        return exchange_failure(335);
    }
    if multiplier <= 0 {
        return exchange_failure(333);
    }
    Outcome::unsupported("❓ exchange property setters, effect removal and delta flush")
}

/// parity: 125 13-skill-handlers.md#125--g_joinfactionreqcmd--partial partial
pub fn handle_join_faction(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    if let Some(out) = faction_gates(ctx, 126, true) {
        return out;
    }
    let mut r = Reader(req);
    let (Some(faction), Some(npc)) = (r.byte(), r.quad()) else {
        return Outcome::unsupported(UNCHECKED_DECODE);
    };
    let kind = CombatTask::JoinFaction { faction, npc };
    zone_kind_queue(ctx, 125, 126, 2, false, kind)
}

/// parity: 127 13-skill-handlers.md#127--g_leavefactionreqcmd--partial partial
pub fn handle_leave_faction(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    if let Some(out) = faction_gates(ctx, 128, true) {
        return out;
    }
    let Some(npc) = Reader(req).quad() else {
        return Outcome::unsupported(UNCHECKED_DECODE);
    };
    zone_kind_queue(ctx, 127, 128, 2, false, CombatTask::LeaveFaction { npc })
}

/// parity: 129 13-skill-handlers.md#129--g_transferfactionreqcmd--partial partial
pub fn handle_transfer_faction(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    if let Some(out) = faction_gates(ctx, 130, true) {
        return out;
    }
    let mut r = Reader(req);
    let (Some(faction), Some(npc), Some(option)) = (r.byte(), r.quad(), r.byte()) else {
        return Outcome::unsupported(UNCHECKED_DECODE);
    };
    let kind = CombatTask::TransferFaction {
        faction,
        npc,
        option,
    };
    zone_kind_queue(ctx, 129, 130, 2, false, kind)
}

/// parity: 131 13-skill-handlers.md#131--g_retrievefactionstatisticsreqcmd--done ported
pub fn handle_retrieve_faction_statistics(ctx: &mut Context<'_>) -> Outcome {
    let calls = ctx.world.faction_statistics_calls;
    let mut out = Outcome::reply(
        132,
        ctx.world
            .faction_statistics
            .iter()
            .flat_map(|q| q.to_le_bytes())
            .collect(),
    );
    let prior = if calls >= 100 {
        out.logs
            .push("retrieve faction statistics: call counter overflow");
        0
    } else {
        calls
    };
    out.changes
        .push(StateChange::FactionStatisticsCalls(prior + 1));
    out
}

// ---- Manipulate, refiner, fish -------------------------------------------------------

/// Signed count 0..=16, all record fields, then pairwise duplicate
/// `(container, position)` rejection.
fn manipulate_records(r: &mut Reader<'_>) -> Option<Vec<ManipulateRecord>> {
    let n = r.short()? as i16;
    if !(0..=16).contains(&n) {
        return None;
    }
    let records = (0..n)
        .map(|_| Some((r.byte()?, r.short()?, r.short()?)))
        .collect::<Option<Vec<_>>>()?;
    for (i, a) in records.iter().enumerate() {
        if records[i + 1..].iter().any(|b| (a.0, a.1) == (b.0, b.1)) {
            return None;
        }
    }
    Some(records)
}

/// Zone, target object kind, optional PC kind 1, then queue. Reason 2.
fn targeted(
    ctx: &mut Context<'_>,
    command: u16,
    failure: u16,
    target: u64,
    target_kind: u64,
    pc_kind: bool,
    kind: CombatTask,
) -> Outcome {
    let fail = Outcome::reason(failure, 2);
    let Some(zone) = ctx.pc().zone else {
        return fail;
    };
    if (pc_kind && ctx.pc().native_kind != 1) || object_kind(target) != target_kind {
        return fail;
    }
    let t = task(ctx, command, zone, kind);
    queue(ctx, t, fail)
}

/// parity: 1253 13-skill-handlers.md#1253--g_manipulatereqcmd--partial partial
pub fn handle_manipulate(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let mut r = Reader(req);
    let decoded = (|| {
        Some((
            r.quad()?,
            r.short()?,
            r.byte()?,
            manipulate_records(&mut r)?,
        ))
    })();
    let Some((target, operation, option, records)) = decoded else {
        return Outcome::reason(1254, 2);
    };
    let kind = CombatTask::Manipulate {
        target,
        operation,
        option,
        records,
    };
    targeted(ctx, 1253, 1254, target, 32, false, kind)
}

/// parity: 1255 13-skill-handlers.md#1255--g_givefactionscoretorefinerreqcmd--partial partial
/// parity: 1258 13-skill-handlers.md#1258--g_takerefinerrewardreqcmd--partial partial
pub fn handle_refiner(ctx: &mut Context<'_>, req: &[u8], command: u16, failure: u16) -> Outcome {
    let Some(target) = Reader(req).quad() else {
        return Outcome::unsupported(UNCHECKED_DECODE);
    };
    let kind = if command == 1255 {
        CombatTask::GiveFactionScoreToRefiner { target }
    } else {
        CombatTask::TakeRefinerReward { target }
    };
    targeted(ctx, command, failure, target, 32, true, kind)
}

/// parity: 1264 13-skill-handlers.md#1264--g_npcmanipulatereqcmd--partial partial
pub fn handle_npc_manipulate(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    if !u0_passes(ctx) {
        return Outcome::reason(1265, 676);
    }
    let mut r = Reader(req);
    let decoded = (|| Some((r.quad()?, r.short()?, manipulate_records(&mut r)?)))();
    let Some((target, operation, records)) = decoded else {
        return Outcome::reason(1265, 2);
    };
    let kind = CombatTask::NpcManipulate {
        target,
        operation,
        records,
    };
    targeted(ctx, 1264, 1265, target, 4, false, kind)
}

/// parity: 1266 13-skill-handlers.md#1266--g_fishmanipulatereqcmd--partial partial
pub fn handle_fish_manipulate(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_FishManipulateFailCmd;
    if !ctx.world.fish_enabled {
        return Outcome::reason(F, 2);
    }
    let mut r = Reader(req);
    let decoded = (|| {
        let target = r.quad()?;
        let position = [r.short()?, r.short()?, r.short()?];
        Some((target, position, r.byte()?, r.short()?, r.byte()?))
    })();
    let Some((target, position, action, operation, option)) = decoded else {
        return Outcome::reason(F, 2);
    };
    let Some(zone) = ctx.pc().zone else {
        return Outcome::reason(F, 2);
    };
    let kind = match action {
        0 => {
            if ctx.pc().native_kind != 1 {
                return Outcome::reason(F, 2);
            }
            CombatTask::FishAction {
                target,
                operation,
                position,
            }
        }
        1 | 2 => {
            if ctx.pc().native_kind != 1 {
                return reason_log(F, 2, "fish manipulate: caster is not a PC");
            }
            CombatTask::FishTask { action, option }
        }
        _ => return Outcome::log("fish manipulate: unknown action"),
    };
    let t = task(ctx, 1266, zone, kind);
    queue(ctx, t, Outcome::reason(F, 2))
}

// ---- Summon management -----------------------------------------------------------------

/// parity: 630 13-skill-handlers.md#630--g_changesummonedreqcmd--partial partial
pub fn handle_change_summoned(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_ChangeSummonedFailCmd;
    let mut r = Reader(req);
    let (Some(key), Some(n)) = (r.quad(), r.short()) else {
        return Outcome::reason(F, 1331);
    };
    if !(0..=6).contains(&(n as i16)) {
        return Outcome::reason(1342, 1337);
    }
    let Some(changes) = (0..n).map(|_| r.byte()).collect::<Option<Vec<_>>>() else {
        return Outcome::reason(F, 1331);
    };
    let Some(zone) = ctx.pc().zone else {
        return Outcome::reason(F, 1);
    };
    if ctx.pc().job != 6 {
        return Outcome::reason(F, 1332);
    }
    let t = task(ctx, 630, zone, CombatTask::ChangeSummoned { key, changes });
    queue(ctx, t, Outcome::reason(F, 1))
}

/// parity: 633 13-skill-handlers.md#633--g_changesummonednamereqcmd--partial partial
pub fn handle_change_summoned_name(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_ChangeSummonedNameFailCmd;
    if !ctx.world.summon_rename_enabled {
        return Outcome::reason(F, 1342);
    }
    let mut r = Reader(req);
    if r.string(21).is_none() {
        return Outcome::reason(F, 1342);
    }
    let Some(n) = r.short() else {
        return Outcome::reason(F, 1342);
    };
    if !(0..=15).contains(&(n as i16)) {
        return Outcome::reason(F, 1346);
    }
    let pairs = (0..n)
        .map(|_| Some((r.short()?, r.short()?)))
        .collect::<Option<Vec<_>>>();
    match pairs {
        Some(p) if p.iter().all(|&(_, second)| second != 0) => {}
        _ => return Outcome::reason(F, 1342),
    }
    if ctx.pc().zone.is_none() {
        return Outcome::reason(F, 1);
    }
    if ctx.pc().life_state != 0 {
        return Outcome::reason(F, 2);
    }
    if ctx.pc().job != 6 {
        return Outcome::reason(F, 1343);
    }
    Outcome::unsupported("❓ summon name service (daemon +2728 +48) and name predicate 0x14012F9F0")
}

// ---- Energy, pet food ---------------------------------------------------------------------

/// parity: 837 13-skill-handlers.md#837--g_setenergymultiplevaluereqcmd--partial partial
pub fn handle_set_energy_multiple(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_SetEnergyMultipleValueFailCmd;
    if !ctx.world.energy_enabled {
        return Outcome::reason(F, 2614);
    }
    let Some(multiple) = Reader(req).byte() else {
        return Outcome::reason(F, 2);
    };
    if !(2..=63).contains(&multiple) {
        return Outcome::reason(F, 2615);
    }
    let Some(mask) = ctx.world.energy_multiple_mask else {
        return Outcome::unsupported("❓ DataManager slot-10309 energy multiple table");
    };
    if mask & (1u64 << multiple) == 0 {
        return Outcome::reason(F, 2615);
    }
    let kind = CombatTask::SetEnergyMultiple { multiple };
    zone_kind_queue(ctx, 837, F, 2, false, kind)
}

/// 842 `hEQi`: E = 1, Q = code (one byte), i = reward ID. Only code 2 is
/// sent; the encoder treatment of 2618/2619/2620 is open.
fn energy_reward_fail(code: u16, reward: i32) -> Outcome {
    if code != 2 {
        return Outcome::unsupported("❓ 842 Q encoder for codes 2618/2619/2620");
    }
    let mut body = 1u16.to_le_bytes().to_vec();
    body.push(code as u8);
    body.extend(reward.to_le_bytes());
    Outcome::reply(842, body)
}

/// parity: 840 13-skill-handlers.md#840--g_openenergypointrewardreqcmd--partial partial
pub fn handle_open_energy_point_reward(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    if !ctx.world.energy_enabled {
        return energy_reward_fail(2618, 0);
    }
    let mut r = Reader(req);
    let decoded = (|| Some((r.int()? as i32, r.byte()?, r.short()? as i16)))();
    let Some((reward, option, value)) = decoded else {
        return energy_reward_fail(2, 0);
    };
    if option > 8 {
        return energy_reward_fail(2619, reward);
    }
    if value < 0 {
        return energy_reward_fail(2620, reward);
    }
    let Some(zone) = ctx.pc().zone else {
        return energy_reward_fail(2, reward);
    };
    let kind = CombatTask::OpenEnergyPointReward {
        reward,
        option,
        value,
    };
    let t = task(ctx, 840, zone, kind);
    queue(ctx, t, energy_reward_fail(2, reward))
}

/// Scheduler `0x140603BD0`: its return is ignored or only logged.
fn schedule(ctx: &mut Context<'_>, t: SkillTask, fail_log: Option<&'static str>) -> Outcome {
    let mut out = queue(ctx, t, Outcome::default());
    if out == Outcome::default() {
        if let Some(log) = fail_log {
            out.logs.push(log);
        }
    }
    out
}

/// parity: 952 13-skill-handlers.md#952--g_feedpetfoodreqcmd--partial partial
pub fn handle_feed_pet_food(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_FeedPetFoodFailCmd;
    let mut r = Reader(req);
    let decoded = (|| Some((r.byte()?, [r.short()?, r.short()?], r.byte()?)))();
    let Some((selector, shorts, option)) = decoded else {
        return Outcome::reason(F, 2);
    };
    let Some(zone) = ctx.pc().zone else {
        return Outcome::reason(F, 2);
    };
    if ctx.pc().native_kind != 1 {
        return reason_log(F, 2, "feed pet food: caster is not a PC");
    }
    let kind = CombatTask::FeedPetFood {
        selector,
        shorts,
        option,
    };
    let t = task(ctx, 952, zone, kind);
    schedule(ctx, t, None)
}

// ---- Training room ----------------------------------------------------------------------

/// parity: 1026 13-skill-handlers.md#1026--g_skilltrainingroomcombatstartreqcmd--partial partial
pub fn handle_training_combat_start(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let mut r = Reader(req);
    let (Some(id), Some(option)) = (r.int(), r.byte()) else {
        return Outcome::reason(1028, 2324);
    };
    let kind = CombatTask::TrainingRoomCombatStart {
        id: id as i32,
        option,
    };
    pc_task(ctx, 1026, 1028, 2, kind.into())
}

/// parity: 1029 13-skill-handlers.md#1029--g_skilltrainingroomcombatstopreqcmd--partial partial
pub fn handle_training_combat_stop(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let Some(option) = Reader(req).byte() else {
        return Outcome::reason(1031, 2329);
    };
    let kind = CombatTask::TrainingRoomCombatStop { option };
    pc_task(ctx, 1029, 1031, 2, kind.into())
}

// ---- Skill skin ------------------------------------------------------------------------

/// parity: 1062 13-skill-handlers.md#1062--g_skillskinacquiredlistreqcmd--partial partial
pub fn handle_skill_skin_acquired_list(ctx: &mut Context<'_>) -> Outcome {
    if !ctx.world.skill_skin_enabled {
        return Outcome::reason(1064, 2378);
    }
    let Some(zone) = ctx.pc().zone else {
        return Outcome::reason(1064, 2);
    };
    let t = task(ctx, 1062, zone, CombatTask::SkillSkinAcquiredList);
    queue(ctx, t, Outcome::reason(1064, 2))
}

/// parity: 1065 13-skill-handlers.md#1065--g_applyskillskinreqcmd--partial partial
pub fn handle_apply_skill_skin(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_ApplySkillSkinFailCmd;
    if !ctx.world.skill_skin_enabled {
        return Outcome::reason(F, 2382);
    }
    let Some(skin) = Reader(req).short().map(|s| s as i16) else {
        return Outcome::reason(F, 2);
    };
    if skin < 0 {
        return Outcome::reason(F, 2);
    }
    if skin == ctx.pc().current_skin {
        return Outcome::reason(F, 2384);
    }
    let Some(zone) = ctx.pc().zone else {
        return Outcome::reason(F, 2);
    };
    let t = task(ctx, 1065, zone, CombatTask::ApplySkillSkin { skin });
    queue(ctx, t, Outcome::reason(F, 2))
}

// ---- Auto combat -----------------------------------------------------------------------

/// Decode, zone and scheduler failures log only.
fn auto_combat(ctx: &mut Context<'_>, command: u16, kind: Option<CombatTask>) -> Outcome {
    let Some(kind) = kind else {
        return Outcome::log("auto combat: decode failed");
    };
    let Some(zone) = ctx.pc().zone else {
        return Outcome::log("auto combat: zone is not found");
    };
    let t = task(ctx, command, zone, kind);
    schedule(ctx, t, Some("auto combat: schedule failed"))
}

/// parity: 1208 13-skill-handlers.md#1208--g_autocombatstartreqcmd--partial partial
pub fn handle_auto_combat_start(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let kind = Reader(req)
        .byte()
        .map(|option| CombatTask::AutoCombatStart { option });
    auto_combat(ctx, 1208, kind)
}

/// parity: 1209 13-skill-handlers.md#1209--g_autocombatstopreqcmd--partial partial
pub fn handle_auto_combat_stop(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let mut r = Reader(req);
    let kind = (|| {
        Some(CombatTask::AutoCombatStop {
            option: r.byte()?,
            reason: r.byte()?,
        })
    })();
    auto_combat(ctx, 1209, kind)
}
