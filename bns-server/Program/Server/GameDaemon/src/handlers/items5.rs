//! Doc 12c part 3: ability modification (1398–1445), warehouse take (1411)
//! and item transform (1581) entry functions. Build 381.8867.
//!
//! Entry functions decode, gate and queue an `ItemTask` with an
//! `AbilityTask` kind. 1411 and 1581 stop at their table/transaction
//! boundaries after the proved entry checks.
use super::items::reason_u64;
use super::items2::{count, pair_fail, pairs, post, u0_passes, KindCheck, Post};
use super::*;

pub const ITEM6_COMMANDS: &[u16] = &[
    ids::G_RevealItemAbilityReqCmd,
    ids::G_AttachEquipGemPieceReqCmd,
    ids::G_R_EnchantToItemReqCmd,
    ids::G_R_ResetEnchantItemInfoReqCmd,
    ids::G_R_PolishToItemReqCmd,
    ids::G_GrowthItemReqCmd,
    ids::G_AwakeningItemReqCmd,
    ids::G_FeedingRandomStatEquipGemReqCmd,
    ids::G_ProcessItemFusionReqCmd,
    ids::G_TakeWarehouseItemReqCmd,
    ids::G_TransformItemReqCmd,
];

type Pairs = Vec<(u16, u16)>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AbilityTask {
    /// 1398, inventory factory kind 27. Prefix +152..+158, pairs +164.
    RevealItemAbility {
        selector: u8,
        a: u16,
        b: u16,
        c: u8,
        pairs: Pairs,
    },
    /// 1400, inventory factory kind 28.
    AttachEquipGemPiece {
        selector: u8,
        a: u16,
        b: u16,
        pairs: Pairs,
    },
    /// 1430, kind 344. Short +144, selector +146, qword +152.
    EnchantToItem { selector: u8, slot: u16, value: u64 },
    /// 1433, kind 345.
    ResetEnchantItemInfo { selector: u8, slot: u16 },
    /// 1442, kind 348. Bytes are distinct and in 1..=5.
    PolishToItem {
        selector: u8,
        slot: u16,
        list: Vec<u8>,
        first: u64,
        second: u64,
    },
    /// 1402, kind 116.
    GrowthItem {
        container: u8,
        targets: [u16; 3],
        pairs: Pairs,
    },
    /// 1405 (kind 117) and 1408 (kind 261, `feeding` true): same layout.
    TwoGroup {
        feeding: bool,
        container: u8,
        targets: [u16; 2],
        first: Pairs,
        second: Pairs,
    },
    /// 1445, kind 349. Count 1..=3; trailing short +174.
    ProcessItemFusion { pairs: Pairs, operand: u16 },
}
impl From<AbilityTask> for ItemTaskKind {
    fn from(task: AbilityTask) -> Self {
        ItemTaskKind::Ability(task)
    }
}

pub(super) fn dispatch_item6(ctx: &mut Context<'_>, command: u16, req: &[u8]) -> Outcome {
    match command {
        ids::G_RevealItemAbilityReqCmd => handle_reveal_item_ability(ctx, req),
        ids::G_AttachEquipGemPieceReqCmd => handle_attach_equip_gem_piece(ctx, req),
        ids::G_R_EnchantToItemReqCmd => handle_enchant_to_item(ctx, req),
        ids::G_R_ResetEnchantItemInfoReqCmd => handle_reset_enchant_item_info(ctx, req),
        ids::G_R_PolishToItemReqCmd => handle_polish_to_item(ctx, req),
        ids::G_GrowthItemReqCmd => handle_growth_item(ctx, req),
        ids::G_AwakeningItemReqCmd => handle_two_group(ctx, req, ids::G_AwakeningItemReqCmd, false),
        ids::G_FeedingRandomStatEquipGemReqCmd => {
            handle_two_group(ctx, req, ids::G_FeedingRandomStatEquipGemReqCmd, true)
        }
        ids::G_ProcessItemFusionReqCmd => handle_process_item_fusion(ctx, req),
        ids::G_TakeWarehouseItemReqCmd => handle_take_warehouse_item(ctx, req),
        ids::G_TransformItemReqCmd => handle_transform_item(ctx, req),
        _ => unreachable!("membership checked"),
    }
}

/// Zone check with its own reason, then queue with the factory/queue reason.
fn zone_then_post(
    ctx: &mut Context<'_>,
    command: u16,
    failure: u16,
    zone_reason: u16,
    queue_reason: u16,
    kind_check: KindCheck,
    task: AbilityTask,
) -> Outcome {
    if ctx.pc().zone.is_none() {
        return Outcome::reason(failure, zone_reason);
    }
    let fail = Outcome::reason(failure, queue_reason);
    post(ctx, command, fail, kind_check, Post::Queue, task)
}

