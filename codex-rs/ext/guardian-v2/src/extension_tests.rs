use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use codex_extension_api::ConversationHistorySnapshot;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ResponseItem;
use codex_extension_api::ThreadStartInput;
use codex_extension_api::ToolCallSource;
use codex_extension_api::ToolName;
use codex_extension_api::ToolPayload;
use codex_extension_api::ToolStartInput;
use codex_features::Feature;
use codex_history::RolloutItem;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::ResponseItemId;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ReasoningItemReasoningSummary;
use codex_protocol::protocol::SessionSource;
use codex_protocol::security_risk::SecurityRiskScore;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::json;

use super::encrypted_parent_compaction;
use crate::sampler::MODEL;

struct TestConversationHistory(Vec<ResponseItem>);

impl ConversationHistorySnapshot for TestConversationHistory {
    fn items(&self) -> Box<dyn Iterator<Item = &ResponseItem> + Send + '_> {
        Box::new(self.0.iter())
    }
}

#[test]
fn encrypted_parent_compaction_preserves_the_latest_valid_item() {
    let older = ResponseItem::Compaction {
        id: Some(ResponseItemId::from_server("cmp_older".to_owned())),
        encrypted_content: "older encrypted summary".to_owned(),
        internal_chat_message_metadata_passthrough: None,
    };
    let latest = ResponseItem::ContextCompaction {
        id: Some(ResponseItemId::from_server("cmp_latest".to_owned())),
        encrypted_content: Some("latest encrypted summary".to_owned()),
        internal_chat_message_metadata_passthrough: None,
    };

    assert_eq!(
        encrypted_parent_compaction([&older, &latest].into_iter()),
        Some(latest.clone())
    );
    assert_eq!(
        encrypted_parent_compaction([&latest, &older].into_iter()),
        Some(older)
    );
}

