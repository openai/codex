#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    codex_code_mode_host::run_stdio().await
}
