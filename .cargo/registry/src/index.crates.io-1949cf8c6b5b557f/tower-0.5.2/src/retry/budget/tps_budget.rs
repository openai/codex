//! Transactions Per Minute (Tps) Budget implementations

use std::{
    fmt,
    sync::{
        atomic::{AtomicIsize, Ordering},
        Mutex,
    },
    time::Duration,
};
use tokio::time::Instant;

use super::Budget;

/// A Transactions Per Minute config for managing retry tokens.
///
/// [`TpsBudget`] uses a token bucket to decide if the request should be retried.
///
/// [`TpsBudget`] works by checking how much retries have been made in a certain period of time.
/// Minimum allowed number of retries are effectively reset on an interval. Allowed number of
/// retries depends on failed request count in recent time frame.
///
/// For more info about [`Budget`], please see the [module-level documentation].
///
/// [module-level documentation]: super
pub struct TpsBudget {
    generation: Mutex<Generation>,
    /// Initial budget allowed for every second.
    reserve: isize,
    /// Slots of a the TTL divided evenly.
    slots: Box<[AtomicIsize]>,
    /// The amount of time represented by each slot.
    window: Duration,
    /// The changers for the current slot to be committed
    /// after the slot expires.
    writer: AtomicIsize,
    /// Amount of tokens to deposit for each put().
    deposit_amount: isize,
    /// Amount of tokens to withdraw for each try_get().
    withdraw_amount: isize,
}

#[derive(Debug)]
struct Generation {
    /// Slot index of the last generation.
    index: usize,
    /// The timestamp since the last generation expired.
    time: Instant,
}

// ===== impl TpsBudget =====

impl TpsBudget {
    /// Create a [`TpsBudget`] that allows for a certain percent of the total
    /// requests to be retried.
    ///
    /// - The `ttl` is the duration of how long a single `deposit` should be
    ///   considered. Must be between 1 and 60 seconds.
    /// - The `min_per_sec` is the minimum rate of retries allowed to accommodate
    ///   clients that have just started issuing requests, or clients that do
    ///   not issue many requests per window.
    /// - The `retry_percent` is the percentage of calls to `deposit` that can
    ///   be retried. This is in addition to any retries allowed for via
    ///   `min_per_sec`. Must be between 0 and 1000.
    ///
    ///   As an example, if `0.1` is used, then for every 10 calls to `deposit`,
    ///   1 retry will be allowed. If `2.0` is used, then every `deposit`
    ///   allows for 2 retries.
    pub fn new(ttl: Duration, min_per_sec: u32, retry_percent: f32) -> Self {
        // assertions taken from finagle
        assert!(ttl >= Duration::from_secs(1));
        assert!(ttl <= Duration::from_secs(60));
        assert!(retry_percent >= 0.0);
        assert!(retry_percent <= 1000.0);
        assert!(min_per_sec < ::std::i32::MAX as u32);

        let (deposit_amount, withdraw_amount) = if retry_percent == 0.0 {
            // If there is no percent, then you gain nothing from deposits.
            // Withdrawals can only be made against the reserve, over time.
            (0, 1)
        } else if retry_percent <= 1.0 {
            (1, (1.0 / retry_percent) as isize)
        } else {
            // Support for when retry_percent is between 1.0 and 1000.0,
            // meaning for every deposit D, D * retry_percent withdrawals
            // can be made.
            (1000, (1000.0 / retry_percent) as isize)
        };
        let reserve = (min_per_sec as isize)
            .saturating_mul(ttl.as_secs() as isize) // ttl is between 1 and 60 seconds
            .saturating_mul(withdraw_amount);

        // AtomicIsize isn't clone, so the slots need to be built in a loop...
        let windows = 10u32;
        let mut slots = Vec::with_capacity(windows as usize);
        for _ in 0..windows {
            slots.push(AtomicIsize::new(0));
        }

        TpsBudget {
            generation: Mutex::new(Generation {
                index: 0,
                time: Instant::now(),
            }),
            reserve,
            slots: slots.into_boxed_slice(),
            window: ttl / windows,
            writer: AtomicIsize::new(0),
            deposit_amount,
            withdraw_amount,
        }
    }

