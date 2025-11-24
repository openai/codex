//! Atomic types.
//!
//! * [`AtomicCell`], a thread-safe mutable memory location.
//! * [`AtomicConsume`], for reading from primitive atomic types with "consume" ordering.

#[cfg(target_has_atomic = "ptr")]
#[cfg(not(crossbeam_loom))]
// Use "wide" sequence lock if the pointer width <= 32 for preventing its counter against wrap
// around.
//
// In narrow architectures (pointer width <= 16), the counter is still <= 32-bit and may be
// vulnerable to wrap around. But it's mostly okay, since in such a primitive hardware, the
// counter will not be increased that fast.
// Note that Rust (and C99) pointers must be at least 16-bit (i.e., 8-bit targets are impossible): https://github.com/rust-lang/rust/pull/49305
#[cfg_attr(
    any(target_pointer_width = "16", target_pointer_width = "32"),
    path = "seq_lock_wide.rs"
)]
mod seq_lock;

#[cfg(target_has_atomic = "ptr")]
// We cannot provide AtomicCell under cfg(crossbeam_loom) because loom's atomic
// types have a different in-memory representation than the underlying type.
// TODO: The latest loom supports fences, so fallback using seqlock may be available.
#[cfg(not(crossbeam_loom))]
mod atomic_cell;
#[cfg(target_has_atomic = "ptr")]
#[cfg(not(crossbeam_loom))]
pub use atomic_cell::AtomicCell;

mod consume;
pub use consume::AtomicConsume;
