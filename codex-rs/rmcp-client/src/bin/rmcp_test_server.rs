#[tokio::main]
async fn main() -> anyhow::Result<()> {
    codex_mcp_test_server::run_echo_stdio_server().await
}
