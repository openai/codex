use crate::git::GitAnalyzer;
use crate::types::{ApiResponse, Commit3D};
use axum::{
    extract::Query,
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::Deserialize;
use std::env;

#[derive(Deserialize)]
pub struct CommitsQuery {
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default)]
    repo_path: Option<String>,
}

fn default_limit() -> usize {
    1000
}

/// GET /api/commits - List commits with 3D coordinates
pub async fn list_commits(Query(params): Query<CommitsQuery>) -> impl IntoResponse {
    let repo_path = params
        .repo_path
        .unwrap_or_else(|| env::current_dir().unwrap().to_string_lossy().to_string());

    match GitAnalyzer::open(&repo_path) {
        Ok(mut analyzer) => match analyzer.analyze_commits(Some(params.limit)) {
            Ok(commits) => {
                tracing::info!("📊 Analyzed {} commits from {}", commits.len(), repo_path);
                (
                    StatusCode::OK,
                    Json(ApiResponse::success(commits))
                )
            }
            Err(e) => {
                tracing::error!("Failed to analyze commits: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiResponse::<Vec<Commit3D>>::error(format!("Analysis error: {}", e)))
                )
            }
        },
        Err(e) => {
            tracing::error!("Failed to open repository: {}", e);
            (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<Vec<Commit3D>>::error(format!("Repository error: {}", e)))
            )
        }
    }
}

