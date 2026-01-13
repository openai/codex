use anyhow::Result;
use rmcp::ServiceExt;
use tokio::task;

use crate::resource_server::ResourceTestToolServer;

pub async fn run_stdio_server() -> Result<()> {
    eprintln!("starting rmcp test server");
    let service = ResourceTestToolServer::new(true);
    let running = service.serve(crate::stdio()).await?;

    running.waiting().await?;
    task::yield_now().await;
    Ok(())
}
