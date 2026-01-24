//! Parameterized test suite for hyper-sdk providers.
//!
//! This module provides reusable test functions that can be run against any provider.
//! Tests are generated using macros to create per-provider test functions.

pub mod basic;
pub mod cross_provider;
pub mod streaming;
pub mod tools;
pub mod vision;
