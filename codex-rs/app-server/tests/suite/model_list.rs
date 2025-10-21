use std::time::Duration;

use app_test_support::McpProcess;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::ListModelsParams;
use codex_app_server_protocol::ListModelsResponse;
use codex_app_server_protocol::Model;
use codex_app_server_protocol::ReasoningEffortOption;
use codex_app_server_protocol::RequestId;
use codex_protocol::config_types::ReasoningEffort;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
const INVALID_REQUEST_ERROR_CODE: i64 = -32600;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn list_models_returns_all_models_with_large_limit() {
    let codex_home = TempDir::new().expect("create temp dir");
    let mut mcp = McpProcess::new(codex_home.path())
        .await
        .expect("spawn mcp process");

    timeout(DEFAULT_TIMEOUT, mcp.initialize())
        .await
        .expect("initialize timeout")
        .expect("initialize success");

    let request_id = mcp
        .send_list_models_request(ListModelsParams {
            page_size: Some(100),
            cursor: None,
        })
        .await
        .expect("send models request");

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await
    .expect("models response timeout")
    .expect("models response");

    let ListModelsResponse { items, next_cursor } =
        to_response::<ListModelsResponse>(response).expect("decode models response");

    let expected_models = vec![
        Model {
            id: "gpt-5-codex".to_string(),
            model: "gpt-5-codex".to_string(),
            display_name: "GPT-5 Codex".to_string(),
            description: "Specialized GPT-5 variant optimized for Codex.".to_string(),
            supported_reasoning_efforts: vec![
                ReasoningEffortOption {
                    reasoning_effort: ReasoningEffort::Low,
                    description: "Fastest responses with limited reasoning".to_string(),
                },
                ReasoningEffortOption {
                    reasoning_effort: ReasoningEffort::Medium,
                    description: "Dynamically adjusts reasoning based on the task".to_string(),
                },
                ReasoningEffortOption {
                    reasoning_effort: ReasoningEffort::High,
                    description: "Maximizes reasoning depth for complex or ambiguous problems"
                        .to_string(),
                },
            ],
            default_reasoning_effort: ReasoningEffort::Medium,
            is_default: true,
        },
        Model {
            id: "gpt-5".to_string(),
            model: "gpt-5".to_string(),
            display_name: "GPT-5".to_string(),
            description: "General-purpose GPT-5 model.".to_string(),
            supported_reasoning_efforts: vec![
                ReasoningEffortOption {
                    reasoning_effort: ReasoningEffort::Minimal,
                    description: "Fastest responses with little reasoning".to_string(),
                },
                ReasoningEffortOption {
                    reasoning_effort: ReasoningEffort::Low,
                    description: "Balances speed with some reasoning; useful for straightforward \
                                   queries and short explanations"
                        .to_string(),
                },
                ReasoningEffortOption {
                    reasoning_effort: ReasoningEffort::Medium,
                    description: "Provides a solid balance of reasoning depth and latency for \
                         general-purpose tasks"
                        .to_string(),
                },
                ReasoningEffortOption {
                    reasoning_effort: ReasoningEffort::High,
                    description: "Maximizes reasoning depth for complex or ambiguous problems"
                        .to_string(),
                },
            ],
            default_reasoning_effort: ReasoningEffort::Medium,
            is_default: false,
        },
    ];

    assert_eq!(items, expected_models);
    assert!(next_cursor.is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn list_models_pagination_works() {
    let codex_home = TempDir::new().expect("create temp dir");
    let mut mcp = McpProcess::new(codex_home.path())
        .await
        .expect("spawn mcp process");

    timeout(DEFAULT_TIMEOUT, mcp.initialize())
        .await
        .expect("initialize timeout")
        .expect("initialize success");

    let first_request = mcp
        .send_list_models_request(ListModelsParams {
            page_size: Some(1),
            cursor: None,
        })
        .await
        .expect("send first page");

    let first_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(first_request)),
    )
    .await
    .expect("first page timeout")
    .expect("first page response");

    let ListModelsResponse {
        items: first_items,
        next_cursor: first_cursor,
    } = to_response::<ListModelsResponse>(first_response).expect("decode first page");

    assert_eq!(first_items.len(), 1);
    assert_eq!(first_items[0].id, "gpt-5-codex");
    let next_cursor = first_cursor.expect("cursor for second page");

    let second_request = mcp
        .send_list_models_request(ListModelsParams {
            page_size: Some(1),
            cursor: Some(next_cursor.clone()),
        })
        .await
        .expect("send second page");

    let second_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(second_request)),
    )
    .await
    .expect("second page timeout")
    .expect("second page response");

    let ListModelsResponse {
        items: second_items,
        next_cursor: second_cursor,
    } = to_response::<ListModelsResponse>(second_response).expect("decode second page");

    assert_eq!(second_items.len(), 1);
    assert_eq!(second_items[0].id, "gpt-5");
    assert!(second_cursor.is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn list_models_rejects_invalid_cursor() {
    let codex_home = TempDir::new().expect("create temp dir");
    let mut mcp = McpProcess::new(codex_home.path())
        .await
        .expect("spawn mcp process");

    timeout(DEFAULT_TIMEOUT, mcp.initialize())
        .await
        .expect("initialize timeout")
        .expect("initialize success");

    let request_id = mcp
        .send_list_models_request(ListModelsParams {
            page_size: None,
            cursor: Some("invalid".to_string()),
        })
        .await
        .expect("send invalid cursor");

    let error: JSONRPCError = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await
    .expect("invalid cursor timeout")
    .expect("invalid cursor error");

    assert_eq!(error.id, RequestId::Integer(request_id));
    assert_eq!(error.error.code, INVALID_REQUEST_ERROR_CODE);
    assert_eq!(error.error.message, "invalid cursor: invalid");
}
