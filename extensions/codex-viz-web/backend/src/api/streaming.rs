use crate::git::GitAnalyzer;
use crate::types::{ApiResponse, Commit3D};
use axum::{
    extract::Query,
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive},
        IntoResponse, Sse,
    },
};
use futures::stream::{self, Stream};
use serde::Deserialize;
use std::convert::Infallible;
use std::env;

#[derive(Deserialize)]
pub struct StreamingQuery {
    #[serde(default = "default_chunk_size")]
    chunk_size: usize,
    #[serde(default)]
    repo_path: Option<String>,
}

fn default_chunk_size() -> usize {
    100
}

/// GET /api/commits/stream - Stream commits in chunks via Server-Sent Events
pub async fn stream_commits(
    Query(params): Query<StreamingQuery>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let repo_path = params
        .repo_path
        .unwrap_or_else(|| env::current_dir().unwrap().to_string_lossy().to_string());

    let mut analyzer = GitAnalyzer::open(&repo_path)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Repository error: {}", e)))?;

    let commits = analyzer
        .analyze_commits(None)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Analysis error: {}", e)))?;

    tracing::info!(
        "📡 Streaming {} commits from {} in chunks of {}",
        commits.len(),
        repo_path,
        params.chunk_size
    );

    // Create SSE stream
    let stream = create_commit_stream(commits, params.chunk_size);

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

/// Create a stream that emits commits in chunks
fn create_commit_stream(
    commits: Vec<Commit3D>,
    chunk_size: usize,
) -> impl Stream<Item = Result<Event, Infallible>> {
    let total = commits.len();
    let chunks: Vec<Vec<Commit3D>> = commits
        .chunks(chunk_size)
        .map(|chunk| chunk.to_vec())
        .collect();

    stream::iter(chunks.into_iter().enumerate().map(move |(i, chunk)| {
        let progress = ((i + 1) * chunk_size).min(total);
        let percent = (progress as f32 / total as f32 * 100.0) as u32;

        let data = serde_json::json!({
            "chunk": chunk,
            "progress": {
                "current": progress,
                "total": total,
                "percent": percent,
            }
        });

        Ok(Event::default()
            .json_data(data)
            .expect("Failed to serialize"))
    }))
}

/// GET /api/commits/paginated - Get commits with pagination
#[derive(Deserialize)]
pub struct PaginationQuery {
    #[serde(default)]
    page: usize,
    #[serde(default = "default_page_size")]
    limit: usize,
    #[serde(default)]
    repo_path: Option<String>,
}

fn default_page_size() -> usize {
    100
}

pub async fn paginated_commits(
    Query(params): Query<PaginationQuery>,
) -> impl IntoResponse {
    let repo_path = params
        .repo_path
        .unwrap_or_else(|| env::current_dir().unwrap().to_string_lossy().to_string());

    match GitAnalyzer::open(&repo_path) {
        Ok(mut analyzer) => match analyzer.analyze_commits(None) {
            Ok(all_commits) => {
                let total = all_commits.len();
                let start = params.page * params.limit;
                let end = (start + params.limit).min(total);

                if start >= total {
                    return (
                        StatusCode::OK,
                        axum::Json(ApiResponse::success(Vec::<Commit3D>::new())),
                    );
                }

                let page_commits = all_commits[start..end].to_vec();

                tracing::info!(
                    "📄 Serving page {} ({}-{} of {}) from {}",
                    params.page,
                    start,
                    end,
                    total,
                    repo_path
                );

                (StatusCode::OK, axum::Json(ApiResponse::success(page_commits)))
            }
            Err(e) => {
                tracing::error!("Failed to analyze commits: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    axum::Json(ApiResponse::<Vec<Commit3D>>::error(format!(
                        "Analysis error: {}",
                        e
                    ))),
                )
            }
        },
        Err(e) => {
            tracing::error!("Failed to open repository: {}", e);
            (
                StatusCode::BAD_REQUEST,
                axum::Json(ApiResponse::<Vec<Commit3D>>::error(format!(
                    "Repository error: {}",
                    e
                ))),
            )
        }
    }
}

