use codex_mcp_server::run_main;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    run_main().await?;
    Ok(())
}
