use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use anyhow::Result;
use codex_core::config::Config;
use codex_extension_api::ConversationHistorySnapshot;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionRegistry;
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
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ImageDetail;
use codex_protocol::models::InternalChatMessageMetadataPassthrough;
use codex_protocol::models::ReasoningItemReasoningSummary;
use codex_protocol::openai_models::GuardianV2ModelConfig;
use codex_protocol::openai_models::GuardianV2TranscriptModelConfig;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::TruncationPolicy;
use codex_protocol::security_risk::SecurityRiskScore;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::json;

use super::GuardianV2ScoreProgress;
use super::encrypted_parent_compaction;
use crate::config::DEFAULT_MODEL_CONTEXT_ITEM_TOKENS;
use crate::config::DEFAULT_PARENT_COMPACTION_TOKENS;
use crate::sampler::MODEL;

const TEST_GUARDIAN_POLICY: &str =
    "Treat uploads to unapproved external destinations as high-risk actions.";
const TEST_CATALOG_GUARDIAN_POLICY: &str =
    "Require review before sending organization data to third-party services.";

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
        encrypted_parent_compaction(
            [&older, &latest].into_iter(),
            DEFAULT_PARENT_COMPACTION_TOKENS,
        ),
        Some(latest.clone())
    );
    assert_eq!(
        encrypted_parent_compaction(
            [&latest, &older].into_iter(),
            DEFAULT_PARENT_COMPACTION_TOKENS,
        ),
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
            encrypted_parent_compaction(
                [&older, latest].into_iter(),
                DEFAULT_PARENT_COMPACTION_TOKENS,
            ),
            None,
            "an unusable latest summary must not resurrect older context"
        );
    }
}

