//! Doc 21: quest progress entry functions. Build 381.8867.
//!
//! Quest data and name services remain adapter boundaries. A queued task is
//! not a completed quest. Do not invent unset decode fields or reply arguments.
use super::skills::{counted, queue};
use super::*;

pub const QUEST2_COMMANDS: &[u16] = &[
    ids::G_AcquireQuestReqCmd,
    ids::G_ProgressMissionReqCmd,
    ids::G_ProgressMissionAllReqCmd,
    ids::G_ProgressMissionByAcquireSummonedReqCmd,
    ids::G_DisposeQuestReqCmd,
    ids::G_MissionStepRollbackReqCmd,
    ids::G_MissionStepTimeExpiredReqCmd,
    ids::G_ResetQuestCompletionTimeReqCmd,
    ids::G_AcquireRenewalQuestReqCmd,
    ids::G_ChallengeListAcquireQuestReqCmd,
    ids::G_ProgressSkipMissionStepReqCmd,
];

/// Server-owned checks from doc 21. Pass means true; Fail means false.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QuestProgressCheck {
    /// Rule config byte +2104.
    AdditionalRewardEnabled,
    /// Data manager slot 1455, virtual +168. RDX retains the item key.
    RewardItem { key: u64 },
    /// Data manager slot 591, virtual +168.
    CostGroup { key: u64 },
    /// Rule byte +2089 and PC short +12544 in 77/177.
    ResetRestricted { pc: u64 },
    /// Daemon +2728 object and its +48 name service are present.
    SummonedNameServicePresent,
    /// Name service virtual +0, with the PC user snapshot and NUL-prefix name.
    SummonedNameAllowed { pc: u64, name: Vec<u16> },
    /// 0x14012F9F0 and rule config +172..+200. Pass means accepted (native 0).
    SummonedNameFilter { name: Vec<u16> },
}

/// Unknown is not approval. Each mock field is an independent native check.
#[derive(Clone, Debug)]
pub struct MemoryQuestProgress {
    pub additional_reward_enabled: Gate,
    pub reward_item: Gate,
    pub cost_group: Gate,
    pub reset_restricted: Gate,
    pub summoned_name_service_present: Gate,
    pub summoned_name_allowed: Gate,
    pub summoned_name_filter: Gate,
    pub calls: Vec<QuestProgressCheck>,
}
impl Default for MemoryQuestProgress {
    fn default() -> Self {
        Self {
            additional_reward_enabled: Gate::Unknown,
            reward_item: Gate::Unknown,
            cost_group: Gate::Unknown,
            reset_restricted: Gate::Unknown,
            summoned_name_service_present: Gate::Unknown,
            summoned_name_allowed: Gate::Unknown,
            summoned_name_filter: Gate::Unknown,
            calls: Vec::new(),
        }
    }
}
impl MemoryQuestProgress {
    pub(super) fn check(&mut self, check: &QuestProgressCheck) -> Gate {
        self.calls.push(check.clone());
        match check {
            QuestProgressCheck::AdditionalRewardEnabled => self.additional_reward_enabled,
            QuestProgressCheck::RewardItem { .. } => self.reward_item,
            QuestProgressCheck::CostGroup { .. } => self.cost_group,
            QuestProgressCheck::ResetRestricted { .. } => self.reset_restricted,
            QuestProgressCheck::SummonedNameServicePresent => self.summoned_name_service_present,
            QuestProgressCheck::SummonedNameAllowed { .. } => self.summoned_name_allowed,
            QuestProgressCheck::SummonedNameFilter { .. } => self.summoned_name_filter,
        }
    }
}

