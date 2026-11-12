//! Doc 24: market entry functions. Build 381.8867.
//!
//! Market sends and account transactions use `Services::market`. Unknown is
//! never approval. Zone actions use `skills::queue`. Task bodies are boundaries.
use super::skills::queue;
use super::*;

pub const MARKET_COMMANDS: &[u16] = &[
    ids::G_RetrieveSaleListReqCmd,
    ids::G_RetrieveBidListReqCmd,
    ids::G_RetrieveBidRankStatusReqCmd,
    ids::G_NPAuctionBidReqCmd,
    ids::G_RegisterSaleReqCmd,
    ids::G_AbandonSaleReqCmd,
    ids::G_UpdateSalePriceReqCmd,
    ids::G_RegisterBidReqCmd,
    ids::G_AbandonBidReqCmd,
    ids::G_UpdateBidPriceReqCmd,
    ids::G_RegisterPurchaseReqCmd,
    ids::G_MarketSearchItemReqCmd,
    ids::G_RetrieveInstantSaleItemLowestPriceReqCmd,
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MarketTask {
    /// 1530: action class 18, vtable `0x140D05EB0`, size 0x208.
    RegisterSale {
        /// +176.
        item: u64,
        /// +188.
        sale_type: u8,
        /// +192.
        price: i64,
        /// +184.
        count: i32,
        /// +152, +154. Byte +516 is forced to 0.
        first: i16,
        second: i16,
    },
    /// 1013: kind 280, vtable `0x140AEBB00`, size 0xC0.
    AuctionBid {
        /// +144, +152.
        world: i16,
        db_id: u64,
        /// +160, +176, +184.
        auction: [u8; 16],
        currency_group: i16,
        price: i64,
    },
}
impl From<MarketTask> for SkillTaskKind {
    fn from(task: MarketTask) -> Self {
        Self::Market(task)
    }
}

/// Server-owned configuration and market submission boundaries.
/// Transaction inputs are context only. The native entries leave their
/// transaction payload qwords at zero; see doc 24. No field mapping is inferred.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MarketCheck {
    /// Global byte +220.
    RegisterSaleEnabled,
    /// Signed maximum at global +8. The entry checks negative prices first.
    SalePrice {
        price: i64,
    },
    /// Signed maximum at global +16. The entry checks negative prices first.
    BidPrice {
        price: i64,
    },
    /// Global DWORD +156 != 0.
    AuctionEscrowEnabled,
    Submit(MarketRequest),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarketRequest {
    pub object_id: u64,
    pub db_id: u64,
    /// PC short +96, used as an unsigned route by market agents.
    pub world: u16,
    pub operation: MarketOperation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MarketOperation {
    /// Dealer 4, `hqqQQqc`: PC, DB ID, first, second, cursor, direction.
    RetrieveSaleList {
        first: u8,
        second: u8,
        cursor: u64,
        direction: i8,
    },
    /// Dealer 7, `hqq`: PC, DB ID.
    RetrieveBidList,
    /// Dealer 3, `hqqCq;`: PC, DB ID, count and sale IDs.
    RetrieveBidRankStatus { sales: Vec<u64> },
    /// Dealer 25, `hqqq`: PC, sale ID, DB ID.
    AbandonSale { sale: u64 },
    /// Dealer 22, `hqqqq`: PC, sale ID, DB ID, price.
    UpdateSalePrice { sale: u64, price: i64 },
    /// Account transaction, vtable `0x1409CE540`.
    RegisterBid { sale: u64, price: i64 },
    /// Dealer 40, `hqqqq`: PC, sale ID, bid ID, DB ID.
    AbandonBid { sale: u64, bid: u64 },
    /// Account transaction, vtable `0x1409CE100`.
    UpdateBidPrice { sale: u64, bid: u64, price: i64 },
    /// Account transaction, vtable `0x1409CDD00`.
    RegisterPurchase { sale: u64, price: i64 },
    /// Reader 52, `hoqQQQQiqqq`. The adapter supplies user +200 (16 bytes).
    SearchItem {
        search_type: u8,
        order_type: u8,
        price_type: u8,
        sale_type: u8,
        item: i32,
        first: u64,
        second: u64,
        third: u64,
    },
    /// Reader 61, `hqi`: PC, item.
    LowestPrice { item: i32 },
}

#[derive(Clone, Debug)]
pub struct MemoryMarket {
    pub register_sale_enabled: Option<bool>,
    pub sale_price_limit: Option<i64>,
    pub bid_price_limit: Option<i64>,
    pub auction_escrow_enabled: Option<bool>,
    pub submit_result: Gate,
    pub calls: Vec<MarketCheck>,
}
impl Default for MemoryMarket {
    fn default() -> Self {
        Self {
            register_sale_enabled: None,
            sale_price_limit: None,
            bid_price_limit: None,
            auction_escrow_enabled: None,
            submit_result: Gate::Unknown,
            calls: vec![],
        }
    }
}
impl MemoryMarket {
    pub(super) fn check(&mut self, check: &MarketCheck) -> Gate {
        self.calls.push(check.clone());
        let value = match check {
            MarketCheck::RegisterSaleEnabled => self.register_sale_enabled,
            MarketCheck::SalePrice { price } => self.sale_price_limit.map(|max| *price <= max),
            MarketCheck::BidPrice { price } => self.bid_price_limit.map(|max| *price <= max),
            MarketCheck::AuctionEscrowEnabled => self.auction_escrow_enabled,
            MarketCheck::Submit(_) => return self.submit_result,
        };
        match value {
            Some(true) => Gate::Pass,
            Some(false) => Gate::Fail,
            None => Gate::Unknown,
        }
    }
}

pub(super) fn dispatch_market(ctx: &mut Context<'_>, command: u16, req: &[u8]) -> Outcome {
    match command {
        ids::G_RetrieveSaleListReqCmd => handle_retrieve_sale_list(ctx, req),
        ids::G_RetrieveBidListReqCmd => handle_retrieve_bid_list(ctx, req),
        ids::G_RetrieveBidRankStatusReqCmd => handle_retrieve_bid_rank_status(ctx, req),
        ids::G_NPAuctionBidReqCmd => handle_np_auction_bid(ctx, req),
        ids::G_RegisterSaleReqCmd => handle_register_sale(ctx, req),
        ids::G_AbandonSaleReqCmd => handle_abandon_sale(ctx, req),
        ids::G_UpdateSalePriceReqCmd => handle_update_sale_price(ctx, req),
        ids::G_RegisterBidReqCmd => handle_register_bid(ctx, req),
        ids::G_AbandonBidReqCmd => handle_abandon_bid(ctx, req),
        ids::G_UpdateBidPriceReqCmd => handle_update_bid_price(ctx, req),
        ids::G_RegisterPurchaseReqCmd => handle_register_purchase(ctx, req),
        ids::G_MarketSearchItemReqCmd => handle_market_search_item(ctx, req),
        ids::G_RetrieveInstantSaleItemLowestPriceReqCmd => {
            handle_retrieve_instant_sale_item_lowest_price(ctx, req)
        }
        _ => unreachable!("membership checked"),
    }
}

fn arena_b(ctx: &Context<'_>) -> bool {
    ctx.world.arena || session::arena_blocked(ctx.pc())
}

fn user_found(ctx: &Context<'_>) -> bool {
    ctx.pc().user_present && !ctx.pc().party_user_blocked
}

fn gates(ctx: &Context<'_>, failure: u16) -> Option<Outcome> {
    if arena_b(ctx) {
        Some(Outcome::reason(failure, 5))
    } else if ctx.pc().t2_blocked {
        Some(Outcome::reason(failure, 7))
    } else {
        None
    }
}

fn check(ctx: &mut Context<'_>, query: MarketCheck, fail: Outcome) -> Option<Outcome> {
    match ctx.services.market(&query) {
        Gate::Pass => None,
        Gate::Fail => Some(fail),
        Gate::Unknown => Some(Outcome::unsupported("❓ market configuration")),
    }
}

fn submit(ctx: &mut Context<'_>, operation: MarketOperation, fail: Outcome) -> Outcome {
    let pc = ctx.pc();
    let request = MarketRequest {
        object_id: pc.object_id,
        db_id: pc.db_id,
        world: pc.short_96 as u16,
        operation,
    };
    match ctx.services.market(&MarketCheck::Submit(request)) {
        Gate::Pass => Outcome::default(),
        Gate::Fail => fail,
        Gate::Unknown => Outcome::unsupported("❓ market agent send/account transaction"),
    }
}

fn task(ctx: &Context<'_>, command: u16, zone: u64, task: MarketTask) -> SkillTask {
    SkillTask {
        command,
        caster: ctx.pc().object_id,
        summon: false,
        zone,
        kind: task.into(),
    }
}

/// parity: 378 24-market-handlers.md#378--g_retrievesalelistreqcmd--partial partial
pub fn handle_retrieve_sale_list(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    if arena_b(ctx) {
        // hEQQqc: reason, first, second, cursor, direction.
        let mut body = 5u16.to_le_bytes().to_vec();
        body.extend([0; 11]);
        return Outcome::reply(380, body);
    }
    let mut r = Reader(req);
    let (Some(first), Some(second), Some(cursor), Some(direction)) =
        (r.byte(), r.byte(), r.quad(), r.byte())
    else {
        return Outcome::unsupported("❓ 378 unchecked decode: uninitialized fields");
    };
    if !user_found(ctx) {
        return Outcome::default();
    }
    submit(
        ctx,
        MarketOperation::RetrieveSaleList {
            first,
            second,
            cursor,
            direction: direction as i8,
        },
        Outcome::default(), // Native ignores the dealer send result.
    )
}

/// parity: 381 24-market-handlers.md#381--g_retrievebidlistreqcmd--partial partial
pub fn handle_retrieve_bid_list(ctx: &mut Context<'_>, _req: &[u8]) -> Outcome {
    if arena_b(ctx) {
        return Outcome::reason(383, 5);
    }
    if !user_found(ctx) {
        return Outcome::default();
    }
    submit(ctx, MarketOperation::RetrieveBidList, Outcome::default())
}

/// parity: 384 24-market-handlers.md#384--g_retrievebidrankstatusreqcmd--partial partial
pub fn handle_retrieve_bid_rank_status(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_RetrieveBidRankStatusFailCmd;
    if arena_b(ctx) {
        return Outcome::reason(F, 5);
    }
    let mut r = Reader(req);
    let Some(count) = r.short().filter(|n| *n <= 32) else {
        return Outcome::reason(F, 2);
    };
    let Some(sales) = (0..count).map(|_| r.quad()).collect::<Option<Vec<_>>>() else {
        return Outcome::reason(F, 2);
    };
    if !user_found(ctx) {
        return Outcome::default();
    }
    submit(
        ctx,
        MarketOperation::RetrieveBidRankStatus { sales },
        Outcome::default(),
    )
}

/// parity: 1013 24-market-handlers.md#1013--g_npauctionbidreqcmd--partial partial
pub fn handle_np_auction_bid(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_NPAuctionBidFailCmd;
    if arena_b(ctx) {
        return Outcome::reason(F, 5);
    }
    let mut r = Reader(req);
    let (Some(auction), Some(currency_group), Some(price)) = (r.take(), r.short(), r.quad()) else {
        return Outcome::reason(F, 2);
    };
    let currency_group = currency_group as i16;
    let price = price as i64;
    if currency_group <= 0 || price <= 0 {
        return Outcome::reason(F, 2303);
    }
    if let Some(out) = check(
        ctx,
        MarketCheck::AuctionEscrowEnabled,
        Outcome::reason(F, 2304),
    ) {
        return out;
    }
    let Some(zone) = ctx.pc().zone else {
        return Outcome::reason(F, 2);
    };
    let pc = ctx.pc();
    if pc.short_96 <= 0 || pc.db_id == 0 {
        return Outcome::reason(F, 2);
    }
    let t = task(
        ctx,
        ids::G_NPAuctionBidReqCmd,
        zone,
        MarketTask::AuctionBid {
            world: pc.short_96,
            db_id: pc.db_id,
            auction,
            currency_group,
            price,
        },
    );
    queue(ctx, t, Outcome::reason(F, 2))
}

/// parity: 1530 24-market-handlers.md#1530--g_registersalereqcmd--partial partial
pub fn handle_register_sale(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_RegisterSaleFailCmd;
    if let Some(out) = check(
        ctx,
        MarketCheck::RegisterSaleEnabled,
        Outcome::reason(F, 1223),
    ) {
        return out;
    }
    if let Some(out) = gates(ctx, F) {
        return out;
    }
    let mut r = Reader(req);
    let (Some(item), Some(sale_type), Some(price), Some(count), Some(first), Some(second)) =
        (r.quad(), r.byte(), r.quad(), r.int(), r.short(), r.short())
    else {
        return Outcome::reason(F, 1213);
    };
    let Some(zone) = ctx.pc().zone else {
        return Outcome::reason(F, 1);
    };
    if ctx.pc().native_kind != 1 {
        return Outcome::reason(F, 1);
    }
    let t = task(
        ctx,
        ids::G_RegisterSaleReqCmd,
        zone,
        MarketTask::RegisterSale {
            item,
            sale_type,
            price: price as i64,
            count: count as i32,
            first: first as i16,
            second: second as i16,
        },
    );
    queue(ctx, t, Outcome::reason(F, 1))
}

/// parity: 1533 24-market-handlers.md#1533--g_abandonsalereqcmd--partial partial
pub fn handle_abandon_sale(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    if let Some(out) = gates(ctx, 1535) {
        return out;
    }
    let Some(sale) = Reader(req).quad() else {
        return Outcome::unsupported("❓ 1533 unchecked decode: uninitialized sale ID");
    };
    if !user_found(ctx) {
        return Outcome::default();
    }
    submit(
        ctx,
        MarketOperation::AbandonSale { sale },
        Outcome::default(),
    )
}

/// parity: 1536 24-market-handlers.md#1536--g_updatesalepricereqcmd--partial partial
pub fn handle_update_sale_price(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_UpdateSalePriceFailCmd;
    if let Some(out) = gates(ctx, F) {
        return out;
    }
    let mut r = Reader(req);
    let Some(sale) = r.quad() else {
        return Outcome::unsupported("❓ 1536 unchecked decode: uninitialized sale ID");
    };
    // Native initializes only the price. It ignores decode failure.
    let price = r.quad().unwrap_or(0) as i64;
    if price < 0 {
        return Outcome::reason(F, 33);
    }
    if let Some(out) = check(
        ctx,
        MarketCheck::SalePrice { price },
        Outcome::reason(F, 33),
    ) {
        return out;
    }
    if !user_found(ctx) {
        return Outcome::default();
    }
    submit(
        ctx,
        MarketOperation::UpdateSalePrice { sale, price },
        Outcome::default(),
    )
}

/// Native initializes every qword before these unchecked decodes. Stop on the
/// first short field, as `0x1406AA7B0` does. Keep all later fields at zero.
fn zero_quads<const N: usize>(req: &[u8]) -> [u64; N] {
    let mut r = Reader(req);
    let mut values = [0; N];
    for value in &mut values {
        let Some(v) = r.quad() else { break };
        *value = v;
    }
    values
}

/// parity: 1539 24-market-handlers.md#1539--g_registerbidreqcmd--partial partial
pub fn handle_register_bid(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_RegisterBidFailCmd;
    if let Some(out) = gates(ctx, F) {
        return out;
    }
    let [sale, price] = zero_quads(req);
    let price = price as i64;
    if price < 0 {
        return Outcome::reason(F, 33);
    }
    if let Some(out) = check(ctx, MarketCheck::BidPrice { price }, Outcome::reason(F, 33)) {
        return out;
    }
    if !user_found(ctx) {
        return Outcome::log("register bid: user is not found");
    }
    submit(
        ctx,
        MarketOperation::RegisterBid { sale, price },
        Outcome::reason(F, 1),
    )
}

/// parity: 1542 24-market-handlers.md#1542--g_abandonbidreqcmd--partial partial
pub fn handle_abandon_bid(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    if let Some(out) = gates(ctx, 1544) {
        return out;
    }
    let mut r = Reader(req);
    let (Some(sale), Some(bid)) = (r.quad(), r.quad()) else {
        return Outcome::unsupported("❓ 1542 unchecked decode: uninitialized sale/bid ID");
    };
    if !user_found(ctx) {
        return Outcome::default();
    }
    submit(
        ctx,
        MarketOperation::AbandonBid { sale, bid },
        Outcome::default(),
    )
}

/// parity: 1545 24-market-handlers.md#1545--g_updatebidpricereqcmd--partial partial
pub fn handle_update_bid_price(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_UpdateBidPriceFailCmd;
    if let Some(out) = gates(ctx, F) {
        return out;
    }
    let [sale, bid, price] = zero_quads(req);
    let price = price as i64;
    // Unlike 1539/1548, this entry gets the user before it checks the price.
    if !user_found(ctx) {
        return Outcome::log("update bid price: user is not found");
    }
    if price < 0 {
        return Outcome::reason(F, 33);
    }
    if let Some(out) = check(ctx, MarketCheck::BidPrice { price }, Outcome::reason(F, 33)) {
        return out;
    }
    submit(
        ctx,
        MarketOperation::UpdateBidPrice { sale, bid, price },
        Outcome::reason(F, 1),
    )
}

/// parity: 1548 24-market-handlers.md#1548--g_registerpurchasereqcmd--partial partial
pub fn handle_register_purchase(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_RegisterPurchaseFailCmd;
    if let Some(out) = gates(ctx, F) {
        return out;
    }
    let [sale, price] = zero_quads(req);
    let price = price as i64;
    if price < 0 {
        return Outcome::reason(F, 33);
    }
    if let Some(out) = check(
        ctx,
        MarketCheck::SalePrice { price },
        Outcome::reason(F, 33),
    ) {
        return out;
    }
    if !user_found(ctx) {
        return Outcome::log("register purchase: user is not found");
    }
    submit(
        ctx,
        MarketOperation::RegisterPurchase { sale, price },
        Outcome::reason(F, 2),
    )
}

/// parity: 1551 24-market-handlers.md#1551--g_marketsearchitemreqcmd--partial partial
pub fn handle_market_search_item(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_MarketSearchItemFailCmd;
    if ctx.world.arena {
        return Outcome::reason(F, 5);
    }
    let mut r = Reader(req);
    let (
        Some(search_type),
        Some(order_type),
        Some(price_type),
        Some(sale_type),
        Some(item),
        Some(first),
        Some(second),
        Some(third),
    ) = (
        r.byte(),
        r.byte(),
        r.byte(),
        r.byte(),
        r.int(),
        r.quad(),
        r.quad(),
        r.quad(),
    )
    else {
        return Outcome::reason(F, 2);
    };
    if !(1..=3).contains(&search_type)
        || !(1..=2).contains(&order_type)
        || !(1..=2).contains(&price_type)
        || !(1..=2).contains(&sale_type)
    {
        return Outcome::reason(F, 1955);
    }
    if !user_found(ctx) {
        return Outcome::reason(F, 2);
    }
    submit(
        ctx,
        MarketOperation::SearchItem {
            search_type,
            order_type,
            price_type,
            sale_type,
            item: item as i32,
            first,
            second,
            third,
        },
        Outcome::reason(F, 2),
    )
}

fn lowest_price_failure(reason: u16) -> Outcome {
    // Native hEi calls supply only the reason. No defined final i32 exists.
    Outcome::unsupported(match reason {
        5 => "❓ 1556 hEi reason 5: final i32 argument not supplied by native caller",
        _ => "❓ 1556 hEi reason 2: final i32 argument not supplied by native caller",
    })
}

/// parity: 1554 24-market-handlers.md#1554--g_retrieveinstantsaleitemlowestpricereqcmd--partial partial
pub fn handle_retrieve_instant_sale_item_lowest_price(
    ctx: &mut Context<'_>,
    req: &[u8],
) -> Outcome {
    if ctx.world.arena {
        return lowest_price_failure(5);
    }
    let Some(item) = Reader(req).int() else {
        return lowest_price_failure(2);
    };
    if !user_found(ctx) {
        return lowest_price_failure(2);
    }
    submit(
        ctx,
        MarketOperation::LowestPrice { item: item as i32 },
        lowest_price_failure(2),
    )
}