#[test]
fn encrypted_parent_compaction_rejects_invalid_latest_item() {
    let older = ResponseItem::Compaction {
        id: Some(ResponseItemId::from_server("cmp_older".to_owned())),
        encrypted_content: "older encrypted summary".to_owned(),
        internal_chat_message_metadata_passthrough: None,
    };
    let invalid = [
        ResponseItem::Compaction {
            id: None,
            encrypted_content: "encrypted summary without an ID".to_owned(),
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::Compaction {
            id: Some(ResponseItemId::from_server("cmp_empty".to_owned())),
            encrypted_content: String::new(),
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::ContextCompaction {
            id: None,
            encrypted_content: Some("encrypted context without an ID".to_owned()),
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::ContextCompaction {
            id: Some(ResponseItemId::from_server("cmp_missing".to_owned())),
            encrypted_content: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::ContextCompaction {
            id: Some(ResponseItemId::from_server("cmp_empty".to_owned())),
            encrypted_content: Some(String::new()),
            internal_chat_message_metadata_passthrough: None,
        },
    ];

    for latest in &invalid {
        assert_eq!(
            encrypted_parent_compaction([&older, latest].into_iter()),
            None,
            "an unusable latest summary must not resurrect older context"
        );
    }
}

async fn sample_conversation_history(
    conversation_history: Vec<ResponseItem>,
) -> Result<(serde_json::Value, TestCodex)> {
    let thread_server = responses::start_mock_server().await;
    let test = test_codex().build_with_auto_env(&thread_server).await?;
    let events = vec![
        ev_assistant_message("sample", r#"{"scores":{"action_risk":0.25}}"#),
        ev_completed("response-1"),
    ];
    let server = responses::start_websocket_server(vec![Vec::new(), vec![events]]).await;
    let provider_info = ModelProviderInfo::create_openai_provider(Some(format!(
        "http://{}/v1",
        server.uri().trim_start_matches("ws://")
    )));
    let auth_manager = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("test-api-key"));
    let mut config = test.config.clone();
    config.model_provider = provider_info;
    config.features.enable(Feature::GuardianV2)?;
    let mut builder = ExtensionRegistryBuilder::new();
    crate::install(
        &mut builder,
        auth_manager,
        Arc::downgrade(&test.thread_manager),
    );
    let registry = builder.build();
    let session_store = ExtensionData::new("session-1");
    let thread_store = test.codex.thread_extension_data();
    registry.thread_lifecycle_contributors()[0]
        .on_thread_start(ThreadStartInput {
            config: &config,
            session_source: &SessionSource::Exec,
            persistent_thread_state_available: false,
            environments: &[],
            mcp_resource_client: None,
            extension_metrics: None,
            session_store: &session_store,
            thread_store,
        })
        .await;
    let turn_store = ExtensionData::new("turn-1");
    let tool_name = ToolName::plain("read_file");
    let tool_payload = ToolPayload::Function {
        arguments: r#"{"path":"README.md"}"#.to_owned(),
    };
    let conversation_history = TestConversationHistory(conversation_history);

    registry.tool_lifecycle_contributors()[0]
        .on_tool_start(ToolStartInput {
            session_store: &session_store,
            thread_store,
            turn_store: &turn_store,
            turn_id: "turn-1",
            call_id: "call-1",
            tool_name: &tool_name,
            payload: &tool_payload,
            conversation_history: Arc::new(conversation_history),
            source: ToolCallSource::Direct,
        })
        .await;

    let request = tokio::time::timeout(
        Duration::from_secs(5),
        server.wait_for_request(/*connection_index*/ 1, /*request_index*/ 0),
    )
    .await?;
    Ok((request.body_json(), test))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contributor_samples_tool_calls_with_the_existing_luna_pool() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let (request, test) = sample_conversation_history(vec![
        ResponseItem::Message {
            id: None,
            role: "user".to_owned(),
            content: vec![ContentItem::InputText {
                text: "Inspect the repository guidelines.".to_owned(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::Reasoning {
            id: None,
            summary: vec![ReasoningItemReasoningSummary::SummaryText {
                text: "Find the repository documentation.".to_owned(),
            }],
            content: None,
            encrypted_content: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::FunctionCall {
            id: None,
            name: "list_dir".to_owned(),
            namespace: None,
            arguments: r#"{"path":"."}"#.to_owned(),
            encrypted_function_args: None,
            call_id: "previous-call".to_owned(),
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::FunctionCallOutput {
            id: None,
            call_id: "previous-call".to_owned(),
            output: FunctionCallOutputPayload::from_text("README.md".to_owned()),
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::FunctionCall {
            id: None,
            name: "read_file".to_owned(),
            namespace: None,
            arguments: r#"{"path":"README.md"}"#.to_owned(),
            encrypted_function_args: None,
            call_id: "call-1".to_owned(),
            internal_chat_message_metadata_passthrough: None,
        },
    ])
    .await?;
    let thread_id = test.session_configured.thread_id;
    let thread_store = test.codex.thread_extension_data();
    assert_eq!(request["model"], "gpt-5.6-luna");
    assert_eq!(
        request["client_metadata"]["thread_id"],
        thread_id.to_string()
    );
    assert_eq!(request["client_metadata"]["turn_id"], "turn-1");
    assert_eq!(request["reasoning"]["effort"], "low");
    assert_eq!(request["reasoning"]["context"], "all_turns");
    assert_eq!(request["text"]["format"]["strict"], true);
    assert_eq!(
        request["text"]["format"]["schema"]["properties"]["scores"]["properties"]["action_risk"],
        json!({"type": "number", "minimum": 0.0, "maximum": 1.0})
    );
    assert_eq!(
        request["input"][2]["content"],
        json!([
            {"type": "input_text", "text": ">>> TRANSCRIPT START\n"},
            {"type": "input_text", "text": "[1] user: Inspect the repository guidelines.\n"},
            {"type": "input_text", "text": "[2] tool list_dir call: {\"path\":\".\"}\n"},
            {"type": "input_text", "text": "[3] tool list_dir result: README.md\n"},
            {"type": "input_text", "text": "[4] tool read_file call: {\"path\":\"README.md\"}\n"},
            {"type": "input_text", "text": ">>> TRANSCRIPT END\n\n"},
            {
                "type": "input_text",
                "text": "The Codex agent has requested the following action:\n"
            },
            {"type": "input_text", "text": ">>> APPROVAL REQUEST START\n"},
            {"type": "input_text", "text": "Planned action JSON:\n"},
            {
                "type": "input_text",
                "text": "{\n  \"path\": \"README.md\",\n  \"tool\": \"read_file\"\n}\n"
            },
            {"type": "input_text", "text": ">>> APPROVAL REQUEST END\n"},
        ])
    );
    let score = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(score) = thread_store.get::<SecurityRiskScore>() {
                return score;
            }
            tokio::task::yield_now().await;
        }
    })
    .await?;
    assert_eq!(
        score.as_ref(),
        &SecurityRiskScore {
            category: "action_risk".to_string(),
            score: 0.25,
        }
    );
    test.codex.ensure_rollout_materialized().await;
    test.codex.flush_rollout().await?;
    let persisted_scores = test
        .codex
        .load_history(/*include_archived*/ false)
        .await?
        .items
        .into_iter()
        .filter_map(|item| match item {
            RolloutItem::SecurityRiskScore(score) => Some(score),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(persisted_scores, vec![score.as_ref().clone()]);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contributor_sends_compacted_conversation_history_to_luna() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let mut history = (0..8)
        .map(|index| ResponseItem::Message {
            id: None,
            role: "user".to_owned(),
            content: vec![ContentItem::InputText {
                text: format!("user turn {index}: {}", "authorization ".repeat(1_000)),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        })
        .collect::<Vec<_>>();
    history.extend((0..12).flat_map(|index| {
        let call_id = format!("call-{index}");
        [
            ResponseItem::FunctionCall {
                id: None,
                name: "exec_command".to_owned(),
                namespace: None,
                arguments: format!("tool evidence {index}: {}", "signal ".repeat(1_000)),
                encrypted_function_args: None,
                call_id: call_id.clone(),
                internal_chat_message_metadata_passthrough: None,
            },
            ResponseItem::FunctionCallOutput {
                id: None,
                call_id,
                output: FunctionCallOutputPayload::from_text(format!(
                    "result evidence {index}: {}",
                    "signal ".repeat(1_000)
                )),
                internal_chat_message_metadata_passthrough: None,
            },
        ]
    }));

    let (request, _test) = sample_conversation_history(history).await?;
    let content = request["input"][2]["content"]
        .as_array()
        .expect("Luna request should contain separate transcript text items");
    let entries = content
        .iter()
        .filter_map(|entry| entry["text"].as_str())
        .collect::<Vec<_>>();

    assert!(entries.iter().any(|entry| entry.contains("user turn 0:")));
    assert!(entries.iter().any(|entry| entry.contains("user turn 7:")));
    assert!(!entries.iter().any(|entry| entry.contains("user turn 1:")));
    assert!(
        entries
            .iter()
            .any(|entry| entry.contains("tool exec_command call: tool evidence 11:"))
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry.contains("tool exec_command result: result evidence 11:"))
    );
    assert!(
        !entries
            .iter()
            .any(|entry| entry.contains("tool evidence 0:"))
    );
    assert!(
        !entries
            .iter()
            .any(|entry| entry.contains("result evidence 0:"))
    );
    assert!(entries.iter().any(|entry| entry.contains("<truncated")));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contributor_reuses_the_latest_compatible_parent_compaction() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let thread_server = responses::start_mock_server().await;
    let test = test_codex().build_with_auto_env(&thread_server).await?;
    let events = vec![
        ev_assistant_message("sample", r#"{"scores":{"action_risk":0.25}}"#),
        ev_completed("response-1"),
    ];
    let server = responses::start_websocket_server(vec![Vec::new(), vec![events]]).await;
    let provider_info = ModelProviderInfo::create_openai_provider(Some(format!(
        "http://{}/v1",
        server.uri().trim_start_matches("ws://")
    )));
    let auth_manager = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("test-api-key"));
    let mut config = test.config.clone();
    config.model_provider = provider_info;
    config.features.enable(Feature::GuardianV2)?;
    let parent_model = test
        .thread_manager
        .get_models_manager()
        .get_model_info(MODEL, &config.to_models_manager_config())
        .await;
    let mut builder = ExtensionRegistryBuilder::new();
    crate::install(
        &mut builder,
        auth_manager,
        Arc::downgrade(&test.thread_manager),
    );
    let registry = builder.build();
    let session_store = ExtensionData::new("session-1");
    let thread_store = test.codex.thread_extension_data();
    thread_store.insert(parent_model);
    registry.thread_lifecycle_contributors()[0]
        .on_thread_start(ThreadStartInput {
            config: &config,
            session_source: &SessionSource::Exec,
            persistent_thread_state_available: false,
            environments: &[],
            mcp_resource_client: None,
            extension_metrics: None,
            session_store: &session_store,
            thread_store,
        })
        .await;
    let turn_store = ExtensionData::new("turn-1");
    let tool_name = ToolName::plain("read_file");
    let tool_payload = ToolPayload::Function {
        arguments: r#"{"path":"README.md"}"#.to_owned(),
    };
    let latest_compaction = ResponseItem::ContextCompaction {
        id: Some(ResponseItemId::from_server("cmp_latest".to_owned())),
        encrypted_content: Some("latest encrypted parent summary".to_owned()),
        internal_chat_message_metadata_passthrough: None,
    };
    let conversation_history = TestConversationHistory(vec![
        ResponseItem::Compaction {
            id: Some(ResponseItemId::from_server("cmp_old".to_owned())),
            encrypted_content: "old encrypted parent summary".to_owned(),
            internal_chat_message_metadata_passthrough: None,
        },
        latest_compaction.clone(),
        ResponseItem::Message {
            id: None,
            role: "user".to_owned(),
            content: vec![ContentItem::InputText {
                text: "Inspect the repository guidelines.".to_owned(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
    ]);

    registry.tool_lifecycle_contributors()[0]
        .on_tool_start(ToolStartInput {
            session_store: &session_store,
            thread_store,
            turn_store: &turn_store,
            turn_id: "turn-1",
            call_id: "call-1",
            tool_name: &tool_name,
            payload: &tool_payload,
            conversation_history: Arc::new(conversation_history),
            source: ToolCallSource::Direct,
        })
        .await;

    let request = tokio::time::timeout(
        Duration::from_secs(5),
        server.wait_for_request(/*connection_index*/ 1, /*request_index*/ 0),
    )
    .await?
    .body_json();
    assert_eq!(request["input"][0]["type"], "additional_tools");
    assert_eq!(request["input"][1]["role"], "developer");
    assert_eq!(
        request["input"][2],
        serde_json::to_value(latest_compaction)?
    );
    assert_eq!(request["input"][3]["role"], "user");

    Ok(())
}