/// Wire Qh. Native stride 4: byte +0, short +2.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuestPosition {
    pub depot: u8,
    pub position: u16,
}
/// Wire Qhh. Native stride 6: byte +0, shorts +2/+4.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuestItemOrder {
    pub position: QuestPosition,
    pub count: u16,
}
/// Wire hcccq. Keep all bit patterns; entry code does not validate these fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuestMission {
    pub quest: u16,
    pub mission: u8,
    pub case: u8,
    pub selector: u8,
    pub target: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuestAdditionalReward {
    pub index: u8,
    pub item_key: u64,
    pub cost_group_key: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QuestProgressTask {
    /// Kind 93, vtable 0x140C71898, size 0x188.
    /// +144 quest, +146 case, +147 selector, +152 target, +160 short.
    /// +168/+172 item count/orders; +268/+272 brand count/positions.
    Acquire {
        quest: u16,
        case: u8,
        selector: u8,
        target: u64,
        value: u16,
        brands: Vec<QuestPosition>,
        items: Vec<QuestItemOrder>,
    },
    /// Kind 94, vtable 0x140CAF170, size 0x188.
    /// +144 quest, +146 mission, +147 case, +148 selector, +152 target,
    /// +160 short; +168/+172 items, +268/+272 brands, +336/+340 gems;
    /// +372 reward index, +376/+384 keys.
    Progress {
        mission: QuestMission,
        value: u16,
        brands: Vec<QuestPosition>,
        gems: Vec<QuestPosition>,
        items: Vec<QuestItemOrder>,
        reward: QuestAdditionalReward,
    },
    /// Kind 94, vtable 0x140C6EE40, size 0x100. Count +144;
    /// arrays +148 quest, +164 mission, +172 case, +180 selector, +192 target.
    ProgressAll { missions: Vec<QuestMission> },
    /// Kind 94, vtable 0x140BC2AF0, size 0xD0. +144 quest, +146 mission,
    /// +147 case, +152 target, +160 NUL-prefix name (zero-filled storage).
    AcquireSummoned {
        quest: u16,
        mission: u8,
        case: u8,
        target: u64,
        name: Vec<u16>,
    },
    /// Kind 92, vtable 0x140CA98A0, size 0xE0. Shorts +144, count +216.
    Dispose { quests: Vec<u16> },
    /// Kind 95, vtable 0x140D16F30, size 0x98. Short +144, bytes +146/+147.
    Rollback { quest: u16, mission: u8, step: u8 },
    /// Kind 96, vtable 0x140D12948, size 0x98. Short +144, byte +146.
    TimeExpired { quest: u16, mission: u8 },
    /// Kind 134, vtable 0x140C7D3E8, size 0xD8. Short +144,
    /// DWORD count +148, short pairs +152 (stride 4).
    ResetCompletionTime { quest: u16, orders: Vec<(u16, u16)> },
    /// Kind 222, vtable 0x140C61A60, size 0x98. Byte +144, short +146.
    ChallengeAcquire { sub_type: u8, quest: u16 },
    /// Kind 180, vtable 0x140CE3258, size 0x98. Short +144, byte +146.
    SkipMissionStep { quest: u16, reason: u8 },
}
impl From<QuestProgressTask> for SkillTaskKind {
    fn from(task: QuestProgressTask) -> Self {
        Self::QuestProgress(task)
    }
}

pub(super) fn dispatch_quest2(ctx: &mut Context<'_>, command: u16, req: &[u8]) -> Outcome {
    match command {
        ids::G_AcquireQuestReqCmd => handle_acquire_quest(ctx, req),
        ids::G_ProgressMissionReqCmd => handle_progress_mission(ctx, req),
        ids::G_ProgressMissionAllReqCmd => handle_progress_mission_all(ctx, req),
        ids::G_ProgressMissionByAcquireSummonedReqCmd => {
            handle_progress_mission_by_acquire_summoned(ctx, req)
        }
        ids::G_DisposeQuestReqCmd => handle_dispose_quest(ctx, req),
        ids::G_MissionStepRollbackReqCmd => handle_mission_step_rollback(ctx, req),
        ids::G_MissionStepTimeExpiredReqCmd => handle_mission_step_time_expired(ctx, req),
        ids::G_ResetQuestCompletionTimeReqCmd => handle_reset_quest_completion_time(ctx, req),
        ids::G_AcquireRenewalQuestReqCmd => handle_acquire_renewal_quest(ctx, req),
        ids::G_ChallengeListAcquireQuestReqCmd => handle_challenge_list_acquire_quest(ctx, req),
        ids::G_ProgressSkipMissionStepReqCmd => handle_progress_skip_mission_step(ctx, req),
        _ => unreachable!("membership checked"),
    }
}

fn task(ctx: &Context<'_>, command: u16, zone: u64, task: QuestProgressTask) -> SkillTask {
    SkillTask {
        command,
        caster: ctx.pc().object_id,
        summon: false,
        zone,
        kind: task.into(),
    }
}
fn position(r: &mut Reader<'_>) -> Option<QuestPosition> {
    Some(QuestPosition {
        depot: r.byte()?,
        position: r.short()?,
    })
}
fn item(r: &mut Reader<'_>) -> Option<QuestItemOrder> {
    Some(QuestItemOrder {
        position: position(r)?,
        count: r.short()?,
    })
}
fn mission(r: &mut Reader<'_>) -> Option<QuestMission> {
    Some(QuestMission {
        quest: r.short()?,
        mission: r.byte()?,
        case: r.byte()?,
        selector: r.byte()?,
        target: r.quad()?,
    })
}
fn duplicate<'a>(positions: impl Iterator<Item = &'a QuestPosition>) -> bool {
    let mut seen = BTreeSet::new();
    positions
        .into_iter()
        .any(|p| !seen.insert((p.depot, p.position)))
}
fn unchecked() -> Outcome {
    Outcome::unsupported("❓ quest progress: unchecked decode, unset fields")
}

