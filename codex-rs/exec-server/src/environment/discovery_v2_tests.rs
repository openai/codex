//! Version compatibility gates discovery RPCs; supported clients retain transport recovery.

use super::DiscoverV2CapabilitiesRequest;
use super::DiscoverV2CapabilitiesResponse;
use super::Environment;
use crate::CAPABILITIES_DISCOVER_V2_METHOD;
use crate::client_api::ExecServerTransportParams;
use codex_config::ScopedSkillsConfig;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_utils_path_uri::PathUri;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;
use tokio_util::task::AbortOnDropHandle;

#[test_case::test_case("0.164.0", true; "minimum_stable")]
#[test_case::test_case("0.168.0+build", true; "newer_stable")]
#[test_case::test_case("0.163.0", false; "older")]
#[test_case::test_case("0.164.0-alpha.1", false; "prerelease_before_cutoff")]
#[test_case::test_case("0.165.0-alpha.1", true; "next_minor_prerelease")]
#[test_case::test_case("", false; "missing")]
#[test_case::test_case("invalid", false; "malformed")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn discovery_checks_version_and_retries_disconnect(
    version: &'static str,
    supported: bool,
) -> anyhow::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let url = format!("ws://{}", listener.local_addr()?);
    let attempts = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&attempts);
    let _server = AbortOnDropHandle::new(tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let mut socket = accept_async(stream).await?;
            while let Some(message) = socket.next().await {
                let Message::Text(text) = message? else {
                    continue;
                };
                let request: serde_json::Value = serde_json::from_str(text.as_ref())?;
                if request.get("id").is_none() {
                    continue;
                }
                let mut info = json!({
                    "shell":{"name":"bash","path":"/bin/bash"},
                    "capabilities":{"capabilityDiscoveryV2":true}
                });
                if !version.is_empty() {
                    info["executorVersion"] = json!(version);
                }
                let result = match request["method"].as_str() {
                    Some("initialize") => {
                        // V2 support is advertised by the executor, not negotiated by the client.
                        assert!(request["params"].get("capabilityDiscoveryV2").is_none());
                        json!({"sessionId":"discovery-recovery", "environmentInfo":info})
                    }
                    Some("environment/info") => info,
                    Some(CAPABILITIES_DISCOVER_V2_METHOD) => {
                        let first = {
                            let mut attempts = recorded.lock().await;
                            attempts.push(request["params"].clone());
                            attempts.len() == 1
                        };
                        if first {
                            break;
                        }
                        serde_json::to_value(DiscoverV2CapabilitiesResponse::default())?
                    }
                    method => anyhow::bail!("unexpected executor method: {method:?}"),
                };
                socket
                    .send(Message::Text(
                        json!({"id":request["id"],"result":result})
                            .to_string()
                            .into(),
                    ))
                    .await?;
            }
        }
        anyhow::Ok(())
    }));
    let environment = Environment::remote_with_transport(
        ExecServerTransportParams::websocket_url(url, Duration::from_secs(5)),
        /*local_runtime_paths*/ None,
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        ScopedSkillsConfig::default(),
    );
    let params = DiscoverV2CapabilitiesRequest {
        cwd: PathUri::parse("file:///caller/project-b")?,
        sandbox: None,
    };
    let response = tokio::time::timeout(
        Duration::from_secs(12),
        environment.discover_capabilities_v2(params.clone()),
    )
    .await?;
    if !supported {
        assert!(
            matches!(&response, Err(crate::ExecServerError::Protocol(message))
                if message == "capabilities/discoverV2 is unavailable in this environment session"),
            "unexpected discovery response: {response:?}"
        );
        assert_eq!(*attempts.lock().await, Vec::<serde_json::Value>::new());
        return Ok(());
    }
    assert_eq!(response?, DiscoverV2CapabilitiesResponse::default());
    assert_eq!(
        *attempts.lock().await,
        vec![
            serde_json::to_value(&params)?,
            serde_json::to_value(&params)?
        ]
    );
    Ok(())
}
