use std::time::Duration;

use anyhow::Result;
use anyhow::anyhow;
use app_test_support::McpProcess;
use app_test_support::to_response;
use app_test_support::write_models_cache;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::Model;
use codex_app_server_protocol::ModelListParams;
use codex_app_server_protocol::ModelListResponse;
use codex_app_server_protocol::ReasoningEffortOption;
use codex_app_server_protocol::RequestId;
use codex_protocol::openai_models::ModelPreset;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
const INVALID_REQUEST_ERROR_CODE: i64 = -32600;

fn model_from_preset(preset: &ModelPreset) -> Model {
    Model {
        id: preset.id.clone(),
        model: preset.model.clone(),
        upgrade: preset.upgrade.as_ref().map(|upgrade| upgrade.id.clone()),
        display_name: preset.display_name.clone(),
        description: preset.description.clone(),
        hidden: !preset.show_in_picker,
        supported_reasoning_efforts: preset
            .supported_reasoning_efforts
            .iter()
            .map(|preset| ReasoningEffortOption {
                reasoning_effort: preset.effort,
                description: preset.description.clone(),
            })
            .collect(),
        default_reasoning_effort: preset.default_reasoning_effort,
        input_modalities: preset.input_modalities.clone(),
        // `write_models_cache()` round-trips through a simplified ModelInfo fixture that does not
        // preserve personality placeholders in base instructions, so app-server list results from
        // cache report `supports_personality = false`.
        // todo(sayan): fix, maybe make roundtrip use ModelInfo only
        supports_personality: false,
        is_default: preset.is_default,
    }
}

fn expected_visible_models() -> Vec<Model> {
    // Filter by supported_in_api to support testing with both ChatGPT and non-ChatGPT auth modes.
    let mut presets =
        ModelPreset::filter_by_auth(codex_core::test_support::all_model_presets().clone(), false);

    // Mirror `ModelsManager::build_available_models()` default selection after auth filtering.
    ModelPreset::mark_default_by_picker_visibility(&mut presets);

    presets
        .iter()
        .filter(|preset| preset.show_in_picker)
        .map(model_from_preset)
        .collect()
}

#[tokio::test]
async fn list_models_returns_all_models_with_large_limit() -> Result<()> {
    let codex_home = TempDir::new()?;
    write_models_cache(codex_home.path())?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;

    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_list_models_request(ModelListParams {
            limit: Some(100),
            cursor: None,
            include_hidden: None,
        })
        .await?;

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;

    let ModelListResponse {
        data: items,
        next_cursor,
    } = to_response::<ModelListResponse>(response)?;

    let expected_models = expected_visible_models();

    assert_eq!(items, expected_models);
    assert!(next_cursor.is_none());
    Ok(())
}

#[tokio::test]
async fn list_models_includes_hidden_models() -> Result<()> {
    let codex_home = TempDir::new()?;
    write_models_cache(codex_home.path())?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;

    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_list_models_request(ModelListParams {
            limit: Some(100),
            cursor: None,
            include_hidden: Some(true),
        })
        .await?;

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;

    let ModelListResponse {
        data: items,
        next_cursor,
    } = to_response::<ModelListResponse>(response)?;

    assert!(items.iter().any(|item| item.hidden));
    assert!(next_cursor.is_none());
    Ok(())
}

#[tokio::test]
async fn list_models_pagination_works() -> Result<()> {
    let codex_home = TempDir::new()?;
    write_models_cache(codex_home.path())?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;

    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let first_request = mcp
        .send_list_models_request(ModelListParams {
            limit: Some(1),
            cursor: None,
            include_hidden: None,
        })
        .await?;

    let first_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(first_request)),
    )
    .await??;

    let ModelListResponse {
        data: first_items,
        next_cursor: first_cursor,
    } = to_response::<ModelListResponse>(first_response)?;

    let expected_models = expected_visible_models();

    assert_eq!(first_items.len(), 1);
    assert_eq!(first_items[0].id, expected_models[0].id);
    let next_cursor = first_cursor.ok_or_else(|| anyhow!("cursor for second page"))?;

    let second_request = mcp
        .send_list_models_request(ModelListParams {
            limit: Some(1),
            cursor: Some(next_cursor.clone()),
            include_hidden: None,
        })
        .await?;

    let second_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(second_request)),
    )
    .await??;

    let ModelListResponse {
        data: second_items,
        next_cursor: second_cursor,
    } = to_response::<ModelListResponse>(second_response)?;

    assert_eq!(second_items.len(), 1);
    assert_eq!(second_items[0].id, expected_models[1].id);
    let third_cursor = second_cursor.ok_or_else(|| anyhow!("cursor for third page"))?;

    let third_request = mcp
        .send_list_models_request(ModelListParams {
            limit: Some(1),
            cursor: Some(third_cursor.clone()),
            include_hidden: None,
        })
        .await?;

    let third_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(third_request)),
    )
    .await??;

    let ModelListResponse {
        data: third_items,
        next_cursor: third_cursor,
    } = to_response::<ModelListResponse>(third_response)?;

    assert_eq!(third_items.len(), 1);
    assert_eq!(third_items[0].id, expected_models[2].id);
    let fourth_cursor = third_cursor.ok_or_else(|| anyhow!("cursor for fourth page"))?;

    let fourth_request = mcp
        .send_list_models_request(ModelListParams {
            limit: Some(1),
            cursor: Some(fourth_cursor.clone()),
            include_hidden: None,
        })
        .await?;

    let fourth_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(fourth_request)),
    )
    .await??;

    let ModelListResponse {
        data: fourth_items,
        next_cursor: fourth_cursor,
    } = to_response::<ModelListResponse>(fourth_response)?;

    assert_eq!(fourth_items.len(), 1);
    assert_eq!(fourth_items[0].id, expected_models[3].id);
    assert!(fourth_cursor.is_none());
    Ok(())
}

#[tokio::test]
async fn list_models_rejects_invalid_cursor() -> Result<()> {
    let codex_home = TempDir::new()?;
    write_models_cache(codex_home.path())?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;

    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_list_models_request(ModelListParams {
            limit: None,
            cursor: Some("invalid".to_string()),
            include_hidden: None,
        })
        .await?;

    let error: JSONRPCError = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(error.id, RequestId::Integer(request_id));
    assert_eq!(error.error.code, INVALID_REQUEST_ERROR_CODE);
    assert_eq!(error.error.message, "invalid cursor: invalid");
    Ok(())
}
