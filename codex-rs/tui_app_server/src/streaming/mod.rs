//! Streaming helpers used by the TUI transcript pipeline.
//!
//! `controller` owns the mutable in-flight markdown snapshots for message and
//! plan streams. `chunking` computes adaptive commit cadence from queue
//! pressure, and `commit_tick` binds that policy to concrete controller drains.
pub(crate) mod chunking;
pub(crate) mod commit_tick;
pub(crate) mod controller;
