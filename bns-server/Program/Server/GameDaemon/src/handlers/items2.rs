//! Doc 12b part 2: closet (934–1428), inventory (1328–1338), repair
//! (1385/1387), decompose (1395), gem and seal (1364–1392) entry functions.
//! Build 381.8867.
//!
//! Entry functions decode, gate and queue an `ItemTask` with an
//! `InventoryTask` kind. Task runners, inventory helpers, item virtuals and
//! client/backend batches are adapter boundaries; doc 12b leaves them unread.
use super::items::{craft_dispatch, reason_u64};
use super::*;

pub const ITEM3_COMMANDS: &[u16] = &[
    ids::G_PutItemIntoClosetReqCmd,
    ids::G_TakeItemFromClosetReqCmd,
    ids::G_SetClosetItemUsedReqCmd,
    ids::G_CollectingListReqCmd,
    ids::G_FishingListReqCmd,
    ids::G_CompleteClosetCollectingReqCmd,
    ids::G_ExpandInventoryReqCmd,
    ids::G_InventoryItemListReqCmd,
    ids::G_DisposeItemReqCmd,
    ids::G_MoveItemReqCmd,
    ids::G_MoveItemListReqCmd,
    ids::G_AttachGemToWeaponReqCmd,
    ids::G_DetachGemFromWeaponReqCmd,
    ids::G_OpenWeaponGemSlotReqCmd,
    ids::G_ExpandWeaponGemSlotReqCmd,
    ids::G_AddWeaponGemSlotReqCmd,
    ids::G_DeleteWeaponGemSlotReqCmd,
    ids::G_IdentifyItemReqCmd,
    ids::G_UnsealItem2ReqCmd,
    ids::G_RepairReqCmd,
    ids::G_RepairWithCampfireReqCmd,
    ids::G_SealItemReqCmd,
    ids::G_DecomposeItem2ReqCmd,
];

/// Gem attach entry: weapon container, weapon slot, gem slot, signed socket
/// index, socket-kind selector.
pub type GemEntry = (u8, u16, u16, i8, u8);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InventoryTask {
    /// 934, kind 189. Slot short +148.
    PutIntoCloset { slot: i16 },
    /// 937, kind 190. Selector +148, destination slot +152.
    TakeFromCloset { selector: i32, dst_slot: i16 },
    /// 1118. Selectors +152 (1..16, no per-entry check).
    SetClosetItemUsed { selectors: Vec<i32> },
    /// 1121, kind 313. Read builder over PC +20496.
    CollectingList,
    /// 1125, kind 318. Read builder over PC +20568.
    FishingList,
    /// 1428. Collection +148; entries `QQhQi` (0..8) at +156.
    CompleteClosetCollecting {
        collection: i32,
        entries: Vec<(u8, u8, u16, u8, u32)>,
    },
    /// 1331, kind 59. Runner lists containers 1..12.
    InventoryList,
    /// 1334, kind 65. Slot/count +144/+146 (container 2 in the runner).
    Dispose { slot: u16, count: i16 },
    /// 1336, kind 66, operation 0.
    Move {
        src_selector: u8,
        src_slot: u16,
        count: i16,
        dst_selector: u8,
        dst_slot: u16,
        token: u64,
    },
    /// 1338, kind 67, operation 0. Triples: source slot, count, destination slot.
    MoveList {
        token: u64,
        src_selector: u8,
        dst_selector: u8,
        first: Vec<(u16, u16, u16)>,
        second: Vec<(u16, u16, u16)>,
    },
    /// 1328, inventory factory kind 26. The qword is an NPC object ID in the
    /// action's NPC branch.
    ExpandInventory {
        selector: u8,
        npc: u64,
        pairs: Vec<(u16, u16)>,
    },
    /// 1385, kind 72. Container/slot pairs (0..96).
    Repair { npc: u64, entries: Vec<(u8, u16)> },
    /// 1387, inventory factory kind 9. Material slot in container 2.
    RepairWithCampfire { campfire: u64, material_slot: u16 },
    /// 1395, kind 139.
    Decompose {
        container: u8,
        slot: u16,
        mode: u8,
        pairs: Vec<(u16, u16)>,
        q1: u8,
        q2: u8,
        auxiliary: Vec<(u8, u64)>,
    },
    /// 1364, kind 73. Entries 0..3.
    AttachGem { entries: Vec<GemEntry> },
    /// 1367, kind 74.
    DetachGem {
        container: u8,
        slot: u16,
        index: i8,
        socket_kind: u8,
    },
    /// 1369, kind 141.
    OpenGemSlot {
        container: u8,
        slot: u16,
        pairs: Vec<(u16, u16)>,
    },
    /// 1372, kind 127.
    ExpandGemSlot {
        container: u8,
        slot: u16,
        operand: u16,
    },
    /// 1375, kind 140.
    AddGemSlot {
        container: u8,
        slot: u16,
        operand: u16,
    },
    /// 1378, kind 141 (own vtable).
    DeleteGemSlot {
        container: u8,
        slot: u16,
        a: u8,
        b: u8,
        pairs: Vec<(u16, u16)>,
    },
    /// 1381, inventory factory kind 11.
    Identify { first: u16, second: u16 },
    /// 1383, inventory factory kind 12.
    Unseal {
        target: u16,
        mode: u8,
        pairs: Vec<(u16, u16)>,
    },
    /// 1392, inventory factory kind 10. Pairs 1..16.
    Seal { target: u16, pairs: Vec<(u16, u16)> },
}

