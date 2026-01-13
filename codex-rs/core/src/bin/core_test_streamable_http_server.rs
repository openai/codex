#[tokio::main]
async fn main() -> anyhow::Result<()> {
    codex_mcp_test_server::run_streamable_http_server().await
}
