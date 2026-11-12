//! Doc 26: arena-event (GM) and arena dungeon recruit entry functions.
//! Build 381.8867.
//!
//! Zone tasks use `skills::queue`. Arena portal records, geo zones and
//! backend sends are opaque and go through `Services::arena_event`.
use super::party::string_body;
use super::skills::queue;
use super::*;

pub const ARENA_EVENT_COMMANDS: &[u16] = &[
    ids::G_EnterArenaReqCmd,
    ids::G_ReturnToArenaLobbyFromDuelReqCmd,
    ids::G_ReturnToArenaLobbyFromDungeonReqCmd,
    ids::G_ArenaSpectateGameReqCmd,
    ids::G_ArenaSpectateReturnToArenaLobbyReqCmd,
    ids::G_ListArenaSpectateGameReqCmd,
    ids::G_ArenaEventTeleportToReqCmd,
    ids::G_ArenaEventRecallReqCmd,
    ids::G_ArenaEventWarpToPosReqCmd,
    ids::G_ArenaEventDuelGameStartReqCmd,
    ids::G_ArenaEventFindArenaSessionReqCmd,
    ids::G_ArenaEventMoveToArenaReqCmd,
    ids::G_ArenaEventReturnToArenaLobbyReqCmd,
    ids::G_ArenaEventReturnToArenaLobbyAllPcReqCmd,
    ids::G_ArenaEventRetreivePcAssetReqCmd,
    ids::G_ArenaEventWarpToDuelRoundStartPcSpawnReqCmd,
    ids::G_ArenaEventSetBattleRoyalStateReqCmd,
    ids::G_ArenaEventInZonePcLocReqCmd,
    ids::G_ArenaEventBattleRoyalScoreReqCmd,
    ids::G_ChoiceReturnToArenaLobbyReqCmd,
    ids::G_PublishRecruitArenaDungeonMemberReqCmd,
    ids::G_CancelRecruitArenaDungeonMemberReqCmd,
];

/// Doc 26 shared fields.
#[derive(Clone, Debug, Default)]
pub struct ArenaEventWorld {
    /// `qword_140EE58A0` byte +792: arena dungeon recruit is enabled (959/963).
    pub recruit_enabled: bool,
    /// Daemon byte +4152: arena attraction of this daemon (1 duel, 2 dungeon ❓ names).
    pub attraction: i8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArenaEventTask {
    /// 723, kind 158, vtable 0x140BAC8D8. Target PC +144.
    TeleportTo { target: u64 },
    /// 726, 0xA8 bytes, base 0x14028DF50 type 15, vtable 0x140AE66D8.
    /// Init 0x140279630(task, 0, zone, pc, target).
    Recall { target: u64 },
    /// 729, 0xA8 bytes, vtable 0x140A91A48, init 0x14027FD10.
    /// DWORD +152 = geo zone ID (the adapter resolves it), words +156/+158/+160.
    WarpToPos { x: u16, y: u16, z: u16 },
    /// 732, DuelTask 0x14036D860, init 0x1402B8010 mode 2.
    DuelGameStart,
    /// 739, kind 159, vtable 0x140BC2450. Portal record +144.
    ReturnToArenaLobby { portal_key: u64 },
    /// 742, kind 328, vtable 0x140B99D08. Portal record +144.
    ReturnToArenaLobbyAllPc { portal_key: u64 },
    /// 745, kind 181, vtable 0x140BB94B8. Caster = target PC, requester +144.
    RetrievePcAsset { requester: u64 },
    /// 748, kind 182, vtable 0x140BC1A38. Byte +144.
    WarpToDuelRoundStartPcSpawn { flag: u8 },
    /// 751, 0xA0 bytes, vtable 0x140BC1A90, init 0x14027FB90. Byte +152 (1 standby, 4 finish).
    SetBattleRoyalState { state: u8 },
    /// 754, kind 331, vtable 0x140B96E28.
    InZonePcLoc,
    /// 757, kind 338, vtable 0x140BC1E08.
    BattleRoyalScore,
    /// 959 / 963: CheckArenaPortalUsableTask 0x14036D670, kind 112, vtable 0x140B9E890.
    /// Portal record +144, mode DWORD +152 (2 publish, 3 cancel).
    CheckPortal { portal_key: u64, mode: u32 },
    /// 708, kind 125, 0x98 bytes, vtable 0x140BA9F00. Portal record +144.
    ReturnFromDuel { portal_key: u64 },
    /// 711, kind 126, 0xA0 bytes, vtable 0x140C36C10. Portal record +144, bool +152.
    ReturnFromDungeon { portal_key: u64, flag: bool },
    /// 717, kind 263, 0x98 bytes, vtable 0x140BAB4E0. Portal record +144.
    SpectateReturn { portal_key: u64 },
}
impl From<ArenaEventTask> for SkillTaskKind {
    fn from(task: ArenaEventTask) -> Self {
        SkillTaskKind::ArenaEvent(task)
    }
}

/// Opaque server operations. Unknown is never approval.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArenaEventOperation {
    /// 705: connection attach (virtual +32), then 0x1405FAD30. No native failure reply.
    AttachEntry { object_id: u64 },
    /// 705: 0x1403F4E20, add to the name table. Fail replies 707 [1].
    RegisterEntryName { object_id: u64 },
    /// 955: 0x140044850. Pass means an entry is already pending (silent stop).
    ChoicePending { db_id: u64 },
    /// 955: 0x1400D0FC0 routes to a destination daemon, not the arena send path.
    ChoiceStaySend {
        world: i16,
        destination: u32,
        command: u16,
        descriptor: &'static str,
        body: Vec<u8>,
    },
    /// DataManager slot 87 virtual +168. Fail means no portal record.
    PortalExists { portal_key: u64 },
    /// Zone qword +16224 (geo zone). Fail means null.
    GeoZone { zone: u64 },
    /// Portal record exists and its type word +2 is in `types`.
    PortalType {
        portal_key: u64,
        types: &'static [i16],
    },
    /// Global config byte +819 (`qword_140EE58A0`).
    CrossOptionEnabled,
    /// 720: user list-time slot `list_type` (user virtual +48, +200, +16 + 8 × type)
    /// plus global DWORD index 207 seconds is not in the future. Pass stamps now.
    SpectateListCooldown { list_type: u8 },
    /// Body-only send through daemon +3840 (0x1400D2EE0) to world `world`.
    Send {
        world: i16,
        command: u16,
        descriptor: &'static str,
        body: Vec<u8>,
    },
}

