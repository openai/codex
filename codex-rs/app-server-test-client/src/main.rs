use anyhow::Result;
use tokio::runtime::Builder;

fn main() -> Result<()> {
    let runtime = Builder::new_current_thread().enable_all().build()?;
    runtime.block_on(codex_app_server_test_client::run())
}
