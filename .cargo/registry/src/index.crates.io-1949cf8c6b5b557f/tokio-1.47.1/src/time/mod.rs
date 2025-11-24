//! Utilities for tracking time.
//!
//! This module provides a number of types for executing code after a set period
//! of time.
//!
//! * [`Sleep`] is a future that does no work and completes at a specific [`Instant`]
//!   in time.
//!
//! * [`Interval`] is a stream yielding a value at a fixed period. It is
//!   initialized with a [`Duration`] and repeatedly yields each time the duration
//!   elapses.
//!
//! * [`Timeout`]: Wraps a future or stream, setting an upper bound to the amount
//!   of time it is allowed to execute. If the future or stream does not
//!   complete in time, then it is canceled and an error is returned.
//!
//! These types are sufficient for handling a large number of scenarios
//! involving time.
//!
//! These types must be used from within the context of the [`Runtime`](crate::runtime::Runtime).
//!
//! # Examples
//!
//! Wait 100ms and print "100 ms have elapsed"
//!
//! ```
//! use std::time::Duration;
//! use tokio::time::sleep;
//!
//! #[tokio::main]
//! async fn main() {
//!     sleep(Duration::from_millis(100)).await;
//!     println!("100 ms have elapsed");
//! }
//! ```
//!
//! Require that an operation takes no more than 1s.
//!
//! ```
//! use tokio::time::{timeout, Duration};
//!
//! async fn long_future() {
//!     // do work here
//! }
//!
//! # async fn dox() {
//! let res = timeout(Duration::from_secs(1), long_future()).await;
//!
//! if res.is_err() {
//!     println!("operation timed out");
//! }
//! # }
//! ```
//!
//! A simple example using [`interval`] to execute a task every two seconds.
//!
//! The difference between [`interval`] and [`sleep`] is that an [`interval`]
//! measures the time since the last tick, which means that `.tick().await` may
//! wait for a shorter time than the duration specified for the interval
//! if some time has passed between calls to `.tick().await`.
//!
//! If the tick in the example below was replaced with [`sleep`], the task
//! would only be executed once every three seconds, and not every two
//! seconds.
//!
//! ```
//! use tokio::time;
//!
//! async fn task_that_takes_a_second() {
//!     println!("hello");
//!     time::sleep(time::Duration::from_secs(1)).await
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     let mut interval = time::interval(time::Duration::from_secs(2));
//!     for _i in 0..5 {
//!         interval.tick().await;
//!         task_that_takes_a_second().await;
//!     }
//! }
//! ```
//!
//! [`interval`]: crate::time::interval()
//! [`sleep`]: sleep()

mod clock;
pub(crate) use self::clock::Clock;
cfg_test_util! {
    pub use clock::{advance, pause, resume};
}

pub mod error;

mod instant;
pub use self::instant::Instant;

mod interval;
pub use interval::{interval, interval_at, Interval, MissedTickBehavior};

mod sleep;
pub use sleep::{sleep, sleep_until, Sleep};

mod timeout;
#[doc(inline)]
pub use timeout::{timeout, timeout_at, Timeout};

// Re-export for convenience
#[doc(no_inline)]
pub use std::time::Duration;
