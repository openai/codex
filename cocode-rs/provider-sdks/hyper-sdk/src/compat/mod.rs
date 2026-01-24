//! Compatibility layer for codex-api integration.
//!
//! This module provides adapters and converters for using hyper-sdk
//! with the existing codex-api infrastructure.

pub mod adapter;
pub mod convert;

pub use adapter::HyperAdapter;
