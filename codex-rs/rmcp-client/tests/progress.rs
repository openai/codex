use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use codex_rmcp_client::ElicitationAction;
use codex_rmcp_client::ElicitationResponse;
use codex_rmcp_client::RmcpClient;
use codex_rmcp_client::SendProgressNotification;
use codex_utils_cargo_bin::CargoBinError;
use futures::FutureExt as _;
use pretty_assertions::assert_eq;
use rmcp::model::ClientCapabilities;
use rmcp::model::ElicitationCapability;
use rmcp::model::FormElicitationCapability;
use rmcp::model::Implementation;
use rmcp::model::InitializeRequestParams;
use rmcp::model::ProtocolVersion;
use serde_json::json;
use tokio::sync::Mutex;

fn stdio_server_bin() -> Result<PathBuf, CargoBinError> {
    codex_utils_cargo_bin::cargo_bin("test_stdio_server")
}

fn init_params() -> InitializeRequestParams {
    InitializeRequestParams {
        meta: None,
        capabilities: ClientCapabilities {
            experimental: None,
            extensions: None,
            roots: None,
            sampling: None,
            elicitation: Some(ElicitationCapability {
                form: Some(FormElicitationCapability {
                    schema_validation: None,
                }),
                url: None,
            }),
            tasks: None,
        },
        client_info: Implementation {
            name: "codex-test".into(),
            version: "0.0.0-test".into(),
            title: Some("Codex rmcp progress test".into()),
            description: None,
            icons: None,
            website_url: None,
        },
        protocol_version: ProtocolVersion::V_2025_06_18,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn rmcp_client_forwards_progress_notifications() -> anyhow::Result<()> {
    let client = RmcpClient::new_stdio_client(
        stdio_server_bin()?.into(),
        Vec::<OsString>::new(),
        None,
        &[],
        None,
    )
    .await?;

    client
        .initialize(
            init_params(),
            Some(Duration::from_secs(5)),
            Box::new(|_, _| {
                async {
                    Ok(ElicitationResponse {
                        action: ElicitationAction::Accept,
                        content: Some(json!({})),
                        meta: None,
                    })
                }
                .boxed()
            }),
        )
        .await?;

    let received_messages = Arc::new(Mutex::new(Vec::new()));
    let progress_notification: SendProgressNotification = Arc::new({
        let received_messages = Arc::clone(&received_messages);
        move |notification| {
            let received_messages = Arc::clone(&received_messages);
            async move {
                received_messages.lock().await.push(notification.message);
            }
            .boxed()
        }
    });

    let result = client
        .call_tool(
            "progress".to_string(),
            Some(json!({ "steps": 3 })),
            None,
            Some(Duration::from_secs(5)),
            Some(progress_notification),
        )
        .await?;

    assert_eq!(result.structured_content, Some(json!({ "steps": 3 })));
    assert_eq!(
        *received_messages.lock().await,
        vec![
            Some("step 1".to_string()),
            Some("step 2".to_string()),
            Some("step 3".to_string()),
        ]
    );

    Ok(())
}