pub(super) fn dispatch_item3(ctx: &mut Context<'_>, command: u16, req: &[u8]) -> Outcome {
    match command {
        ids::G_PutItemIntoClosetReqCmd => handle_put_item_into_closet(ctx, req),
        ids::G_TakeItemFromClosetReqCmd => handle_take_item_from_closet(ctx, req),
        ids::G_SetClosetItemUsedReqCmd => handle_set_closet_item_used(ctx, req),
        ids::G_CollectingListReqCmd => handle_collecting_list(ctx, req),
        ids::G_FishingListReqCmd => handle_fishing_list(ctx, req),
        ids::G_CompleteClosetCollectingReqCmd => handle_complete_closet_collecting(ctx, req),
        ids::G_InventoryItemListReqCmd => handle_inventory_item_list(ctx, req),
        ids::G_DisposeItemReqCmd => handle_dispose_item(ctx, req),
        ids::G_MoveItemReqCmd => handle_move_item(ctx, req),
        ids::G_MoveItemListReqCmd => handle_move_item_list(ctx, req),
        ids::G_ExpandInventoryReqCmd => handle_expand_inventory(ctx, req),
        ids::G_RepairReqCmd => handle_repair(ctx, req),
        ids::G_RepairWithCampfireReqCmd => handle_repair_with_campfire(ctx, req),
        ids::G_DecomposeItem2ReqCmd => handle_decompose_item2(ctx, req),
        ids::G_AttachGemToWeaponReqCmd => handle_attach_gem_to_weapon(ctx, req),
        ids::G_DetachGemFromWeaponReqCmd => handle_detach_gem_from_weapon(ctx, req),
        ids::G_OpenWeaponGemSlotReqCmd => handle_open_weapon_gem_slot(ctx, req),
        ids::G_ExpandWeaponGemSlotReqCmd => handle_expand_weapon_gem_slot(ctx, req),
        ids::G_AddWeaponGemSlotReqCmd => handle_add_weapon_gem_slot(ctx, req),
        ids::G_DeleteWeaponGemSlotReqCmd => handle_delete_weapon_gem_slot(ctx, req),
        ids::G_IdentifyItemReqCmd => handle_identify_item(ctx, req),
        ids::G_UnsealItem2ReqCmd => handle_unseal_item2(ctx, req),
        ids::G_SealItemReqCmd => handle_seal_item(ctx, req),
        _ => unreachable!("membership checked"),
    }
}

const UNCHECKED_DECODE: &str = "❓ unchecked item decode (short body, field not zero-initialized)";

/// What a wrong PC kind (+46 ≠ 1) does at entry.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum KindCheck {
    /// No PC kind test in the entry.
    None,
    /// Log/assert without a reply; the task is not posted.
    Log,
    /// Initializer failure: log and send the entry failure.
    Fail,
}
/// How the task is posted.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Post {
    /// Part-1 queue `0x140254FD0`; a failed queue sends the entry failure.
    Queue,
    /// `0x140603BD0`; the entry does not test the result.
    Dispatch,
}

