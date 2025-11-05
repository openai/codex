use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use chrono::{DateTime, Utc};

/// Comment on a specific commit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    pub id: String,
    pub commit_sha: String,
    pub author: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Shared view state for collaboration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedView {
    pub id: String,
    pub created_by: String,
    pub repo_path: String,
    pub view_mode: String,
    pub filters: ViewFilters,
    pub camera_position: [f32; 3],
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewFilters {
    pub authors: Vec<String>,
    pub branches: Vec<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
}

/// In-memory storage (should be replaced with database in production)
#[derive(Clone)]
pub struct CollaborationState {
    pub comments: Arc<RwLock<HashMap<String, Vec<Comment>>>>,
    pub shared_views: Arc<RwLock<HashMap<String, SharedView>>>,
}

impl CollaborationState {
    pub fn new() -> Self {
        Self {
            comments: Arc::new(RwLock::new(HashMap::new())),
            shared_views: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

// API Handlers

/// POST /api/comments/:commit_sha - Add comment to commit
#[derive(Deserialize)]
pub struct AddCommentRequest {
    author: String,
    content: String,
}

pub async fn add_comment(
    State(state): State<CollaborationState>,
    Path(commit_sha): Path<String>,
    Json(payload): Json<AddCommentRequest>,
) -> impl IntoResponse {
    let comment = Comment {
        id: uuid::Uuid::new_v4().to_string(),
        commit_sha: commit_sha.clone(),
        author: payload.author,
        content: payload.content,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    let mut comments = state.comments.write().unwrap();
    comments
        .entry(commit_sha)
        .or_insert_with(Vec::new)
        .push(comment.clone());

    tracing::info!("💬 Comment added: {}", comment.id);

    (StatusCode::CREATED, Json(comment))
}

/// GET /api/comments/:commit_sha - Get comments for commit
pub async fn get_comments(
    State(state): State<CollaborationState>,
    Path(commit_sha): Path<String>,
) -> impl IntoResponse {
    let comments = state.comments.read().unwrap();
    let commit_comments = comments.get(&commit_sha).cloned().unwrap_or_default();

    (StatusCode::OK, Json(commit_comments))
}

/// DELETE /api/comments/:comment_id - Delete comment
pub async fn delete_comment(
    State(state): State<CollaborationState>,
    Path(comment_id): Path<String>,
) -> impl IntoResponse {
    let mut comments = state.comments.write().unwrap();
    
    for commit_comments in comments.values_mut() {
        commit_comments.retain(|c| c.id != comment_id);
    }

    tracing::info!("🗑️ Comment deleted: {}", comment_id);

    StatusCode::NO_CONTENT
}

/// POST /api/views/share - Create shared view
#[derive(Deserialize)]
pub struct ShareViewRequest {
    created_by: String,
    repo_path: String,
    view_mode: String,
    filters: ViewFilters,
    camera_position: [f32; 3],
}

pub async fn share_view(
    State(state): State<CollaborationState>,
    Json(payload): Json<ShareViewRequest>,
) -> impl IntoResponse {
    let view_id = generate_short_id();
    
    let shared_view = SharedView {
        id: view_id.clone(),
        created_by: payload.created_by,
        repo_path: payload.repo_path,
        view_mode: payload.view_mode,
        filters: payload.filters,
        camera_position: payload.camera_position,
        created_at: Utc::now(),
    };

    let mut views = state.shared_views.write().unwrap();
    views.insert(view_id.clone(), shared_view.clone());

    tracing::info!("🔗 Shared view created: {}", view_id);

    (StatusCode::CREATED, Json(shared_view))
}

/// GET /api/views/:view_id - Get shared view
pub async fn get_shared_view(
    State(state): State<CollaborationState>,
    Path(view_id): Path<String>,
) -> Result<Json<SharedView>, (StatusCode, Json<serde_json::Value>)> {
    let views = state.shared_views.read().unwrap();
    
    if let Some(view) = views.get(&view_id) {
        Ok(Json(view.clone()))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "View not found" })),
        ))
    }
}

/// Generate short ID for shareable links
fn generate_short_id() -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    Utc::now().timestamp_nanos_opt().unwrap_or(0).hash(&mut hasher);
    
    let hash = hasher.finish();
    format!("{:x}", hash)[..8].to_string()
}

