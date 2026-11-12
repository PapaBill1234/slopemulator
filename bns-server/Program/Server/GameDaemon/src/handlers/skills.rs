//! Doc 13 part 1: skill use, cancel/trigger, gather/loot, summon recall,
//! exhaustion/respawn, vitality, skill list/pages, traits and mastery entry
//! functions. Build 381.8867.
//!
//! Entry functions decode, gate and queue a `SkillTask` through
//! `Services::queue_skill`. Retained tasks run through `execute_skill_task`.
//! Unread actions and accepted mutations stop at explicit adapter boundaries.
//! Coordinates (`u`) decode as shorts, as in doc 11 and the doc 13 notation.
use super::items::reason_u64;
use super::items2::{pairs, u0_passes};
use super::*;

pub const SKILL_COMMANDS: &[u16] = &[
    ids::G_UseSkillReqCmd,
    ids::G_UseSkillToGroundReqCmd,
    ids::G_CancelActionReqCmd,
    ids::G_SkillTriggerReqCmd,
    ids::G_GatherReqCmd,
    ids::G_GatherCancelReqCmd,
    ids::G_LootReqCmd,
    ids::G_RecallSummonReqCmd,
    ids::G_SpecialSkillGaugeFullReqCmd,
    ids::G_SelectTargetReqCmd,
    ids::G_DieInExhaustionReqCmd,
    ids::G_RestorationInExhaustionReqCmd,
    ids::G_HelpRestorationReqCmd,
    ids::G_RespawnReqCmd,
    ids::G_SaveVitalityReqCmd,
    ids::G_RechargeVitalityReqCmd,
    ids::G_SkillList2ReqCmd,
    ids::G_SetMasteryStatPointReqCmd,
    ids::G_SkillPageTotalReqCmd,
    ids::G_ChangeSkillPageReqCmd,
    ids::G_ExpandSkillPageReqCmd,
    ids::G_RenameSkillPageReqCmd,
    ids::G_TrainTraitReqCmd,
    ids::G_UnlockMasteryLevelReqCmd,
];

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SkillModifiers {
    /// 0..=8 equip positions.
    pub equip_positions: Vec<u8>,
    /// 0..=38 set-item keys.
    pub set_item_keys: Vec<u64>,
    /// 0..=4 item-skill equip positions.
    pub item_skill_positions: Vec<u8>,
    /// 0..=26 badge equip positions.
    pub badge_positions: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SkillTaskKind {
    /// 1226: action factory `0x1404BE5A0`, init `0x1404988A0` mode 1.
    UseSkill {
        skill: u32,
        variation: u8,
        target: u64,
        request: u16,
        item_selector: u8,
        modifiers: SkillModifiers,
    },
    /// 1228: init `0x140498D00` mode 2.
    UseSkillToGround {
        skill: u32,
        variation: u8,
        position: [u16; 3],
        request: u16,
        item_selector: u8,
        modifiers: SkillModifiers,
    },
    /// 1229: option DWORD +144.
    CancelAction {
        option: u8,
    },
    /// 1231, kind 13. The task is a no-op in this build.
    SkillTrigger {
        trigger: u8,
    },
    /// 1270, action class 15.
    Gather {
        target: u64,
    },
    /// 77, kind 76.
    GatherCancel,
    /// 1272, action class 14.
    Loot {
        target: u64,
    },
    /// 71, action class 50 in the summon zone. Yaw is not copied.
    RecallSummon {
        position: [u16; 3],
    },
    /// 74, kind 272. The PC scheduler ignores the submit result.
    SpecialSkillGaugeFull,
    /// 134, kind 11.
    SelectTarget {
        target: u64,
    },
    /// 135, kind 15.
    DieInExhaustion,
    /// 137, action class 16.
    RestorationInExhaustion,
    /// 139, action class 17.
    HelpRestoration {
        target: u64,
    },
    /// 142, init `0x140291520`.
    Respawn {
        option: u8,
    },
    /// 824, kind 119.
    RechargeVitality,
    /// 113, kind 82.
    SkillList,
    /// 117, kind 269.
    SetMasteryStatPoint {
        shorts: [i16; 2],
        bytes: [i8; 5],
    },
    /// 1485, kind 153.
    SkillPageTotal,
    /// 1488, kind 150.
    ChangeSkillPage {
        page: u8,
    },
    /// 1491, kind 152.
    ExpandSkillPage {
        page: u8,
        name: Vec<u16>,
        pairs: Vec<(u16, u16)>,
    },
    /// 1494, kind 151.
    RenameSkillPage {
        page: u8,
        name: Vec<u16>,
    },
    /// 1497, kind 128. `build_up[0]` is forced to 1.
    TrainTrait {
        selectors: [u8; 2],
        build_up: [u8; 6],
        traits: Vec<(u32, i8)>,
    },
    /// 1389, action class 29.
    UnlockMasteryLevel {
        pairs: Vec<(u16, u16)>,
    },
    /// Doc 13 part 2 (`skills2.rs`).
    Combat(CombatTask),
    /// Doc 16 chat/social zone tasks.
    Chat(ChatTask),
    /// Doc 17 mentoring zone tasks.
    Mentoring(MentoringTask),
    /// Doc 18 challenge-party zone tasks.
    ChallengeParty(ChallengeTask),
    /// Doc 19 misc zone tasks.
    Misc(MiscTask),
    /// Doc 29 NPC trade and dungeon-step zone tasks.
    NpcTrade(NpcTradeTask),
    Season(SeasonTask),
    /// Doc 25 arena lobby zone tasks.
    ArenaLobby(ArenaLobbyTask),
    ArenaEvent(ArenaEventTask),
    /// Doc 27 duel and party-battle zone tasks.
    Duel(DuelTask),
    /// Doc 24 market zone actions and tasks.
    Market(MarketTask),
    /// Doc 23 post task type. Reserved: native post entries use account transactions.
    Post(PostTask),
    /// Doc 22 shop and store zone tasks.
    Shop(ShopTask),
    /// Doc 20 quest, discovery and survey zone tasks.
    Quest(QuestTask),
    /// Doc 21 quest progress zone tasks.
    QuestProgress(QuestProgressTask),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillTask {
    pub command: u16,
    /// PC object ID (the owner when `summon` is true).
    pub caster: u64,
    /// The controlled summon is the selected creature.
    pub summon: bool,
    pub zone: u64,
    pub kind: SkillTaskKind,
}

/// Server-owned lookup. A missing object is not the same as unknown state.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SkillLookup<T> {
    #[default]
    Unknown,
    Missing,
    Found(T),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecallActionState {
    /// Current action-manager virtual +88(0), action short +146.
    pub current_action_class: u16,
    /// Live summon-owner reply route. None is not an assumed requester route.
    pub reply_owner: Option<u64>,
    /// Live owner +6968, not the summon position or the requester's wire data.
    pub owner_position: Option<[i16; 3]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecallGeometry {
    /// Signed geometry result byte +10.
    pub environment: i8,
    /// Positive environment: the zone pointer must exist and virtual +232
    /// must return zero. Environment zero does not use these two fields.
    pub environment_exists: bool,
    pub environment_blocked: bool,
    pub ground_z: i16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpecialGaugeState {
    /// Signed PC byte +276 and global signed DWORD +2672.
    pub level: i8,
    pub minimum_level: i32,
}

/// The fields apply to the retired gather action after operation replacement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatherCancelState {
    /// PC action queue +24, action short +146. Class 1 is the idle singleton.
    pub operation_class: u16,
    /// Cancelled action +112. This need not equal the cancel task's zone.
    pub action_zone: u64,
    pub action_flags: u8,
    pub parent_attached: bool,
    pub timer_present: bool,
    pub timer_scheduler_active: bool,
    pub timer_linked: bool,
    pub target: u64,
    pub broadcast: SkillLookup<GatherCancelBroadcast>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatherCancelBroadcast {
    pub zone_has_creatures: bool,
    /// Controlled creature (native kind 1..3) resolved by caster ID in the
    /// cancelled action's zone. Do not assume that this lookup finds the PC.
    pub caster_found: bool,
    /// Config +1530 !=0 and config +792 ==1.
    pub personal_copy_enabled: bool,
    pub personal_buffer_present: bool,
    /// Server geometry-derived broadcast cell and PC byte +39836.
    pub cell: u32,
    pub visibility: u8,
}

/// Buffered native action events. These are not immediate network sends.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SkillEffect {
    GatherCancelled,
    AppendPersonalAction {
        caster: u64,
        payload: Vec<u8>,
    },
    AppendZoneAction {
        zone: u64,
        cell: u32,
        event_kind: u32,
        visibility: u8,
        payload: Vec<u8>,
    },
    GatherTimerUnlinked,
    GatherParentDetached,
    GatherOperationReplaced,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MasteryStatState {
    pub native_kind: u16,
    pub life_state: u8,
    pub combat_flags: u8,
    pub owner: SkillLookup<MasteryStatOwner>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MasteryStatOwner {
    pub object_id: u64,
    pub native_kind: u16,
    /// Signed PC byte +284; unsigned job byte +100 in the pick-table key.
    pub table_key: i8,
    pub job: u8,
    pub additional_points: i16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MasteryStatLimit {
    /// Slot-1803 signed shorts +44/+50/+52.
    pub total: i16,
    pub attacker: i16,
    pub defender: i16,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SkillListState {
    /// Owner read from the attached PC's skill manager, not a wire field.
    pub reply_owner: Option<u64>,
    pub pages: [SkillListPage; 5],
}

/// Native ordered-map traversal for one of the five command-113 groups.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SkillListPage {
    /// Bytes +237/+304/+372/+440/+508. Only bit 0 selects a group.
    pub flags: u8,
    /// The native map size is a separate gate, not a filtered entry count.
    pub map_size: u64,
    /// Signed bytes +587..+591, encoded as their low byte.
    pub value: i8,
    /// Native in-order traversal. None represents a null map value.
    pub entries: Vec<Option<SkillListEntry>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillListEntry {
    /// Object byte +74 !=0 selects an entry.
    pub enabled: u8,
    pub id: u32,
    /// Raw object bytes +48/+49. Do not infer their typed meaning.
    pub bytes: [u8; 2],
}

/// An adapter supplies live state when the task runs, not when it is queued.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SkillCompletionState {
    pub recall: SkillLookup<RecallActionState>,
    /// Missing means that the geometry lookup failed.
    pub recall_geometry: SkillLookup<RecallGeometry>,
    pub gauge: SkillLookup<SpecialGaugeState>,
    pub gather: SkillLookup<GatherCancelState>,
    pub exchange: SkillLookup<ExchangeFactionState>,
    pub skill_list: SkillLookup<SkillListState>,
    pub mastery: SkillLookup<MasteryStatState>,
}

/// Mock results for opaque scheduler, gauge reads and property operations.
#[derive(Clone, Debug)]
pub struct MemorySkillCompletion {
    pub state: Option<SkillCompletionState>,
    pub schedule_result: Gate,
    pub scheduled: Vec<SkillTask>,
    /// Each read can observe a new tick. Two failure paths read twice.
    pub gauge_values: std::collections::VecDeque<i32>,
    pub gauge_reads: Vec<SkillTask>,
    pub gauge_property_result: Gate,
    pub gauge_property_calls: Vec<SkillTask>,
    pub faction_level: Option<i32>,
    pub faction_level_calls: Vec<SkillTask>,
    /// None = unknown table; a known map without a key = missing record.
    pub mastery_limits: Option<std::collections::BTreeMap<i8, MasteryStatLimit>>,
    pub mastery_costs: Option<std::collections::BTreeMap<u32, i16>>,
    pub mastery_limit_calls: Vec<i8>,
    pub mastery_cost_calls: Vec<u32>,
}
impl Default for MemorySkillCompletion {
    fn default() -> Self {
        Self {
            state: None,
            schedule_result: Gate::Unknown,
            scheduled: vec![],
            gauge_values: std::collections::VecDeque::new(),
            gauge_reads: vec![],
            gauge_property_result: Gate::Unknown,
            gauge_property_calls: vec![],
            faction_level: None,
            faction_level_calls: vec![],
            mastery_limits: None,
            mastery_costs: None,
            mastery_limit_calls: vec![],
            mastery_cost_calls: vec![],
        }
    }
}

/// parity: 135 13-skill-handlers.md#135--g_dieinexhaustionreqcmd--partial partial
/// parity: 137 13-skill-handlers.md#137--g_restorationinexhaustionreqcmd--partial partial
/// parity: 1485 13-skill-handlers.md#1485--g_skillpagetotalreqcmd--partial partial
/// parity: 821 13-skill-handlers.md#821--g_savevitalityreqcmd--done ported
/// These four go through `pc_task` or log only (821).
pub(super) fn dispatch_skill(ctx: &mut Context<'_>, command: u16, req: &[u8]) -> Outcome {
    match command {
        ids::G_UseSkillReqCmd => handle_use_skill(ctx, req, false),
        ids::G_UseSkillToGroundReqCmd => handle_use_skill(ctx, req, true),
        ids::G_CancelActionReqCmd => handle_cancel_action(ctx, req),
        ids::G_SkillTriggerReqCmd => handle_skill_trigger(ctx, req),
        ids::G_GatherReqCmd => handle_gather(ctx, req),
        ids::G_GatherCancelReqCmd => handle_gather_cancel(ctx, req),
        ids::G_LootReqCmd => handle_loot(ctx, req),
        ids::G_RecallSummonReqCmd => handle_recall_summon(ctx, req),
        ids::G_SpecialSkillGaugeFullReqCmd => handle_special_skill_gauge_full(ctx, req),
        ids::G_SelectTargetReqCmd => handle_select_target(ctx, req),
        ids::G_DieInExhaustionReqCmd => pc_task(
            ctx,
            ids::G_DieInExhaustionReqCmd,
            ids::G_DieInExhaustionFailCmd,
            1,
            SkillTaskKind::DieInExhaustion,
        ),
        ids::G_RestorationInExhaustionReqCmd => pc_task(
            ctx,
            ids::G_RestorationInExhaustionReqCmd,
            ids::G_RestorationInExhaustionFailCmd,
            1,
            SkillTaskKind::RestorationInExhaustion,
        ),
        ids::G_HelpRestorationReqCmd => handle_help_restoration(ctx, req),
        ids::G_RespawnReqCmd => handle_respawn(ctx, req),
        ids::G_SaveVitalityReqCmd => Outcome::log("save vitality: deprecated request"),
        ids::G_RechargeVitalityReqCmd => handle_recharge_vitality(ctx, req),
        ids::G_SkillList2ReqCmd => handle_skill_list(ctx, req),
        ids::G_SetMasteryStatPointReqCmd => handle_set_mastery_stat_point(ctx, req),
        ids::G_SkillPageTotalReqCmd => pc_task(
            ctx,
            ids::G_SkillPageTotalReqCmd,
            ids::G_SkillPageTotalFailCmd,
            2,
            SkillTaskKind::SkillPageTotal,
        ),
        ids::G_ChangeSkillPageReqCmd => handle_change_skill_page(ctx, req),
        ids::G_ExpandSkillPageReqCmd => handle_expand_skill_page(ctx, req),
        ids::G_RenameSkillPageReqCmd => handle_rename_skill_page(ctx, req),
        ids::G_TrainTraitReqCmd => handle_train_trait(ctx, req),
        ids::G_UnlockMasteryLevelReqCmd => handle_unlock_mastery_level(ctx, req),
        _ => unreachable!("membership checked"),
    }
}

pub(super) const UNCHECKED_DECODE: &str =
    "❓ unchecked skill decode (short body, field not zero-initialized)";

/// Selected creature for a control selector (doc 13, 1226 rule).
#[derive(Clone, Copy)]
struct Caster {
    summon: bool,
    kind: u8,
    zone: Option<u64>,
}
fn caster(ctx: &Context<'_>, selector: u8) -> Option<Caster> {
    let pc = ctx.pc();
    if selector != 1 {
        return Some(Caster {
            summon: false,
            kind: pc.native_kind,
            zone: pc.zone,
        });
    }
    let s = pc
        .summon
        .as_ref()
        .filter(|s| matches!(s.control_state, 1 | 2))?;
    Some(Caster {
        summon: true,
        kind: s.native_kind,
        zone: s.zone,
    })
}

/// `hEc`: reason, selector.
fn reason_selector(command: u16, reason: u16, selector: u8) -> Outcome {
    let mut body = reason.to_le_bytes().to_vec();
    body.push(selector);
    Outcome::reply(command, body)
}

/// Queue a task; `fail` on adapter failure, unsupported on Unknown.
pub(super) fn queue(ctx: &mut Context<'_>, task: SkillTask, fail: Outcome) -> Outcome {
    match ctx.services.queue_skill(&task) {
        Gate::Pass => Outcome {
            changes: vec![StateChange::Queued(task.command)],
            ..Outcome::default()
        },
        Gate::Fail => fail,
        Gate::Unknown => {
            Outcome::unsupported("❓ skill task/action allocation, init and queue adapter")
        }
    }
}

/// PC task: zone, PC kind 1 (assert then reply), queue. One reason for all.
pub(super) fn pc_task(
    ctx: &mut Context<'_>,
    command: u16,
    failure: u16,
    reason: u16,
    kind: SkillTaskKind,
) -> Outcome {
    let pc = ctx.pc();
    let Some(zone) = pc.zone else {
        return Outcome::reason(failure, reason);
    };
    if pc.native_kind != 1 {
        let mut out = Outcome::reason(failure, reason);
        out.logs.push("skill task: caster is not a PC");
        return out;
    }
    let task = SkillTask {
        command,
        caster: pc.object_id,
        summon: false,
        zone,
        kind,
    };
    queue(ctx, task, Outcome::reason(failure, reason))
}

// ---- Skill use, cancel, trigger ---------------------------------------------------

pub(super) fn counted<T>(
    r: &mut Reader<'_>,
    max: u16,
    item: impl Fn(&mut Reader<'_>) -> Option<T>,
) -> Option<Vec<T>> {
    let n = r.short()? as i16;
    if !(0..=max as i16).contains(&n) {
        return None;
    }
    (0..n).map(|_| item(r)).collect()
}

/// parity: 1226 13-skill-handlers.md#1226--g_useskillreqcmd--partial partial
/// parity: 1228 13-skill-handlers.md#1228--g_useskilltogroundreqcmd--partial partial
pub fn handle_use_skill(ctx: &mut Context<'_>, req: &[u8], ground: bool) -> Outcome {
    const F: u16 = ids::G_UseSkillFailCmd;
    let fail = |reason: u16, selector: u8, skill: u32, variation: u8| {
        let mut body = reason.to_le_bytes().to_vec();
        body.push(selector);
        body.extend(skill.to_le_bytes());
        body.push(variation);
        Outcome::reply(F, body)
    };
    let mut r = Reader(req);
    let scalars = (|| {
        let selector = r.byte()?;
        let skill = r.int()?;
        let variation = r.byte()?;
        let target = if ground {
            Err([r.short()?, r.short()?, r.short()?])
        } else {
            Ok(r.quad()?)
        };
        Some((selector, skill, variation, target, r.short()?, r.byte()?))
    })();
    let Some((selector, skill, variation, target, request, item_selector)) = scalars else {
        return fail(1, 0, 0, 0);
    };
    let failure = fail(1, selector, skill, variation);
    let modifiers = (|| {
        Some(SkillModifiers {
            equip_positions: counted(&mut r, 8, |r| r.byte())?,
            set_item_keys: counted(&mut r, 38, |r| r.quad())?,
            item_skill_positions: counted(&mut r, 4, |r| r.byte())?,
            badge_positions: counted(&mut r, 26, |r| r.byte())?,
        })
    })();
    let Some(modifiers) = modifiers else {
        return failure;
    };
    let Some(Caster {
        summon,
        zone: Some(zone),
        ..
    }) = caster(ctx, selector)
    else {
        return failure;
    };
    let kind = match target {
        Ok(target) => SkillTaskKind::UseSkill {
            skill,
            variation,
            target,
            request,
            item_selector,
            modifiers,
        },
        Err(position) => SkillTaskKind::UseSkillToGround {
            skill,
            variation,
            position,
            request,
            item_selector,
            modifiers,
        },
    };
    let task = SkillTask {
        command: if ground {
            ids::G_UseSkillToGroundReqCmd
        } else {
            ids::G_UseSkillReqCmd
        },
        caster: ctx.pc().object_id,
        summon,
        zone,
        kind,
    };
    queue(ctx, task, failure)
}

/// Selector + one byte; failure `hEc` reason 1.
fn selector_task(
    ctx: &mut Context<'_>,
    req: &[u8],
    command: u16,
    build: fn(u8) -> SkillTaskKind,
) -> Outcome {
    let failure = command + 1;
    let mut r = Reader(req);
    let (Some(selector), Some(value)) = (r.byte(), r.byte()) else {
        return Outcome::unsupported(UNCHECKED_DECODE);
    };
    let Some(Caster {
        summon,
        zone: Some(zone),
        ..
    }) = caster(ctx, selector)
    else {
        return reason_selector(failure, 1, selector);
    };
    let task = SkillTask {
        command,
        caster: ctx.pc().object_id,
        summon,
        zone,
        kind: build(value),
    };
    queue(ctx, task, reason_selector(failure, 1, selector))
}

/// parity: 1229 13-skill-handlers.md#1229--g_cancelactionreqcmd--partial partial
pub fn handle_cancel_action(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    selector_task(ctx, req, 1229, |option| SkillTaskKind::CancelAction {
        option,
    })
}

/// parity: 1231 13-skill-handlers.md#1231--g_skilltriggerreqcmd--done ported
pub fn handle_skill_trigger(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    selector_task(ctx, req, 1231, |trigger| SkillTaskKind::SkillTrigger {
        trigger,
    })
}

// ---- Gather, loot, summon recall, gauge, target ----------------------------------

/// parity: 1270 13-skill-handlers.md#1270--g_gatherreqcmd--partial partial
pub fn handle_gather(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    if ctx.pc().zone.is_none() {
        return Outcome::reason(1271, 1);
    }
    if !u0_passes(ctx) {
        return Outcome::reason(1271, 683);
    }
    let Some(target) = Reader(req).quad() else {
        return Outcome::unsupported(UNCHECKED_DECODE);
    };
    pc_task(ctx, 1270, 1271, 1, SkillTaskKind::Gather { target })
}

/// parity: 77 13-skill-handlers.md#77--g_gathercancelreqcmd--done done
pub fn handle_gather_cancel(ctx: &mut Context<'_>, _: &[u8]) -> Outcome {
    pc_task(ctx, 77, 78, 1, SkillTaskKind::GatherCancel)
}

/// parity: 1272 13-skill-handlers.md#1272--g_lootreqcmd--partial partial
pub fn handle_loot(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_LootFailCmd;
    let Some(target) = Reader(req).quad() else {
        return Outcome::unsupported(UNCHECKED_DECODE);
    };
    let pc = ctx.pc();
    if pc.zone.is_none() {
        return Outcome::reason(F, 1);
    }
    if pc.native_kind != 1 {
        let mut out = Outcome::reason(F, 1);
        out.logs.push("loot: caster is not a PC");
        return out;
    }
    if target >> 48 != 176 {
        // Reason 537, then fall through: reason 1 as well.
        let mut out = Outcome::reason(F, 537);
        out.replies.extend(Outcome::reason(F, 1).replies);
        return out;
    }
    pc_task(ctx, 1272, F, 1, SkillTaskKind::Loot { target })
}

/// parity: 71 13-skill-handlers.md#71--g_recallsummonreqcmd--partial partial
pub fn handle_recall_summon(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_RecallSummonFailCmd;
    let pc = ctx.pc();
    let Some(s) = pc
        .summon
        .as_ref()
        .filter(|s| matches!(s.control_state, 1 | 2))
    else {
        return Outcome::reason(F, 2);
    };
    if s.native_kind != 2 {
        return Outcome::reason(F, 175);
    }
    if s.has_entry && s.race == 11 {
        return Outcome::reason(F, 179);
    }
    let Some(zone) = s.zone else {
        return Outcome::reason(F, 2);
    };
    let mut r = Reader(req);
    // The unchecked decoder's yaw destination is not used by the action.
    let (Some(x), Some(y), Some(z)) = (r.short(), r.short(), r.short()) else {
        return Outcome::unsupported(UNCHECKED_DECODE);
    };
    let task = SkillTask {
        command: ids::G_RecallSummonReqCmd,
        caster: pc.object_id,
        summon: true,
        zone,
        kind: SkillTaskKind::RecallSummon {
            position: [x, y, z],
        },
    };
    queue(ctx, task, Outcome::reason(F, 2))
}

/// parity: 74 13-skill-handlers.md#74--g_specialskillgaugefullreqcmd--partial partial
pub fn handle_special_skill_gauge_full(ctx: &mut Context<'_>, _: &[u8]) -> Outcome {
    // Dispatch already consumed command 74. The native command-only decoder
    // succeeds for this framed request. It does not inspect body bytes.
    let pc = ctx.pc();
    let Some(zone) = pc.zone else {
        // Instructions set reason 2 but supply no fifth variadic argument.
        // Do not send an invented trailing short.
        return Outcome::unsupported("❓ 74 missing-zone reply reads an unsupplied short");
    };
    if pc.native_kind != 1 {
        return Outcome::unsupported("❓ 74 wrong-kind reason-2 reply reads an unsupplied short");
    }
    let task = SkillTask {
        command: ids::G_SpecialSkillGaugeFullReqCmd,
        caster: pc.object_id,
        summon: false,
        zone,
        kind: SkillTaskKind::SpecialSkillGaugeFull,
    };
    match ctx.services.schedule_skill(&task) {
        Gate::Pass => Outcome {
            changes: vec![StateChange::Queued(74)],
            ..Outcome::default()
        },
        // 0x14038A4ED does not test the scheduler result.
        Gate::Fail => Outcome::default(),
        Gate::Unknown => Outcome::unsupported("❓ PC skill scheduler 0x140603BD0"),
    }
}

/// parity: 134 13-skill-handlers.md#134--g_selecttargetreqcmd--partial partial
pub fn handle_select_target(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let Some(target) = Reader(req).quad() else {
        return Outcome::unsupported(UNCHECKED_DECODE);
    };
    let pc = ctx.pc();
    let Some(zone) = pc.zone else {
        return Outcome::log("select target: zone is not found");
    };
    let task = SkillTask {
        command: ids::G_SelectTargetReqCmd,
        caster: pc.object_id,
        summon: false,
        zone,
        kind: SkillTaskKind::SelectTarget { target },
    };
    queue(ctx, task, Outcome::log("select target: queue failed"))
}

/// parity: 139 13-skill-handlers.md#139--g_helprestorationreqcmd--partial partial
pub fn handle_help_restoration(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_HelpRestorationFailCmd;
    let mut r = Reader(req);
    let (Some(selector), Some(target)) = (r.byte(), r.quad()) else {
        return Outcome::unsupported(UNCHECKED_DECODE);
    };
    let failure = reason_selector(F, 1, selector);
    let Some(Caster {
        summon,
        kind,
        zone: Some(zone),
    }) = caster(ctx, selector)
    else {
        return failure;
    };
    if !(1..=3).contains(&kind) {
        let mut out = failure;
        out.logs.push("help restoration: invalid caster kind");
        return out;
    }
    let task = SkillTask {
        command: ids::G_HelpRestorationReqCmd,
        caster: ctx.pc().object_id,
        summon,
        zone,
        kind: SkillTaskKind::HelpRestoration { target },
    };
    queue(ctx, task, failure)
}

/// parity: 142 13-skill-handlers.md#142--g_respawnreqcmd--partial partial
pub fn handle_respawn(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let Some(option) = Reader(req).byte() else {
        return Outcome::unsupported(UNCHECKED_DECODE);
    };
    let pc = ctx.pc();
    let Some(zone) = pc.zone else {
        return Outcome::reason(144, 1);
    };
    let task = SkillTask {
        command: ids::G_RespawnReqCmd,
        caster: pc.object_id,
        summon: false,
        zone,
        kind: SkillTaskKind::Respawn { option },
    };
    queue(ctx, task, Outcome::reason(144, 1))
}

// ---- Explicit task execution ----------------------------------------------------

/// Execute only a task that the adapter retained after a successful submit.
/// Queueing a request never calls this function by itself.
pub fn execute_skill_task(ctx: &mut Context<'_>, task: &SkillTask) -> Outcome {
    // This context supplies the original PC reply route. Do not run a stale
    // task in a different session or reinterpret another task's payload.
    if task.caster != ctx.pc().object_id {
        return Outcome::unsupported("❓ skill task reply route does not match this PC");
    }
    match (&task.kind, task.command, task.summon) {
        (SkillTaskKind::RecallSummon { position }, 71, true) => {
            execute_recall_summon(ctx, task, *position)
        }
        (SkillTaskKind::SpecialSkillGaugeFull, 74, false) => execute_special_gauge(ctx, task),
        (SkillTaskKind::GatherCancel, 77, false) => execute_gather_cancel(ctx, task),
        (SkillTaskKind::SkillList, 113, false) => execute_skill_list(ctx, task),
        (SkillTaskKind::SetMasteryStatPoint { shorts, bytes }, 117, false) => {
            execute_mastery_stat_point(ctx, task, *shorts, *bytes)
        }
        (SkillTaskKind::Combat(CombatTask::ExchangeFactionScore { amount, .. }), 83, false) => {
            super::skills2::execute_exchange_faction_score(ctx, task, *amount)
        }
        _ => Outcome::unsupported("❓ skill task/action continuation"),
    }
}

/// parity: 71 13-skill-handlers.md#71--g_recallsummonreqcmd--partial partial
fn execute_recall_summon(ctx: &mut Context<'_>, task: &SkillTask, position: [u16; 3]) -> Outcome {
    let Some(state) = ctx.services.skill_completion_state() else {
        return Outcome::unsupported("❓ recall live action state");
    };
    let recall = match &state.recall {
        SkillLookup::Found(recall) => recall,
        SkillLookup::Missing | SkillLookup::Unknown => {
            // Native dereferences the parent/current action. Do not invent
            // a safe reason for an invalid native object graph.
            return Outcome::unsupported("❓ recall parent/current action pointer");
        }
    };
    if recall.reply_owner != Some(task.caster) {
        return Outcome::unsupported("❓ recall live summon-owner reply route changed");
    }
    if recall.current_action_class != 1 {
        return Outcome::reason(72, 178);
    }
    let geometry = match &state.recall_geometry {
        SkillLookup::Unknown => return Outcome::unsupported("❓ recall geometry lookup"),
        SkillLookup::Missing => return Outcome::reason(72, 177),
        SkillLookup::Found(geometry) => geometry,
    };
    if geometry.environment < 0 {
        let mut out = Outcome::reason(72, 177);
        out.logs.push("recall: invalid negative environment");
        return out;
    }
    if geometry.environment > 0 && (!geometry.environment_exists || geometry.environment_blocked) {
        return Outcome::reason(72, 177);
    }
    let Some(owner) = recall.owner_position else {
        return Outcome::unsupported("❓ recall null owner is dereferenced natively");
    };
    // Test the requested signed XYZ before replacing Z with geometry height.
    let distance_squared: i64 = position
        .into_iter()
        .zip(owner)
        .map(|(requested, owner)| {
            let delta = i64::from(requested as i16) - i64::from(owner);
            delta * delta
        })
        .sum();
    if distance_squared > 15_625 {
        return Outcome::reason(72, 176);
    }
    Outcome::unsupported("❓ recall yaw, movement event, relocation and delta mutation")
}

fn gauge_failure(reason: u16, gauge: i32) -> Outcome {
    let mut body = reason.to_le_bytes().to_vec();
    body.extend((gauge as u16).to_le_bytes());
    Outcome::reply(76, body)
}

/// parity: 74 13-skill-handlers.md#74--g_specialskillgaugefullreqcmd--partial partial
fn execute_special_gauge(ctx: &mut Context<'_>, task: &SkillTask) -> Outcome {
    let Some(state) = ctx.services.skill_completion_state() else {
        return Outcome::unsupported("❓ special gauge attached PC and level configuration");
    };
    let gauge = match &state.gauge {
        SkillLookup::Unknown => {
            return Outcome::unsupported("❓ special gauge attached PC and level configuration");
        }
        SkillLookup::Missing => return Outcome::log("special gauge task: PC is null"),
        SkillLookup::Found(gauge) => gauge,
    };
    if gauge.minimum_level == 0 || i32::from(gauge.level) < gauge.minimum_level {
        let mut out = gauge_failure(1550, 0);
        out.logs.push("special gauge task: level gate failed");
        return out;
    }
    let Some(current) = ctx.services.skill_gauge_value(task) else {
        return Outcome::unsupported("❓ current special gauge 0x1405F5140");
    };
    if current < 10_000 {
        // Native reads again after its diagnostic log. Do not echo the
        // first value when a later tick can produce another value.
        let mut out = match ctx.services.skill_gauge_value(task) {
            Some(current) => gauge_failure(1549, current),
            None => Outcome::unsupported("❓ second special gauge read for reply 76"),
        };
        out.logs.push("special gauge task: gauge is below 10000");
        return out;
    }
    match ctx.services.set_skill_gauge_full(task) {
        Gate::Unknown => Outcome::unsupported("❓ property 226 setter, delta and flush adapter"),
        Gate::Pass => Outcome::reply(75, vec![]),
        Gate::Fail => {
            let mut out = match ctx.services.skill_gauge_value(task) {
                Some(current) => gauge_failure(2, current),
                None => Outcome::unsupported("❓ special gauge read after property failure"),
            };
            out.logs
                .push("special gauge task: property operation failed");
            out
        }
    }
}

/// parity: 77 13-skill-handlers.md#77--g_gathercancelreqcmd--done done
fn execute_gather_cancel(ctx: &mut Context<'_>, task: &SkillTask) -> Outcome {
    let Some(state) = ctx.services.skill_completion_state() else {
        return Outcome::unsupported("❓ live PC gather operation state");
    };
    let gather = match &mut state.gather {
        SkillLookup::Found(gather) => gather,
        SkillLookup::Unknown | SkillLookup::Missing => {
            return Outcome::unsupported("❓ PC gather action-manager/current-operation pointer");
        }
    };
    if gather.operation_class != 15 {
        let mut out = Outcome::reason(78, 684);
        out.logs.push("gather cancel: not gathering");
        return out;
    }
    let mut out = Outcome::default();
    if gather.action_flags & 4 != 0 {
        out.logs.push("gather cancel: action is already cancelled");
    } else {
        gather.action_flags |= 4;
        out.skill_effects.push(SkillEffect::GatherCancelled);
        if !gather.parent_attached {
            out.status =
                Status::Unsupported("❓ gather callback dereferences a missing action parent");
            return out;
        }
        // 0x1400244C0 forwards cancellation reason 4 to virtual +192.
        // The gather callback emits event kind 42 with completion byte 2.
        let broadcast = match &gather.broadcast {
            SkillLookup::Found(broadcast) => broadcast,
            SkillLookup::Unknown | SkillLookup::Missing => {
                out.status = Status::Unsupported("❓ gather zone action-buffer metadata");
                return out;
            }
        };
        if broadcast.zone_has_creatures {
            let mut payload = vec![42];
            payload.extend(task.caster.to_le_bytes());
            payload.extend(gather.target.to_le_bytes());
            payload.push(2);
            if broadcast.caster_found && broadcast.personal_copy_enabled {
                if broadcast.personal_buffer_present {
                    out.skill_effects.push(SkillEffect::AppendPersonalAction {
                        caster: task.caster,
                        payload: payload.clone(),
                    });
                } else {
                    out.logs
                        .push("gather cancel: controlled-creature action buffer is null");
                }
            }
            out.skill_effects.push(SkillEffect::AppendZoneAction {
                zone: gather.action_zone,
                cell: broadcast.cell,
                event_kind: 42,
                visibility: if broadcast.caster_found {
                    broadcast.visibility
                } else {
                    0
                },
                payload,
            });
        }
        if gather.timer_present && gather.timer_scheduler_active && gather.timer_linked {
            gather.timer_linked = false;
            out.skill_effects.push(SkillEffect::GatherTimerUnlinked);
        }
        if gather.action_flags & 0xA == 0 && gather.parent_attached {
            gather.parent_attached = false;
            out.skill_effects.push(SkillEffect::GatherParentDetached);
        }
    }
    // This replacement also runs if the old action was already cancelled.
    gather.operation_class = 1;
    out.skill_effects.push(SkillEffect::GatherOperationReplaced);
    out
}

/// parity: 113 13-skill-handlers.md#113--g_skilllist2reqcmd--done done
fn execute_skill_list(ctx: &mut Context<'_>, task: &SkillTask) -> Outcome {
    let Some(state) = ctx.services.skill_completion_state() else {
        return Outcome::unsupported("❓ skill-list live manager state");
    };
    let list = match &state.skill_list {
        SkillLookup::Found(list) => list,
        SkillLookup::Missing | SkillLookup::Unknown => {
            return Outcome::unsupported("❓ skill-list attached PC/manager pointer");
        }
    };
    if list.reply_owner != Some(task.caster) {
        return Outcome::unsupported("❓ skill-list manager-owner reply route");
    }
    // Native capacity includes the outer size and command (four bytes).
    const BODY_CAPACITY: usize = 3996;
    const OVERFLOW: &str = "❓ skill-list native 4000-byte composition assertion";
    let mut body = vec![0, 0];
    let mut group_count = 0u16;
    for (index, page) in list.pages.iter().enumerate() {
        if page.flags & 1 == 0 || page.map_size == 0 {
            continue;
        }
        if body.len() + 4 > BODY_CAPACITY {
            return Outcome::unsupported(OVERFLOW);
        }
        body.extend([index as u8 + 1, page.value as u8]);
        let count_offset = body.len();
        body.extend([0, 0]);
        let mut count = 0u16;
        for entry in page.entries.iter().flatten().filter(|e| e.enabled != 0) {
            if body.len() + 6 > BODY_CAPACITY {
                return Outcome::unsupported(OVERFLOW);
            }
            body.extend(entry.id.to_le_bytes());
            body.extend(entry.bytes);
            count += 1;
        }
        body[count_offset..count_offset + 2].copy_from_slice(&count.to_le_bytes());
        group_count += 1;
    }
    body[..2].copy_from_slice(&group_count.to_le_bytes());
    Outcome::reply(114, body)
}

fn mastery_failure(reason: u16) -> Outcome {
    let mut out = Outcome::reason(119, reason);
    out.logs.push("mastery stat: task validation failed");
    out
}

/// parity: 117 13-skill-handlers.md#117--g_setmasterystatpointreqcmd--partial partial
fn execute_mastery_stat_point(
    ctx: &mut Context<'_>,
    task: &SkillTask,
    shorts: [i16; 2],
    bytes: [i8; 5],
) -> Outcome {
    let Some(state) = ctx.services.skill_completion_state() else {
        return Outcome::unsupported("❓ mastery stat live attached-PC state");
    };
    let mastery = match &state.mastery {
        SkillLookup::Found(mastery) => mastery,
        SkillLookup::Missing | SkillLookup::Unknown => {
            return Outcome::unsupported("❓ mastery stat attached-PC pointer");
        }
    };
    if mastery.native_kind != 1 {
        return Outcome::default();
    }
    if matches!(mastery.life_state, 1 | 2) {
        return mastery_failure(2);
    }
    if mastery.combat_flags & 1 != 0 {
        return mastery_failure(701);
    }
    let owner = match &mastery.owner {
        SkillLookup::Found(owner) => owner.clone(),
        SkillLookup::Missing | SkillLookup::Unknown => {
            return Outcome::unsupported("❓ mastery stat skill-manager owner pointer");
        }
    };
    if owner.native_kind != 1 {
        return mastery_failure(2);
    }
    if owner.object_id != task.caster {
        return Outcome::unsupported("❓ mastery stat skill-manager owner changed");
    }
    let limit = match ctx.services.skill_mastery_limit(owner.table_key) {
        SkillLookup::Unknown => return Outcome::unsupported("❓ mastery stat slot-1803 table"),
        SkillLookup::Missing => return mastery_failure(2),
        SkillLookup::Found(limit) => limit,
    };
    let [attacker, defender] = shorts.map(i32::from);
    let total = i32::from(limit.total) + i32::from(owner.additional_points);
    if attacker + defender > total {
        return mastery_failure(2);
    }
    if attacker > i32::from(limit.attacker) {
        return mastery_failure(2);
    }
    if defender > i32::from(limit.defender) {
        return mastery_failure(2);
    }
    let mut cost = 0i32;
    for (index, pick) in bytes.into_iter().enumerate() {
        if pick <= 0 {
            continue;
        }
        let key = index as u32 | (u32::from(pick as u8) << 8) | (u32::from(owner.job) << 16);
        match ctx.services.skill_mastery_cost(key) {
            SkillLookup::Unknown => return Outcome::unsupported("❓ mastery stat slot-1827 table"),
            SkillLookup::Missing => return mastery_failure(2),
            SkillLookup::Found(value) => cost += i32::from(value),
        }
    }
    if total - defender - attacker < cost {
        return mastery_failure(2);
    }
    Outcome::unsupported("❓ mastery stat property writes, persistence and effect refresh")
}

// ---- Vitality, skill list, mastery points, pages ----------------------------------

/// parity: 824 13-skill-handlers.md#824--g_rechargevitalityreqcmd--partial partial
pub fn handle_recharge_vitality(ctx: &mut Context<'_>, _: &[u8]) -> Outcome {
    // UTC seconds are captured before the gates. The command-only decode
    // always succeeds for a framed message.
    let fail = reason_u64(826, 1, ctx.world.utc_seconds as u64);
    if !ctx.world.vitality_recharge_enabled {
        return fail;
    }
    let pc = ctx.pc();
    let Some(zone) = pc.zone else {
        return fail;
    };
    if pc.native_kind != 1 {
        let mut out = fail;
        out.logs.push("recharge vitality: caster is not a PC");
        return out;
    }
    let task = SkillTask {
        command: ids::G_RechargeVitalityReqCmd,
        caster: pc.object_id,
        summon: false,
        zone,
        kind: SkillTaskKind::RechargeVitality,
    };
    queue(ctx, task, fail)
}

/// parity: 113 13-skill-handlers.md#113--g_skilllist2reqcmd--done done
pub fn handle_skill_list(ctx: &mut Context<'_>, _: &[u8]) -> Outcome {
    let pc = ctx.pc();
    let Some(zone) = pc.zone else {
        return Outcome::reason(115, 2);
    };
    let task = SkillTask {
        command: ids::G_SkillList2ReqCmd,
        caster: pc.object_id,
        summon: false,
        zone,
        kind: SkillTaskKind::SkillList,
    };
    queue(ctx, task, Outcome::reason(115, 2))
}

/// parity: 117 13-skill-handlers.md#117--g_setmasterystatpointreqcmd--partial partial
pub fn handle_set_mastery_stat_point(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let mut r = Reader(req);
    let fields = (|| {
        let shorts = [r.short()? as i16, r.short()? as i16];
        let mut bytes = [0i8; 5];
        for b in &mut bytes {
            *b = r.byte()? as i8;
        }
        Some((shorts, bytes))
    })();
    let Some(zone) = ctx.pc().zone else {
        return Outcome::reason(119, 2);
    };
    if ctx.pc().native_kind != 1 {
        let mut out = Outcome::reason(119, 2);
        out.logs.push("mastery stat: caster is not a PC");
        return out;
    }
    let Some((shorts, bytes)) = fields else {
        return Outcome::unsupported(UNCHECKED_DECODE);
    };
    if shorts.iter().any(|&s| s < 0) || bytes.iter().any(|&b| b < 0) {
        return Outcome::reason(119, 2);
    }
    let task = SkillTask {
        command: ids::G_SetMasteryStatPointReqCmd,
        caster: ctx.pc().object_id,
        summon: false,
        zone,
        kind: SkillTaskKind::SetMasteryStatPoint { shorts, bytes },
    };
    queue(ctx, task, Outcome::reason(119, 2))
}

/// parity: 1488 13-skill-handlers.md#1488--g_changeskillpagereqcmd--partial partial
pub fn handle_change_skill_page(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let Some(page) = Reader(req).byte() else {
        return Outcome::unsupported(UNCHECKED_DECODE);
    };
    pc_task(ctx, 1488, 1490, 2, SkillTaskKind::ChangeSkillPage { page })
}

/// ❓ Decoder capacity 32 UTF-16 units, modelled as length ≤ 32.
const PAGE_NAME_CAPACITY: usize = 33;

/// parity: 1491 13-skill-handlers.md#1491--g_expandskillpagereqcmd--partial partial
pub fn handle_expand_skill_page(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_ExpandSkillPageFailCmd;
    let mut r = Reader(req);
    let (Some(page), Some(name)) = (r.byte(), r.string(PAGE_NAME_CAPACITY)) else {
        let mut out = Outcome::reason(1330, 1);
        out.logs.push("expand skill page: fixed decode failed");
        return out;
    };
    let Ok(pairs) = pairs(&mut r, 0, 15) else {
        return Outcome::reason(F, 693);
    };
    let pc = ctx.pc();
    let Some(zone) = pc.zone else {
        return Outcome::reason(F, 1);
    };
    if pc.native_kind != 1 {
        // Another kind silently skips initialization and queueing.
        return Outcome::default();
    }
    let task = SkillTask {
        command: ids::G_ExpandSkillPageReqCmd,
        caster: pc.object_id,
        summon: false,
        zone,
        kind: SkillTaskKind::ExpandSkillPage { page, name, pairs },
    };
    queue(ctx, task, Outcome::reason(F, 1))
}

/// parity: 1494 13-skill-handlers.md#1494--g_renameskillpagereqcmd--partial partial
pub fn handle_rename_skill_page(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let mut r = Reader(req);
    let (Some(page), Some(name)) = (r.byte(), r.string(PAGE_NAME_CAPACITY)) else {
        return Outcome::unsupported(UNCHECKED_DECODE);
    };
    pc_task(
        ctx,
        1494,
        1496,
        2,
        SkillTaskKind::RenameSkillPage { page, name },
    )
}

// ---- Traits and mastery -----------------------------------------------------------

/// parity: 1497 13-skill-handlers.md#1497--g_traintraitreqcmd--partial partial
pub fn handle_train_trait(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_TrainTraitFailCmd;
    let fail = |reason: u16, selectors: [u8; 2]| {
        let mut body = reason.to_le_bytes().to_vec();
        body.extend(selectors);
        Outcome::reply(F, body)
    };
    const NONE: [u8; 2] = [0xFF, 0xFF];
    let Some(zone) = ctx.pc().zone else {
        return fail(2, NONE);
    };
    if !ctx.pc().user_present {
        return fail(2, NONE);
    }
    // Rate gate on the session object. Equality passes. The timestamp is set
    // before decoding and is not rolled back by later failures.
    let now = ctx.world.tick;
    let last = ctx.pc().trait_train_tick;
    if last != 0 && last.saturating_add(ctx.world.trait_train_interval) > now {
        return fail(707, NONE);
    }
    ctx.session.pc.as_mut().expect("PC").trait_train_tick = now;
    let mut out = train_trait_body(ctx, req, zone, &fail);
    out.changes.insert(0, StateChange::TraitTrainTick(now));
    out
}

fn train_trait_body(
    ctx: &mut Context<'_>,
    req: &[u8],
    zone: u64,
    fail: &dyn Fn(u16, [u8; 2]) -> Outcome,
) -> Outcome {
    let mut r = Reader(req);
    let mut fixed = [0u8; 7];
    for b in &mut fixed {
        let Some(v) = r.byte() else {
            return fail(2, [0xFF, 0xFF]);
        };
        *b = v;
    }
    let selectors = [fixed[0], fixed[1]];
    if selectors[1] > 9 {
        return fail(711, selectors);
    }
    if !(1..=5).contains(&selectors[0]) {
        return fail(706, selectors);
    }
    let mut build_up = [1u8; 6];
    build_up[1..].copy_from_slice(&fixed[2..]);
    if build_up.iter().any(|&b| b > 3) {
        return fail(710, selectors);
    }
    let Some(n) = r.short().map(|n| n as i16).filter(|n| (0..=50).contains(n)) else {
        return fail(712, selectors);
    };
    let mut traits = Vec::new();
    for _ in 0..n {
        let (Some(id), Some(level)) = (r.int(), r.byte()) else {
            return fail(702, selectors);
        };
        if id == 0 {
            return fail(713, selectors);
        }
        let max = match ctx.services.trait_max_level(id) {
            None => return Outcome::unsupported("❓ trait table (manager slot 2379)"),
            Some(None) => return fail(713, selectors),
            Some(Some(max)) => max,
        };
        if level as i8 > max {
            return fail(717, selectors);
        }
        traits.push((id, level as i8));
    }
    let task = SkillTask {
        command: ids::G_TrainTraitReqCmd,
        caster: ctx.pc().object_id,
        summon: false,
        zone,
        kind: SkillTaskKind::TrainTrait {
            selectors,
            build_up,
            traits,
        },
    };
    queue(ctx, task, fail(2, selectors))
}

/// parity: 1389 13-skill-handlers.md#1389--g_unlockmasterylevelreqcmd--partial partial
pub fn handle_unlock_mastery_level(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let Ok(pairs) = pairs(&mut Reader(req), 0, 15) else {
        return Outcome::reason(1391, 470);
    };
    if ctx.pc().t2_blocked {
        return Outcome::reason(1391, 7);
    }
    let pc = ctx.pc();
    let Some(zone) = pc.zone else {
        return Outcome::reason(1391, 1);
    };
    if pc.native_kind != 1 {
        return Outcome::log("unlock mastery level: caster is not a PC");
    }
    let task = SkillTask {
        command: ids::G_UnlockMasteryLevelReqCmd,
        caster: pc.object_id,
        summon: false,
        zone,
        kind: SkillTaskKind::UnlockMasteryLevel { pairs },
    };
    queue(ctx, task, Outcome::reason(1391, 1))
}
