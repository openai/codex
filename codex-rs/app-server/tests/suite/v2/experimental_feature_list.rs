use std::time::Duration;

use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::TestAppServer;
use app_test_support::create_mock_responses_server_repeating_assistant;
use app_test_support::to_response;
use app_test_support::write_chatgpt_auth;
use codex_app_server_protocol::ConfigReadParams;
use codex_app_server_protocol::ConfigReadResponse;
use codex_app_server_protocol::ExperimentalFeature;
use codex_app_server_protocol::ExperimentalFeatureEnablementSetParams;
use codex_app_server_protocol::ExperimentalFeatureEnablementSetResponse;
use codex_app_server_protocol::ExperimentalFeatureListParams;
use codex_app_server_protocol::ExperimentalFeatureListResponse;
use codex_app_server_protocol::ExperimentalFeatureStage;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_config::LoaderOverrides;
use codex_config::types::AuthCredentialsStoreMode;
use codex_core::config::ConfigBuilder;
use codex_features::FEATURES;
use codex_features::Stage;
use pretty_assertions::assert_eq;
use serde::de::DeserializeOwned;
use serde_json::json;
use std::collections::BTreeMap;
use tempfile::TempDir;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

#[tokio::test]
async fn experimental_feature_list_returns_feature_metadata_with_stage() -> Result<()> {
    let codex_home = TempDir::new()?;
    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .loader_overrides(LoaderOverrides::with_managed_config_path_for_tests(
            codex_home.path().join("managed_config.toml"),
        ))
        .build()
        .await?;
    let mut mcp = TestAppServer::new(codex_home.path()).await?;

    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_experimental_feature_list_request(ExperimentalFeatureListParams::default())
        .await?;

    let actual = read_response::<ExperimentalFeatureListResponse>(&mut mcp, request_id).await?;
    let expected_data = FEATURES
        .iter()
        .map(|spec| {
            let (stage, display_name, description, announcement) = match spec.stage {
                Stage::Experimental {
                    name,
                    menu_description,
                    announcement,
                } => (
                    ExperimentalFeatureStage::Beta,
                    Some(name.to_string()),
                    Some(menu_description.to_string()),
                    Some(announcement.to_string()),
                ),
                Stage::UnderDevelopment => {
                    (ExperimentalFeatureStage::UnderDevelopment, None, None, None)
                }
                Stage::Stable => (ExperimentalFeatureStage::Stable, None, None, None),
                Stage::Deprecated => (ExperimentalFeatureStage::Deprecated, None, None, None),
                Stage::Removed => (ExperimentalFeatureStage::Removed, None, None, None),
            };

            ExperimentalFeature {
                name: spec.key.to_string(),
                stage,
                display_name,
                description,
                announcement,
                enabled: config.features.enabled(spec.id),
                default_enabled: spec.default_enabled,
            }
        })
        .collect::<Vec<_>>();
    let expected = ExperimentalFeatureListResponse {
        data: expected_data,
        next_cursor: None,
    };

    assert_eq!(actual, expected);
    Ok(())
}

#[tokio::test]
async fn experimental_feature_list_resolves_thread_project_config() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    let workspace = TempDir::new()?;
    let server_uri = server.uri();
    let workspace_key = workspace.path().to_string_lossy().replace('\\', "\\\\");
    std::fs::write(
        codex_home.path().join("config.toml"),
        format!(
            r#"model = "mock-model"
approval_policy = "never"
sandbox_mode = "read-only"
model_provider = "mock_provider"

[projects."{workspace_key}"]
trust_level = "trusted"

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{server_uri}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
"#
        ),
    )?;
    let project_config_dir = workspace.path().join(".codex");
    std::fs::create_dir_all(&project_config_dir)?;
    std::fs::write(
        project_config_dir.join("config.toml"),
        r#"[features]
memories = true
"#,
    )?;

    let mut mcp = TestAppServer::new_without_managed_config(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let thread_start_id = mcp
        .send_thread_start_request(ThreadStartParams {
            cwd: Some(workspace.path().display().to_string()),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } =
        read_response::<ThreadStartResponse>(&mut mcp, thread_start_id).await?;

    let request_id = mcp
        .send_experimental_feature_list_request(ExperimentalFeatureListParams {
            cursor: None,
            limit: None,
            thread_id: Some(thread.id),
        })
        .await?;

    let actual = read_response::<ExperimentalFeatureListResponse>(&mut mcp, request_id).await?;
    let memories = actual
        .data
        .iter()
        .find(|feature| feature.name == "memories")
        .expect("memories feature should be present");
    assert!(memories.enabled);

    Ok(())
}

#[tokio::test]
async fn experimental_feature_list_rejects_unknown_thread_id() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_experimental_feature_list_request(ExperimentalFeatureListParams {
            cursor: None,
            limit: None,
            thread_id: Some("00000000-0000-4000-8000-000000000001".to_string()),
        })
        .await?;
    let JSONRPCError { error, .. } = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(error.code, -32600);
    assert!(
        error
            .message
            .contains("thread not found: 00000000-0000-4000-8000-000000000001"),
        "{}",
        error.message
    );

    Ok(())
}

#[tokio::test]
async fn experimental_feature_enablement_set_applies_to_global_and_thread_config_reads()
-> Result<()> {
    let codex_home = TempDir::new()?;
    let project_cwd = codex_home.path().join("project");
    std::fs::create_dir_all(&project_cwd)?;

    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let actual = set_experimental_feature_enablement(
        &mut mcp,
        BTreeMap::from([("auth_elicitation".to_string(), true)]),
    )
    .await?;
    assert_eq!(
        actual,
        ExperimentalFeatureEnablementSetResponse {
            enablement: BTreeMap::from([("auth_elicitation".to_string(), true)]),
        }
    );

    for cwd in [None, Some(project_cwd.display().to_string())] {
        let ConfigReadResponse { config, .. } = read_config(&mut mcp, cwd).await?;

        assert_eq!(
            config
                .additional
                .get("features")
                .and_then(|features| features.get("auth_elicitation")),
            Some(&json!(true))
        );
    }

    Ok(())
}

