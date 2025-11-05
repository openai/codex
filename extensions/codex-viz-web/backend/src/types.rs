use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// 3D coordinates for commit visualization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit3D {
    pub sha: String,
    pub message: String,
    pub author: String,
    pub author_email: String,
    pub timestamp: DateTime<Utc>,
    pub branch: String,
    pub parents: Vec<String>,
    
    // 3D coordinates
    pub x: f32,  // Branch axis
    pub y: f32,  // Time axis
    pub z: f32,  // Depth axis (parent-child relationship)
    
    // Color for author differentiation
    pub color: String,
}

/// File change statistics for heatmap
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStats {
    pub path: String,
    pub change_count: u32,
    pub additions: u32,
    pub deletions: u32,
    pub last_modified: DateTime<Utc>,
    pub authors: Vec<String>,
    
    // Heatmap visualization data
    pub heat_level: f32,  // 0.0 to 1.0
    pub size: u64,        // File size in bytes
}

/// Branch graph node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchNode {
    pub name: String,
    pub head_sha: String,
    pub is_active: bool,
    pub merge_count: u32,
    pub created_at: DateTime<Utc>,
    pub last_commit: DateTime<Utc>,
    
    // Graph visualization data
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub connections: Vec<BranchConnection>,
}

/// Connection between branches (merge points)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchConnection {
    pub target_branch: String,
    pub merge_sha: String,
    pub connection_type: ConnectionType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConnectionType {
    Merge,
    Fork,
    Rebase,
}

/// Real-time event for WebSocket
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RealtimeEvent {
    NewCommit {
        commit: Commit3D,
    },
    FileChanged {
        path: String,
        change_type: ChangeType,
    },
    BranchCreated {
        branch: BranchNode,
    },
    BranchDeleted {
        branch_name: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeType {
    Added,
    Modified,
    Deleted,
}

/// API response wrapper
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message.into()),
        }
    }
}

