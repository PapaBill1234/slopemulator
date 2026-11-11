//! Pure build-381 validation subsets. No sockets or complete session approval.
//! Source: Docs/Server/GameDaemon/07-server-validation.md, Step 5 parts 1–4.
//! Data adapters must use server-owned state. No real game records are embedded.
//! TODO ❓ Full movement transitions, skill dispatch, data loading and wire integration.
//!
//! Restricted hooks: 381 0x1405DF767 and 0x14036DE10. No implementation.
//! Restricted private-object boundary: 381 0x1406E3100. No implementation.

pub mod db;
pub mod handlers;
pub mod ids;
pub mod kick;
pub mod link;
pub mod validation;

/// Server object coordinates at +268/+270/+272, not request-owned positions.
/// Spec: Step 5 part 3, Common position; 381 0x1403DE170, 0x140573EA0.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Position {
    pub x: i16,
    pub y: i16,
    pub z: i16,
}

impl Position {
    pub(crate) fn squared_distance(self, other: Self) -> i64 {
        let dx = i64::from(self.x) - i64::from(other.x);
        let dy = i64::from(self.y) - i64::from(other.y);
        let dz = i64::from(self.z) - i64::from(other.z);
        dx * dx + dy * dy + dz * dz
    }
}
