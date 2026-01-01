//! Hook executors for different hook types.
//!
//! This module provides executors for:
//! - Command hooks (bash/shell execution)
//! - Callback hooks (native Rust callbacks)

pub mod callback;
pub mod command;