    fn expire(&self) {
        let mut gen = self.generation.lock().expect("generation lock");

        let now = Instant::now();
        let diff = now.saturating_duration_since(gen.time);
        if diff < self.window {
            // not expired yet
            return;
        }

        let to_commit = self.writer.swap(0, Ordering::SeqCst);
        self.slots[gen.index].store(to_commit, Ordering::SeqCst);

        let mut diff = diff;
        let mut idx = (gen.index + 1) % self.slots.len();
        while diff > self.window {
            self.slots[idx].store(0, Ordering::SeqCst);
            diff -= self.window;
            idx = (idx + 1) % self.slots.len();
        }

        gen.index = idx;
        gen.time = now;
    }

    fn sum(&self) -> isize {
        let current = self.writer.load(Ordering::SeqCst);
        let windowed_sum: isize = self
            .slots
            .iter()
            .map(|slot| slot.load(Ordering::SeqCst))
            // fold() is used instead of sum() to determine overflow behavior
            .fold(0, isize::saturating_add);

        current
            .saturating_add(windowed_sum)
            .saturating_add(self.reserve)
    }

    fn put(&self, amt: isize) {
        self.expire();
        self.writer.fetch_add(amt, Ordering::SeqCst);
    }

    fn try_get(&self, amt: isize) -> bool {
        debug_assert!(amt >= 0);

        self.expire();

        let sum = self.sum();
        if sum >= amt {
            self.writer.fetch_add(-amt, Ordering::SeqCst);
            true
        } else {
            false
        }
    }
}

impl Budget for TpsBudget {
    fn deposit(&self) {
        self.put(self.deposit_amount)
    }

    fn withdraw(&self) -> bool {
        self.try_get(self.withdraw_amount)
    }
}

impl Default for TpsBudget {
    fn default() -> Self {
        TpsBudget::new(Duration::from_secs(10), 10, 0.2)
    }
}

impl fmt::Debug for TpsBudget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Budget")
            .field("deposit", &self.deposit_amount)
            .field("withdraw", &self.withdraw_amount)
            .field("balance", &self.sum())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use crate::retry::budget::Budget;

    use super::*;
    use tokio::time;

    #[test]
    fn tps_empty() {
        let bgt = TpsBudget::new(Duration::from_secs(1), 0, 1.0);
        assert!(!bgt.withdraw());
    }

    #[tokio::test]
    async fn tps_leaky() {
        time::pause();

        let bgt = TpsBudget::new(Duration::from_secs(1), 0, 1.0);
        bgt.deposit();

        time::advance(Duration::from_secs(3)).await;

        assert!(!bgt.withdraw());
    }

    #[tokio::test]
    async fn tps_slots() {
        time::pause();

        let bgt = TpsBudget::new(Duration::from_secs(1), 0, 0.5);
        bgt.deposit();
        bgt.deposit();
        time::advance(Duration::from_millis(901)).await;
        // 900ms later, the deposit should still be valid
        assert!(bgt.withdraw());

        // blank slate
        time::advance(Duration::from_millis(2001)).await;

        bgt.deposit();
        time::advance(Duration::from_millis(301)).await;
        bgt.deposit();
        time::advance(Duration::from_millis(801)).await;
        bgt.deposit();

        // the first deposit is expired, but the 2nd should still be valid,
        // combining with the 3rd
        assert!(bgt.withdraw());
    }

    #[tokio::test]
    async fn tps_reserve() {
        let bgt = TpsBudget::new(Duration::from_secs(1), 5, 1.0);
        assert!(bgt.withdraw());
        assert!(bgt.withdraw());
        assert!(bgt.withdraw());
        assert!(bgt.withdraw());
        assert!(bgt.withdraw());

        assert!(!bgt.withdraw());
    }
}
