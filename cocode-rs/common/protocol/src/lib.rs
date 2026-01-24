//! Protocol types for cocode multi-provider SDK.
//!
//! This crate provides the foundational types used across the cocode ecosystem:
//! - Model capabilities and reasoning levels
//! - Model configuration types
//! - Shell and truncation policies

pub mod features;
pub mod model;

// Model types
pub use model::Capability;
pub use model::ConfigShellToolType;
pub use model::ModelInfo;
pub use model::ReasoningEffort;
pub use model::TruncationMode;
pub use model::TruncationPolicyConfig;
pub use model::effort_rank;
pub use model::nearest_effort;

// Feature types
pub use features::Feature;
pub use features::FeatureSpec;
pub use features::Features;
pub use features::Stage;
pub use features::all_features;
pub use features::feature_for_key;
pub use features::is_known_feature_key;
