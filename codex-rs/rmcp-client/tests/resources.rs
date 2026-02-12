use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

use codex_rmcp_client::ElicitationAction;
use codex_rmcp_client::ElicitationResponse;
use codex_rmcp_client::RmcpClient;
use codex_utils_cargo_bin::CargoBinError;
use futures::FutureExt as _;
use rmcp::model::AnnotateAble;
use rmcp::model::ClientCapabilities;
use rmcp::model::ElicitationCapability;
use rmcp::model::FormElicitationCapability;
use rmcp::model::Implementation;
use rmcp::model::InitializeRequestParams;
use rmcp::model::ListResourceTemplatesResult;
use rmcp::model::ProtocolVersion;
use rmcp::model::ReadResourceRequestParams;
use rmcp::model::ResourceContents;
use serde_json::json;

const RESOURCE_URI: &str = "memo://codex/example-note";

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
            title: Some("Codex rmcp resource test".into()),
            description: None,
            icons: None,
            website_url: None,
        },
        protocol_version: ProtocolVersion::V_2025_06_18,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn rmcp_client_can_list_and_read_resources() -> anyhow::Result<()> {
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
                    })
                }
                .boxed()
            }),
        )
        .await?;

    let list = client
        .list_resources(None, Some(Duration::from_secs(5)))
        .await?;
    let memo = list
        .resources
        .iter()
        .find(|resource| resource.uri == RESOURCE_URI)
        .expect("memo resource present");
    assert_eq!(
        memo,
        &rmcp::model::RawResource {
            uri: RESOURCE_URI.to_string(),
            name: "example-note".to_string(),
            title: Some("Example Note".to_string()),
            description: Some("A sample MCP resource exposed for integration tests.".to_string()),
            mime_type: Some("text/plain".to_string()),
            size: None,
            icons: None,
            meta: None,
        }
        .no_annotation()
    );
    let templates = client
        .list_resource_templates(None, Some(Duration::from_secs(5)))
        .await?;
    assert_eq!(
        templates,
        ListResourceTemplatesResult {
            meta: None,
            next_cursor: None,
            resource_templates: vec![
                rmcp::model::RawResourceTemplate {
                    uri_template: "memo://codex/{slug}".to_string(),
                    name: "codex-memo".to_string(),
                    title: Some("Codex Memo".to_string()),
                    description: Some(
                        "Template for memo://codex/{slug} resources used in tests.".to_string(),
                    ),
                    mime_type: Some("text/plain".to_string()),
                    icons: None,
                }
                .no_annotation()
            ],
        }
    );

    let read = client
        .read_resource(
            ReadResourceRequestParams {
                meta: None,
                uri: RESOURCE_URI.to_string(),
            },
            Some(Duration::from_secs(5)),
        )
        .await?;
    let text = read.contents.first().expect("resource contents present");
    assert_eq!(
        text,
        &ResourceContents::TextResourceContents {
            uri: RESOURCE_URI.to_string(),
            mime_type: Some("text/plain".to_string()),
            text: "This is a sample MCP resource served by the rmcp test server.".to_string(),
            meta: None,
        }
    );

    Ok(())
}