/// Entry tail: zone, PC kind, then post.
pub(super) fn post(
    ctx: &mut Context<'_>,
    command: u16,
    fail: Outcome,
    kind_check: KindCheck,
    mode: Post,
    task: impl Into<ItemTaskKind>,
) -> Outcome {
    let pc = ctx.pc().clone();
    let Some(zone) = pc.zone else {
        return fail;
    };
    if kind_check != KindCheck::None && pc.native_kind != 1 {
        if kind_check == KindCheck::Fail {
            let mut out = fail;
            out.logs.push("inventory task: caster is not a PC");
            return out;
        }
        return Outcome::log("inventory task: caster is not a PC");
    }
    let task = ItemTask {
        command,
        caster: pc.object_id,
        zone,
        kind: task.into(),
    };
    if mode == Post::Dispatch {
        return craft_dispatch(ctx, task);
    }
    match ctx.services.queue_item(&task) {
        Gate::Pass => Outcome {
            changes: vec![
                StateChange::Queued(command),
                StateChange::ItemTaskQueued(task),
            ],
            ..Outcome::default()
        },
        Gate::Fail => fail,
        Gate::Unknown => Outcome::unsupported("❓ inventory task allocation/queue adapter"),
    }
}

/// U0: PC +20696 user (only when PC +20752 = 0), non-null, user +120 = 0.
pub(super) fn u0_passes(ctx: &Context<'_>) -> bool {
    let pc = ctx.pc();
    pc.user_present && !pc.party_user_blocked
}

/// Signed-short count with an unsigned comparison `min..=max` (negative
/// counts fail). A missing count also fails.
pub(super) fn count(r: &mut Reader<'_>, min: u16, max: u16) -> Option<u16> {
    r.short().filter(|c| (min..=max).contains(c))
}
/// Counted short pairs. Err(true): count failure. Err(false): entry decode failure.
pub(super) fn pairs(r: &mut Reader<'_>, min: u16, max: u16) -> Result<Vec<(u16, u16)>, bool> {
    let n = count(r, min, max).ok_or(true)?;
    (0..n)
        .map(|_| Some((r.short()?, r.short()?)))
        .collect::<Option<_>>()
        .ok_or(false)
}
pub(super) fn pair_fail(err: bool, command: u16, count_reason: u16, decode_reason: u16) -> Outcome {
    Outcome::reason(command, if err { count_reason } else { decode_reason })
}

// ---- Closet -------------------------------------------------------------

/// parity: 934 12b-item-handlers.md#command-934--g_putitemintoclosetreqcmd partial
pub fn handle_put_item_into_closet(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let Some(slot) = Reader(req).short().map(|s| s as i16) else {
        return Outcome::reason(936, 2168);
    };
    if ctx.pc().zone.is_some() && ctx.pc().native_kind == 1 && slot <= 0 {
        return Outcome::reason(936, 2);
    }
    let task = InventoryTask::PutIntoCloset { slot };
    post(
        ctx,
        934,
        Outcome::reason(936, 2),
        KindCheck::Fail,
        Post::Dispatch,
        task,
    )
}

/// parity: 937 12b-item-handlers.md#command-937--g_takeitemfromclosetreqcmd partial
pub fn handle_take_item_from_closet(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let mut r = Reader(req);
    let (Some(selector), Some(dst_slot)) = (r.int(), r.short()) else {
        return Outcome::reason(939, 2173);
    };
    let (selector, dst_slot) = (selector as i32, dst_slot as i16);
    if ctx.pc().zone.is_some() && ctx.pc().native_kind == 1 && (selector <= 0 || dst_slot < 1) {
        return Outcome::reason(939, 2);
    }
    let task = InventoryTask::TakeFromCloset { selector, dst_slot };
    post(
        ctx,
        937,
        Outcome::reason(939, 2),
        KindCheck::Fail,
        Post::Dispatch,
        task,
    )
}

/// parity: 1118 12b-item-handlers.md#command-1118--g_setclosetitemusedreqcmd partial
pub fn handle_set_closet_item_used(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let fail = || Outcome::reason(1120, 2);
    if ctx.pc().zone.is_none() {
        return fail();
    }
    if ctx.pc().native_kind != 1 {
        let mut out = fail();
        out.logs.push("set closet item used: caster is not a PC");
        return out;
    }
    let mut r = Reader(req);
    let Some(n) = count(&mut r, 1, 16) else {
        return fail();
    };
    let Some(selectors) = (0..n).map(|_| r.int().map(|v| v as i32)).collect() else {
        return fail();
    };
    let task = InventoryTask::SetClosetItemUsed { selectors };
    post(ctx, 1118, fail(), KindCheck::Fail, Post::Dispatch, task)
}

