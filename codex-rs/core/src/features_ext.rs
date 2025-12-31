//! Extension features to minimize modifications to features.rs
//!
//! This module defines extension feature specifications separately from the core
//! FEATURES array to reduce upstream merge conflicts during syncs.

use crate::features::Feature;
use crate::features::FeatureSpec;
use crate::features::Stage;

/// Extension feature specifications.
/// Add new ext features here instead of modifying features.rs FEATURES array.
/// Use `features::all_features()` to access all features (core + ext).
pub(crate) const EXT_FEATURES: &[FeatureSpec] = &[
    FeatureSpec {
        id: Feature::SmartEdit,
        key: "smart_edit",
        stage: Stage::Stable,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::RichGrep,
        key: "rich_grep",
        stage: Stage::Stable,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::EnhancedListDir,
        key: "enhanced_list_dir",
        stage: Stage::Stable,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::WebFetch,
        key: "web_fetch",
        stage: Stage::Experimental,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::CodeSearch,
        key: "code_search",
        stage: Stage::Experimental,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::Lsp,
        key: "lsp",
        stage: Stage::Experimental,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::McpResourceTools,
        key: "mcp_resource_tools",
        stage: Stage::Stable,
        default_enabled: true,
    },
    FeatureSpec {
        id: Feature::Subagent,
        key: "subagent",
        stage: Stage::Experimental,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::CompactV2,
        key: "compact_v2",
        stage: Stage::Experimental,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::MicroCompact,
        key: "micro_compact",
        stage: Stage::Experimental,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::WebSearch,
        key: "web_search",
        stage: Stage::Experimental,
        default_enabled: false,
    },
];
