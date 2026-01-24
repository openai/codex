//! Parameterized test suite for codex-api adapters.
//!
//! This module provides reusable test functions that can be run against any provider.
//! Tests are generated using macros to create per-provider test functions.

pub mod basic;
pub mod multi_turn_tools;
pub mod reasoning;
pub mod tools;
pub mod vision;