/// parity: 1121 12b-item-handlers.md#command-1121--g_collectinglistreqcmd partial
pub fn handle_collecting_list(ctx: &mut Context<'_>, _: &[u8]) -> Outcome {
    let fail = Outcome::reason(1123, 2);
    let task = InventoryTask::CollectingList;
    post(ctx, 1121, fail, KindCheck::None, Post::Queue, task)
}

/// parity: 1125 12b-item-handlers.md#command-1125--g_fishinglistreqcmd partial
pub fn handle_fishing_list(ctx: &mut Context<'_>, _: &[u8]) -> Outcome {
    let fail = Outcome::reason(1127, 2);
    let task = InventoryTask::FishingList;
    post(ctx, 1125, fail, KindCheck::None, Post::Queue, task)
}

/// parity: 1428 12b-item-handlers.md#command-1428--g_completeclosetcollectingreqcmd partial
pub fn handle_complete_closet_collecting(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    // 1429 `hEi`: the trailing integer at the entry failure sites is not
    // recovered. Doc 12b: do not assume zero.
    let fail = || Outcome::unsupported("❓ 1429 entry failure trailing integer not recovered");
    if ctx.pc().zone.is_none() || ctx.pc().native_kind != 1 {
        return fail();
    }
    let mut r = Reader(req);
    let Some(collection) = r.int().map(|v| v as i32).filter(|v| *v > 0) else {
        return fail();
    };
    let Some(n) = count(&mut r, 0, 8) else {
        return fail();
    };
    let entries = (0..n)
        .map(|_| Some((r.byte()?, r.byte()?, r.short()?, r.byte()?, r.int()?)))
        .collect::<Option<Vec<_>>>();
    let Some(entries) = entries else {
        return fail();
    };
    let task = InventoryTask::CompleteClosetCollecting {
        collection,
        entries,
    };
    post(ctx, 1428, fail(), KindCheck::Fail, Post::Dispatch, task)
}

// ---- Inventory ------------------------------------------------------------

/// parity: 1331 12b-item-handlers.md#command-1331--g_inventoryitemlistreqcmd partial
pub fn handle_inventory_item_list(ctx: &mut Context<'_>, _: &[u8]) -> Outcome {
    let fail = Outcome::reason(1333, 1);
    post(
        ctx,
        1331,
        fail,
        KindCheck::Log,
        Post::Queue,
        InventoryTask::InventoryList,
    )
}

/// parity: 1334 12b-item-handlers.md#command-1334--g_disposeitemreqcmd partial
pub fn handle_dispose_item(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let mut r = Reader(req);
    let (Some(slot), Some(count)) = (r.short(), r.short()) else {
        return Outcome::unsupported(UNCHECKED_DECODE);
    };
    let task = InventoryTask::Dispose {
        slot,
        count: count as i16,
    };
    post(
        ctx,
        1334,
        reason_u64(1335, 1, 0),
        KindCheck::Log,
        Post::Queue,
        task,
    )
}

/// parity: 1336 12b-item-handlers.md#command-1336--g_moveitemreqcmd partial
pub fn handle_move_item(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let mut r = Reader(req);
    let fields = (|| {
        Some((
            r.byte()?,
            r.short()?,
            r.short()?,
            r.byte()?,
            r.short()?,
            r.quad()?,
        ))
    })();
    let Some((src_selector, src_slot, count, dst_selector, dst_slot, token)) = fields else {
        return Outcome::unsupported(UNCHECKED_DECODE);
    };
    let task = InventoryTask::Move {
        src_selector,
        src_slot,
        count: count as i16,
        dst_selector,
        dst_slot,
        token,
    };
    post(
        ctx,
        1336,
        Outcome::reason(1337, 1),
        KindCheck::Log,
        Post::Queue,
        task,
    )
}

