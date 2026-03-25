use std::time::Duration;

use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::to_response;
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
use codex_core::config::ConfigBuilder;
use codex_features::FEATURES;
use codex_features::Stage;
use pretty_assertions::assert_eq;
use serde::de::DeserializeOwned;
use serde_json::json;
use std::collections::BTreeMap;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test]
async fn experimental_feature_list_returns_feature_metadata_with_stage() -> Result<()> {
    let codex_home = TempDir::new()?;
    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .build()
        .await?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;

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
async fn experimental_feature_enablement_set_applies_to_global_and_thread_config_reads()
-> Result<()> {
    let codex_home = TempDir::new()?;
    let project_cwd = codex_home.path().join("project");
    std::fs::create_dir_all(&project_cwd)?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let actual =
        set_experimental_feature_enablement(&mut mcp, BTreeMap::from([("apps".to_string(), true)]))
            .await?;
    assert_eq!(
        actual,
        ExperimentalFeatureEnablementSetResponse {
            enablement: BTreeMap::from([("apps".to_string(), true)]),
        }
    );

    for cwd in [None, Some(project_cwd.display().to_string())] {
        let ConfigReadResponse { config, .. } = read_config(&mut mcp, cwd).await?;

        assert_eq!(
            config
                .additional
                .get("features")
                .and_then(|features| features.get("apps")),
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
        "[features]\napps = false\n",
    )?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let actual =
        set_experimental_feature_enablement(&mut mcp, BTreeMap::from([("apps".to_string(), true)]))
            .await?;
    assert_eq!(
        actual,
        ExperimentalFeatureEnablementSetResponse {
            enablement: BTreeMap::from([("apps".to_string(), true)]),
        }
    );

    let ConfigReadResponse { config, .. } = read_config(&mut mcp, /*cwd*/ None).await?;

    assert_eq!(
        config
            .additional
            .get("features")
            .and_then(|features| features.get("apps")),
        Some(&json!(false))
    );

    Ok(())
}

#[tokio::test]
async fn experimental_feature_enablement_set_only_updates_named_features() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    set_experimental_feature_enablement(&mut mcp, BTreeMap::from([("apps".to_string(), true)]))
        .await?;
    let actual = set_experimental_feature_enablement(
        &mut mcp,
        BTreeMap::from([("plugins".to_string(), true)]),
    )
    .await?;

    assert_eq!(
        actual,
        ExperimentalFeatureEnablementSetResponse {
            enablement: BTreeMap::from([("plugins".to_string(), true)]),
        }
    );

    let ConfigReadResponse { config, .. } = read_config(&mut mcp, /*cwd*/ None).await?;

    assert_eq!(
        config
            .additional
            .get("features")
            .and_then(|features| features.get("apps")),
        Some(&json!(true))
    );
    assert_eq!(
        config
            .additional
            .get("features")
            .and_then(|features| features.get("plugins")),
        Some(&json!(true))
    );

    Ok(())
}

#[tokio::test]
async fn experimental_feature_enablement_set_empty_map_is_no_op() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    set_experimental_feature_enablement(&mut mcp, BTreeMap::from([("apps".to_string(), true)]))
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
            .and_then(|features| features.get("apps")),
        Some(&json!(true))
    );

    Ok(())
}

#[tokio::test]
async fn experimental_feature_enablement_set_rejects_non_allowlisted_feature() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_experimental_feature_enablement_set_request(ExperimentalFeatureEnablementSetParams {
            enablement: BTreeMap::from([("personality".to_string(), true)]),
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
            .contains("unsupported feature enablement `personality`"),
        "{}",
        error.message
    );
    assert!(error.message.contains("apps, plugins"), "{}", error.message);

    Ok(())
}

async fn set_experimental_feature_enablement(
    mcp: &mut McpProcess,
    enablement: BTreeMap<String, bool>,
) -> Result<ExperimentalFeatureEnablementSetResponse> {
    let request_id = mcp
        .send_experimental_feature_enablement_set_request(ExperimentalFeatureEnablementSetParams {
            enablement,
        })
        .await?;
    read_response(mcp, request_id).await
}

async fn read_config(mcp: &mut McpProcess, cwd: Option<String>) -> Result<ConfigReadResponse> {
    let request_id = mcp
        .send_config_read_request(ConfigReadParams {
            include_layers: false,
            cwd,
        })
        .await?;
    read_response(mcp, request_id).await
}

async fn read_response<T: DeserializeOwned>(mcp: &mut McpProcess, request_id: i64) -> Result<T> {
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    to_response(response)
}
