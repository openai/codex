use crate::git::GitAnalyzer;
use crate::types::{ApiResponse, FileStats};
use axum::{
    extract::Query,
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::Deserialize;
use std::env;

#[derive(Deserialize)]
pub struct HeatmapQuery {
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default)]
    repo_path: Option<String>,
}

fn default_limit() -> usize {
    1000
}

/// GET /api/files/heatmap - Get file change statistics
pub async fn get_heatmap(Query(params): Query<HeatmapQuery>) -> impl IntoResponse {
    let repo_path = params
        .repo_path
        .unwrap_or_else(|| env::current_dir().unwrap().to_string_lossy().to_string());

    match GitAnalyzer::open(&repo_path) {
        Ok(analyzer) => match analyzer.analyze_file_stats(Some(params.limit)) {
            Ok(stats) => {
                tracing::info!("📁 Analyzed {} files from {}", stats.len(), repo_path);
                (
                    StatusCode::OK,
                    Json(ApiResponse::success(stats))
                )
            }
            Err(e) => {
                tracing::error!("Failed to analyze file stats: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiResponse::<Vec<FileStats>>::error(format!("Analysis error: {}", e)))
                )
            }
        },
        Err(e) => {
            tracing::error!("Failed to open repository: {}", e);
            (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<Vec<FileStats>>::error(format!("Repository error: {}", e)))
            )
        }
    }
}