#[tokio::test]
async fn experimental_feature_enablement_set_does_not_override_user_config() -> Result<()> {
    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        "[features]\nmemories = false\n",
    )?;
    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let actual = set_experimental_feature_enablement(
        &mut mcp,
        BTreeMap::from([("memories".to_string(), true)]),
    )
    .await?;
    assert_eq!(
        actual,
        ExperimentalFeatureEnablementSetResponse {
            enablement: BTreeMap::from([("memories".to_string(), true)]),
        }
    );

    let ConfigReadResponse { config, .. } = read_config(&mut mcp, /*cwd*/ None).await?;

    assert_eq!(
        config
            .additional
            .get("features")
            .and_then(|features| features.get("memories")),
        Some(&json!(false))
    );

    Ok(())
}

#[tokio::test]
async fn experimental_feature_enablement_set_only_updates_named_features() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    set_experimental_feature_enablement(
        &mut mcp,
        BTreeMap::from([("mentions_v2".to_string(), true)]),
    )
    .await?;
    let actual = set_experimental_feature_enablement(
        &mut mcp,
        BTreeMap::from([
            ("auth_elicitation".to_string(), true),
            ("memories".to_string(), true),
            ("remote_plugin".to_string(), true),
            ("tool_suggest".to_string(), false),
        ]),
    )
    .await?;

    assert_eq!(
        actual,
        ExperimentalFeatureEnablementSetResponse {
            enablement: BTreeMap::from([
                ("auth_elicitation".to_string(), true),
                ("memories".to_string(), true),
                ("remote_plugin".to_string(), true),
                ("tool_suggest".to_string(), false),
            ]),
        }
    );

    let ConfigReadResponse { config, .. } = read_config(&mut mcp, /*cwd*/ None).await?;

    assert_eq!(
        config
            .additional
            .get("features")
            .and_then(|features| features.get("mentions_v2")),
        Some(&json!(true))
    );
    assert_eq!(
        config
            .additional
            .get("features")
            .and_then(|features| features.get("auth_elicitation")),
        Some(&json!(true))
    );
    assert_eq!(
        config
            .additional
            .get("features")
            .and_then(|features| features.get("memories")),
        Some(&json!(true))
    );
    assert_eq!(
        config
            .additional
            .get("features")
            .and_then(|features| features.get("remote_plugin")),
        Some(&json!(true))
    );
    assert_eq!(
        config
            .additional
            .get("features")
            .and_then(|features| features.get("tool_suggest")),
        Some(&json!(false))
    );

    Ok(())
}

#[tokio::test]
async fn experimental_feature_enablement_set_allows_remote_control() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;
    let remote_control_enabled = false;
    let enablement = BTreeMap::from([("remote_control".to_string(), remote_control_enabled)]);

    let actual = set_experimental_feature_enablement(&mut mcp, enablement.clone()).await?;

    assert_eq!(
        actual,
        ExperimentalFeatureEnablementSetResponse { enablement }
    );

    Ok(())
}

#[tokio::test]
async fn experimental_feature_enablement_set_empty_map_is_no_op() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    set_experimental_feature_enablement(
        &mut mcp,
        BTreeMap::from([("mentions_v2".to_string(), true)]),
    )
    .await?;
    let actual = set_experimental_feature_enablement(&mut mcp, BTreeMap::new()).await?;

    assert_eq!(
        actual,
        ExperimentalFeatureEnablementSetResponse {
            enablement: BTreeMap::new(),
        }
    );

    let ConfigReadResponse { config, .. } = read_config(&mut mcp, /*cwd*/ None).await?;

    assert_eq!(
        config
            .additional
            .get("features")
            .and_then(|features| features.get("mentions_v2")),
        Some(&json!(true))
    );

    Ok(())
}

#[tokio::test]
async fn experimental_feature_enablement_set_ignores_invalid_features() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let actual = set_experimental_feature_enablement(
        &mut mcp,
        BTreeMap::from([
            ("apps".to_string(), false),
            ("auth_elicitation".to_string(), true),
            ("connectors".to_string(), false),
            ("personality".to_string(), false),
            ("plugins".to_string(), false),
            ("tool_call_mcp_elicitation".to_string(), false),
            ("unknown_feature".to_string(), true),
        ]),
    )
    .await?;

    assert_eq!(
        actual,
        ExperimentalFeatureEnablementSetResponse {
            enablement: BTreeMap::from([("auth_elicitation".to_string(), true)]),
        }
    );

    Ok(())
}

async fn set_experimental_feature_enablement(
    mcp: &mut TestAppServer,
    enablement: BTreeMap<String, bool>,
) -> Result<ExperimentalFeatureEnablementSetResponse> {
    let request_id = mcp
        .send_experimental_feature_enablement_set_request(ExperimentalFeatureEnablementSetParams {
            enablement,
        })
        .await?;
    read_response(mcp, request_id).await
}

async fn read_config(mcp: &mut TestAppServer, cwd: Option<String>) -> Result<ConfigReadResponse> {
    let request_id = mcp
        .send_config_read_request(ConfigReadParams {
            include_layers: false,
            cwd,
        })
        .await?;
    read_response(mcp, request_id).await
}

async fn read_response<T: DeserializeOwned>(mcp: &mut TestAppServer, request_id: i64) -> Result<T> {
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    to_response(response)
}
