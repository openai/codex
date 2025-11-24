//! Channel flavors.
//!
//! There are six flavors:
//!
//! 1. `at` - Channel that delivers a message after a certain amount of time.
//! 2. `array` - Bounded channel based on a preallocated array.
//! 3. `list` - Unbounded channel implemented as a linked list.
//! 4. `never` - Channel that never delivers messages.
//! 5. `tick` - Channel that delivers messages periodically.
//! 6. `zero` - Zero-capacity channel.

pub(crate) mod array;
pub(crate) mod at;
pub(crate) mod list;
pub(crate) mod never;
pub(crate) mod tick;
pub(crate) mod zero;
