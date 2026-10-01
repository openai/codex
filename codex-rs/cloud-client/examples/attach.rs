//! Standalone consumer; prints event kinds without logging payloads.
use codex_cloud_client::Client;
use codex_cloud_client::Credentials;
use codex_cloud_client::ResumeRequest;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use futures::StreamExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let endpoint = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("expected native gRPC origin"))?;
    let thread_id = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("expected existing thread ID"))?;
    let credentials = Credentials::new(
        &std::env::var("CODEX_CLOUD_ACCESS_TOKEN")?,
        &std::env::var("CODEX_CLOUD_ACCOUNT_ID")?,
    )?;
    let client = Client::new(
        &HttpClientFactory::new(OutboundProxyPolicy::RespectSystemProxy),
        &endpoint,
        credentials,
    )?;
    client
        .resume(&ResumeRequest {
            thread_id: thread_id.clone(),
            wait: false,
            shared_access: false,
        })
        .await?;
    let mut events = client.attach(&thread_id).await?;
    while let Some(event) = events.next().await {
        println!("{:?}", event?);
    }
    Ok(())
}
