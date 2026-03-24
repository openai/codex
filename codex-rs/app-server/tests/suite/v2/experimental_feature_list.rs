use std::time::Duration;

use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::to_response;
use codex_app_server_protocol::ConfigReadParams;
use codex_app_server_protocol::ConfigReadResponse;
use codex_app_server_protocol::ExperimentalFeature;
use codex_app_server_protocol::ExperimentalFeatureListParams;
use codex_app_server_protocol::ExperimentalFeatureListResponse;
use codex_app_server_protocol::ExperimentalFeatureOverridesSetParams;
use codex_app_server_protocol::ExperimentalFeatureOverridesSetResponse;
use codex_app_server_protocol::ExperimentalFeatureStage;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_core::config::ConfigBuilder;
use codex_features::FEATURES;
use codex_features::Stage;
use pretty_assertions::assert_eq;
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

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;

    let actual = to_response::<ExperimentalFeatureListResponse>(response)?;
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
async fn experimental_feature_overrides_set_applies_to_global_and_thread_config_reads() -> Result<()>
{
    let codex_home = TempDir::new()?;
    let project_cwd = codex_home.path().join("project");
    std::fs::create_dir_all(&project_cwd)?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_experimental_feature_overrides_set_request(ExperimentalFeatureOverridesSetParams {
            overrides: BTreeMap::from([("apps".to_string(), true)]),
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let actual = to_response::<ExperimentalFeatureOverridesSetResponse>(response)?;
    assert_eq!(
        actual,
        ExperimentalFeatureOverridesSetResponse {
            overrides: BTreeMap::from([("apps".to_string(), true)]),
        }
    );

    for cwd in [None, Some(project_cwd.display().to_string())] {
        let request_id = mcp
            .send_config_read_request(ConfigReadParams {
                include_layers: false,
                cwd,
            })
            .await?;
        let response: JSONRPCResponse = timeout(
            DEFAULT_TIMEOUT,
            mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
        )
        .await??;
        let ConfigReadResponse { config, .. } = to_response(response)?;

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
async fn experimental_feature_overrides_set_does_not_override_user_config() -> Result<()> {
    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        "[features]\napps = false\n",
    )?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_experimental_feature_overrides_set_request(ExperimentalFeatureOverridesSetParams {
            overrides: BTreeMap::from([("apps".to_string(), true)]),
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let actual = to_response::<ExperimentalFeatureOverridesSetResponse>(response)?;
    assert_eq!(
        actual,
        ExperimentalFeatureOverridesSetResponse {
            overrides: BTreeMap::from([("apps".to_string(), true)]),
        }
    );

    let request_id = mcp
        .send_config_read_request(ConfigReadParams {
            include_layers: false,
            cwd: None,
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let ConfigReadResponse { config, .. } = to_response(response)?;

    assert_eq!(
        config
            .additional
            .get("features")
            .and_then(|features| features.get("apps")),
        Some(&json!(false))
    );

    Ok(())
}
