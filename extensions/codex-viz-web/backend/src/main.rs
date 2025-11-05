use axum::{
    routing::{get, post, delete},
    Router,
};
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod api;
mod git;
mod types;
mod websocket;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "codex_viz_backend=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("🚀 Codex Viz Backend starting...");

    // Create collaboration state
    let collab_state = api::collaboration::CollaborationState::new();

    // Build our application with routes
    let app = Router::new()
        // API routes
        .route("/api/commits", get(api::commits::list_commits))
        .route("/api/commits/stream", get(api::streaming::stream_commits))
        .route("/api/commits/paginated", get(api::streaming::paginated_commits))
        .route("/api/files/heatmap", get(api::files::get_heatmap))
        .route("/api/branches/graph", get(api::branches::get_graph))
        // Collaboration routes
        .route("/api/comments/:commit_sha", post(api::collaboration::add_comment))
        .route("/api/comments/:commit_sha", get(api::collaboration::get_comments))
        .route("/api/comments/:comment_id", delete(api::collaboration::delete_comment))
        .route("/api/views/share", post(api::collaboration::share_view))
        .route("/api/views/:view_id", get(api::collaboration::get_shared_view))
        // WebSocket route
        .route("/api/realtime", get(websocket::handler))
        // Health check
        .route("/health", get(health_check))
        // Add collaboration state
        .with_state(collab_state)
        // Add middleware
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http());

    // Run server
    let addr = SocketAddr::from(([127, 0, 0, 1], 3001));
    tracing::info!("🌐 Server listening on http://{}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_check() -> &'static str {
    "OK"
}

