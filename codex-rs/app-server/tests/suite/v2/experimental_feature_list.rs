use std::time::Duration;

use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::to_response;
use codex_app_server_protocol::ExperimentalFeature;
use codex_app_server_protocol::ExperimentalFeatureListParams;
use codex_app_server_protocol::ExperimentalFeatureListResponse;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_core::features::FEATURES;
use codex_core::features::Stage;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test]
async fn experimental_feature_list_returns_experimental_feature_metadata() -> Result<()> {
    let codex_home = TempDir::new()?;
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
        .filter_map(|spec| {
            let Stage::Experimental {
                name,
                menu_description,
                announcement,
            } = spec.stage
            else {
                return None;
            };

            Some(ExperimentalFeature {
                flag_name: spec.key.to_string(),
                display_name: name.to_string(),
                description: menu_description.to_string(),
                announcement: announcement.to_string(),
                enabled: spec.default_enabled,
                default_enabled: spec.default_enabled,
            })
        })
        .collect::<Vec<_>>();
    let expected = ExperimentalFeatureListResponse {
        data: expected_data,
        next_cursor: None,
    };

    assert_eq!(actual, expected);
    Ok(())
}
