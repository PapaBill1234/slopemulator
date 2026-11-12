//! BoardGachaDraw failure only. Rejected movement/skills do not use this path.
//! TODO ❓ Kick identity/reason, +252 provenance and final close are not mapped.
use crate::ids;

/// Numeric conversion, not client enum names or a claim of backend reachability.
/// Spec: Step 5 part 4, Backend reason conversion; 381 0x140018AE0.
pub fn backend_reason(code: i32) -> i16 {
    match code {
        0 => 0,
        2..=8 | 21..=23 | 45 => 2,
        15..=19 => (code - 15 + 1959) as i16,
        25..=35 => (code - 25 + 16) as i16,
        41 => 27,
        46..=48 => (code - 46 + 2515) as i16,
        49..=55 => (code - 49 + 2523) as i16,
        56 | 57 => 2532,
        59..=62 => (code - 59 + 2533) as i16,
        _ => 1,
    }
}

/// Spec: Step 5 part 4, Direct failure reasons; 381 0x1401C84F0.
pub fn abort_reason(supplied: Option<i16>) -> i16 {
    supplied.unwrap_or(1)
}

/// Verified trigger categories only. This does not implement the transaction.
/// Spec: Step 5 part 4, Direct failure reasons; 381 0x1401C6AC0,
/// 0x1401C8050, 0x1401C82D0, 0x1401C7A80, 0x1401C76B0, 0x1401C7D80.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailureReason {
    FinishOrTaskInitialization,
    TryDrawTicket,
    ConditionOrConsume,
    Abort(Option<i16>),
    Expiry(i16),
    Backend(i32),
}

impl FailureReason {
    /// Spec: Step 5 part 4, Direct failure reasons; 381 0x1401C84F0,
    /// 0x1401C8720, 0x1401C8600, 0x1401C6AC0.
    pub fn client_reason(self) -> i16 {
        match self {
            Self::FinishOrTaskInitialization => 2,
            Self::TryDrawTicket => 2530,
            Self::ConditionOrConsume => 2531,
            Self::Abort(reason) => abort_reason(reason),
            Self::Expiry(reason) => reason,
            Self::Backend(code) => backend_reason(code),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailureAction {
    SendReply {
        command: i16,
        client_reason: i16,
    },
    ReleaseSessionReference,
    LogMissingSession,
    LogKick,
    /// Boundary only. Identity and separate kick reason remain unknown.
    EnterUserManagerKickBoundary,
    LogTransactionFailure,
    SetTransactionState(u8),
}

/// Ordered, inert plan. No socket close and no generic validation kick.
/// `kick_flag_252` is supplied server transaction state, never inferred from failure.
/// Spec: Step 5 part 4, Ordered failure actions; 381 0x1401C8820,
/// reference release 0x14000B730, kick boundary 0x1403EF8E0, state 0x1400D47D0.
pub fn board_gacha_failure(
    has_session: bool,
    kick_flag_252: bool,
    reason: FailureReason,
) -> Vec<FailureAction> {
    use FailureAction::*;
    let mut actions = Vec::new();
    if has_session {
        actions.push(SendReply {
            command: ids::G_BoardGachaDrawByItemFailCmd as i16,
            client_reason: reason.client_reason(),
        });
        actions.push(ReleaseSessionReference);
    } else {
        actions.push(LogMissingSession);
    }
    if kick_flag_252 {
        actions.push(LogKick);
        actions.push(EnterUserManagerKickBoundary);
    }
    actions.push(LogTransactionFailure);
    actions.push(SetTransactionState(5));
    actions
}
