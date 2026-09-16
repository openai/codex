//! Verifies connector-scoped exposure, server restrictions, and MCP dispatch.

use codex_core::config::Config;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::McpServerContribution;
use codex_extension_api::McpServerContributionContext;
use codex_extension_api::McpServerContributor;
use codex_features::Feature;
use codex_protocol::config_types::ToolExposureSurface;
use codex_protocol::openai_models::ToolMode;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::apps_test_server::search_capable_apps_builder;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::Arc;
use wiremock::Mock;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_partial_json;
use wiremock::matchers::method;

struct AppsServer(Vec<ToolExposureSurface>);

impl McpServerContributor<Config> for AppsServer {
    fn id(&self) -> &'static str {
        "app_exposure_fixture"
    }

    fn contribute<'a>(
        &'a self,
        context: McpServerContributionContext<'a, Config>,
    ) -> ExtensionFuture<'a, Vec<McpServerContribution>> {
        Box::pin(async move {
            let mut config = codex_mcp::hosted_plugin_runtime_mcp_server_config(
                &context.config().chatgpt_base_url,
                /*apps_mcp_product_sku*/ None,
                context.originator(),
            );
            config.omit_tools_from = Some(self.0.clone());
            vec![McpServerContribution::HostedApps {
                config: Box::new(config),
                protocol_mode: None,
            }]
        })
    }
}

pub(super) struct ExposureCase {
    mode: ToolMode,
    app_config: &'static str,
    server_omissions: &'static [&'static str],
    direct_only: bool,
    expect_direct: bool,
    expect_exec: bool,
}

impl ExposureCase {
    pub(super) fn non_deferred(mode: ToolMode) -> Self {
        Self {
            mode,
            app_config: "omit_tools_from = [\"deferred\"]",
            server_omissions: &[],
            direct_only: false,
            expect_direct: mode != ToolMode::CodeModeOnly,
            expect_exec: mode != ToolMode::Direct,
        }
    }
}

#[test_case::test_case(ToolMode::Direct; "direct")]
#[test_case::test_case(ToolMode::CodeMode; "code_mode")]
#[test_case::test_case(ToolMode::CodeModeOnly; "code_mode_only")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn non_deferred_connector_exposure_and_dispatch(mode: ToolMode) -> anyhow::Result<()> {
    connector_exposure_requests(ExposureCase::non_deferred(mode)).await?;
    Ok(())
}

