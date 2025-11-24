//! Additional utilities for tracking time.
//!
//! This module provides additional utilities for executing code after a set period
//! of time. Currently there is only one:
//!
//! * `DelayQueue`: A queue where items are returned once the requested delay
//!   has expired.
//!
//! This type must be used from within the context of the `Runtime`.

use std::time::Duration;

mod wheel;

pub mod delay_queue;

// re-export `FutureExt` to avoid breaking change
#[doc(inline)]
pub use crate::future::FutureExt;

#[doc(inline)]
pub use delay_queue::DelayQueue;

// ===== Internal utils =====

enum Round {
    Up,
    Down,
}

/// Convert a `Duration` to milliseconds, rounding up and saturating at
/// `u64::MAX`.
///
/// The saturating is fine because `u64::MAX` milliseconds are still many
/// million years.
#[inline]
fn ms(duration: Duration, round: Round) -> u64 {
    const NANOS_PER_MILLI: u32 = 1_000_000;
    const MILLIS_PER_SEC: u64 = 1_000;

    // Round up.
    let millis = match round {
        Round::Up => (duration.subsec_nanos() + NANOS_PER_MILLI - 1) / NANOS_PER_MILLI,
        Round::Down => duration.subsec_millis(),
    };

    duration
        .as_secs()
        .saturating_mul(MILLIS_PER_SEC)
        .saturating_add(u64::from(millis))
}