/// parity: 1338 12b-item-handlers.md#command-1338--g_moveitemlistreqcmd partial
pub fn handle_move_item_list(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let mut r = Reader(req);
    let (Some(token), Some(src_selector), Some(dst_selector)) = (r.quad(), r.byte(), r.byte())
    else {
        return Outcome::reason(1337, 292);
    };
    let mut groups = [vec![], vec![]];
    for group in &mut groups {
        let Some(n) = count(&mut r, 0, 96) else {
            return Outcome::reason(1337, 293);
        };
        for _ in 0..n {
            let (Some(a), Some(b), Some(c)) = (r.short(), r.short(), r.short()) else {
                return Outcome::reason(1337, 292);
            };
            group.push((a, b, c));
        }
    }
    let [first, second] = groups;
    let task = InventoryTask::MoveList {
        token,
        src_selector,
        dst_selector,
        first,
        second,
    };
    post(
        ctx,
        1338,
        Outcome::reason(1337, 2),
        KindCheck::Fail,
        Post::Queue,
        task,
    )
}

/// parity: 1328 12b-item-handlers.md#command-1328--g_expandinventoryreqcmd partial
pub fn handle_expand_inventory(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let mut r = Reader(req);
    let (Some(selector), Some(npc)) = (r.byte(), r.quad()) else {
        let mut out = Outcome::reason(1330, 1);
        out.logs.push("expand inventory: prefix decode failed");
        return out;
    };
    // Count 0..15: unexpected failure command 1344.
    let pairs = match pairs(&mut r, 0, 15) {
        Ok(p) => p,
        Err(e) => return pair_fail(e, 1344, 349, 348),
    };
    let task = InventoryTask::ExpandInventory {
        selector,
        npc,
        pairs,
    };
    post(
        ctx,
        1328,
        Outcome::reason(1330, 1),
        KindCheck::Log,
        Post::Queue,
        task,
    )
}

// ---- Repair and decompose ---------------------------------------------------

/// parity: 1385 12b-item-handlers.md#command-1385--g_repairreqcmd partial
pub fn handle_repair(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    if !u0_passes(ctx) {
        return Outcome::reason(1386, 455);
    }
    let mut r = Reader(req);
    let Some(npc) = r.quad() else {
        return Outcome::reason(1386, 448);
    };
    let Some(n) = count(&mut r, 0, 96) else {
        return Outcome::reason(1386, 449);
    };
    let Some(entries) = (0..n).map(|_| Some((r.byte()?, r.short()?))).collect() else {
        return Outcome::reason(1386, 448);
    };
    let task = InventoryTask::Repair { npc, entries };
    post(
        ctx,
        1385,
        Outcome::reason(1386, 2),
        KindCheck::Log,
        Post::Queue,
        task,
    )
}

/// parity: 1387 12b-item-handlers.md#command-1387--g_repairwithcampfirereqcmd partial
pub fn handle_repair_with_campfire(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    if !u0_passes(ctx) {
        return Outcome::reason(1388, 464);
    }
    let mut r = Reader(req);
    let (Some(campfire), Some(material_slot)) = (r.quad(), r.short()) else {
        return Outcome::unsupported(UNCHECKED_DECODE);
    };
    let task = InventoryTask::RepairWithCampfire {
        campfire,
        material_slot,
    };
    post(
        ctx,
        1387,
        Outcome::reason(1388, 2),
        KindCheck::Fail,
        Post::Queue,
        task,
    )
}

/// parity: 1395 12b-item-handlers.md#command-1395--g_decomposeitem2reqcmd partial
pub fn handle_decompose_item2(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_DecomposeItem2FailCmd;
    if !u0_passes(ctx) {
        return Outcome::reason(F, 489);
    }
    let mut r = Reader(req);
    let (Some(container), Some(slot), Some(mode)) = (r.byte(), r.short(), r.byte()) else {
        return Outcome::reason(F, 480);
    };
    // Bit mask 332 = {2, 3, 6, 8}; mode is an unsigned byte.
    if !matches!(container, 2 | 3 | 6 | 8) || mode > 6 {
        return Outcome::reason(F, 482);
    }
    let pairs = match pairs(&mut r, 0, 16) {
        Ok(p) => p,
        Err(e) => return pair_fail(e, F, 482, 480),
    };
    let (Some(q1), Some(q2)) = (r.byte(), r.byte()) else {
        return Outcome::reason(F, 480);
    };
    let Some(n) = count(&mut r, 0, 8) else {
        return Outcome::reason(F, 483);
    };
    let Some(auxiliary) = (0..n).map(|_| Some((r.byte()?, r.quad()?))).collect() else {
        return Outcome::reason(F, 483);
    };
    let task = InventoryTask::Decompose {
        container,
        slot,
        mode,
        pairs,
        q1,
        q2,
        auxiliary,
    };
    post(
        ctx,
        1395,
        Outcome::reason(F, 1),
        KindCheck::Log,
        Post::Queue,
        task,
    )
}