/// parity: 1398 12c-item-handlers.md#command-1398--g_revealitemabilityreqcmd partial
pub fn handle_reveal_item_ability(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_RevealItemAbilityFailCmd;
    if !u0_passes(ctx) {
        return Outcome::reason(F, 575);
    }
    let mut r = Reader(req);
    let (Some(selector), Some(a), Some(b), Some(c)) = (r.byte(), r.short(), r.short(), r.byte())
    else {
        return Outcome::reason(F, 565);
    };
    let pairs = match pairs(&mut r, 0, 15) {
        Ok(p) => p,
        Err(e) => return pair_fail(e, F, 569, 565),
    };
    let task = AbilityTask::RevealItemAbility {
        selector,
        a,
        b,
        c,
        pairs,
    };
    zone_then_post(ctx, 1398, F, 2, 1, KindCheck::Log, task)
}

/// parity: 1400 12c-item-handlers.md#command-1400--g_attachequipgempiecereqcmd partial
pub fn handle_attach_equip_gem_piece(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_AttachEquipGemPieceFailCmd;
    if !u0_passes(ctx) {
        return Outcome::reason(F, 585);
    }
    let mut r = Reader(req);
    let (Some(selector), Some(a), Some(b)) = (r.byte(), r.short(), r.short()) else {
        return Outcome::reason(F, 576);
    };
    let pairs = match pairs(&mut r, 0, 15) {
        Ok(p) => p,
        Err(e) => return pair_fail(e, F, 580, 576),
    };
    let task = AbilityTask::AttachEquipGemPiece {
        selector,
        a,
        b,
        pairs,
    };
    zone_then_post(ctx, 1400, F, 2, 1, KindCheck::Log, task)
}

/// parity: 1430 12c-item-handlers.md#command-1430--g_r_enchanttoitemreqcmd partial
pub fn handle_enchant_to_item(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_R_EnchantToItemFailCmd;
    let mut r = Reader(req);
    let (Some(selector), Some(slot), Some(value)) = (r.byte(), r.short(), r.quad()) else {
        return Outcome::reason(F, 2);
    };
    if slot == 0 {
        return Outcome::reason(F, 2563);
    }
    let task = AbilityTask::EnchantToItem {
        selector,
        slot,
        value,
    };
    zone_then_post(ctx, 1430, F, 2, 2, KindCheck::Fail, task)
}

/// parity: 1433 12c-item-handlers.md#command-1433--g_r_resetenchantiteminforeqcmd partial
pub fn handle_reset_enchant_item_info(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_R_ResetEnchantItemInfoFailCmd;
    let mut r = Reader(req);
    let (Some(selector), Some(slot)) = (r.byte(), r.short()) else {
        return Outcome::reason(F, 2);
    };
    let task = AbilityTask::ResetEnchantItemInfo { selector, slot };
    zone_then_post(ctx, 1433, F, 2, 2, KindCheck::Fail, task)
}

/// parity: 1442 12c-item-handlers.md#command-1442--g_r_polishtoitemreqcmd partial
pub fn handle_polish_to_item(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_R_PolishToItemFailCmd;
    let mut r = Reader(req);
    let (Some(selector), Some(slot)) = (r.byte(), r.short()) else {
        return Outcome::reason(F, 2);
    };
    let Some(n) = count(&mut r, 0, 3) else {
        return Outcome::reason(F, 3);
    };
    let mut list = Vec::new();
    for _ in 0..n {
        let Some(b) = r.byte() else {
            return Outcome::reason(F, 2);
        };
        if !(1..=5).contains(&b) || list.contains(&b) {
            return Outcome::reason(F, 2595);
        }
        list.push(b);
    }
    let (Some(first), Some(second)) = (r.quad(), r.quad()) else {
        return Outcome::reason(F, 2);
    };
    let task = AbilityTask::PolishToItem {
        selector,
        slot,
        list,
        first,
        second,
    };
    zone_then_post(ctx, 1442, F, 2, 2, KindCheck::Fail, task)
}

/// Pairs whose first short aliases target A are rejected for container 2.
fn aliases(container: u8, a: u16, pairs: &[(u16, u16)]) -> bool {
    container == 2 && pairs.iter().any(|p| p.0 == a)
}