/// 705: keys at pending-entry +0/+32/+24. A successful lookup consumes the entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArenaEntryKey {
    pub key_a: u64,
    pub world: u16,
    pub key_b: u64,
}
#[derive(Clone, Debug)]
pub struct ArenaPendingEntry {
    pub key: ArenaEntryKey,
    /// None models a missing +720 object or its byte +1017 being set.
    pub pc: Option<Box<Pc>>,
}
#[derive(Clone, Debug, Default)]
pub enum ArenaEntryResult {
    #[default]
    Unknown,
    Missing,
    Found(Option<Box<Pc>>),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArenaStayRecord {
    /// Session record qword +120 and destination DWORD +136.
    pub route: u64,
    pub destination: u32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArenaChoicePortal {
    pub key: u64,
    pub portal_type: i16,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArenaChoiceQuery {
    Session {
        db_id: u64,
    },
    LastAttraction {
        db_id: u64,
    },
    /// First matching table record; key 0 when no match. Lookup follows the scan.
    Portal {
        attraction: u8,
    },
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ArenaChoiceResult {
    #[default]
    Unknown,
    Missing,
    Session(ArenaStayRecord),
    Attraction(u8),
    PortalKey(u64),
}

#[derive(Clone, Debug)]
pub struct MemoryArenaEvent {
    /// None is unknown. Some(empty) is a known empty pending-entry list.
    pub pending_entries: Option<Vec<ArenaPendingEntry>>,
    pub entry_lookups: Vec<ArenaEntryKey>,
    pub entry_attach: Gate,
    pub entry_name: Gate,
    pub choice_pending: Gate,
    /// Outer None is unknown; inner None is a known absent record.
    pub choice_session: Option<Option<ArenaStayRecord>>,
    pub last_attraction: Option<Option<u8>>,
    pub choice_portals: Option<Vec<ArenaChoicePortal>>,
    pub choice_queries: Vec<ArenaChoiceQuery>,
    pub portal: Gate,
    /// Portal type word +2. None = Unknown.
    pub portal_type: Option<i16>,
    pub cross_option: Gate,
    pub list_cooldown: Gate,
    pub geo_zone: Gate,
    pub send: Gate,
    pub calls: Vec<ArenaEventOperation>,
}
impl Default for MemoryArenaEvent {
    fn default() -> Self {
        Self {
            pending_entries: None,
            entry_lookups: vec![],
            entry_attach: Gate::Unknown,
            entry_name: Gate::Unknown,
            choice_pending: Gate::Unknown,
            choice_session: None,
            last_attraction: None,
            choice_portals: None,
            choice_queries: vec![],
            portal: Gate::Unknown,
            portal_type: None,
            cross_option: Gate::Unknown,
            list_cooldown: Gate::Unknown,
            geo_zone: Gate::Unknown,
            send: Gate::Unknown,
            calls: Vec::new(),
        }
    }
}
impl MemoryArenaEvent {
    pub fn take_entry(&mut self, key: &ArenaEntryKey) -> ArenaEntryResult {
        self.entry_lookups.push(key.clone());
        let Some(entries) = self.pending_entries.as_mut() else {
            return ArenaEntryResult::Unknown;
        };
        match entries.iter().position(|entry| entry.key == *key) {
            Some(index) => ArenaEntryResult::Found(entries.swap_remove(index).pc),
            None => ArenaEntryResult::Missing,
        }
    }
    pub fn choice(&mut self, query: &ArenaChoiceQuery) -> ArenaChoiceResult {
        self.choice_queries.push(query.clone());
        match query {
            ArenaChoiceQuery::Session { .. } => match &self.choice_session {
                Some(Some(record)) => ArenaChoiceResult::Session(record.clone()),
                Some(None) => ArenaChoiceResult::Missing,
                None => ArenaChoiceResult::Unknown,
            },
            ArenaChoiceQuery::LastAttraction { .. } => match self.last_attraction {
                Some(Some(attraction)) => ArenaChoiceResult::Attraction(attraction),
                Some(None) => ArenaChoiceResult::Missing,
                None => ArenaChoiceResult::Unknown,
            },
            ArenaChoiceQuery::Portal { attraction } => match &self.choice_portals {
                Some(records) => ArenaChoiceResult::PortalKey(
                    records
                        .iter()
                        .find(|record| {
                            // 0x140042750 accepts 7, 8, 10. It reads no portal field.
                            (matches!(attraction, 7 | 8 | 10) && record.portal_type == 2)
                                || ((1..=6).contains(attraction) && record.portal_type == 1)
                        })
                        .map_or(0, |record| record.key),
                ),
                None => ArenaChoiceResult::Unknown,
            },
        }
    }
    pub fn check(&mut self, operation: &ArenaEventOperation) -> Gate {
        self.calls.push(operation.clone());
        match operation {
            ArenaEventOperation::AttachEntry { .. } => self.entry_attach,
            ArenaEventOperation::RegisterEntryName { .. } => self.entry_name,
            ArenaEventOperation::ChoicePending { .. } => self.choice_pending,
            ArenaEventOperation::ChoiceStaySend { .. } => self.send,
            ArenaEventOperation::PortalExists { .. } => self.portal,
            ArenaEventOperation::GeoZone { .. } => self.geo_zone,
            ArenaEventOperation::Send { .. } => self.send,
            ArenaEventOperation::PortalType { types, .. } => match self.portal_type {
                Some(t) if types.contains(&t) => Gate::Pass,
                Some(_) => Gate::Fail,
                None => Gate::Unknown,
            },
            ArenaEventOperation::CrossOptionEnabled => self.cross_option,
            ArenaEventOperation::SpectateListCooldown { .. } => self.list_cooldown,
        }
    }
}

pub(super) fn dispatch_arena_event(ctx: &mut Context<'_>, command: u16, req: &[u8]) -> Outcome {
    match command {
        ids::G_EnterArenaReqCmd => handle_enter_arena(ctx, req),
        ids::G_ChoiceReturnToArenaLobbyReqCmd => handle_choice_return_to_arena_lobby(ctx, req),
        ids::G_ReturnToArenaLobbyFromDuelReqCmd => handle_return_to_arena_lobby_from_duel(ctx, req),
        ids::G_ReturnToArenaLobbyFromDungeonReqCmd => {
            handle_return_to_arena_lobby_from_dungeon(ctx, req)
        }
        ids::G_ArenaSpectateGameReqCmd => handle_arena_spectate_game(ctx, req),
        ids::G_ArenaSpectateReturnToArenaLobbyReqCmd => {
            handle_arena_spectate_return_to_arena_lobby(ctx, req)
        }
        ids::G_ListArenaSpectateGameReqCmd => handle_list_arena_spectate_game(ctx, req),
        ids::G_ArenaEventTeleportToReqCmd => handle_arena_event_teleport_to(ctx, req),
        ids::G_ArenaEventRecallReqCmd => handle_arena_event_recall(ctx, req),
        ids::G_ArenaEventWarpToPosReqCmd => handle_arena_event_warp_to_pos(ctx, req),
        ids::G_ArenaEventDuelGameStartReqCmd => handle_arena_event_duel_game_start(ctx, req),
        ids::G_ArenaEventFindArenaSessionReqCmd => handle_arena_event_find_arena_session(ctx, req),
        ids::G_ArenaEventMoveToArenaReqCmd => handle_arena_event_move_to_arena(ctx, req),
        ids::G_ArenaEventReturnToArenaLobbyReqCmd => {
            handle_arena_event_return_to_arena_lobby(ctx, req)
        }
        ids::G_ArenaEventReturnToArenaLobbyAllPcReqCmd => {
            handle_arena_event_return_to_arena_lobby_all_pc(ctx, req)
        }
        ids::G_ArenaEventRetreivePcAssetReqCmd => handle_arena_event_retrieve_pc_asset(ctx, req),
        ids::G_ArenaEventWarpToDuelRoundStartPcSpawnReqCmd => {
            handle_arena_event_warp_to_duel_round_start(ctx, req)
        }
        ids::G_ArenaEventSetBattleRoyalStateReqCmd => {
            handle_arena_event_set_battle_royal_state(ctx, req)
        }
        ids::G_ArenaEventInZonePcLocReqCmd => handle_arena_event_in_zone_pc_loc(ctx, req),
        ids::G_ArenaEventBattleRoyalScoreReqCmd => handle_arena_event_battle_royal_score(ctx, req),
        ids::G_PublishRecruitArenaDungeonMemberReqCmd => {
            handle_publish_recruit_arena_dungeon_member(ctx, req)
        }
        ids::G_CancelRecruitArenaDungeonMemberReqCmd => {
            handle_cancel_recruit_arena_dungeon_member(ctx, req)
        }
        _ => unreachable!("membership checked"),
    }
}

fn unknown() -> Outcome {
    Outcome::unsupported("❓ arena event portal, geo zone or send adapter")
}
fn gate(ctx: &mut Context<'_>, operation: ArenaEventOperation) -> Option<bool> {
    match ctx.services.arena_event(&operation) {
        Gate::Pass => Some(true),
        Gate::Fail => Some(false),
        Gate::Unknown => None,
    }
}
/// PC byte +532 in 1..=7 (`CheckArenaEventPacketUsable`).
fn authorized(pc: &Pc) -> bool {
    (1..=7).contains(&pc.gm_grade)
}
/// cfg +237 ≠ 0 and cfg +248 ≠ 0: this daemon is an arena daemon.
fn in_arena(ctx: &Context<'_>) -> bool {
    ctx.world.party_config.arena_enabled && ctx.world.arena
}
fn task_for(command: u16, caster: u64, zone: u64, kind: ArenaEventTask) -> SkillTask {
    SkillTask {
        command,
        caster,
        summon: false,
        zone,
        kind: kind.into(),
    }
}
fn task(ctx: &Context<'_>, command: u16, zone: u64, kind: ArenaEventTask) -> SkillTask {
    task_for(command, ctx.pc().object_id, zone, kind)
}
/// Queue, then send an empty success reply (descriptor `h`, `asc_1408CFBE8`).
fn queue_reply(ctx: &mut Context<'_>, t: SkillTask, ok: u16, fail: Outcome) -> Outcome {
    let mut out = queue(ctx, t, fail);
    if out
        .changes
        .iter()
        .any(|c| matches!(c, StateChange::Queued(_)))
    {
        out.replies.push(Message {
            command: ok,
            body: Vec::new(),
        });
    }
    out
}
fn logged(mut out: Outcome, text: &'static str) -> Outcome {
    out.logs.push(text);
    out
}

/// parity: 705 26-arena-event-handlers.md#705--g_enterarenareqcmd--partial partial
pub fn handle_enter_arena(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let mut r = Reader(req);
    let (Some(key_a), Some(world), Some(key_b)) = (r.quad(), r.short(), r.quad()) else {
        return Outcome {
            status: Status::NativeFalse,
            logs: vec!["arena entry: decompose failed"],
            ..Outcome::default()
        };
    };
    let fail = |reason| {
        let mut out = Outcome::reason(707, reason);
        out.status = Status::NativeFalse;
        out
    };
    let key = ArenaEntryKey {
        key_a,
        world,
        key_b,
    };
    let pc = match ctx.services.take_arena_entry(&key) {
        ArenaEntryResult::Unknown => return unknown(),
        ArenaEntryResult::Missing => return fail(1887),
        ArenaEntryResult::Found(None) => return fail(2),
        ArenaEntryResult::Found(Some(pc)) => pc,
    };
    // The adapter owns the connection attach. It has no native failure branch.
    let object_id = pc.object_id;
    if gate(ctx, ArenaEventOperation::AttachEntry { object_id }) != Some(true) {
        return unknown();
    }
    ctx.session.pc = Some(*pc);
    match gate(ctx, ArenaEventOperation::RegisterEntryName { object_id }) {
        Some(true) => {}
        Some(false) => return fail(1),
        None => return unknown(),
    }
    // Stop before the restricted probe. Do not mark world entry as complete.
    Outcome::unsupported("❓ arena entry: restricted probe skipped; login history and world entry")
}

/// parity: 955 26-arena-event-handlers.md#955--g_choicereturntoarenalobbyreqcmd--partial partial
pub fn handle_choice_return_to_arena_lobby(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_ChoiceReturnToArenaLobbyFailCmd;
    if !ctx.world.party_config.arena_enabled {
        return Outcome::reason(F, 2206);
    }
    if ctx.world.arena {
        return Outcome::reason(F, 5);
    }
    let Some(choice) = Reader(req).byte() else {
        return Outcome::reason(F, 2);
    };
    let db_id = ctx.pc().db_id;
    match gate(ctx, ArenaEventOperation::ChoicePending { db_id }) {
        Some(true) => return Outcome::log("arena choice: already pending"),
        Some(false) => {}
        None => return unknown(),
    }
    if choice == 0 {
        let record = match ctx
            .services
            .arena_choice(&ArenaChoiceQuery::Session { db_id })
        {
            ArenaChoiceResult::Session(record) => record,
            ArenaChoiceResult::Missing => return Outcome::reason(F, 2208),
            _ => return unknown(),
        };
        let mut body = vec![1];
        body.extend(db_id.to_le_bytes());
        body.extend(record.route.to_le_bytes());
        string_body(&mut body, party_model::name_prefix(&ctx.pc().name));
        let operation = ArenaEventOperation::ChoiceStaySend {
            world: ctx.pc().short_96,
            destination: record.destination,
            command: 493,
            descriptor: "hQqqs",
            body,
        };
        return match gate(ctx, operation) {
            Some(true) if ctx.world.misc.trace_route_log => {
                Outcome::log("arena choice: stay telemetry")
            }
            Some(true) => Outcome::default(),
            Some(false) => Outcome::reason(F, 5),
            None => unknown(),
        };
    }
    let pc = ctx.pc();
    if !pc.user_present || pc.party_user_blocked || pc.party_user_state != 0 {
        return Outcome::reason(F, 2209);
    }
    let attraction = match ctx
        .services
        .arena_choice(&ArenaChoiceQuery::LastAttraction { db_id })
    {
        ArenaChoiceResult::Attraction(attraction) => attraction,
        ArenaChoiceResult::Missing => return Outcome::reason(F, 2208),
        _ => return unknown(),
    };
    let portal_key = match ctx
        .services
        .arena_choice(&ArenaChoiceQuery::Portal { attraction })
    {
        ArenaChoiceResult::PortalKey(key) => key,
        _ => return unknown(),
    };
    match portal(ctx, portal_key) {
        Some(true) => {}
        Some(false) => return Outcome::reason(F, 1687),
        None => return unknown(),
    }
    let Some(zone) = ctx.pc().zone else {
        return Outcome::reason(F, 2);
    };
    let t = task(
        ctx,
        ids::G_ChoiceReturnToArenaLobbyReqCmd,
        zone,
        ArenaEventTask::CheckPortal {
            portal_key,
            mode: 1,
        },
    );
    queue(ctx, t, Outcome::reason(F, 2))
}

/// parity: 723 26-arena-event-handlers.md#723--g_arenaeventteleporttoreqcmd--partial partial
pub fn handle_arena_event_teleport_to(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_ArenaEventTeleportToFailCmd;
    if !authorized(ctx.pc()) {
        return logged(
            Outcome::reason(F, 2),
            "arena event teleport: not authorized",
        );
    }
    let Some(name) = Reader(req).string(33) else {
        return logged(
            Outcome::reason(F, 2),
            "arena event teleport: decompose failed",
        );
    };
    let Some(target) = ctx.world.mentoring.by_name(&name, 6).map(|pc| pc.object_id) else {
        return logged(
            Outcome::reason(F, 1916),
            "arena event teleport: target is null",
        );
    };
    let Some(zone) = ctx.pc().zone else {
        return logged(
            Outcome::reason(F, 2),
            "arena event teleport: cannot find zone",
        );
    };
    let t = task(ctx, 723, zone, ArenaEventTask::TeleportTo { target });
    queue(ctx, t, Outcome::reason(F, 2))
}

/// parity: 726 26-arena-event-handlers.md#726--g_arenaeventrecallreqcmd--partial partial
pub fn handle_arena_event_recall(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_ArenaEventRecallFailCmd;
    if !in_arena(ctx) {
        return logged(Outcome::reason(F, 2), "arena event recall: not is arena");
    }
    if !authorized(ctx.pc()) {
        return logged(Outcome::reason(F, 2), "arena event recall: not authorized");
    }
    let Some(name) = Reader(req).string(33) else {
        return logged(
            Outcome::reason(F, 2),
            "arena event recall: decompose failed",
        );
    };
    let Some((target, target_zone)) = ctx
        .world
        .mentoring
        .by_name(&name, 6)
        .map(|pc| (pc.object_id, pc.zone))
    else {
        return logged(
            Outcome::reason(F, 1917),
            "arena event recall: failed to find pc",
        );
    };
    let Some(zone) = ctx.pc().zone else {
        return logged(Outcome::reason(F, 2), "arena event recall: zone is null");
    };
    if target_zone != Some(zone) {
        return logged(
            Outcome::reason(F, 1918),
            "arena event recall: different zone",
        );
    }
    let t = task(ctx, 726, zone, ArenaEventTask::Recall { target });
    queue_reply(ctx, t, 727, Outcome::reason(F, 2))
}

/// parity: 729 26-arena-event-handlers.md#729--g_arenaeventwarptoposreqcmd--partial partial
pub fn handle_arena_event_warp_to_pos(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_ArenaEventWarpToPosFailCmd;
    if !in_arena(ctx) {
        return logged(Outcome::reason(F, 2), "arena event warp: not is arena");
    }
    if !authorized(ctx.pc()) {
        return logged(Outcome::reason(F, 2), "arena event warp: not authorized");
    }
    let mut r = Reader(req);
    let (Some(x), Some(y), Some(z)) = (r.short(), r.short(), r.short()) else {
        return logged(Outcome::reason(F, 2), "arena event warp: decompose failed");
    };
    let Some(zone) = ctx.pc().zone else {
        return logged(Outcome::reason(F, 2), "arena event warp: zone is null");
    };
    match gate(ctx, ArenaEventOperation::GeoZone { zone }) {
        Some(true) => {}
        Some(false) => return logged(Outcome::reason(F, 2), "arena event warp: geo zone is null"),
        None => return unknown(),
    }
    let t = task(ctx, 729, zone, ArenaEventTask::WarpToPos { x, y, z });
    queue_reply(ctx, t, 730, Outcome::reason(F, 2))
}

/// parity: 732 26-arena-event-handlers.md#732--g_arenaeventduelgamestartreqcmd--partial partial
pub fn handle_arena_event_duel_game_start(ctx: &mut Context<'_>, _req: &[u8]) -> Outcome {
    // Native quirk: every failure replies 282 (G_DuelReadyRoundAck), not 734.
    const F: u16 = ids::G_DuelReadyRoundFailCmd;
    if !ctx.world.party_config.arena_enabled {
        return logged(
            Outcome::reason(F, 2),
            "arena event duel start: arena not enabled",
        );
    }
    if !ctx.world.arena {
        return logged(
            Outcome::reason(F, 2),
            "arena event duel start: not arena daemon",
        );
    }
    if ctx.world.arena_event.attraction != 1 {
        return logged(
            Outcome::reason(F, 2),
            "arena event duel start: invalid attraction",
        );
    }
    if !authorized(ctx.pc()) {
        return logged(
            Outcome::reason(F, 2),
            "arena event duel start: not authorized",
        );
    }
    let Some(zone) = ctx.pc().zone else {
        return logged(
            Outcome::reason(F, 2),
            "arena event duel start: cannot find zone",
        );
    };
    let t = task(ctx, 732, zone, ArenaEventTask::DuelGameStart);
    queue(ctx, t, Outcome::reason(F, 2))
}

fn send(
    ctx: &mut Context<'_>,
    command: u16,
    descriptor: &'static str,
    body: Vec<u8>,
) -> Option<bool> {
    let world = ctx.pc().short_96;
    gate(
        ctx,
        ArenaEventOperation::Send {
            world,
            command,
            descriptor,
            body,
        },
    )
}

/// parity: 733 26-arena-event-handlers.md#733--g_arenaeventfindarenasessionreqcmd--partial partial
pub fn handle_arena_event_find_arena_session(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_ArenaEventFindArenaSessionFailCmd;
    if !ctx.world.party_config.arena_enabled {
        return logged(
            Outcome::reason(F, 1919),
            "arena event find session: arena not enabled",
        );
    }
    if ctx.world.arena {
        return logged(
            Outcome::reason(F, 5),
            "arena event find session: already in arena",
        );
    }
    if !authorized(ctx.pc()) {
        return logged(
            Outcome::reason(F, 2),
            "arena event find session: not authorized",
        );
    }
    let mut r = Reader(req);
    let (Some(kind), Some(name)) = (r.byte(), r.string(128)) else {
        return logged(
            Outcome::reason(F, 2),
            "arena event find session: decompose failed",
        );
    };
    // 0 → 1; 1 or 2 → 2; other → invalid.
    let attraction = match kind as i8 {
        0 => 1u8,
        1 | 2 => 2,
        _ => {
            return logged(
                Outcome::reason(F, 2),
                "arena event find session: invalid attraction",
            );
        }
    };
    let pc = ctx.pc();
    let mut body = pc.db_id.to_le_bytes().to_vec();
    body.extend(pc.object_id.to_le_bytes());
    body.push(attraction);
    string_body(&mut body, &name);
    match send(ctx, 140, "hqqQs", body) {
        Some(true) => Outcome::default(),
        Some(false) => logged(
            Outcome::reason(F, 1919),
            "arena event find session: arena disabled",
        ),
        None => unknown(),
    }
}

/// parity: 736 26-arena-event-handlers.md#736--g_arenaeventmovetoarenareqcmd--partial partial
pub fn handle_arena_event_move_to_arena(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_ArenaEventMoveToArenaFailCmd;
    if !ctx.world.party_config.arena_enabled {
        return logged(
            Outcome::reason(F, 1921),
            "arena event move: arena not enabled",
        );
    }
    if ctx.world.arena {
        return logged(Outcome::reason(F, 5), "arena event move: already in arena");
    }
    if !authorized(ctx.pc()) {
        return logged(Outcome::reason(F, 2), "arena event move: not authorized");
    }
    let Some(arena_id) = Reader(req).int().map(|v| v as i32) else {
        return logged(Outcome::reason(F, 2), "arena event move: decompose failed");
    };
    if arena_id <= 0 {
        return logged(Outcome::reason(F, 2), "arena event move: invalid arena id");
    }
    let pc = ctx.pc();
    let reason = if pc.arena_state != 3 {
        1922
    } else if pc.arena_qwords[0] == 0 {
        1923
    } else {
        0
    };
    if reason != 0 {
        return logged(Outcome::reason(F, reason), "arena event move: fail to send");
    }
    let mut body = pc.arena_lobby.route.to_le_bytes().to_vec();
    body.extend(pc.db_id.to_le_bytes());
    body.extend(pc.object_id.to_le_bytes());
    body.extend(pc.short_96.to_le_bytes());
    body.extend(arena_id.to_le_bytes());
    // Trailing `Q` is the constant 2 (`mov [rsp+var_30], 2` at 0x1403BDF25).
    body.push(2);
    match send(ctx, 137, "hqqqhiQ", body) {
        Some(true) => Outcome::default(),
        Some(false) => logged(Outcome::reason(F, 1921), "arena event move: fail to send"),
        None => unknown(),
    }
}

fn portal(ctx: &mut Context<'_>, portal_key: u64) -> Option<bool> {
    gate(ctx, ArenaEventOperation::PortalExists { portal_key })
}

/// parity: 739 26-arena-event-handlers.md#739--g_arenaeventreturntoarenalobbyreqcmd--partial partial
pub fn handle_arena_event_return_to_arena_lobby(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_ArenaEventReturnToArenaLobbyFailCmd;
    if !ctx.world.party_config.arena_enabled {
        return logged(
            Outcome::reason(F, 1928),
            "arena event return: arena not enabled",
        );
    }
    if !ctx.world.arena {
        return logged(Outcome::reason(F, 1929), "arena event return: not in arena");
    }
    if !authorized(ctx.pc()) {
        return logged(Outcome::reason(F, 2), "arena event return: not authorized");
    }
    let Some(portal_key) = Reader(req).quad() else {
        return logged(
            Outcome::reason(F, 2),
            "arena event return: decompose failed",
        );
    };
    match portal(ctx, portal_key) {
        Some(true) => {}
        Some(false) => {
            return logged(
                Outcome::reason(F, 1927),
                "arena event return: invalid portal key",
            )
        }
        None => return unknown(),
    }
    let Some(zone) = ctx.pc().zone else {
        return logged(Outcome::reason(F, 2), "arena event return: zone is null");
    };
    let t = task(
        ctx,
        ids::G_ArenaEventReturnToArenaLobbyReqCmd,
        zone,
        ArenaEventTask::ReturnToArenaLobby { portal_key },
    );
    queue(ctx, t, Outcome::reason(F, 2))
}

/// parity: 742 26-arena-event-handlers.md#742--g_arenaeventreturntoarenalobbyallpcreqcmd--partial partial
pub fn handle_arena_event_return_to_arena_lobby_all_pc(
    ctx: &mut Context<'_>,
    req: &[u8],
) -> Outcome {
    const F: u16 = ids::G_ArenaEventReturnToArenaLobbyAllPcFailCmd;
    if !in_arena(ctx) {
        return logged(
            Outcome::reason(F, 2),
            "arena event return all: not is arena",
        );
    }
    if !authorized(ctx.pc()) {
        return logged(
            Outcome::reason(F, 2),
            "arena event return all: not authorized",
        );
    }
    let Some(portal_key) = Reader(req).quad() else {
        return logged(
            Outcome::reason(F, 2),
            "arena event return all: decompose failed",
        );
    };
    match portal(ctx, portal_key) {
        Some(true) => {}
        Some(false) => {
            return logged(
                Outcome::reason(F, 2),
                "arena event return all: invalid portal key",
            )
        }
        None => return unknown(),
    }
    let Some(zone) = ctx.pc().zone else {
        return logged(
            Outcome::reason(F, 2),
            "arena event return all: zone is null",
        );
    };
    let t = task(
        ctx,
        ids::G_ArenaEventReturnToArenaLobbyAllPcReqCmd,
        zone,
        ArenaEventTask::ReturnToArenaLobbyAllPc { portal_key },
    );
    // Native quirk: the queue failure replies 741, not 744.
    queue_reply(ctx, t, 743, Outcome::reason(741, 2))
}

/// parity: 745 26-arena-event-handlers.md#745--g_arenaeventretreivepcassetreqcmd--partial partial
pub fn handle_arena_event_retrieve_pc_asset(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_ArenaEventRetreivePcAssetFailCmd;
    if !authorized(ctx.pc()) {
        return logged(
            Outcome::reason(F, 2),
            "arena event pc asset: not authorized",
        );
    }
    let Some(name) = Reader(req).string(33) else {
        return logged(
            Outcome::reason(F, 2),
            "arena event pc asset: decompose failed",
        );
    };
    let Some((target, target_zone)) = ctx
        .world
        .mentoring
        .by_name(&name, 6)
        .map(|pc| (pc.object_id, pc.zone))
    else {
        return logged(
            Outcome::reason(F, 1932),
            "arena event pc asset: cannot find target",
        );
    };
    let Some(zone) = target_zone else {
        return logged(Outcome::reason(F, 2), "arena event pc asset: zone is null");
    };
    let requester = ctx.pc().object_id;
    let t = task_for(
        ids::G_ArenaEventRetreivePcAssetReqCmd,
        target,
        zone,
        ArenaEventTask::RetrievePcAsset { requester },
    );
    queue(ctx, t, Outcome::reason(F, 2))
}

/// parity: 748 26-arena-event-handlers.md#748--g_arenaeventwarptoduelroundstartpcspawnreqcmd--partial partial
pub fn handle_arena_event_warp_to_duel_round_start(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_ArenaEventWarpToDuelRoundStartPcSpawnFailCmd;
    if !ctx.world.party_config.arena_enabled {
        return logged(
            Outcome::reason(F, 1933),
            "arena event duel spawn: arena not enabled",
        );
    }
    if !ctx.world.arena {
        return logged(
            Outcome::reason(F, 1934),
            "arena event duel spawn: not in arena",
        );
    }
    if !authorized(ctx.pc()) {
        return logged(
            Outcome::reason(F, 2),
            "arena event duel spawn: not authorized",
        );
    }
    let Some(flag) = Reader(req).byte() else {
        return logged(
            Outcome::reason(F, 2),
            "arena event duel spawn: decompose failed",
        );
    };
    if flag > 1 {
        return logged(
            Outcome::reason(F, 2),
            "arena event duel spawn: invalid param",
        );
    }
    let Some(zone) = ctx.pc().zone else {
        return logged(
            Outcome::reason(F, 2),
            "arena event duel spawn: zone is null",
        );
    };
    let t = task(
        ctx,
        ids::G_ArenaEventWarpToDuelRoundStartPcSpawnReqCmd,
        zone,
        ArenaEventTask::WarpToDuelRoundStartPcSpawn { flag },
    );
    queue(ctx, t, Outcome::reason(F, 2))
}

/// Case-insensitive ASCII compare, like `wcsicmp` for these literals.
fn eq_ignore_case(text: &[u16], literal: &str) -> bool {
    text.len() == literal.len()
        && text
            .iter()
            .zip(literal.bytes())
            .all(|(&a, b)| a < 0x80 && (a as u8).eq_ignore_ascii_case(&b))
}

/// parity: 751 26-arena-event-handlers.md#751--g_arenaeventsetbattleroyalstatereqcmd--partial partial
pub fn handle_arena_event_set_battle_royal_state(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_ArenaEventSetBattleRoyalStateFailCmd;
    if !in_arena(ctx) {
        return logged(
            Outcome::reason(F, 2),
            "arena event battle royal: not is arena",
        );
    }
    if !authorized(ctx.pc()) {
        return logged(
            Outcome::reason(F, 2),
            "arena event battle royal: not authorized",
        );
    }
    let Some(text) = Reader(req).string(128) else {
        return logged(
            Outcome::reason(F, 2),
            "arena event battle royal: decompose failed",
        );
    };
    let Some(zone) = ctx.pc().zone else {
        return logged(
            Outcome::reason(F, 2),
            "arena event battle royal: zone is null",
        );
    };
    let text: Vec<u16> = text.into_iter().take_while(|&c| c != 0).collect();
    let state = if eq_ignore_case(&text, "standby") {
        1
    } else if eq_ignore_case(&text, "finish") {
        4
    } else {
        return logged(
            Outcome::reason(F, 2),
            "arena event battle royal: failed to initialize",
        );
    };
    let t = task(
        ctx,
        ids::G_ArenaEventSetBattleRoyalStateReqCmd,
        zone,
        ArenaEventTask::SetBattleRoyalState { state },
    );
    queue_reply(ctx, t, 752, Outcome::reason(F, 2))
}

fn zone_event(ctx: &mut Context<'_>, command: u16, fail: u16, kind: ArenaEventTask) -> Outcome {
    if !in_arena(ctx) {
        return logged(Outcome::reason(fail, 2), "arena event: not is arena");
    }
    if !authorized(ctx.pc()) {
        return logged(Outcome::reason(fail, 2), "arena event: not authorized");
    }
    // Decode "h" only: an empty body cannot fail here.
    let Some(zone) = ctx.pc().zone else {
        return logged(Outcome::reason(fail, 2), "arena event: zone is null");
    };
    let t = task(ctx, command, zone, kind);
    queue(ctx, t, Outcome::reason(fail, 2))
}

/// parity: 754 26-arena-event-handlers.md#754--g_arenaeventinzonepclocreqcmd--partial partial
pub fn handle_arena_event_in_zone_pc_loc(ctx: &mut Context<'_>, _req: &[u8]) -> Outcome {
    zone_event(ctx, 754, 756, ArenaEventTask::InZonePcLoc)
}

/// parity: 757 26-arena-event-handlers.md#757--g_arenaeventbattleroyalscorereqcmd--partial partial
pub fn handle_arena_event_battle_royal_score(ctx: &mut Context<'_>, _req: &[u8]) -> Outcome {
    zone_event(ctx, 757, 759, ArenaEventTask::BattleRoyalScore)
}

fn recruit(ctx: &mut Context<'_>, req: &[u8], command: u16, fail: u16, mode: u32) -> Outcome {
    if !ctx.world.party_config.arena_enabled {
        return logged(
            Outcome::reason(fail, 2214),
            "arena recruit: arena not enabled",
        );
    }
    if !ctx.world.arena {
        return logged(Outcome::reason(fail, 2215), "arena recruit: not in arena");
    }
    if !ctx.world.arena_event.recruit_enabled {
        return logged(Outcome::reason(fail, 2213), "arena recruit: not in arena");
    }
    if ctx.world.arena_event.attraction != 2 {
        return logged(
            Outcome::reason(fail, 2214),
            "arena recruit: not dungeon attraction",
        );
    }
    let Some(portal_key) = Reader(req).quad() else {
        return logged(Outcome::reason(fail, 2), "arena recruit: decompose failed");
    };
    match portal(ctx, portal_key) {
        Some(true) => {}
        Some(false) => {
            return logged(
                Outcome::reason(fail, 2216),
                "arena recruit: invalid portal key",
            )
        }
        None => return unknown(),
    }
    let Some(zone) = ctx.pc().zone else {
        return logged(Outcome::reason(fail, 2), "arena recruit: zone is null");
    };
    let t = task(
        ctx,
        command,
        zone,
        ArenaEventTask::CheckPortal { portal_key, mode },
    );
    queue(ctx, t, Outcome::reason(fail, 2))
}

/// parity: 959 26-arena-event-handlers.md#959--g_publishrecruitarenadungeonmemberreqcmd--partial partial
pub fn handle_publish_recruit_arena_dungeon_member(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    recruit(ctx, req, 959, 962, 2)
}

/// parity: 963 26-arena-event-handlers.md#963--g_cancelrecruitarenadungeonmemberreqcmd--partial partial
pub fn handle_cancel_recruit_arena_dungeon_member(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    recruit(ctx, req, 963, 966, 3)
}

fn portal_type(ctx: &mut Context<'_>, portal_key: u64, types: &'static [i16]) -> Option<bool> {
    gate(ctx, ArenaEventOperation::PortalType { portal_key, types })
}

/// 708 / 711 shared flow. `attraction` is the required daemon byte +4152.
fn return_to_lobby(
    ctx: &mut Context<'_>,
    req: &[u8],
    command: u16,
    fail: u16,
    attraction: i8,
) -> Outcome {
    if !ctx.world.party_config.arena_enabled {
        return logged(
            Outcome::reason(fail, 1889),
            "arena cannot return: arena not enabled",
        );
    }
    if !ctx.world.arena {
        return logged(
            Outcome::reason(fail, 1890),
            "arena cannot return: not in arena",
        );
    }
    if ctx.world.arena_event.attraction != attraction {
        return logged(
            Outcome::reason(fail, 2),
            "arena cannot return: wrong attraction",
        );
    }
    let mut r = Reader(req);
    let Some(portal_key) = r.quad() else {
        return logged(
            Outcome::reason(fail, 2),
            "arena cannot return: decompose failed",
        );
    };
    let flag = if command == 711 {
        match r.byte() {
            Some(b) => b != 0,
            None => {
                return logged(
                    Outcome::reason(fail, 2),
                    "arena cannot return: decompose failed",
                )
            }
        }
    } else {
        false
    };
    let types: &'static [i16] = if command == 711 { &[1, 2] } else { &[0] };
    match portal_type(ctx, portal_key, types) {
        Some(true) => {}
        Some(false) => {
            return logged(
                Outcome::reason(fail, 1888),
                "arena cannot return: invalid portal",
            )
        }
        None => return unknown(),
    }
    let Some(zone) = ctx.pc().zone else {
        return logged(
            Outcome::reason(fail, 2),
            "arena cannot return: zone is null",
        );
    };
    let kind = if command == 711 {
        ArenaEventTask::ReturnFromDungeon { portal_key, flag }
    } else {
        ArenaEventTask::ReturnFromDuel { portal_key }
    };
    let t = task(ctx, command, zone, kind);
    queue(ctx, t, Outcome::reason(fail, 2))
}

/// parity: 708 26-arena-event-handlers.md#708--g_returntoarenalobbyfromduelreqcmd--partial partial
pub fn handle_return_to_arena_lobby_from_duel(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    return_to_lobby(ctx, req, 708, 710, 1)
}

/// parity: 711 26-arena-event-handlers.md#711--g_returntoarenalobbyfromdungeonreqcmd--partial partial
pub fn handle_return_to_arena_lobby_from_dungeon(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    return_to_lobby(ctx, req, 711, 713, 2)
}

/// Reply `hEs`: reason, then the decoded name (empty before decode).
fn reason_name(command: u16, reason: u16, name: &[u16]) -> Outcome {
    let mut body = reason.to_le_bytes().to_vec();
    string_body(&mut body, name);
    Outcome {
        replies: vec![Message { command, body }],
        ..Outcome::default()
    }
}

/// parity: 714 26-arena-event-handlers.md#714--g_arenaspectategamereqcmd--partial partial
pub fn handle_arena_spectate_game(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_ArenaSpectateGameFailCmd;
    let mut r = Reader(req);
    let (Some(portal_key), Some(name)) = (r.quad(), r.string(33)) else {
        return logged(reason_name(F, 2, &[]), "arena spectate: decompose failed");
    };
    match portal(ctx, portal_key) {
        Some(true) => {}
        Some(false) => return logged(reason_name(F, 2, &name), "arena spectate: invalid portal"),
        None => return unknown(),
    }
    match gate(ctx, ArenaEventOperation::CrossOptionEnabled) {
        Some(true) => {}
        Some(false) => return logged(reason_name(F, 2, &name), "arena spectate: cross option off"),
        None => return unknown(),
    }
    if !ctx.world.party_config.arena_enabled {
        return logged(
            reason_name(F, 1895, &name),
            "arena spectate: arena not enabled",
        );
    }
    if ctx.world.arena {
        return logged(reason_name(F, 5, &name), "arena spectate: already in arena");
    }
    if ctx.pc().arena_state != 3 {
        return logged(reason_name(F, 1896, &name), "arena spectate: not in lobby");
    }
    // Portal type 0 → attraction 1; 1 or 2 → 2; other → reason 2.
    let attraction = match portal_type(ctx, portal_key, &[0]) {
        Some(true) => 1u8,
        Some(false) => match portal_type(ctx, portal_key, &[1, 2]) {
            Some(true) => 2,
            Some(false) => {
                return logged(
                    reason_name(F, 2, &name),
                    "arena spectate: invalid portal type",
                )
            }
            None => return unknown(),
        },
        None => return unknown(),
    };
    let pc = ctx.pc();
    let mut body = pc.arena_lobby.route.to_le_bytes().to_vec();
    body.extend(pc.db_id.to_le_bytes());
    body.extend(pc.object_id.to_le_bytes());
    body.extend(pc.short_96.to_le_bytes());
    body.push(attraction);
    string_body(&mut body, &name);
    match send(ctx, 127, "hqqqhQs", body) {
        Some(true) => Outcome::default(),
        Some(false) => logged(reason_name(F, 1895, &name), "arena spectate: fail to send"),
        None => unknown(),
    }
}

/// parity: 717 26-arena-event-handlers.md#717--g_arenaspectatereturntoarenalobbyreqcmd--partial partial
pub fn handle_arena_spectate_return_to_arena_lobby(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_ArenaSpectateReturnToArenaLobbyFailCmd;
    if !ctx.world.party_config.arena_enabled {
        return logged(
            Outcome::reason(F, 1907),
            "arena cannot return: arena not enabled",
        );
    }
    if !ctx.world.arena {
        return logged(
            Outcome::reason(F, 1908),
            "arena cannot return: not in arena",
        );
    }
    let Some(portal_key) = Reader(req).quad() else {
        return logged(
            Outcome::reason(F, 2),
            "arena cannot return: decompose failed",
        );
    };
    match portal(ctx, portal_key) {
        Some(true) => {}
        Some(false) => {
            return logged(
                Outcome::reason(F, 1906),
                "arena cannot return: invalid portal",
            )
        }
        None => return unknown(),
    }
    let Some(zone) = ctx.pc().zone else {
        return logged(Outcome::reason(F, 2), "arena cannot return: zone is null");
    };
    let t = task(
        ctx,
        ids::G_ArenaSpectateReturnToArenaLobbyReqCmd,
        zone,
        ArenaEventTask::SpectateReturn { portal_key },
    );
    queue(ctx, t, Outcome::reason(F, 2))
}

/// parity: 720 26-arena-event-handlers.md#720--g_listarenaspectategamereqcmd--partial partial
pub fn handle_list_arena_spectate_game(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_ListArenaSpectateGameFailCmd;
    let mut r = Reader(req);
    let (Some(portal_key), Some(list_type), Some(page)) = (r.quad(), r.byte(), r.byte()) else {
        return logged(Outcome::reason(F, 2), "arena cannot list: decompose failed");
    };
    if list_type > 1 {
        return logged(
            Outcome::reason(F, 2),
            "arena cannot list: invalid list type",
        );
    }
    if !ctx.world.party_config.arena_enabled {
        return logged(
            Outcome::reason(F, 1911),
            "arena cannot list: arena not enabled",
        );
    }
    if ctx.world.arena || ctx.pc().arena_state != 3 {
        return logged(Outcome::reason(F, 1912), "arena cannot list: not in lobby");
    }
    let pc = ctx.pc();
    if !pc.user_present || pc.party_user_blocked {
        return logged(Outcome::reason(F, 2), "arena cannot list: no user");
    }
    match gate(ctx, ArenaEventOperation::SpectateListCooldown { list_type }) {
        Some(true) => {}
        Some(false) => return logged(Outcome::reason(F, 1915), "arena cannot list: too often"),
        None => return unknown(),
    }
    match portal_type(ctx, portal_key, &[0]) {
        Some(true) => {}
        Some(false) => {
            return logged(
                Outcome::reason(F, 1913),
                "arena cannot list: invalid portal",
            )
        }
        None => return unknown(),
    }
    if page != 1 {
        return logged(Outcome::reason(F, 1914), "arena cannot list: invalid page");
    }
    let pc = ctx.pc();
    let mut body = pc.object_id.to_le_bytes().to_vec();
    body.extend(pc.db_id.to_le_bytes());
    body.extend(pc.arena_lobby.route.to_le_bytes());
    body.extend(pc.guild_id.to_le_bytes());
    // ❓ The last byte is the portal attraction register; it is 1 on this path.
    body.extend([list_type, 1, 1]);
    match send(ctx, 133, "hqqqqQcc", body) {
        Some(true) => Outcome::default(),
        Some(false) => logged(Outcome::reason(F, 1911), "arena cannot list: fail to send"),
        None => unknown(),
    }
}