#[test_case::test_case("", &[], false, false; "unconfigured")]
#[test_case::test_case("omit_tools_from = [\"deferred\"]", &["code_mode"], true, false; "server_excludes_exec")]
#[test_case::test_case("omit_tools_from = [\"deferred\"]", &["direct"], false, true; "server_excludes_direct")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn connector_omissions_respect_server_restrictions(
    app_config: &'static str,
    server_omissions: &'static [&'static str],
    expect_direct: bool,
    expect_exec: bool,
) -> anyhow::Result<()> {
    connector_exposure_requests(ExposureCase {
        app_config,
        server_omissions,
        expect_direct,
        expect_exec,
        ..ExposureCase::non_deferred(ToolMode::CodeMode)
    })
    .await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn connector_omissions_preserve_direct_only_namespace() -> anyhow::Result<()> {
    connector_exposure_requests(ExposureCase {
        direct_only: true,
        expect_direct: true,
        expect_exec: false,
        ..ExposureCase::non_deferred(ToolMode::CodeModeOnly)
    })
    .await?;
    Ok(())
}

pub(super) async fn connector_exposure_requests(
    case: ExposureCase,
) -> anyhow::Result<Vec<responses::ResponsesRequest>> {
    let server = responses::start_mock_server().await;
    AppsTestServer::mount_searchable(&server).await?;
    for rpc_method in ["tools/list", "tools/call"] {
        Mock::given(method("POST"))
            .and(body_partial_json(json!({"method": rpc_method})))
            .respond_with(move |request: &wiremock::Request| {
                let body: serde_json::Value =
                    request.body_json().expect("valid MCP fixture request");
                let result = if rpc_method == "tools/list" {
                    json!({"tools": [
                        {
                            "name": "lookup",
                            "description": "Look up calendar events",
                            "inputSchema": {"type": "object", "properties": {}},
                            "annotations": {"readOnlyHint": true},
                            "_meta": {"connector_id": "calendar", "connector_name": "calendar"}
                        },
                        {
                            "name": "read",
                            "description": "Read notes",
                            "inputSchema": {"type": "object", "properties": {}},
                            "annotations": {"readOnlyHint": true},
                            "_meta": {"connector_id": "notes", "connector_name": "notes"}
                        }
                    ]})
                } else {
                    assert_eq!(body["params"]["name"], "lookup");
                    json!({"content": [{"type": "text", "text": "calendar-lookup-ok"}]})
                };
                ResponseTemplate::new(200).set_body_json(json!({
                    "jsonrpc": "2.0", "id": body["id"], "result": result
                }))
            })
            .with_priority(1)
            .mount(&server)
            .await;
    }
    let namespace = "mcp__codex_apps__calendar";
    let tool_call = if case.expect_exec {
        responses::ev_custom_tool_call(
            "lookup",
            "exec",
            &format!("text((await tools.{namespace}__lookup({{}})).content[0].text);"),
        )
    } else if case.expect_direct {
        responses::ev_function_call_with_namespace("lookup", namespace, "lookup", "{}")
    } else {
        responses::ev_assistant_message("message", "done")
    };
    let calls_tool = case.expect_direct || case.expect_exec;
    let mut events = vec![responses::sse(vec![
        tool_call,
        responses::ev_completed("first"),
    ])];
    if calls_tool {
        events.push(responses::sse(vec![responses::ev_completed("second")]));
    }
    let response_mock = responses::mount_sse_sequence(&server, events).await;
    let auth = codex_login::CodexAuth::create_dummy_chatgpt_auth_for_testing();
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.mcp_server_contributor(Arc::new(AppsServer(serde_json::from_value(json!(
        case.server_omissions
    ))?)));
    let mut builder = search_capable_apps_builder(server.uri())
        .with_code_mode_host_program(codex_utils_cargo_bin::cargo_bin("codex-code-mode-host")?)
        .with_pre_build_hook(move |home| {
            std::fs::write(
                home.join("config.toml"),
                format!("[apps.calendar]\n{}\n", case.app_config),
            )
            .expect("valid fixture configuration");
        })
        .with_auth(auth)
        .with_extensions(Arc::new(extensions.build()))
        .with_model_info_override("gpt-5.5", move |model| model.tool_mode = Some(case.mode))
        .with_config(move |config| {
            if case.direct_only {
                config.code_mode.direct_only_tool_namespaces = vec![namespace.to_string()];
            }
            config.analytics_enabled = Some(false);
            config
                .features
                .enable(Feature::CodeModeHost)
                .expect("code mode host");
        });
    let test = builder.build_with_auto_env(&server).await?;
    test.submit_turn("Look up my calendar.").await?;
    let requests = response_mock.requests();
    assert_eq!(requests.len(), if calls_tool { 2 } else { 1 });
    assert_eq!(
        requests[0].tool_by_name(namespace, "lookup").is_some(),
        case.expect_direct
    );
    assert!(
        requests[0]
            .tool_by_name("mcp__codex_apps__notes", "read")
            .is_none()
    );
    let body = requests[0].body_json();
    let exec_description = body["tools"]
        .as_array()
        .expect("request tool list")
        .iter()
        .find(|tool| tool["name"] == "exec")
        .and_then(|tool| tool["description"].as_str())
        .unwrap_or_default();
    assert_eq!(
        exec_description.contains(&format!("{namespace}__lookup(")),
        case.expect_exec && case.mode == ToolMode::CodeModeOnly
    );
    assert!(!exec_description.contains("mcp__codex_apps__notes__read("));
    if calls_tool {
        assert!(requests[1].body_contains_text("calendar-lookup-ok"));
    }
    test.codex.shutdown_and_wait().await?;
    Ok(requests)
}
