//! Connection pool (`Docs/Architecture/scale-rules.md`, rule D1).
//!
//! The pool holds idle connections and limits the count of connections in use. It does not know
//! the driver: the caller opens a connection when the lease has none.
//!
//! A connection goes back to the pool only through [`Lease::give`]. A call that fails, that hits
//! its time limit or that is cancelled drops its connection. Thus a connection with an unknown
//! protocol state is never used again.
//!
//! No lock is held across an `.await` (rule O4).

use std::sync::Mutex;
use tokio::sync::{Semaphore, SemaphorePermit};

pub struct Pool<C> {
    idle: Mutex<Vec<C>>,
    permits: Semaphore,
}

/// The right to use one connection. Drop it to free the place in the pool.
pub struct Lease<'a, C> {
    pool: &'a Pool<C>,
    _permit: SemaphorePermit<'a>,
}

impl<C> Pool<C> {
    /// `size`: largest count of connections in use at one time (at least 1).
    pub fn new(size: usize) -> Self {
        let size = size.max(1);
        Self {
            idle: Mutex::new(Vec::with_capacity(size)),
            permits: Semaphore::new(size),
        }
    }

    /// Wait for a free place. Callers get their places in the order of their calls.
    pub async fn acquire(&self) -> Lease<'_, C> {
        let permit = self
            .permits
            .acquire()
            .await
            .expect("the pool does not close its semaphore");
        Lease {
            pool: self,
            _permit: permit,
        }
    }

    /// Count of idle connections.
    pub fn idle(&self) -> usize {
        self.idle.lock().map_or(0, |idle| idle.len())
    }
}

impl<C> Lease<'_, C> {
    /// An idle connection, if the pool has one. If not, the caller opens a new connection.
    pub fn take(&mut self) -> Option<C> {
        self.pool.idle.lock().ok()?.pop()
    }

    /// Give a good connection back and free the place.
    pub fn give(self, connection: C) {
        if let Ok(mut idle) = self.pool.idle.lock() {
            idle.push(connection);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::Arc, time::Duration};

    #[tokio::test]
    async fn a_given_connection_is_used_again_and_a_dropped_one_is_not() {
        let pool = Pool::<u32>::new(2);
        let mut lease = pool.acquire().await;
        assert_eq!(lease.take(), None);
        lease.give(7);
        assert_eq!(pool.idle(), 1);
        let mut lease = pool.acquire().await;
        assert_eq!(lease.take(), Some(7));
        // The call failed: the lease is dropped and the connection does not come back.
        drop(lease);
        assert_eq!(pool.idle(), 0);
        assert_eq!(pool.acquire().await.take(), None);
    }

    #[tokio::test]
    async fn the_size_limits_the_connections_in_use() {
        let pool = Arc::new(Pool::<u32>::new(2));
        let first = pool.acquire().await;
        let _second = pool.acquire().await;
        let waiter = tokio::spawn({
            let pool = pool.clone();
            async move {
                let _lease = pool.acquire().await;
            }
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(!waiter.is_finished());
        drop(first);
        tokio::time::timeout(Duration::from_secs(1), waiter)
            .await
            .unwrap()
            .unwrap();
    }
}