/// parity: 1402 12c-item-handlers.md#command-1402--g_growthitemreqcmd partial
pub fn handle_growth_item(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_GrowthItemFailCmd;
    if !ctx.world.item_growth_enabled {
        return Outcome::reason(F, 586);
    }
    let mut r = Reader(req);
    let (Some(container), Some(a), Some(b), Some(c)) = (r.byte(), r.short(), r.short(), r.short())
    else {
        return Outcome::reason(F, 2);
    };
    if container == 2 && (a == b || a == c) {
        return Outcome::reason(F, 2);
    }
    let pairs = match pairs(&mut r, 0, 15) {
        Ok(p) => p,
        Err(e) => return pair_fail(e, F, 598, 2),
    };
    if aliases(container, a, &pairs) {
        return Outcome::reason(F, 2);
    }
    let task = AbilityTask::GrowthItem {
        container,
        targets: [a, b, c],
        pairs,
    };
    zone_then_post(ctx, 1402, F, 2, 2, KindCheck::Fail, task)
}

/// parity: 1405 12c-item-handlers.md#command-1405--g_awakeningitemreqcmd partial
/// parity: 1408 12c-item-handlers.md#command-1408--g_feedingrandomstatequipgemreqcmd partial
pub fn handle_two_group(ctx: &mut Context<'_>, req: &[u8], command: u16, feeding: bool) -> Outcome {
    let failure = command + 2;
    let (disabled, bad_count) = if feeding { (626, 635) } else { (586, 619) };
    if !ctx.world.item_growth_enabled {
        return Outcome::reason(failure, disabled);
    }
    let mut r = Reader(req);
    let (Some(container), Some(a), Some(b)) = (r.byte(), r.short(), r.short()) else {
        return Outcome::reason(failure, 2);
    };
    if container == 2 && a == b {
        return Outcome::reason(failure, 2);
    }
    let mut groups: [Pairs; 2] = Default::default();
    for group in &mut groups {
        *group = match pairs(&mut r, 0, 15) {
            Ok(p) => p,
            Err(e) => return pair_fail(e, failure, bad_count, 2),
        };
        if aliases(container, a, group) {
            return Outcome::reason(failure, 2);
        }
    }
    let [first, second] = groups;
    let task = AbilityTask::TwoGroup {
        feeding,
        container,
        targets: [a, b],
        first,
        second,
    };
    zone_then_post(ctx, command, failure, 2, 2, KindCheck::Fail, task)
}

/// parity: 1445 12c-item-handlers.md#command-1445--g_processitemfusionreqcmd partial
pub fn handle_process_item_fusion(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_ProcessItemFusionFailCmd;
    if !u0_passes(ctx) {
        return Outcome::reason(F, 2612);
    }
    if ctx.pc().zone.is_none() {
        return Outcome::reason(F, 2);
    }
    let mut r = Reader(req);
    let Ok(pairs) = pairs(&mut r, 1, 3) else {
        return Outcome::reason(F, 2598);
    };
    let Some(operand) = r.short() else {
        return Outcome::reason(F, 2598);
    };
    let task = AbilityTask::ProcessItemFusion { pairs, operand };
    zone_then_post(ctx, 1445, F, 2, 2, KindCheck::Fail, task)
}

/// parity: 1411 12c-item-handlers.md#command-1411--g_takewarehouseitemreqcmd partial
pub fn handle_take_warehouse_item(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_TakeWarehouseItemFailCmd;
    if !ctx.world.account_inventory_enabled {
        return Outcome::reason(F, 2);
    }
    if ctx.world.arena {
        return Outcome::reason(F, 5);
    }
    if !ctx.pc().user_present || ctx.pc().zone.is_none() {
        return Outcome::reason(F, 1);
    }
    let mut r = Reader(req);
    let (Some(_), Some(_), Some(value)) = (r.quad(), r.quad(), r.int()) else {
        return Outcome::unsupported("❓ unchecked warehouse decode (short body)");
    };
    if value.wrapping_sub(1) <= 998 {
        return Outcome::reason(F, 1964);
    }
    Outcome::unsupported("❓ warehouse table tm[1455] and transaction 663818")
}

/// parity: 1581 12c-item-handlers.md#command-1581--g_transformitemreqcmd partial
pub fn handle_transform_item(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_TransformItemFailCmd;
    if !u0_passes(ctx) {
        return reason_u64(F, 2012, 0);
    }
    if ctx.pc().zone.is_none() {
        return reason_u64(F, 2, 0);
    }
    let mut r = Reader(req);
    let Some(key) = r.quad() else {
        return reason_u64(F, 1980, 0);
    };
    for count_reason in [1987, 1989, 1996, 2002] {
        match pairs(&mut r, 0, 16) {
            Ok(_) => {}
            Err(true) => return reason_u64(F, count_reason, key),
            Err(false) => return reason_u64(F, 1980, key),
        }
    }
    let (Some(_mode), Some(_), Some(_)) = (r.byte(), r.int(), r.blob(8)) else {
        return reason_u64(F, 1980, key);
    };
    Outcome::unsupported("❓ transform task initializer 0x14052CBF0 and mode-1 transaction")
}
