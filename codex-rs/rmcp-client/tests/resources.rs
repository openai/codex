use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

use codex_rmcp_client::ElicitationAction;
use codex_rmcp_client::ElicitationResponse;
use codex_rmcp_client::RmcpClient;
use codex_utils_cargo_bin::CargoBinError;
use futures::FutureExt as _;
use opentelemetry::global;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::SdkTracerProvider;
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
use tracing::Instrument;
use tracing::dispatcher::DefaultGuard;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

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

struct TestTracingContext {
    _provider: SdkTracerProvider,
    _guard: DefaultGuard,
}

fn install_test_tracing(tracer_name: &str) -> TestTracingContext {
    global::set_text_map_propagator(TraceContextPropagator::new());

    let provider = SdkTracerProvider::builder().build();
    let tracer = provider.tracer(tracer_name.to_string());
    let subscriber =
        tracing_subscriber::registry().with(tracing_opentelemetry::layer().with_tracer(tracer));

    TestTracingContext {
        _provider: provider,
        _guard: subscriber.set_default(),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn rmcp_client_can_list_and_read_resources() -> anyhow::Result<()> {
    let client = RmcpClient::new_stdio_client(
        stdio_server_bin()?.into(),
        Vec::<OsString>::new(),
        /*env*/ None,
        &[],
        /*cwd*/ None,
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

    let list = client
        .list_resources(/*params*/ None, Some(Duration::from_secs(5)))
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
        .list_resource_templates(/*params*/ None, Some(Duration::from_secs(5)))
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

#[tokio::test(flavor = "current_thread")]
async fn stdio_tool_call_propagates_trace_metadata() -> anyhow::Result<()> {
    let _trace = install_test_tracing("rmcp-stdio-trace-test");
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

    let result = async {
        client
            .call_tool(
                "echo".to_string(),
                Some(json!({ "message": "ping" })),
                None,
                Some(Duration::from_secs(5)),
            )
            .await
    }
    .instrument(tracing::info_span!("rmcp.client.trace_test"))
    .await?;

    assert_eq!(result.is_error, Some(false));
    let structured = result.structured_content.expect("structured content");
    assert_eq!(structured["echo"], json!("ECHOING: ping"));
    assert_eq!(structured["env"], serde_json::Value::Null);
    assert!(
        structured["tracestate"].is_null()
            || structured["tracestate"].as_str().is_some_and(str::is_empty)
    );
    let traceparent = structured["traceparent"]
        .as_str()
        .expect("traceparent should be propagated via request metadata");
    assert!(traceparent.starts_with("00-"));

    Ok(())
}
