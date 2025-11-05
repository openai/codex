use crate::git::GitAnalyzer;
use crate::types::{ApiResponse, BranchNode};
use axum::{
    extract::Query,
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::Deserialize;
use std::env;

#[derive(Deserialize)]
pub struct BranchQuery {
    #[serde(default)]
    repo_path: Option<String>,
}

/// GET /api/branches/graph - Get branch structure graph
pub async fn get_graph(Query(params): Query<BranchQuery>) -> impl IntoResponse {
    let repo_path = params
        .repo_path
        .unwrap_or_else(|| env::current_dir().unwrap().to_string_lossy().to_string());

    match GitAnalyzer::open(&repo_path) {
        Ok(mut analyzer) => match analyzer.analyze_branches() {
            Ok(branches) => {
                tracing::info!("🌿 Analyzed {} branches from {}", branches.len(), repo_path);
                (
                    StatusCode::OK,
                    Json(ApiResponse::success(branches))
                )
            }
            Err(e) => {
                tracing::error!("Failed to analyze branches: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiResponse::<Vec<BranchNode>>::error(format!("Analysis error: {}", e)))
                )
            }
        },
        Err(e) => {
            tracing::error!("Failed to open repository: {}", e);
            (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<Vec<BranchNode>>::error(format!("Repository error: {}", e)))
            )
        }
    }
}

