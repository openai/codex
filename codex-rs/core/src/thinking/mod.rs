//! Ultrathink mode module for enhanced reasoning capabilities.
//!
//! This module provides:
//! - Keyword detection for "ultrathink" in messages
//! - Session-level toggle state
//! - Effective effort computation with priority chain
//! - Configuration types for per-provider customization

mod detector;
mod integration;
mod state;
mod types;

pub use detector::detect_ultrathink;
pub use detector::extract_keyword_positions;
pub use integration::EffortResult;
pub use integration::compute_effective_effort;
pub use state::ThinkingState;
pub use types::DEFAULT_ULTRATHINK_BUDGET;
pub use types::UltrathinkConfig;