// ---- Gem and seal -------------------------------------------------------------

/// Socket index bound: 0..8 for socket kind 0, else 0..2.
fn socket_ok(index: i8, socket_kind: u8) -> bool {
    let limit = if socket_kind == 0 { 8 } else { 2 };
    (0..limit).contains(&index)
}

/// parity: 1364 12b-item-handlers.md#command-1364--g_attachgemtoweaponreqcmd partial
pub fn handle_attach_gem_to_weapon(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    // 1366 `hEc`: the early failure sites supply no recovered index argument.
    // Reasons: U0 379, count 373, decode 372, socket 378, zone/queue 1.
    let fail =
        |_reason: u16| Outcome::unsupported("❓ 1366 entry failure index argument not recovered");
    if !u0_passes(ctx) {
        return fail(379);
    }
    let mut r = Reader(req);
    let Some(n) = count(&mut r, 0, 3) else {
        return fail(373);
    };
    let entries = (0..n)
        .map(|_| {
            Some((
                r.byte()?,
                r.short()?,
                r.short()?,
                r.byte()? as i8,
                r.byte()?,
            ))
        })
        .collect::<Option<Vec<GemEntry>>>();
    let Some(entries) = entries else {
        return fail(372);
    };
    if entries.iter().any(|e| !socket_ok(e.3, e.4)) {
        return fail(378);
    }
    let task = InventoryTask::AttachGem { entries };
    post(ctx, 1364, fail(1), KindCheck::Log, Post::Queue, task)
}

/// parity: 1367 12b-item-handlers.md#command-1367--g_detachgemfromweaponreqcmd partial
pub fn handle_detach_gem_from_weapon(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    if !u0_passes(ctx) {
        return Outcome::reason(1368, 385);
    }
    let mut r = Reader(req);
    let (Some(container), Some(slot), Some(index)) = (r.byte(), r.short(), r.byte()) else {
        return Outcome::unsupported(UNCHECKED_DECODE);
    };
    // Socket kind is zero-initialized before the unchecked decode.
    let socket_kind = r.byte().unwrap_or(0);
    let index = index as i8;
    if !socket_ok(index, socket_kind) {
        return Outcome::reason(1368, 382);
    }
    let task = InventoryTask::DetachGem {
        container,
        slot,
        index,
        socket_kind,
    };
    post(
        ctx,
        1367,
        Outcome::reason(1368, 1),
        KindCheck::Log,
        Post::Queue,
        task,
    )
}

/// parity: 1369 12b-item-handlers.md#command-1369--g_openweapongemslotreqcmd partial
pub fn handle_open_weapon_gem_slot(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    if !u0_passes(ctx) {
        return Outcome::reason(1371, 392);
    }
    let mut r = Reader(req);
    let (Some(container), Some(slot)) = (r.byte(), r.short()) else {
        return Outcome::reason(1371, 386);
    };
    let pairs = match pairs(&mut r, 0, 16) {
        Ok(p) => p,
        Err(e) => return pair_fail(e, 1371, 387, 386),
    };
    let task = InventoryTask::OpenGemSlot {
        container,
        slot,
        pairs,
    };
    post(
        ctx,
        1369,
        Outcome::reason(1371, 1),
        KindCheck::Log,
        Post::Queue,
        task,
    )
}

fn gem_slot_operand(
    ctx: &mut Context<'_>,
    req: &[u8],
    command: u16,
    u0_reason: u16,
    build: fn(u8, u16, u16) -> InventoryTask,
) -> Outcome {
    let failure = command + 2;
    if !u0_passes(ctx) {
        return Outcome::reason(failure, u0_reason);
    }
    let mut r = Reader(req);
    let (Some(container), Some(slot), Some(operand)) = (r.byte(), r.short(), r.short()) else {
        return Outcome::unsupported(UNCHECKED_DECODE);
    };
    let task = build(container, slot, operand);
    post(
        ctx,
        command,
        Outcome::reason(failure, 1),
        KindCheck::Log,
        Post::Queue,
        task,
    )
}