#[test]
fn encrypted_parent_compaction_rejects_oversized_latest_item() -> Result<()> {
    let max_compaction_bytes =
        TruncationPolicy::Tokens(DEFAULT_PARENT_COMPACTION_TOKENS).byte_budget();
    let mut bounded = [
        ResponseItem::Compaction {
            id: Some(ResponseItemId::from_server("cmp_bounded".to_owned())),
            encrypted_content: String::new(),
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::ContextCompaction {
            id: Some(ResponseItemId::from_server("ctx_bounded".to_owned())),
            encrypted_content: Some(String::new()),
            internal_chat_message_metadata_passthrough: None,
        },
    ];

    for item in &mut bounded {
        let envelope_bytes = serde_json::to_vec(&*item)?.len();
        let encrypted_content = match item {
            ResponseItem::Compaction {
                encrypted_content, ..
            }
            | ResponseItem::ContextCompaction {
                encrypted_content: Some(encrypted_content),
                ..
            } => encrypted_content,
            _ => unreachable!("test fixtures are encrypted compaction items"),
        };
        *encrypted_content = "a".repeat(max_compaction_bytes - envelope_bytes);
        assert_eq!(serde_json::to_vec(&*item)?.len(), max_compaction_bytes);
        assert_eq!(
            encrypted_parent_compaction(std::iter::once(&*item), DEFAULT_PARENT_COMPACTION_TOKENS,),
            Some(item.clone())
        );

        let mut oversized = item.clone();
        match &mut oversized {
            ResponseItem::Compaction {
                encrypted_content, ..
            }
            | ResponseItem::ContextCompaction {
                encrypted_content: Some(encrypted_content),
                ..
            } => encrypted_content.push('a'),
            _ => unreachable!("test fixtures are encrypted compaction items"),
        }
        assert_eq!(
            serde_json::to_vec(&oversized)?.len(),
            max_compaction_bytes + 1
        );
        assert_eq!(
            encrypted_parent_compaction(
                [&*item, &oversized].into_iter(),
                DEFAULT_PARENT_COMPACTION_TOKENS,
            ),
            None,
            "an oversized latest summary must not resurrect older context"
        );
    }

    let oversized_metadata = ResponseItem::ContextCompaction {
        id: Some(ResponseItemId::from_server(
            "ctx_oversized_metadata".to_owned(),
        )),
        encrypted_content: Some("bounded encrypted summary".to_owned()),
        internal_chat_message_metadata_passthrough: Some(InternalChatMessageMetadataPassthrough {
            turn_id: Some("a".repeat(max_compaction_bytes)),
            ..Default::default()
        }),
    };
    assert!(serde_json::to_vec(&oversized_metadata)?.len() > max_compaction_bytes);
    assert_eq!(
        encrypted_parent_compaction(
            [&bounded[0], &oversized_metadata].into_iter(),
            DEFAULT_PARENT_COMPACTION_TOKENS,
        ),
        None,
        "oversized passthrough metadata must not bypass the complete-item limit"
    );

    Ok(())
}

async fn sample_conversation_history(
    conversation_history: Vec<ResponseItem>,
    arguments: &str,
    guardian_policy: Option<&str>,
) -> Result<(serde_json::Value, TestCodex, ExtensionRegistry<Config>)> {
    sample_configured_conversation_history(
        conversation_history,
        arguments,
        guardian_policy,
        "",
        /*model_defaults*/ None,
    )
    .await
}

async fn sample_configured_conversation_history(
    conversation_history: Vec<ResponseItem>,
    arguments: &str,
    guardian_policy: Option<&str>,
    guardian_config: &str,
    model_defaults: Option<GuardianV2ModelConfig>,
) -> Result<(serde_json::Value, TestCodex, ExtensionRegistry<Config>)> {
    let thread_server = responses::start_mock_server().await;
    let guardian_policy = guardian_policy.map(str::to_owned);
    let guardian_config = guardian_config.to_owned();
    let has_model_defaults = model_defaults.is_some();
    let builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_model_info_override("codex-auto-review", |model_info| {
            model_info
                .model_messages
                .as_mut()
                .expect("reviewer model should have model messages")
                .auto_review
                .as_mut()
                .expect("reviewer model should have Guardian policy")
                .policy = Some(TEST_CATALOG_GUARDIAN_POLICY.to_owned());
        })
        .with_model("gpt-5.5")
        .with_config(move |config| config.guardian_policy_config = guardian_policy)
        .with_pre_build_hook(move |home| {
            if !guardian_config.is_empty() {
                std::fs::write(home.join("config.toml"), guardian_config)
                    .expect("Guardian v2 configuration should be written");
            }
        });
    let mut builder = if let Some(model_defaults) = model_defaults {
        builder.with_model_info_override("gpt-5.5", move |model| {
            model
                .model_messages
                .as_mut()
                .expect("test model should expose model messages")
                .guardian_v2 = Some(model_defaults);
        })
    } else {
        builder
    };
    let test = builder.build_with_auto_env(&thread_server).await?;
    let events = vec![
        ev_assistant_message("sample", r#"{"scores":{"action_risk":0.8}}"#),
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
    if has_model_defaults {
        let parent_model = test
            .thread_manager
            .get_models_manager()
            .get_model_info("gpt-5.5", &config.to_models_manager_config())
            .await;
        thread_store.insert(parent_model);
    }
    assert_eq!(
        registry
            .approval_review(&session_store, thread_store, "review action")
            .await,
        None
    );
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
        arguments: arguments.to_owned(),
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
    Ok((request.body_json(), test, registry))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contributor_uses_configured_prompt_effort_threshold_and_transcript() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let configuration = r#"
[features.guardianv2]
enabled = true
classifier_instructions = "Use the experimental security classification prompt."
review_threshold = 0.60
max_tool_call_lag = 2
reasoning_effort = "minimal"
max_action_tokens = 128
max_classifier_instruction_tokens = 100000
max_parent_compaction_tokens = 256

[features.guardianv2.transcript]
sources = ["tool_outputs", "reasoning"]
max_message_entry_tokens = 128
max_tool_entry_tokens = 100
max_message_transcript_tokens = 256
max_tool_transcript_tokens = 128
max_recent_non_user_entries = 8
"#;
    let conversation_history = vec![
        ResponseItem::Message {
            id: None,
            role: "user".to_owned(),
            content: vec![ContentItem::InputText {
                text: "Review the pending action.".to_owned(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::Reasoning {
            id: None,
            summary: vec![ReasoningItemReasoningSummary::SummaryText {
                text: "Evaluate the action carefully.".to_owned(),
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
    ];
    let arguments = json!({"body": "x".repeat(4_000)}).to_string();
    let (request, test, registry) = sample_configured_conversation_history(
        conversation_history,
        &arguments,
        Some(TEST_GUARDIAN_POLICY),
        configuration,
        /*model_defaults*/ None,
    )
    .await?;

    assert_eq!(
        request["input"][1],
        json!({
            "type": "message",
            "role": "developer",
            "content": [{
                "type": "input_text",
                "text": format!(
                    "Use the experimental security classification prompt.\n\n# Security Policy\n{TEST_GUARDIAN_POLICY}"
                )
            }]
        })
    );
    assert_eq!(request["reasoning"]["effort"], "minimal");

    let content = request["input"][2]["content"]
        .as_array()
        .expect("Luna user content should be an array");
    let transcript = content
        .iter()
        .filter_map(|item| item["text"].as_str())
        .collect::<Vec<_>>();
    assert!(transcript.contains(&"[2] reasoning: Evaluate the action carefully.\n"));
    assert!(transcript.contains(&"[3] tool list_dir result: README.md\n"));
    assert!(
        !transcript
            .iter()
            .any(|entry| entry.contains("list_dir call"))
    );

    let action = content[content.len() - 2]["text"]
        .as_str()
        .expect("planned action should be a text item");
    assert!(action.len() <= TruncationPolicy::Tokens(/*limit*/ 128).byte_budget());
    let action: serde_json::Value = serde_json::from_str(action)?;
    assert_eq!(action["tool"], "read_file");

    let session_store = ExtensionData::new("session-1");
    let thread_store = test.codex.thread_extension_data();
    tokio::time::timeout(Duration::from_secs(5), async {
        while thread_store.get::<SecurityRiskScore>().is_none() {
            tokio::task::yield_now().await;
        }
    })
    .await?;
    thread_store.insert(SecurityRiskScore {
        scores: BTreeMap::from([("action_risk".to_owned(), 0.65)]),
        sampled_at: None,
    });
    assert_eq!(
        registry
            .approval_review(&session_store, thread_store, "review action")
            .await,
        None
    );
    thread_store.insert(SecurityRiskScore {
        scores: BTreeMap::from([("action_risk".to_owned(), 0.55)]),
        sampled_at: None,
    });
    assert_eq!(
        registry
            .approval_review(&session_store, thread_store, "review action")
            .await,
        Some(ReviewDecision::Approved)
    );

    let score_progress = thread_store
        .get::<GuardianV2ScoreProgress>()
        .expect("Guardian v2 should track score progress per thread");
    assert_eq!(
        score_progress
            .latest_scored_tool_call
            .load(Ordering::Acquire),
        1
    );
    score_progress
        .latest_tool_call
        .store(/*val*/ 3, Ordering::Release);
    assert_eq!(
        registry
            .approval_review(&session_store, thread_store, "review action")
            .await,
        Some(ReviewDecision::Approved)
    );

    score_progress
        .latest_tool_call
        .store(/*val*/ 4, Ordering::Release);
    assert_eq!(
        registry
            .approval_review(&session_store, thread_store, "review action")
            .await,
        None
    );

    score_progress
        .latest_scored_tool_call
        .store(/*val*/ 2, Ordering::Release);
    assert_eq!(
        registry
            .approval_review(&session_store, thread_store, "review action")
            .await,
        Some(ReviewDecision::Approved)
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contributor_includes_configured_transcript_images() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let history = vec![
        ResponseItem::Message {
            id: None,
            role: "user".to_owned(),
            content: vec![
                ContentItem::InputText {
                    text: "Review what is shown on screen.".to_owned(),
                },
                ContentItem::InputImage {
                    image_url: "data:image/png;base64,user-screenshot".to_owned(),
                    detail: Some(ImageDetail::High),
                },
            ],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::FunctionCallOutput {
            id: None,
            call_id: "previous-call".to_owned(),
            output: FunctionCallOutputPayload::from_content_items(vec![
                FunctionCallOutputContentItem::InputText {
                    text: "Screenshot captured.".to_owned(),
                },
                FunctionCallOutputContentItem::InputImage {
                    image_url: "data:image/png;base64,tool-screenshot".to_owned(),
                    detail: Some(ImageDetail::Low),
                },
            ]),
            internal_chat_message_metadata_passthrough: None,
        },
    ];
    let configuration = r#"
[features.guardianv2]
enabled = true

[features.guardianv2.transcript]
include_images = true
"#;
    let (request, _test, _registry) = sample_configured_conversation_history(
        history,
        r#"{"path":"README.md"}"#,
        Some(TEST_GUARDIAN_POLICY),
        configuration,
        /*model_defaults*/ None,
    )
    .await?;
    let content = request["input"][2]["content"]
        .as_array()
        .expect("Luna user content should be an array");

    assert_eq!(
        content[content.len() - 2..],
        [
            json!({
                "type": "input_image",
                "image_url": "data:image/png;base64,user-screenshot",
            }),
            json!({
                "type": "input_image",
                "image_url": "data:image/png;base64,tool-screenshot",
            }),
        ]
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contributor_uses_model_defaults_and_preserves_local_overrides() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let model_defaults = GuardianV2ModelConfig {
        classifier_instructions: Some("Use the experimental model-owned prompt.".to_owned()),
        review_threshold_basis_points: Some(6_000),
        reasoning_effort: Some(ReasoningEffort::Minimal),
        transcript: Some(GuardianV2TranscriptModelConfig {
            sources: Some(vec!["reasoning".to_owned()]),
            max_message_entry_tokens: Some(128),
            max_message_transcript_tokens: Some(256),
            ..Default::default()
        }),
        max_action_tokens: Some(128),
        max_classifier_instruction_tokens: Some(256),
        max_parent_compaction_tokens: Some(384),
    };
    let conversation_history = vec![
        ResponseItem::Message {
            id: None,
            role: "user".to_owned(),
            content: vec![ContentItem::InputText {
                text: "Review the pending action.".to_owned(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::Reasoning {
            id: None,
            summary: vec![ReasoningItemReasoningSummary::SummaryText {
                text: "Use the experimental transcript.".to_owned(),
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
    ];
    let arguments = json!({"body": "x".repeat(4_000)}).to_string();
    let local_config = "[features.guardianv2]\nenabled = true\nreview_threshold = 0.70\n";
    let (request, test, registry) = sample_configured_conversation_history(
        conversation_history,
        &arguments,
        Some(TEST_GUARDIAN_POLICY),
        local_config,
        Some(model_defaults),
    )
    .await?;

    assert_eq!(
        request["input"][1],
        json!({
            "type": "message",
            "role": "developer",
            "content": [{
                "type": "input_text",
                "text": format!(
                    "Use the experimental model-owned prompt.\n\n# Security Policy\n{TEST_GUARDIAN_POLICY}"
                )
            }]
        })
    );
    assert_eq!(request["reasoning"]["effort"], "minimal");
    let content = request["input"][2]["content"]
        .as_array()
        .expect("Luna user content should be an array");
    let transcript = content
        .iter()
        .filter_map(|item| item["text"].as_str())
        .collect::<Vec<_>>();
    assert!(transcript.contains(&"[2] reasoning: Use the experimental transcript.\n"));
    assert!(!transcript.iter().any(|entry| entry.contains("list_dir")));
    let action = content[content.len() - 2]["text"]
        .as_str()
        .expect("planned action should be a text item");
    assert!(action.len() <= TruncationPolicy::Tokens(/*limit*/ 128).byte_budget());

    let session_store = ExtensionData::new("session-1");
    let thread_store = test.codex.thread_extension_data();
    assert_eq!(
        thread_store
            .get::<crate::config::GuardianV2Config>()
            .expect("Guardian v2 configuration should be installed")
            .max_parent_compaction_tokens,
        384
    );
    tokio::time::timeout(Duration::from_secs(5), async {
        while thread_store.get::<SecurityRiskScore>().is_none() {
            tokio::task::yield_now().await;
        }
    })
    .await?;
    thread_store.insert(SecurityRiskScore {
        scores: BTreeMap::from([("action_risk".to_owned(), 0.65)]),
        sampled_at: None,
    });
    assert_eq!(
        registry
            .approval_review(&session_store, thread_store, "review action")
            .await,
        Some(ReviewDecision::Approved)
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contributor_samples_tool_calls_with_the_existing_luna_pool() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let conversation_history = vec![
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
    ];
    let (request, test, registry) = sample_conversation_history(
        conversation_history,
        r#"{"path":"README.md"}"#,
        Some(TEST_GUARDIAN_POLICY),
    )
    .await?;
    let thread_id = test.session_configured.thread_id;
    let session_store = ExtensionData::new("session-1");
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
        request["input"][1],
        json!({
            "type": "message",
            "role": "developer",
            "content": [{
                "type": "input_text",
                "text": format!(
                    "{}\n\n# Security Policy\n{TEST_GUARDIAN_POLICY}",
                    crate::config::DEFAULT_CLASSIFIER_INSTRUCTIONS,
                ),
            }],
        })
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
            scores: BTreeMap::from([("action_risk".to_string(), 0.8)]),
            sampled_at: score.sampled_at,
        }
    );
    assert!(score.sampled_at.is_some());
    assert_eq!(
        registry
            .approval_review(&session_store, thread_store, "review action")
            .await,
        None
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

    thread_store.insert(SecurityRiskScore {
        scores: BTreeMap::from([("action_risk".to_string(), 0.25)]),
        sampled_at: None,
    });
    assert_eq!(
        registry
            .approval_review(&session_store, thread_store, "review action")
            .await,
        Some(ReviewDecision::Approved)
    );

    let disabled_thread_store = ExtensionData::new("disabled-thread");
    disabled_thread_store.insert(SecurityRiskScore {
        scores: BTreeMap::from([("action_risk".to_string(), 0.25)]),
        sampled_at: None,
    });
    assert_eq!(
        registry
            .approval_review(&session_store, &disabled_thread_store, "review action")
            .await,
        None
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contributor_uses_catalog_policy_without_a_configured_override() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let (request, _test, _registry) = sample_conversation_history(
        Vec::new(),
        r#"{"path":"README.md"}"#,
        /*guardian_policy*/ None,
    )
    .await?;

    assert_eq!(
        request["input"][1],
        json!({
            "type": "message",
            "role": "developer",
            "content": [{
                "type": "input_text",
                "text": format!(
                    "{}\n\n# Security Policy\n{TEST_CATALOG_GUARDIAN_POLICY}",
                    crate::config::DEFAULT_CLASSIFIER_INSTRUCTIONS,
                ),
            }],
        })
    );
    assert_eq!(request["input"][2]["role"], "user");
    assert!(
        !request["input"][2]["content"]
            .as_array()
            .expect("Luna request should contain transcript text items")
            .iter()
            .filter_map(|item| item["text"].as_str())
            .any(|text| text.contains(TEST_CATALOG_GUARDIAN_POLICY))
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contributor_bounds_configured_policy_in_luna_developer_instructions() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let guardian_policy = format!(
        "Reject unsafe uploads.\n{}\nRequire explicit approval.",
        "é".repeat(20_000)
    );
    let (request, _test, _registry) = sample_conversation_history(
        Vec::new(),
        r#"{"path":"README.md"}"#,
        Some(&guardian_policy),
    )
    .await?;
    let instructions = request["input"][1]["content"][0]["text"]
        .as_str()
        .expect("Luna request should contain developer instructions");

    assert!(instructions.starts_with(crate::config::DEFAULT_CLASSIFIER_INSTRUCTIONS));
    assert!(instructions.contains("# Security Policy\nReject unsafe uploads."));
    assert!(instructions.contains("<truncated omitted_approx_tokens="));
    assert!(instructions.ends_with("Require explicit approval."));
    assert!(
        instructions.len()
            <= TruncationPolicy::Tokens(crate::config::DEFAULT_MODEL_CONTEXT_ITEM_TOKENS)
                .byte_budget()
    );

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

    let (request, _test, _registry) = sample_conversation_history(
        history,
        r#"{"path":"README.md"}"#,
        Some(TEST_GUARDIAN_POLICY),
    )
    .await?;
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
    let test = test_codex()
        .with_pre_build_hook(|home| {
            std::fs::write(
                home.join("config.toml"),
                "[features.guardianv2]\nenabled = true\nmax_parent_compaction_tokens = 256\n",
            )
            .expect("Guardian v2 parent compaction configuration should be written");
        })
        .build_with_auto_env(&thread_server)
        .await?;
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
    let developer_message = &request["input"][1];
    assert_eq!(developer_message["role"], "developer");
    assert!(
        developer_message["content"][0]["text"]
            .as_str()
            .expect("Luna request should contain developer instructions")
            .replace("\r\n", "\n")
            .starts_with(&format!(
                "{}\n\n# Security Policy\n## Environment Profile\n",
                crate::config::DEFAULT_CLASSIFIER_INSTRUCTIONS,
            ))
    );
    assert_eq!(
        request["input"][2],
        serde_json::to_value(&latest_compaction)?
    );
    assert_eq!(request["input"][3]["role"], "user");

    let previous_score = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(score) = thread_store.get::<SecurityRiskScore>() {
                return score;
            }
            tokio::task::yield_now().await;
        }
    })
    .await?;
    assert_eq!(previous_score.scores.get("action_risk"), Some(&0.25));
    assert_eq!(
        registry
            .approval_review(&session_store, thread_store, "review action")
            .await,
        Some(ReviewDecision::Approved)
    );

    let oversized_compaction = ResponseItem::Compaction {
        id: Some(ResponseItemId::from_server("cmp_oversized".to_owned())),
        encrypted_content: "a".repeat(TruncationPolicy::Tokens(/*limit*/ 256).byte_budget()),
        internal_chat_message_metadata_passthrough: None,
    };
    registry.tool_lifecycle_contributors()[0]
        .on_tool_start(ToolStartInput {
            session_store: &session_store,
            thread_store,
            turn_store: &turn_store,
            turn_id: "turn-1",
            call_id: "call-2",
            tool_name: &tool_name,
            payload: &tool_payload,
            conversation_history: Arc::new(TestConversationHistory(vec![
                latest_compaction,
                oversized_compaction,
            ])),
            source: ToolCallSource::Direct,
        })
        .await;

    let fail_closed_score = thread_store
        .get::<SecurityRiskScore>()
        .expect("an oversized compaction should immediately receive the maximum risk score");
    assert_eq!(
        fail_closed_score.as_ref(),
        &SecurityRiskScore {
            scores: BTreeMap::from([("action_risk".to_owned(), 1.0)]),
            sampled_at: fail_closed_score.sampled_at,
        }
    );
    assert_eq!(
        registry
            .approval_review(&session_store, thread_store, "review action")
            .await,
        None
    );
    assert_eq!(
        server.connections().iter().map(Vec::len).sum::<usize>(),
        1,
        "an oversized latest compaction must bypass Luna rather than reuse stale context"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contributor_bounds_oversized_actions_and_fairly_truncates_nested_fields() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let arguments = json!({
        "attachments": [{
            "content": "🦀\"\\\n".repeat(20_000),
            "name": "financials.csv",
        }],
        "call_id": "untrusted-call",
        "metadata": { "reason": "b".repeat(100_000) },
        "path": "a".repeat(100_000),
        "recipient": "finance@example.com",
        "tool": "untrusted-tool",
    })
    .to_string();
    let (request, _test, _registry) =
        sample_conversation_history(Vec::new(), &arguments, Some(TEST_GUARDIAN_POLICY)).await?;
    let content = request["input"][2]["content"]
        .as_array()
        .expect("Luna user content should contain separate text items");
    let action_text = content[content.len() - 2]["text"]
        .as_str()
        .expect("the current action should be an input text item");
    let action = serde_json::from_str::<serde_json::Value>(action_text)?;
    let max_action_bytes =
        TruncationPolicy::Tokens(DEFAULT_MODEL_CONTEXT_ITEM_TOKENS).byte_budget();
    assert!(action_text.ends_with('\n'));
    assert!(
        action_text.len() <= max_action_bytes,
        "the complete model-visible action must remain bounded"
    );
    assert!(
        action_text.len() >= max_action_bytes * 9 / 10,
        "water-filling should use the available action budget"
    );
    assert_eq!(action["tool"], "read_file");
    assert_eq!(action["call_id"], "untrusted-call");
    assert_eq!(action["recipient"], "finance@example.com");
    assert_eq!(action["attachments"][0]["name"], "financials.csv");
    assert!(action.get("arguments_preview").is_none());
    assert!(action.get("truncated").is_none());
    let retained_values = [
        &action["path"],
        &action["metadata"]["reason"],
        &action["attachments"][0]["content"],
    ]
    .map(|value| {
        value
            .as_str()
            .expect("action string field should remain present")
    });
    for text in retained_values {
        assert!(text.contains("<truncated omitted_approx_tokens=\""));
    }
    let smallest_retained = retained_values.iter().map(|text| text.len()).min().unwrap();
    let largest_retained = retained_values.iter().map(|text| text.len()).max().unwrap();
    assert!(
        largest_retained.saturating_sub(smallest_retained) <= 16,
        "long nested strings should receive comparable shares of the action budget"
    );

    Ok(())
}

#[test]
fn guardian_action_bounds_structurally_oversized_arrays() -> Result<()> {
    let action = super::GuardianAction {
        tool_name: ToolName::plain("inspect_values"),
        payload: ToolPayload::Function {
            arguments: json!({
                "call_id": "genuine-call",
                "tool": "spoofed-tool",
                "values": (0..6_000).collect::<Vec<_>>(),
            })
            .to_string(),
        },
    };

    let rendered = action.render(DEFAULT_MODEL_CONTEXT_ITEM_TOKENS)?;
    assert!(
        rendered.len().saturating_add(1)
            <= TruncationPolicy::Tokens(DEFAULT_MODEL_CONTEXT_ITEM_TOKENS).byte_budget()
    );
    let action = serde_json::from_str::<serde_json::Value>(&rendered)?;
    assert_eq!(
        action,
        json!({
            "_guardian_omitted_fields": 1,
            "call_id": "genuine-call",
            "tool": "inspect_values",
        })
    );

    Ok(())
}

#[test]
fn guardian_action_bounds_structurally_oversized_object_keys() -> Result<()> {
    let oversized_key = "oversized_key_".to_owned()
        + &"k".repeat(TruncationPolicy::Tokens(DEFAULT_MODEL_CONTEXT_ITEM_TOKENS).byte_budget());
    let mut arguments = serde_json::Map::from_iter([
        (
            "_guardian_omitted_fields".to_owned(),
            json!("actual-tool-argument"),
        ),
        ("call_id".to_owned(), json!("genuine-call")),
        ("cmd".to_owned(), json!("remove-important-file")),
        ("tool".to_owned(), json!("spoofed-tool")),
        (oversized_key.clone(), json!(true)),
    ]);
    for index in 0..600 {
        arguments.insert(format!("a_{index:04}_{}", "k".repeat(64)), json!(index));
    }
    let original_field_count = arguments.len();
    let action = super::GuardianAction {
        tool_name: ToolName::plain("inspect_fields"),
        payload: ToolPayload::Function {
            arguments: serde_json::Value::Object(arguments).to_string(),
        },
    };

    let rendered = action.render(DEFAULT_MODEL_CONTEXT_ITEM_TOKENS)?;
    assert!(
        rendered.len().saturating_add(1)
            <= TruncationPolicy::Tokens(DEFAULT_MODEL_CONTEXT_ITEM_TOKENS).byte_budget()
    );
    let action = serde_json::from_str::<serde_json::Value>(&rendered)?;
    let fields = action
        .as_object()
        .expect("the bounded action must remain a JSON object");
    assert_eq!(fields.get("tool"), Some(&json!("inspect_fields")));
    assert_eq!(fields.get("call_id"), Some(&json!("genuine-call")));
    assert_eq!(fields.get("cmd"), Some(&json!("remove-important-file")));
    assert_eq!(
        fields.get("_guardian_omitted_fields"),
        Some(&json!("actual-tool-argument"))
    );
    assert!(fields.len() < original_field_count);
    assert!(!fields.contains_key(&oversized_key));
    assert!(
        fields
            .get("_guardian_omitted_fields_")
            .and_then(serde_json::Value::as_u64)
            .is_some_and(|omitted| omitted > 0)
    );

    Ok(())
}
