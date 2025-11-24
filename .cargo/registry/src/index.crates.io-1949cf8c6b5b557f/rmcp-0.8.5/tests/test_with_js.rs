use rmcp::{
    ServiceExt,
    service::QuitReason,
    transport::{
        ConfigureCommandExt, SseServer, StreamableHttpClientTransport, StreamableHttpServerConfig,
        TokioChildProcess,
        streamable_http_server::{
            session::local::LocalSessionManager, tower::StreamableHttpService,
        },
    },
};
use tokio_util::sync::CancellationToken;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
mod common;
use common::calculator::Calculator;

const SSE_BIND_ADDRESS: &str = "127.0.0.1:8000";
const STREAMABLE_HTTP_BIND_ADDRESS: &str = "127.0.0.1:8001";
const STREAMABLE_HTTP_JS_BIND_ADDRESS: &str = "127.0.0.1:8002";

#[tokio::test]
async fn test_with_js_client() -> anyhow::Result<()> {
    let _ = tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "debug".to_string().into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .try_init();
    tokio::process::Command::new("npm")
        .arg("install")
        .current_dir("tests/test_with_js")
        .spawn()?
        .wait()
        .await?;

    let ct = SseServer::serve(SSE_BIND_ADDRESS.parse()?)
        .await?
        .with_service(Calculator::default);

    let exit_status = tokio::process::Command::new("node")
        .arg("tests/test_with_js/client.js")
        .spawn()?
        .wait()
        .await?;
    assert!(exit_status.success());
    ct.cancel();
    Ok(())
}

#[tokio::test]
async fn test_with_js_server() -> anyhow::Result<()> {
    let _ = tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "debug".to_string().into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .try_init();
    tokio::process::Command::new("npm")
        .arg("install")
        .current_dir("tests/test_with_js")
        .spawn()?
        .wait()
        .await?;
    let transport =
        TokioChildProcess::new(tokio::process::Command::new("node").configure(|cmd| {
            cmd.arg("tests/test_with_js/server.js");
        }))?;

    let client = ().serve(transport).await?;
    let resources = client.list_all_resources().await?;
    tracing::info!("{:#?}", resources);
    let tools = client.list_all_tools().await?;
    tracing::info!("{:#?}", tools);

    client.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn test_with_js_streamable_http_client() -> anyhow::Result<()> {
    let _ = tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "debug".to_string().into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .try_init();
    tokio::process::Command::new("npm")
        .arg("install")
        .current_dir("tests/test_with_js")
        .spawn()?
        .wait()
        .await?;

    let service: StreamableHttpService<Calculator, LocalSessionManager> =
        StreamableHttpService::new(
            || Ok(Calculator::new()),
            Default::default(),
            StreamableHttpServerConfig {
                stateful_mode: true,
                sse_keep_alive: None,
            },
        );
    let router = axum::Router::new().nest_service("/mcp", service);
    let tcp_listener = tokio::net::TcpListener::bind(STREAMABLE_HTTP_BIND_ADDRESS).await?;
    let ct = CancellationToken::new();
    let handle = tokio::spawn({
        let ct = ct.clone();
        async move {
            let _ = axum::serve(tcp_listener, router)
                .with_graceful_shutdown(async move { ct.cancelled_owned().await })
                .await;
        }
    });
    let exit_status = tokio::process::Command::new("node")
        .arg("tests/test_with_js/streamable_client.js")
        .spawn()?
        .wait()
        .await?;
    assert!(exit_status.success());
    ct.cancel();
    handle.await?;
    Ok(())
}

#[tokio::test]
async fn test_with_js_streamable_http_server() -> anyhow::Result<()> {
    let _ = tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "debug".to_string().into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .try_init();
    tokio::process::Command::new("npm")
        .arg("install")
        .current_dir("tests/test_with_js")
        .spawn()?
        .wait()
        .await?;

    let transport = StreamableHttpClientTransport::from_uri(format!(
        "http://{STREAMABLE_HTTP_JS_BIND_ADDRESS}/mcp"
    ));

    let mut server = tokio::process::Command::new("node")
        .arg("tests/test_with_js/streamable_server.js")
        .spawn()?;

    // waiting for server up
    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

    let client = ().serve(transport).await?;
    let resources = client.list_all_resources().await?;
    tracing::info!("{:#?}", resources);
    let tools = client.list_all_tools().await?;
    tracing::info!("{:#?}", tools);
    let quit_reason = client.cancel().await?;
    server.kill().await?;
    assert!(matches!(quit_reason, QuitReason::Cancelled));
    Ok(())
}