/// parity: 1448 21-quest-progress-handlers.md#1448--g_acquirequestreqcmd--partial partial
pub fn handle_acquire_quest(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let mut r = Reader(req);
    let (Some(quest), Some(case), Some(selector), Some(target), Some(value)) =
        (r.short(), r.byte(), r.byte(), r.quad(), r.short())
    else {
        return Outcome::reason(1449, 955);
    };
    let Some(brands) = counted(&mut r, 16, position) else {
        return Outcome::reason(1449, 955);
    };
    if duplicate(brands.iter()) {
        return Outcome::reason(1449, 965);
    }
    // Native quirk: item failures use ProgressMission's failure command.
    let Some(items) = counted(&mut r, 16, item) else {
        return Outcome::reason(1451, 1001);
    };
    if duplicate(items.iter().map(|i| &i.position)) {
        return Outcome::reason(1451, 1013);
    }
    let Some(zone) = ctx.pc().zone else {
        return Outcome::reason(1449, 2);
    };
    let t = task(
        ctx,
        ids::G_AcquireQuestReqCmd,
        zone,
        QuestProgressTask::Acquire {
            quest,
            case,
            selector,
            target,
            value,
            brands,
            items,
        },
    );
    queue(ctx, t, Outcome::reason(1449, 2))
}

/// parity: 1450 21-quest-progress-handlers.md#1450--g_progressmissionreqcmd--partial partial
pub fn handle_progress_mission(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_ProgressMissionFailCmd;
    let mut r = Reader(req);
    let (Some(mission), Some(value)) = (mission(&mut r), r.short()) else {
        return Outcome::reason(F, 1001);
    };
    let Some(brands) = counted(&mut r, 16, position) else {
        return Outcome::reason(F, 1001);
    };
    if duplicate(brands.iter()) {
        return Outcome::reason(F, 1013);
    }
    // Native requires either zero or exactly eight gem positions.
    let Some(n) = r.short() else {
        return Outcome::reason(F, 1001);
    };
    if n != 0 && n != 8 {
        return Outcome::reason(F, 1001);
    }
    let Some(gems) = (0..n).map(|_| position(&mut r)).collect::<Option<Vec<_>>>() else {
        return Outcome::reason(F, 1001);
    };
    if duplicate(gems.iter()) {
        return Outcome::reason(F, 1013);
    }
    let Some(items) = counted(&mut r, 16, item) else {
        return Outcome::reason(F, 1001);
    };
    if duplicate(items.iter().map(|i| &i.position)) {
        return Outcome::reason(F, 1013);
    }
    let (Some(index), Some(item_key), Some(cost_group_key)) = (r.byte(), r.quad(), r.quad()) else {
        return Outcome::reason(F, 1001);
    };
    match ctx
        .services
        .quest_progress(&QuestProgressCheck::AdditionalRewardEnabled)
    {
        Gate::Unknown => {
            return Outcome::unsupported("❓ dungeon additional reward configuration +2104")
        }
        Gate::Fail => {
            if index != 0 || item_key != 0 || cost_group_key != 0 {
                return Outcome::reason(F, 1001);
            }
        }
        Gate::Pass => {
            if index > 4 {
                return Outcome::reason(F, 1001);
            }
            if index != 0 {
                if item_key == 0 || cost_group_key == 0 {
                    return Outcome::reason(F, 1001);
                }
                for check in [
                    QuestProgressCheck::RewardItem { key: item_key },
                    QuestProgressCheck::CostGroup {
                        key: cost_group_key,
                    },
                ] {
                    match ctx.services.quest_progress(&check) {
                        Gate::Pass => {}
                        Gate::Fail => return Outcome::reason(F, 1001),
                        Gate::Unknown => {
                            return Outcome::unsupported(
                                "❓ dungeon additional reward record lookup",
                            )
                        }
                    }
                }
            }
        }
    }
    let Some(zone) = ctx.pc().zone else {
        return Outcome::reason(F, 2);
    };
    let t = task(
        ctx,
        ids::G_ProgressMissionReqCmd,
        zone,
        QuestProgressTask::Progress {
            mission,
            value,
            brands,
            gems,
            items,
            reward: QuestAdditionalReward {
                index,
                item_key,
                cost_group_key,
            },
        },
    );
    queue(ctx, t, Outcome::reason(F, 2))
}

