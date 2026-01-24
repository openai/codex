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
        stage: Stage::Experimental {
            name: "Web fetch",
            menu_description: "Allow fetching content from web URLs.",
            announcement: "NEW! Try Web fetch to retrieve content from URLs. Enable in /experimental!",
        },
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::Retrieval,
        key: "code_search",
        stage: Stage::Experimental {
            name: "Code search",
            menu_description: "Enable semantic code search with embeddings.",
            announcement: "NEW! Try Code search for semantic code retrieval. Enable in /experimental!",
        },
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::Lsp,
        key: "lsp",
        stage: Stage::Experimental {
            name: "LSP integration",
            menu_description: "Enable Language Server Protocol integration for diagnostics.",
            announcement: "NEW! Try LSP integration for better code diagnostics. Enable in /experimental!",
        },
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
        stage: Stage::Experimental {
            name: "Subagent",
            menu_description: "Enable spawning subagents for complex tasks.",
            announcement: "NEW! Try Subagents for complex multi-step tasks. Enable in /experimental!",
        },
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::CompactV2,
        key: "compact_v2",
        stage: Stage::Experimental {
            name: "Compact v2",
            menu_description: "Enable improved conversation compaction algorithm.",
            announcement: "NEW! Try Compact v2 for better context management. Enable in /experimental!",
        },
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::MicroCompact,
        key: "micro_compact",
        stage: Stage::Experimental {
            name: "Micro compact",
            menu_description: "Enable micro-compaction for incremental context reduction.",
            announcement: "NEW! Try Micro compact for incremental compaction. Enable in /experimental!",
        },
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::WebSearch,
        key: "web_search",
        stage: Stage::Experimental {
            name: "Web search",
            menu_description: "Enable web search capability.",
            announcement: "NEW! Try Web search for real-time information. Enable in /experimental!",
        },
        default_enabled: false,
    },
];
