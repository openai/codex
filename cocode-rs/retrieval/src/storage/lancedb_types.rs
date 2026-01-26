//! LanceDB auxiliary types.
//!
//! Contains types for file metadata, index policy, and status.

use serde::Deserialize;
use serde::Serialize;

/// File metadata for change detection.
///
/// Contains the metadata needed for change detection without
/// loading the full chunk content.
#[derive(Debug, Clone)]
pub struct FileMetadata {
    /// File path (relative to workspace)
    pub filepath: String,
    /// Workspace identifier
    pub workspace: String,
    /// Content hash for change detection
    pub content_hash: String,
    /// File modification time
    pub mtime: i64,
    /// Index timestamp
    pub indexed_at: i64,
}

// ============================================================================
// Index Policy and Status
// ============================================================================

/// Index creation policy.
///
/// Defines when and how to create vector/FTS indexes.
#[derive(Debug, Clone)]
pub struct IndexPolicy {
    /// Create vector index after N chunks (0 = never auto-create).
    pub chunk_threshold: i64,
    /// Create FTS index after N chunks (0 = never auto-create).
    pub fts_chunk_threshold: i64,
    /// Force index rebuild even if index exists.
    pub force_rebuild: bool,
}

impl Default for IndexPolicy {
    fn default() -> Self {
        Self {
            chunk_threshold: 10_000,    // 10k chunks for vector index
            fts_chunk_threshold: 1_000, // 1k chunks for FTS index
            force_rebuild: false,
        }
    }
}

impl IndexPolicy {
    /// Create a policy that never auto-creates indexes.
    pub fn never() -> Self {
        Self {
            chunk_threshold: 0,
            fts_chunk_threshold: 0,
            force_rebuild: false,
        }
    }

    /// Create a policy for immediate index creation.
    pub fn immediate() -> Self {
        Self {
            chunk_threshold: 1,
            fts_chunk_threshold: 1,
            force_rebuild: false,
        }
    }

    /// Set vector index threshold.
    pub fn with_vector_threshold(mut self, threshold: i64) -> Self {
        self.chunk_threshold = threshold;
        self
    }

    /// Set FTS index threshold.
    pub fn with_fts_threshold(mut self, threshold: i64) -> Self {
        self.fts_chunk_threshold = threshold;
        self
    }

    /// Enable force rebuild.
    pub fn with_force_rebuild(mut self) -> Self {
        self.force_rebuild = true;
        self
    }
}

/// Index status information.
#[derive(Debug, Clone, Default)]
pub struct IndexStatus {
    /// Whether table exists.
    pub table_exists: bool,
    /// Current chunk count.
    pub chunk_count: i64,
    /// Whether vector index creation is recommended.
    pub vector_index_recommended: bool,
    /// Whether FTS index creation is recommended.
    pub fts_index_recommended: bool,
}

impl IndexStatus {
    /// Check if any index creation is recommended.
    pub fn needs_indexing(&self) -> bool {
        self.vector_index_recommended || self.fts_index_recommended
    }
}

/// Configuration for index policy from TOML.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct IndexPolicyConfig {
    /// Create vector index after N chunks (0 = never).
    #[serde(default = "default_chunk_threshold")]
    pub chunk_threshold: i64,

    /// Create FTS index after N chunks (0 = never).
    #[serde(default = "default_fts_chunk_threshold")]
    pub fts_chunk_threshold: i64,
}

fn default_chunk_threshold() -> i64 {
    10_000
}

fn default_fts_chunk_threshold() -> i64 {
    1_000
}

impl From<&IndexPolicyConfig> for IndexPolicy {
    fn from(config: &IndexPolicyConfig) -> Self {
        Self {
            chunk_threshold: config.chunk_threshold,
            fts_chunk_threshold: config.fts_chunk_threshold,
            force_rebuild: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_policy() {
        let policy = IndexPolicy::default();
        assert_eq!(policy.chunk_threshold, 10_000);
        assert_eq!(policy.fts_chunk_threshold, 1_000);
        assert!(!policy.force_rebuild);
    }

    #[test]
    fn test_never_policy() {
        let policy = IndexPolicy::never();
        assert_eq!(policy.chunk_threshold, 0);
        assert_eq!(policy.fts_chunk_threshold, 0);
    }

    #[test]
    fn test_immediate_policy() {
        let policy = IndexPolicy::immediate();
        assert_eq!(policy.chunk_threshold, 1);
        assert_eq!(policy.fts_chunk_threshold, 1);
    }

    #[test]
    fn test_policy_builder() {
        let policy = IndexPolicy::default()
            .with_vector_threshold(5_000)
            .with_fts_threshold(500)
            .with_force_rebuild();

        assert_eq!(policy.chunk_threshold, 5_000);
        assert_eq!(policy.fts_chunk_threshold, 500);
        assert!(policy.force_rebuild);
    }

    #[test]
    fn test_index_status_needs_indexing() {
        let status = IndexStatus::default();
        assert!(!status.needs_indexing());

        let status = IndexStatus {
            vector_index_recommended: true,
            ..Default::default()
        };
        assert!(status.needs_indexing());
    }

    #[test]
    fn test_config_to_policy() {
        let config = IndexPolicyConfig {
            chunk_threshold: 20_000,
            fts_chunk_threshold: 2_000,
        };
        let policy = IndexPolicy::from(&config);
        assert_eq!(policy.chunk_threshold, 20_000);
        assert_eq!(policy.fts_chunk_threshold, 2_000);
        assert!(!policy.force_rebuild);
    }
}