fn all_failure() -> Outcome {
    // hEhcc: reason, quest, mission, case. Native supplies zero for each field.
    Outcome::reply(1453, vec![2, 0, 0, 0, 0, 0])
}
/// parity: 1452 21-quest-progress-handlers.md#1452--g_progressmissionallreqcmd--partial partial
pub fn handle_progress_mission_all(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let Some(missions) = counted(&mut Reader(req), 8, mission) else {
        return all_failure();
    };
    let Some(zone) = ctx.pc().zone else {
        return all_failure();
    };
    let t = task(ctx, 1452, zone, QuestProgressTask::ProgressAll { missions });
    queue(ctx, t, all_failure())
}

/// parity: 1454 21-quest-progress-handlers.md#1454--g_progressmissionbyacquiresummonedreqcmd--partial partial
pub fn handle_progress_mission_by_acquire_summoned(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let mut r = Reader(req);
    let decoded = (|| Some((r.short()?, r.byte()?, r.byte()?, r.quad()?, r.string(21)?)))();
    let Some(zone) = ctx.pc().zone else {
        return Outcome::reason(1455, 1);
    };
    if ctx.pc().job != 6 {
        return Outcome::reason(1455, 1060);
    }
    let Some((quest, mission, case, target, name)) = decoded else {
        return unchecked();
    };
    let name = party_model::name_prefix(&name).to_vec();
    let pc = ctx.pc().object_id;
    match ctx
        .services
        .quest_progress(&QuestProgressCheck::SummonedNameServicePresent)
    {
        Gate::Unknown => return Outcome::unsupported("❓ summoned name service presence"),
        Gate::Fail => {}
        Gate::Pass => {
            if !ctx.pc().user_present || ctx.pc().party_user_blocked {
                return Outcome::reason(1455, 2);
            }
            match ctx
                .services
                .quest_progress(&QuestProgressCheck::SummonedNameAllowed {
                    pc,
                    name: name.clone(),
                }) {
                Gate::Pass => {}
                Gate::Fail => return Outcome::reason(1455, 1062),
                Gate::Unknown => return Outcome::unsupported("❓ summoned name service result"),
            }
        }
    }
    match ctx
        .services
        .quest_progress(&QuestProgressCheck::SummonedNameFilter { name: name.clone() })
    {
        Gate::Pass => {}
        Gate::Fail => return Outcome::reason(1455, 1062),
        Gate::Unknown => return Outcome::unsupported("❓ summoned name rule filter"),
    }
    let t = task(
        ctx,
        ids::G_ProgressMissionByAcquireSummonedReqCmd,
        zone,
        QuestProgressTask::AcquireSummoned {
            quest,
            mission,
            case,
            target,
            name,
        },
    );
    queue(ctx, t, Outcome::reason(1455, 1))
}

/// parity: 1458 21-quest-progress-handlers.md#1458--g_disposequestreqcmd--partial partial
pub fn handle_dispose_quest(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let Some(quests) = counted(&mut Reader(req), 34, |r| r.short()) else {
        return Outcome::reason(1459, 2);
    };
    let Some(zone) = ctx.pc().zone else {
        return Outcome::reason(1459, 2);
    };
    let t = task(ctx, 1458, zone, QuestProgressTask::Dispose { quests });
    queue(ctx, t, Outcome::reason(1459, 2))
}

/// parity: 1465 21-quest-progress-handlers.md#1465--g_missionsteprollbackreqcmd--partial partial
pub fn handle_mission_step_rollback(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let mut r = Reader(req);
    let decoded = (r.short(), r.byte(), r.byte());
    let Some(zone) = ctx.pc().zone else {
        return Outcome::reason(1466, 2);
    };
    let (Some(quest), Some(mission), Some(step)) = decoded else {
        return unchecked();
    };
    let t = task(
        ctx,
        ids::G_MissionStepRollbackReqCmd,
        zone,
        QuestProgressTask::Rollback {
            quest,
            mission,
            step,
        },
    );
    queue(ctx, t, Outcome::reason(1466, 2))
}