/// parity: 1372 12b-item-handlers.md#command-1372--g_expandweapongemslotreqcmd partial
pub fn handle_expand_weapon_gem_slot(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    gem_slot_operand(ctx, req, 1372, 400, |container, slot, operand| {
        InventoryTask::ExpandGemSlot {
            container,
            slot,
            operand,
        }
    })
}

/// parity: 1375 12b-item-handlers.md#command-1375--g_addweapongemslotreqcmd partial
pub fn handle_add_weapon_gem_slot(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    gem_slot_operand(ctx, req, 1375, 406, |container, slot, operand| {
        InventoryTask::AddGemSlot {
            container,
            slot,
            operand,
        }
    })
}

/// parity: 1378 12b-item-handlers.md#command-1378--g_deleteweapongemslotreqcmd partial
pub fn handle_delete_weapon_gem_slot(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    if !u0_passes(ctx) {
        return Outcome::reason(1380, 414);
    }
    let mut r = Reader(req);
    let (Some(container), Some(slot), Some(a), Some(b)) = (r.byte(), r.short(), r.byte(), r.byte())
    else {
        return Outcome::reason(1380, 408);
    };
    if b > 3 {
        return Outcome::reason(1380, 411);
    }
    let pairs = match pairs(&mut r, 0, 16) {
        Ok(p) => p,
        Err(e) => return pair_fail(e, 1380, 411, 408),
    };
    let task = InventoryTask::DeleteGemSlot {
        container,
        slot,
        a,
        b,
        pairs,
    };
    post(
        ctx,
        1378,
        Outcome::reason(1380, 1),
        KindCheck::Log,
        Post::Queue,
        task,
    )
}

/// parity: 1381 12b-item-handlers.md#command-1381--g_identifyitemreqcmd partial
pub fn handle_identify_item(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    // Decode (unchecked) before the PC-state check. No U0 gate.
    let mut r = Reader(req);
    let fields = (r.short(), r.short());
    if ctx.pc().t2_blocked {
        return Outcome::reason(1382, 7);
    }
    let (Some(first), Some(second)) = fields else {
        return Outcome::unsupported(UNCHECKED_DECODE);
    };
    let task = InventoryTask::Identify { first, second };
    post(
        ctx,
        1381,
        Outcome::reason(1382, 1),
        KindCheck::Log,
        Post::Queue,
        task,
    )
}

/// parity: 1383 12b-item-handlers.md#command-1383--g_unsealitem2reqcmd partial
pub fn handle_unseal_item2(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    if !ctx.world.unseal_enabled {
        return Outcome::log("unseal item: feature disabled");
    }
    if !u0_passes(ctx) {
        return Outcome::reason(1384, 446);
    }
    let mut r = Reader(req);
    let (Some(target), Some(mode)) = (r.short(), r.byte()) else {
        return Outcome::reason(1384, 441);
    };
    if mode > 3 {
        return Outcome::reason(1384, 443);
    }
    let pairs = match pairs(&mut r, 0, 16) {
        Ok(p) => p,
        Err(e) => return pair_fail(e, 1384, 443, 441),
    };
    let task = InventoryTask::Unseal {
        target,
        mode,
        pairs,
    };
    post(
        ctx,
        1383,
        Outcome::reason(1384, 1),
        KindCheck::Log,
        Post::Queue,
        task,
    )
}

/// parity: 1392 12b-item-handlers.md#command-1392--g_sealitemreqcmd partial
pub fn handle_seal_item(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    if !u0_passes(ctx) {
        return Outcome::reason(1394, 478);
    }
    let mut r = Reader(req);
    let Some(target) = r.short() else {
        return Outcome::reason(1394, 473);
    };
    let pairs = match pairs(&mut r, 1, 16) {
        Ok(p) => p,
        Err(e) => return pair_fail(e, 1394, 475, 473),
    };
    let task = InventoryTask::Seal { target, pairs };
    post(
        ctx,
        1392,
        Outcome::reason(1394, 1),
        KindCheck::Log,
        Post::Queue,
        task,
    )
}

impl From<InventoryTask> for ItemTaskKind {
    fn from(task: InventoryTask) -> Self {
        ItemTaskKind::Inventory(task)
    }
}