/// parity: 1467 21-quest-progress-handlers.md#1467--g_missionsteptimeexpiredreqcmd--partial partial
pub fn handle_mission_step_time_expired(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let mut r = Reader(req);
    let decoded = (r.short(), r.byte());
    let Some(zone) = ctx.pc().zone else {
        return Outcome::reason(1468, 2);
    };
    let (Some(quest), Some(mission)) = decoded else {
        return unchecked();
    };
    let t = task(
        ctx,
        ids::G_MissionStepTimeExpiredReqCmd,
        zone,
        QuestProgressTask::TimeExpired { quest, mission },
    );
    queue(ctx, t, Outcome::reason(1468, 2))
}

/// parity: 1469 21-quest-progress-handlers.md#1469--g_resetquestcompletiontimereqcmd--partial partial
pub fn handle_reset_quest_completion_time(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_ResetQuestCompletionTimeFailCmd;
    let mut r = Reader(req);
    let Some(quest) = r.short() else {
        return Outcome::reason(F, 1091);
    };
    let Some(n) = r.short().map(|n| n as i16) else {
        return Outcome::reason(F, 1092);
    };
    if !(0..16).contains(&n) {
        return Outcome::reason(F, 1092);
    }
    let Some(orders) = (0..n)
        .map(|_| Some((r.short()?, r.short()?)))
        .collect::<Option<Vec<_>>>()
    else {
        return Outcome::reason(F, 1091);
    };
    let Some(zone) = ctx.pc().zone else {
        return Outcome::reason(F, 2);
    };
    let check = QuestProgressCheck::ResetRestricted {
        pc: ctx.pc().object_id,
    };
    match ctx.services.quest_progress(&check) {
        Gate::Pass => return Outcome::reason(F, 1098),
        Gate::Fail => {}
        Gate::Unknown => {
            return Outcome::unsupported("❓ reset quest zone attraction restriction +2089/+12544")
        }
    }
    if ctx.pc().native_kind != 1 {
        return Outcome::reason(F, 2);
    }
    let t = task(
        ctx,
        ids::G_ResetQuestCompletionTimeReqCmd,
        zone,
        QuestProgressTask::ResetCompletionTime { quest, orders },
    );
    queue(ctx, t, Outcome::reason(F, 2))
}

/// parity: 1472 21-quest-progress-handlers.md#1472--g_acquirerenewalquestreqcmd--partial partial
pub fn handle_acquire_renewal_quest(_ctx: &mut Context<'_>, _req: &[u8]) -> Outcome {
    let mut out = Outcome::reason(1474, 2);
    out.logs.push("acquire renewal quest: disabled");
    out
}

/// parity: 1475 21-quest-progress-handlers.md#1475--g_challengelistacquirequestreqcmd--partial partial
pub fn handle_challenge_list_acquire_quest(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let mut r = Reader(req);
    // Decode result ignored. The native locals start at -1 and zero.
    let sub_type = r.byte().unwrap_or(0xff);
    let quest = r.short().unwrap_or(0);
    if quest == 0 {
        return Outcome::reason(1477, 1111);
    }
    let Some(zone) = ctx.pc().zone else {
        return Outcome::reason(1477, 2);
    };
    if ctx.pc().native_kind != 1 || sub_type > 1 {
        return Outcome::reason(1477, 2);
    }
    let t = task(
        ctx,
        ids::G_ChallengeListAcquireQuestReqCmd,
        zone,
        QuestProgressTask::ChallengeAcquire { sub_type, quest },
    );
    queue(ctx, t, Outcome::reason(1477, 2))
}

fn skip_failure() -> Outcome {
    // Both native calls pass only reason 2 for hEhQ. The two stack arguments
    // are not set. Do not substitute request fields or emit a short packet.
    Outcome::unsupported("❓ reply 1480 hEhQ: native quest/reason arguments are unset")
}
/// parity: 1478 21-quest-progress-handlers.md#1478--g_progressskipmissionstepreqcmd--partial partial
pub fn handle_progress_skip_mission_step(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let mut r = Reader(req);
    let decoded = (r.short(), r.byte());
    let Some(zone) = ctx.pc().zone else {
        return skip_failure();
    };
    let (Some(quest), Some(reason)) = decoded else {
        return unchecked();
    };
    let t = task(
        ctx,
        ids::G_ProgressSkipMissionStepReqCmd,
        zone,
        QuestProgressTask::SkipMissionStep { quest, reason },
    );
    queue(ctx, t, skip_failure())
}
