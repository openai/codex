use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::SystemTime;

use anyhow::Result;
use codex_core::config::Config;
use codex_core::config::ConfigBuilder;
use codex_core::config::LoaderOverrides;
use codex_core::context::ContextualUserFragment;
use codex_core::context::InternalContextSource;
use codex_core::context::InternalModelContextFragment;
use codex_core::context::NodeReplReviewEvidence;
use codex_extension_api::ConversationHistorySnapshot;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionMetrics;
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
use codex_login::ExternalAuth;
use codex_login::ExternalAuthFuture;
use codex_login::ExternalAuthRefreshContext;
use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::ResponseItemId;
use codex_protocol::ThreadId;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ImageDetail;
use codex_protocol::models::InternalChatMessageMetadataPassthrough;
use codex_protocol::models::LocalShellAction;
use codex_protocol::models::LocalShellExecAction;
use codex_protocol::models::LocalShellStatus;
use codex_protocol::models::MessagePhase;
use codex_protocol::models::ReasoningItemReasoningSummary;
use codex_protocol::openai_models::GuardianScope;
use codex_protocol::openai_models::GuardianV2ModelConfig;
use codex_protocol::openai_models::GuardianV2TranscriptModelConfig;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::TruncationPolicy;
use codex_protocol::security_risk::SecurityRiskScore;
use core_test_support::responses;
use core_test_support::responses::WebSocketConnectionConfig;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::json;

use super::GuardianV2Extension;
use super::GuardianV2ScoreProgress;
use super::ParentCompactionError;
use super::StrictReviewReason;
use super::encrypted_parent_compaction;

use crate::async_scorer::config::CLASSIFICATION_OUTPUT_INSTRUCTIONS;
use crate::async_scorer::config::DEFAULT_MODEL_CONTEXT_ITEM_TOKENS;
use crate::async_scorer::config::DEFAULT_PARENT_COMPACTION_TOKENS;
use crate::async_scorer::config::GuardianV2Config;
use crate::async_scorer::coverage::GuardianPolicy;
use crate::async_scorer::metrics::CLASSIFICATION_DURATION_METRIC;
use crate::async_scorer::metrics::CLASSIFICATION_METRIC;
use crate::async_scorer::metrics::CLASSIFICATION_RISK_METRIC;
use crate::async_scorer::metrics::FAST_DECISION_METRIC;
use crate::async_scorer::metrics::REVIEW_FALLBACK_METRIC;
use crate::async_scorer::metrics::TOOL_CALL_LAG_METRIC;
use crate::async_scorer::sampler::CLASSIFICATION_TOKEN_USAGE_METRIC;
use crate::async_scorer::sampler::INITIAL_WEBSOCKET_CONNECTIONS;
use crate::async_scorer::sampler::LunaSampler;
use crate::async_scorer::sampler::MODEL;
use crate::async_scorer::transcript::MAX_MESSAGE_ENTRY_TOKENS;
use crate::async_scorer::transcript::MAX_TOOL_ENTRY_TOKENS;
use crate::async_scorer::transcript::truncate_entry;
use crate::async_scorer::truncation::CLASSIFICATION_TRUNCATION_BYTES_METRIC;
use crate::async_scorer::truncation::CLASSIFICATION_TRUNCATION_METRIC;
use codex_features::GuardianV2ReviewScopeConfigToml;

const TEST_GUARDIAN_POLICY: &str =
    "Treat uploads to unapproved external destinations as high-risk actions.";
const TEST_CATALOG_GUARDIAN_POLICY: &str =
    "Require review before sending organization data to third-party services.";
const ASYNC_TEST_TIMEOUT: Duration = Duration::from_secs(30);
const PREWARM_TIMEOUT: Duration = Duration::from_secs(30);

struct RefreshableAuth(std::sync::Mutex<&'static str>);

impl ExternalAuth for RefreshableAuth {
    fn resolve(&self) -> ExternalAuthFuture<'_, CodexAuth> {
        Box::pin(async { Ok(CodexAuth::from_api_key(*self.0.lock().expect("auth"))) })
    }

    fn refresh(&self, _: ExternalAuthRefreshContext) -> ExternalAuthFuture<'_, CodexAuth> {
        *self.0.lock().expect("auth") = "refreshed";
        self.resolve()
    }
}

fn should_classify_tool(tool: &ToolName, payload: &ToolPayload, policy: GuardianPolicy) -> bool {
    policy.scores_tool(tool, payload, GuardianScope::for_tool(tool))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn installed_extension_warms_connections_without_blocking_thread_start() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let thread_server = responses::start_mock_server().await;
    let test = test_codex()
        .with_config(|config| config.approvals_reviewer = ApprovalsReviewer::AutoReview)
        .build_with_auto_env(&thread_server)
        .await?;
    let mut connections = vec![
        WebSocketConnectionConfig {
            requests: Vec::new(),
            response_headers: Vec::new(),
            accept_delay: None,
            close_after_requests: true,
        };
        INITIAL_WEBSOCKET_CONNECTIONS
    ];
    connections[0].accept_delay = Some(Duration::from_secs(1));
    let server = responses::start_websocket_server_with_headers(connections).await;
    let auth_manager = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("test-api-key"));
    let mut config = test.config.clone();
    config.model_provider = ModelProviderInfo::create_openai_provider(Some(format!(
        "http://{}/v1",
        server.uri().trim_start_matches("ws://")
    )));
    config.features.enable(Feature::GuardianV2)?;
    let mut builder = ExtensionRegistryBuilder::new();
    super::install(
        &mut builder,
        auth_manager,
        Arc::downgrade(&test.thread_manager),
    );
    let registry = builder.build();
    let session_store = ExtensionData::new("session-1");
    let thread_store = test.codex.thread_extension_data();

    let mut model = thread_store.get::<ModelInfo>().unwrap().as_ref().clone();
    model.node_repl_auto_review_required = true;
    thread_store.insert(model);

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

    assert!(server.handshakes().is_empty());
    assert!(thread_store.get::<LunaSampler>().is_some());
    thread_store
        .get::<LunaSampler>()
        .expect("Guardian v2 should initialize")
        .wait_for_prewarm(PREWARM_TIMEOUT)
        .await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn installed_extension_reconnects_after_auth_refresh() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let thread_server = responses::start_mock_server().await;
    let test = test_codex()
        .with_config(|config| config.approvals_reviewer = ApprovalsReviewer::AutoReview)
        .build_with_auto_env(&thread_server)
        .await?;
    let events = vec![
        ev_assistant_message("sample", "low"),
        ev_completed("response-1"),
    ];
    // Keep the sampled connection open for another request so only auth
    // invalidation, not a server close, forces the next handshake.
    let mut connections = vec![Vec::new(); INITIAL_WEBSOCKET_CONNECTIONS - 1];
    connections.push(vec![events.clone(), events.clone()]);
    connections.push(vec![events]);
    let server = responses::start_websocket_server(connections).await;
    let auth_manager = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("original"));
    auth_manager
        .set_external_auth(Arc::new(RefreshableAuth(std::sync::Mutex::new("original"))))
        .await?;
    let mut config = test.config.clone();
    config.model_provider = ModelProviderInfo::create_openai_provider(Some(format!(
        "http://{}/v1",
        server.uri().trim_start_matches("ws://")
    )));
    config.features.enable(Feature::GuardianV2)?;
    let mut builder = ExtensionRegistryBuilder::new();
    super::install(
        &mut builder,
        auth_manager.clone(),
        Arc::downgrade(&test.thread_manager),
    );
    let registry = builder.build();
    let session_store = ExtensionData::new("session-1");
    let thread_store = test.codex.thread_extension_data();
    let mut model = test
        .thread_manager
        .get_models_manager()
        .get_model_info("gpt-5.5", &config.to_models_manager_config())
        .await;
    model.node_repl_auto_review_required = true;
    thread_store.insert(model);
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
    thread_store
        .get::<LunaSampler>()
        .expect("Guardian v2 should initialize")
        .wait_for_prewarm(PREWARM_TIMEOUT)
        .await?;
    let progress = thread_store
        .get::<GuardianV2ScoreProgress>()
        .expect("Guardian v2 should initialize");
    let turn_store = ExtensionData::new("turn-1");
    let tool_name = ToolName::namespaced("mcp__node_repl__", "js");
    let payload = ToolPayload::Function {
        arguments: r#"{"path":"README.md"}"#.to_owned(),
    };

    for (call_index, call_id) in [(1, "call-1"), (2, "call-2")] {
        if call_index == 2 {
            auth_manager.refresh_token_from_authority().await?;
        }
        registry.tool_lifecycle_contributors()[0]
            .on_tool_start(ToolStartInput {
                session_store: &session_store,
                thread_store,
                turn_store: &turn_store,
                turn_id: "turn-1",
                root_turn_id: None,
                call_id,
                tool_name: &tool_name,
                mcp_tool: None,
                payload: &payload,
                conversation_history: Arc::new(TestConversationHistory(Vec::new())),
                source: ToolCallSource::Direct,
            })
            .await;
        tokio::time::timeout(ASYNC_TEST_TIMEOUT, async {
            while progress.latest_scored_tool_call.load(Ordering::Acquire) < call_index {
                tokio::task::yield_now().await;
            }
        })
        .await?;
        assert_eq!(
            registry
                .fast_approval_decision(
                    &session_store,
                    thread_store,
                    r#"{"tool":"mcp_tool_call","server":"node_repl"}"#,
                    /*extension_metrics*/ None,
                )
                .await,
            Some(ReviewDecision::Approved)
        );
    }

    let mut expected_authorizations =
        vec![Some("Bearer original".to_owned()); INITIAL_WEBSOCKET_CONNECTIONS];
    expected_authorizations.push(Some("Bearer refreshed".to_owned()));
    assert_eq!(
        server
            .handshakes()
            .iter()
            .map(|handshake| handshake.header("authorization"))
            .collect::<Vec<_>>(),
        expected_authorizations
    );

    let mut expected_requests = vec![0; INITIAL_WEBSOCKET_CONNECTIONS - 1];
    expected_requests.extend([1, 1]);
    assert_eq!(
        server
            .connections()
            .iter()
            .map(Vec::len)
            .collect::<Vec<_>>(),
        expected_requests
    );
    let requests = server
        .connections()
        .into_iter()
        .flatten()
        .map(|request| request.body_json())
        .collect::<Vec<_>>();
    for request in &requests {
        responses::assert_parent_turn(request, Some("turn-1"))?;
        responses::assert_root_turn(request, /*expected*/ None)?;
    }
    assert_ne!(
        requests[0]["client_metadata"]["turn_id"],
        requests[1]["client_metadata"]["turn_id"]
    );
    Ok(())
}

#[derive(Clone, Debug, PartialEq)]
enum RecordedMetric {
    Histogram(String, i64, Vec<(String, String)>),
    Counter(String, i64, Vec<(String, String)>),
}

fn fast_decision_metric(decision: &str, reason: &str) -> RecordedMetric {
    RecordedMetric::Counter(
        FAST_DECISION_METRIC.to_owned(),
        1,
        vec![
            ("decision".to_owned(), decision.to_owned()),
            ("reason".to_owned(), reason.to_owned()),
        ],
    )
}

#[derive(Default)]
struct RecordingMetrics(Mutex<Vec<RecordedMetric>>);

impl ExtensionMetrics for RecordingMetrics {
    fn counter(&self, name: &str, inc: i64, tags: &[(&str, &str)]) {
        self.0.lock().unwrap().push(RecordedMetric::Counter(
            name.to_owned(),
            inc,
            tags.iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                .collect(),
        ));
    }

    fn histogram(&self, name: &str, value: i64, tags: &[(&str, &str)]) {
        if name == "codex.guardian_v2.connection.duration_ms" {
            return;
        }
        self.0.lock().unwrap().push(RecordedMetric::Histogram(
            name.to_owned(),
            value,
            tags.iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                .collect(),
        ));
    }
}

fn user_instruction(text: &str) -> ResponseItem {
    ResponseItem::Message {
        id: Some(ResponseItemId::new("msg")),
        role: "user".to_owned(),
        content: vec![ContentItem::InputText {
            text: text.to_owned(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: Some(InternalChatMessageMetadataPassthrough {
            content_item_kinds: Some(vec![codex_protocol::models::ContentItemKind(
                "user.text".to_owned(),
            )]),
            ..Default::default()
        }),
    }
}

struct TestConversationHistory(Vec<ResponseItem>);

struct TestRetainedHistory {
    current: TestConversationHistory,
    retained: Vec<ResponseItem>,
    compaction_model_hash: Option<String>,
}

impl ConversationHistorySnapshot for TestRetainedHistory {
    fn latest_compaction_model_hash(&self) -> Option<&str> {
        self.compaction_model_hash.as_deref()
    }
    fn history_version(&self) -> u64 {
        self.current.history_version()
    }

    fn user_message_revision(&self) -> u64 {
        self.current.user_message_revision()
    }

    fn items(&self) -> Box<dyn Iterator<Item = &ResponseItem> + Send + '_> {
        self.current.items()
    }

    fn review_items(&self) -> Box<dyn Iterator<Item = &ResponseItem> + Send + '_> {
        Box::new(self.retained.iter())
    }
}

impl ConversationHistorySnapshot for TestConversationHistory {
    fn history_version(&self) -> u64 {
        0
    }

    fn user_message_revision(&self) -> u64 {
        self.0.iter().filter(|item| item.is_user_message()).count() as u64
    }

    fn items(&self) -> Box<dyn Iterator<Item = &ResponseItem> + Send + '_> {
        Box::new(self.0.iter())
    }
}

#[test]
fn fail_closed_score_preserves_classification_order() {
    let thread_store = ExtensionData::new("thread-1");
    let newer_sampled_at = SystemTime::UNIX_EPOCH + Duration::from_secs(1);
    let newest_sampled_at = newer_sampled_at + Duration::from_secs(1);
    let newer_score = SecurityRiskScore {
        scores: BTreeMap::from([("action_risk".to_owned(), 0.25)]),
        call_id: None,
        action: None,
        sampled_at: Some(newer_sampled_at.into()),
    };
    thread_store.insert(newer_score.clone());

    GuardianV2Extension::record_fail_closed_score(&thread_store, SystemTime::UNIX_EPOCH);
    assert_eq!(
        thread_store.get::<SecurityRiskScore>().as_deref(),
        Some(&newer_score)
    );

    GuardianV2Extension::record_fail_closed_score(&thread_store, newest_sampled_at);
    let fail_closed_score = SecurityRiskScore {
        scores: BTreeMap::from([("action_risk".to_owned(), 1.0)]),
        call_id: None,
        action: None,
        sampled_at: Some(newest_sampled_at.into()),
    };
    assert!(!thread_store.insert_if(newer_score.clone(), |previous| {
        previous.is_none_or(|previous| previous.sampled_at < newer_score.sampled_at)
    }));
    assert_eq!(
        thread_store.get::<SecurityRiskScore>().as_deref(),
        Some(&fail_closed_score)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sandboxed_shell_classification_respects_review_scope() -> Result<()> {
    let sandboxed = ToolPayload::Function {
        arguments: r#"{"cmd":"pwd"}"#.to_owned(),
    };
    let additional_permissions = ToolPayload::Function {
        arguments: r#"{"cmd":"pwd","sandbox_permissions":"with_additional_permissions"}"#
            .to_owned(),
    };
    let unsandboxed = ToolPayload::Function {
        arguments: r#"{"cmd":"pwd","sandbox_permissions":"require_escalated"}"#.to_owned(),
    };

    let tool_name = ToolName::plain("exec_command");
    let standard_scope = GuardianPolicy::from_legacy(Some(&GuardianV2ReviewScopeConfigToml {
        computer_use_only: Some(false),
        sandboxed_exec_commands: Some(false),
    }));
    assert!(!should_classify_tool(
        &tool_name,
        &sandboxed,
        standard_scope.clone(),
    ));
    assert!(!should_classify_tool(
        &tool_name,
        &additional_permissions,
        standard_scope.clone(),
    ));
    assert!(should_classify_tool(
        &tool_name,
        &unsandboxed,
        standard_scope.clone(),
    ));
    assert!(should_classify_tool(
        &tool_name,
        &sandboxed,
        GuardianPolicy::from_legacy(Some(&GuardianV2ReviewScopeConfigToml {
            computer_use_only: Some(false),
            sandboxed_exec_commands: Some(true),
        })),
    ));
    assert!(should_classify_tool(
        &ToolName::plain("read_file"),
        &sandboxed,
        standard_scope.clone(),
    ));
    assert!(should_classify_tool(
        &ToolName::namespaced("mcp", "exec_command"),
        &sandboxed,
        standard_scope.clone(),
    ));
    skip_if_no_network!(Ok(()));

    let fixture = GuardianFailureFixture::new().await?;
    let thread_store = fixture.test.codex.thread_extension_data();
    let score_progress = thread_store
        .get::<GuardianV2ScoreProgress>()
        .expect("Guardian v2 should track score progress per thread");
    let latest_scored_tool_call = score_progress
        .latest_scored_tool_call
        .load(Ordering::Acquire);
    let turn_store = ExtensionData::new("turn-1");
    let tool_name = ToolName::plain("exec_command");
    let payload = ToolPayload::Function {
        arguments: r#"{"cmd":"pwd"}"#.to_owned(),
    };

    fixture.registry.tool_lifecycle_contributors()[0]
        .on_tool_start(ToolStartInput {
            session_store: &fixture.session_store,
            thread_store,
            turn_store: &turn_store,
            turn_id: "turn-1",
            root_turn_id: None,
            call_id: "call-2",
            tool_name: &tool_name,
            mcp_tool: None,
            payload: &payload,
            conversation_history: Arc::new(TestConversationHistory(Vec::new())),
            source: ToolCallSource::Direct,
        })
        .await;

    assert_eq!(
        score_progress.latest_tool_call.load(Ordering::Acquire),
        latest_scored_tool_call + 1
    );
    assert_eq!(
        score_progress
            .latest_scored_tool_call
            .load(Ordering::Acquire),
        latest_scored_tool_call
    );
    Ok(())
}

#[test]
fn computer_use_only_classification_recognizes_direct_and_code_mode_tools() {
    let payload = ToolPayload::Function {
        arguments: r#"{"code":"await browser.goto('https://example.com')"}"#.to_owned(),
    };
    for (tool_name, expected) in [
        (ToolName::namespaced("mcp__node_repl__", "js"), true),
        (ToolName::namespaced("mcp__cua_repl__", "js"), true),
        (ToolName::plain("mcp__node_repl__js"), true),
        (ToolName::plain("mcp__cua_repl__js"), true),
        (ToolName::namespaced("mcp__ordinary__", "js"), false),
        (ToolName::plain("read_file"), false),
        (ToolName::plain("exec_command"), false),
    ] {
        assert_eq!(
            should_classify_tool(
                &tool_name,
                &payload,
                GuardianPolicy::from_legacy(/*scope*/ None)
            ),
            expected,
            "unexpected classification scope for {tool_name}"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn computer_use_only_scores_cannot_approve_other_actions() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let fixture = GuardianFailureFixture::new().await?;
    let thread_store = fixture.test.codex.thread_extension_data();
    let mut model = thread_store.get::<ModelInfo>().unwrap().as_ref().clone();
    model.node_repl_auto_review_required = true;
    thread_store.insert(model);
    let mut config = thread_store
        .get::<crate::async_scorer::config::GuardianV2Config>()
        .expect("Guardian v2 should have initialized")
        .as_ref()
        .clone();
    config.policy = GuardianPolicy::from_legacy(/*scope*/ None);
    thread_store.insert(config);
    thread_store.insert(SecurityRiskScore {
        scores: BTreeMap::from([("action_risk".to_owned(), 0.25)]),
        call_id: None,
        action: None,
        sampled_at: None,
    });
    thread_store.insert(RecordingMetrics::default());
    let progress = thread_store
        .get::<GuardianV2ScoreProgress>()
        .expect("Guardian v2 should track score progress per thread");
    // The seeded low score belongs to the model selected above.
    let authorization =
        super::super::authorization::ScoreAuthorization::current(&fixture.test.codex).await;
    *progress.authorization.lock().unwrap() = Some(authorization);
    let latest_tool_call = progress.latest_tool_call.load(Ordering::Acquire);
    let turn_store = ExtensionData::new("turn-1");
    let ordinary_tool = ToolName::namespaced("mcp__ordinary__", "write_record");
    let payload = ToolPayload::Function {
        arguments: r#"{"record":"sensitive"}"#.to_owned(),
    };
    fixture.registry.tool_lifecycle_contributors()[0]
        .on_tool_start(ToolStartInput {
            session_store: &fixture.session_store,
            thread_store,
            turn_store: &turn_store,
            turn_id: "turn-1",
            root_turn_id: None,
            call_id: "ordinary-call",
            tool_name: &ordinary_tool,
            mcp_tool: None,
            payload: &payload,
            conversation_history: Arc::new(TestConversationHistory(Vec::new())),
            source: ToolCallSource::CodeMode {
                cell_id: "cell-1".to_owned(),
                runtime_tool_call_id: "nested-1".to_owned(),
            },
        })
        .await;
    assert_eq!(
        progress.latest_tool_call.load(Ordering::Acquire),
        latest_tool_call,
        "unrelated code-mode calls must not age browser/CUA scores"
    );

    for (action, expected) in [
        (
            json!({"tool": "mcp_tool_call", "server": "node_repl", "tool_name": "js"}),
            Some(ReviewDecision::Approved),
        ),
        (
            json!({"tool": "mcp_tool_call", "server": "cua_repl", "tool_name": "js"}),
            Some(ReviewDecision::Approved),
        ),
        (
            json!({"tool": "mcp_tool_call", "server": "ordinary", "tool_name": "js"}),
            None,
        ),
        (json!({"tool": "exec_command", "server": "node_repl"}), None),
    ] {
        let prompt = action.to_string();
        assert_eq!(
            fixture
                .registry
                .fast_approval_decision(
                    &fixture.session_store,
                    thread_store,
                    &prompt,
                    thread_store
                        .get::<RecordingMetrics>()
                        .map(|metrics| metrics as Arc<dyn ExtensionMetrics>),
                )
                .await,
            expected,
            "unexpected fast approval for {action}"
        );
    }
    assert_eq!(
        fixture
            .registry
            .fast_approval_decision(
                &fixture.session_store,
                thread_store,
                "not valid JSON",
                thread_store
                    .get::<RecordingMetrics>()
                    .map(|metrics| metrics as Arc<dyn ExtensionMetrics>),
            )
            .await,
        None,
        "malformed approval actions must not reuse a browser score"
    );
    let mut changed_model = fixture
        .test
        .thread_manager
        .get_models_manager()
        .get_model_info("gpt-5.5", &fixture.test.config.to_models_manager_config())
        .await;
    changed_model.guardian = Some(codex_protocol::openai_models::GuardianModelPolicy {
        computer_use: Some(codex_protocol::openai_models::GuardianReviewMode::Adaptive),
        ..Default::default()
    });
    thread_store.insert(changed_model);
    assert_eq!(
        fixture
            .registry
            .fast_approval_decision(
                &fixture.session_store,
                thread_store,
                &json!({"tool": "mcp_tool_call", "server": "node_repl", "tool_name": "js"})
                    .to_string(),
                /*extension_metrics*/ None,
            )
            .await,
        None,
        "a score from the previous model policy must not approve a call"
    );
    let fast_decisions = thread_store
        .get::<RecordingMetrics>()
        .unwrap()
        .0
        .lock()
        .unwrap()
        .iter()
        .filter(|sample| {
            matches!(
                sample,
                RecordedMetric::Counter(name, 1, _) if name == FAST_DECISION_METRIC
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        fast_decisions,
        vec![
            fast_decision_metric("approved", "low_risk"),
            fast_decision_metric("approved", "low_risk"),
            fast_decision_metric("deferred", "out_of_scope"),
            fast_decision_metric("deferred", "out_of_scope"),
            fast_decision_metric("deferred", "out_of_scope"),
        ]
    );

    let mut model = thread_store.get::<ModelInfo>().unwrap().as_ref().clone();
    model.guardian = None; // Exercise the legacy fallback after the catalog case above.
    model.node_repl_auto_review_required = false;
    thread_store.insert(model.clone());
    fixture
        .score_tool(ToolName::namespaced("mcp__node_repl__", "js"))
        .await;
    assert!(
        progress.latest_failed_tool_call.load(Ordering::Acquire)
            > progress.latest_scored_tool_call.load(Ordering::Acquire)
    );
    for required in [false, true] {
        model.node_repl_auto_review_required = required;
        thread_store.insert(model.clone());
        // Also reject a low score published by an older, in-flight classifier.
        thread_store.insert(SecurityRiskScore {
            scores: BTreeMap::from([("action_risk".to_owned(), 0.0)]),
            call_id: None,
            action: None,
            sampled_at: None,
        });
        assert_eq!(
            fixture
                .registry
                .fast_approval_decision(
                    &fixture.session_store,
                    thread_store,
                    r#"{"tool":"mcp_tool_call","server":"node_repl","tool_name":"js"}"#,
                    /*extension_metrics*/ None,
                )
                .await,
            None,
            "switching back to a reviewed model must not revive a skipped score"
        );
    }

    Ok(())
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
        Ok(Some(latest.clone()))
    );
    assert_eq!(
        encrypted_parent_compaction(
            [&latest, &older].into_iter(),
            DEFAULT_PARENT_COMPACTION_TOKENS,
        ),
        Ok(Some(older))
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
            Err(ParentCompactionError::Unusable),
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
            Ok(Some(item.clone()))
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
            Err(ParentCompactionError::Oversized),
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
        Err(ParentCompactionError::Oversized),
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
    sample_configured_conversation_history_with_source(
        conversation_history,
        arguments,
        guardian_policy,
        guardian_config,
        model_defaults,
        ToolCallSource::Direct,
    )
    .await
}

async fn sample_configured_conversation_history_with_source(
    conversation_history: Vec<ResponseItem>,
    arguments: &str,
    guardian_policy: Option<&str>,
    guardian_config: &str,
    model_defaults: Option<GuardianV2ModelConfig>,
    source: ToolCallSource,
) -> Result<(serde_json::Value, TestCodex, ExtensionRegistry<Config>)> {
    let thread_server = responses::start_mock_server().await;
    let guardian_policy = guardian_policy.map(str::to_owned);
    let guardian_config = format!(
        "{guardian_config}\n[features.guardianv2.review_scope]\ncomputer_use_only = false\n"
    );
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
        .with_config(move |config| {
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            config.guardian_policy_config = guardian_policy;
        })
        .with_pre_build_hook(move |home| {
            std::fs::write(home.join("config.toml"), guardian_config)
                .expect("Guardian v2 configuration should be written");
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
    let mut completed = ev_completed("response-1");
    completed["response"]["usage"] = json!({
        "input_tokens": 120,
        "input_tokens_details": {"cached_tokens": 40, "cache_write_tokens": 20},
        "output_tokens": 30,
        "output_tokens_details": {"reasoning_tokens": 10},
        "total_tokens": 150,
    });
    let events = vec![ev_assistant_message("sample", "high"), completed];
    let mut connections = vec![Vec::new(); INITIAL_WEBSOCKET_CONNECTIONS - 1];
    connections.push(vec![events]);
    let server = responses::start_websocket_server(connections).await;
    let provider_info = ModelProviderInfo::create_openai_provider(Some(format!(
        "http://{}/v1",
        server.uri().trim_start_matches("ws://")
    )));
    let auth_manager = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("test-api-key"));
    let mut config = test.config.clone();
    config.model_provider = provider_info;
    config.features.enable(Feature::GuardianV2)?;
    let mut builder = ExtensionRegistryBuilder::new();
    super::install(
        &mut builder,
        auth_manager,
        Arc::downgrade(&test.thread_manager),
    );
    let registry = builder.build();
    let session_store = ExtensionData::new("session-1");
    let thread_store = test.codex.thread_extension_data();
    thread_store.insert(RecordingMetrics::default());
    let metrics = thread_store.get::<RecordingMetrics>().unwrap();
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
            .fast_approval_decision(
                &session_store,
                thread_store,
                "review action",
                /*extension_metrics*/ None,
            )
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
            extension_metrics: Some(metrics),
            session_store: &session_store,
            thread_store,
        })
        .await;
    thread_store
        .get::<LunaSampler>()
        .expect("Guardian v2 should initialize")
        .wait_for_prewarm(PREWARM_TIMEOUT)
        .await?;
    let turn_store = ExtensionData::new("turn-1");
    let tool_name = ToolName::plain("read_file");
    let tool_payload = ToolPayload::Function {
        arguments: arguments.to_owned(),
    };
    if !conversation_history.is_empty() {
        Box::pin(
            test.codex
                .inject_response_items(conversation_history.clone()),
        )
        .await?;
    }
    let conversation_history = test.codex.conversation_history_snapshot().await;

    registry.tool_lifecycle_contributors()[0]
        .on_tool_start(ToolStartInput {
            session_store: &session_store,
            thread_store,
            turn_store: &turn_store,
            turn_id: "turn-1",
            root_turn_id: Some("root-turn"),
            call_id: "call-1",
            tool_name: &tool_name,
            mcp_tool: None,
            payload: &tool_payload,
            conversation_history,
            source,
        })
        .await;

    let request = tokio::time::timeout(
        ASYNC_TEST_TIMEOUT,
        server.wait_for_request(
            /*connection_index*/ INITIAL_WEBSOCKET_CONNECTIONS - 1,
            /*request_index*/ 0,
        ),
    )
    .await?;
    Ok((request.body_json(), test, registry))
}

struct GuardianFailureFixture {
    test: TestCodex,
    registry: ExtensionRegistry<Config>,
    session_store: ExtensionData,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn disabled_code_mode_invalidates_cached_scores() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let fixture = GuardianFailureFixture::new().await?;
    let thread_store = fixture.test.codex.thread_extension_data();
    let mut model = thread_store
        .get::<codex_protocol::openai_models::ModelInfo>()
        .expect("resolved model")
        .as_ref()
        .clone();
    model.guardian = Some(codex_protocol::openai_models::GuardianModelPolicy {
        computer_use: Some(codex_protocol::openai_models::GuardianReviewMode::Adaptive),
        ..Default::default()
    });
    thread_store.insert(model);
    let progress = thread_store
        .get::<GuardianV2ScoreProgress>()
        .expect("score progress");
    let before = (
        progress.latest_tool_call.load(Ordering::Acquire),
        progress.latest_failed_tool_call.load(Ordering::Acquire),
    );
    fixture.score_tool(ToolName::plain("wait")).await;
    assert_eq!(
        (
            progress.latest_tool_call.load(Ordering::Acquire),
            progress.latest_failed_tool_call.load(Ordering::Acquire),
        ),
        before,
    );
    fixture.score_tool(ToolName::plain("exec")).await;
    assert_eq!(
        (
            progress.latest_tool_call.load(Ordering::Acquire),
            progress.latest_failed_tool_call.load(Ordering::Acquire),
        ),
        (before.0 + 1, before.0 + 1),
    );
    // An MCP tool with the same name remains in the MCP category.
    fixture
        .score_tool(ToolName::namespaced("mcp__ordinary", "wait"))
        .await;
    assert_eq!(
        (
            progress.latest_tool_call.load(Ordering::Acquire),
            progress.latest_failed_tool_call.load(Ordering::Acquire),
        ),
        (before.0 + 2, before.0 + 2),
    );
    Ok(())
}

impl GuardianFailureFixture {
    async fn new() -> Result<Self> {
        Self::with_config("").await
    }

    async fn with_config(guardian_config: &str) -> Result<Self> {
        let (_, test, registry) = sample_configured_conversation_history(
            Vec::new(),
            r#"{"path":"README.md"}"#,
            Some(TEST_GUARDIAN_POLICY),
            guardian_config,
            /*model_defaults*/ None,
        )
        .await?;
        let thread_store = test.codex.thread_extension_data();
        let score_progress = thread_store
            .get::<GuardianV2ScoreProgress>()
            .expect("Guardian v2 should track score progress per thread");
        tokio::time::timeout(ASYNC_TEST_TIMEOUT, async {
            while thread_store.get::<SecurityRiskScore>().is_none()
                || score_progress
                    .latest_scored_tool_call
                    .load(Ordering::Acquire)
                    < score_progress.latest_tool_call.load(Ordering::Acquire)
            {
                tokio::task::yield_now().await;
            }
        })
        .await?;

        Ok(Self {
            test,
            registry,
            session_store: ExtensionData::new("session-1"),
        })
    }

    async fn score_tool(&self, tool_name: ToolName) {
        let thread_store = self.test.codex.thread_extension_data();
        thread_store.insert(SecurityRiskScore {
            scores: BTreeMap::from([("action_risk".to_owned(), 0.25)]),
            call_id: None,
            action: None,
            sampled_at: None,
        });
        let turn_store = ExtensionData::new("turn-1");
        let payload = ToolPayload::Function {
            arguments: r#"{"path":"README.md"}"#.to_owned(),
        };
        self.registry.tool_lifecycle_contributors()[0]
            .on_tool_start(ToolStartInput {
                session_store: &self.session_store,
                thread_store,
                turn_store: &turn_store,
                turn_id: "turn-1",
                root_turn_id: None,
                call_id: "call-1",
                tool_name: &tool_name,
                mcp_tool: None,
                payload: &payload,
                conversation_history: Arc::new(TestConversationHistory(Vec::new())),
                source: ToolCallSource::Direct,
            })
            .await;
    }

    async fn assert_fails_closed(&self, expected_reason: &str) -> Result<()> {
        let thread_store = self.test.codex.thread_extension_data();
        let score_progress = thread_store
            .get::<GuardianV2ScoreProgress>()
            .expect("Guardian v2 should track score progress per thread");
        tokio::time::timeout(ASYNC_TEST_TIMEOUT, async {
            while thread_store
                .get::<SecurityRiskScore>()
                .is_none_or(|score| score.scores.get("action_risk") != Some(&1.0))
                && score_progress
                    .latest_failed_tool_call
                    .load(Ordering::Acquire)
                    <= score_progress
                        .latest_scored_tool_call
                        .load(Ordering::Acquire)
            {
                tokio::task::yield_now().await;
            }
        })
        .await?;
        thread_store.insert(RecordingMetrics::default());
        assert_eq!(
            self.registry
                .fast_approval_decision(
                    &self.session_store,
                    thread_store,
                    "review action",
                    thread_store
                        .get::<RecordingMetrics>()
                        .map(|metrics| metrics as Arc<dyn ExtensionMetrics>),
                )
                .await,
            None
        );
        assert!(
            thread_store
                .get::<RecordingMetrics>()
                .unwrap()
                .0
                .lock()
                .unwrap()
                .iter()
                .any(|sample| sample == &fast_decision_metric("deferred", expected_reason))
        );
        Ok(())
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contributor_fails_closed_when_thread_lookup_fails() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let fixture = GuardianFailureFixture::new().await?;
    fixture
        .test
        .thread_manager
        .remove_thread(&fixture.test.session_configured.thread_id)
        .await
        .expect("the test thread should exist before simulating a failed lookup");

    fixture.score_tool(ToolName::plain("read_file")).await;
    fixture.assert_fails_closed("scoring_failure").await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contributor_fails_closed_when_model_configuration_is_invalid() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let fixture = GuardianFailureFixture::new().await?;
    let mut parent_model = fixture
        .test
        .thread_manager
        .get_models_manager()
        .get_model_info("gpt-5.5", &fixture.test.config.to_models_manager_config())
        .await;
    parent_model
        .model_messages
        .as_mut()
        .expect("test model should expose model messages")
        .guardian_v2 = Some(GuardianV2ModelConfig {
        max_action_tokens: Some(1),
        ..Default::default()
    });
    fixture
        .test
        .codex
        .thread_extension_data()
        .insert(parent_model);

    fixture.score_tool(ToolName::plain("read_file")).await;
    fixture.assert_fails_closed("elevated_risk").await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contributor_fails_closed_when_action_serialization_fails() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let fixture = GuardianFailureFixture::new().await?;
    let oversized_tool_name = ToolName::plain(
        "x".repeat(TruncationPolicy::Tokens(DEFAULT_MODEL_CONTEXT_ITEM_TOKENS).byte_budget()),
    );

    fixture.score_tool(oversized_tool_name).await;
    fixture.assert_fails_closed("elevated_risk").await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contributor_fails_closed_when_luna_classification_fails() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let fixture = GuardianFailureFixture::new().await?;
    let invalid_score = vec![
        ev_assistant_message("sample", "invalid"),
        ev_completed("response-invalid"),
    ];
    let mut connections = vec![Vec::new(); INITIAL_WEBSOCKET_CONNECTIONS - 1];
    connections.push(vec![invalid_score]);
    let server = responses::start_websocket_server(connections).await;
    let mut config = fixture.test.config.clone();
    config.model_provider = ModelProviderInfo::create_openai_provider(Some(format!(
        "http://{}/v1",
        server.uri().trim_start_matches("ws://")
    )));
    config.features.enable(Feature::GuardianV2)?;
    fixture.registry.thread_lifecycle_contributors()[0]
        .on_thread_start(ThreadStartInput {
            config: &config,
            session_source: &SessionSource::Exec,
            persistent_thread_state_available: false,
            environments: &[],
            mcp_resource_client: None,
            extension_metrics: None,
            session_store: &fixture.session_store,
            thread_store: fixture.test.codex.thread_extension_data(),
        })
        .await;
    fixture
        .test
        .codex
        .thread_extension_data()
        .get::<LunaSampler>()
        .expect("Guardian v2 should initialize")
        .wait_for_prewarm(PREWARM_TIMEOUT)
        .await?;

    fixture.score_tool(ToolName::plain("read_file")).await;
    fixture.assert_fails_closed("elevated_risk").await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contributor_renders_policy_inside_a_configured_prompt() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let configuration = r#"
[features.guardianv2]
enabled = true
classifier_instructions = "Predict future violations.\n# Security Policy\n{{ tenant_policy_config }}\nReturn action_risk."
"#;
    let (request, _test, _registry) = sample_configured_conversation_history(
        Vec::new(),
        r#"{"path":"README.md"}"#,
        Some(TEST_GUARDIAN_POLICY),
        configuration,
        /*model_defaults*/ None,
    )
    .await?;

    assert_eq!(
        request["input"][1],
        json!({
            "type": "message",
            "id": request["input"][1]["id"],
            "role": "developer",
            "content": [{
                "type": "input_text",
                "text": format!(
                    "Predict future violations.\n# Security Policy\n{TEST_GUARDIAN_POLICY}\nReturn action_risk.\n\n{CLASSIFICATION_OUTPUT_INSTRUCTIONS}"
                ),
            }],
        })
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contributor_truncates_legacy_prompt_after_appending_policy() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let template = "legacy instructions ".repeat(200);
    let configuration = format!(
        r#"
[features.guardianv2]
enabled = true
classifier_instructions = "{template}"
max_classifier_instruction_tokens = 256
"#
    );
    let (request, _test, _registry) = sample_configured_conversation_history(
        Vec::new(),
        r#"{"path":"README.md"}"#,
        Some(TEST_GUARDIAN_POLICY),
        &configuration,
        /*model_defaults*/ None,
    )
    .await?;

    assert_eq!(
        request["input"][1],
        json!({
            "type": "message",
            "id": request["input"][1]["id"],
            "role": "developer",
            "content": [{
                "type": "input_text",
                "text": truncate_entry(
                    &format!(
                        "{template}\n\n# Security Policy\n{TEST_GUARDIAN_POLICY}\n\n{CLASSIFICATION_OUTPUT_INSTRUCTIONS}"
                    ),
                    /*max_tokens*/ 256,
                ),
            }],
        })
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contributor_uses_configured_prompt_and_expires_scores_at_default_lag() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let configuration = r#"
[features.guardianv2]
enabled = true
classifier_instructions = "Use the experimental security classification prompt."
review_threshold = 0.60
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
        user_instruction("Review the pending action."),
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
            call_id: Some("previous-call".to_owned()),
            name: None,
            namespace: None,
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
            "id": request["input"][1]["id"],
            "role": "developer",
            "content": [{
                "type": "input_text",
                "text": format!(
                    "Use the experimental security classification prompt.\n\n# Security Policy\n{TEST_GUARDIAN_POLICY}\n\n{CLASSIFICATION_OUTPUT_INSTRUCTIONS}"
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
    let original_action_bytes = i64::try_from(
        serde_json::to_string_pretty(&json!({
            "body": "x".repeat(4_000),
            "tool": "read_file",
        }))?
        .len(),
    )?;
    let action: serde_json::Value = serde_json::from_str(action)?;
    let retained_action_bytes = i64::try_from(serde_json::to_string_pretty(&action)?.len())?;
    assert_eq!(action["tool"], "read_file");

    let session_store = ExtensionData::new("session-1");
    let thread_store = test.codex.thread_extension_data();
    let score_progress = thread_store
        .get::<GuardianV2ScoreProgress>()
        .expect("Guardian v2 should track score progress per thread");
    let metrics = thread_store.get::<RecordingMetrics>().unwrap();
    tokio::time::timeout(ASYNC_TEST_TIMEOUT, async {
        while score_progress
            .latest_scored_tool_call
            .load(Ordering::Acquire)
            == 0
            || metrics.0.lock().unwrap().len() < 14
        {
            tokio::task::yield_now().await;
        }
    })
    .await?;
    assert_eq!(thread_store.get::<StrictReviewReason>(), None);
    thread_store.insert(SecurityRiskScore {
        scores: BTreeMap::from([("action_risk".to_owned(), 0.65)]),
        call_id: None,
        action: None,
        sampled_at: None,
    });
    assert_eq!(
        registry
            .fast_approval_decision(
                &session_store,
                thread_store,
                "review action",
                thread_store
                    .get::<RecordingMetrics>()
                    .map(|metrics| metrics as Arc<dyn ExtensionMetrics>),
            )
            .await,
        None
    );
    assert_eq!(
        thread_store.remove::<StrictReviewReason>().as_deref(),
        Some(&StrictReviewReason::ElevatedRisk)
    );
    thread_store.insert(SecurityRiskScore {
        scores: BTreeMap::from([("action_risk".to_owned(), 0.55)]),
        call_id: None,
        action: None,
        sampled_at: None,
    });
    assert_eq!(
        registry
            .fast_approval_decision(
                &session_store,
                thread_store,
                "review action",
                thread_store
                    .get::<RecordingMetrics>()
                    .map(|metrics| metrics as Arc<dyn ExtensionMetrics>),
            )
            .await,
        Some(ReviewDecision::Approved)
    );

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
            .fast_approval_decision(
                &session_store,
                thread_store,
                "review action",
                thread_store
                    .get::<RecordingMetrics>()
                    .map(|metrics| metrics as Arc<dyn ExtensionMetrics>),
            )
            .await,
        Some(ReviewDecision::Approved)
    );

    let initial_metrics = thread_store.get::<RecordingMetrics>().unwrap();
    thread_store.insert(RecordingMetrics::default());
    score_progress
        .latest_tool_call
        .store(/*val*/ 4, Ordering::Release);
    assert_eq!(
        registry
            .fast_approval_decision(
                &session_store,
                thread_store,
                "review action",
                thread_store
                    .get::<RecordingMetrics>()
                    .map(|metrics| metrics as Arc<dyn ExtensionMetrics>),
            )
            .await,
        None
    );
    assert_eq!(
        thread_store.remove::<StrictReviewReason>().as_deref(),
        Some(&StrictReviewReason::StaleScore)
    );

    score_progress
        .latest_scored_tool_call
        .store(/*val*/ 2, Ordering::Release);
    assert_eq!(
        registry
            .fast_approval_decision(
                &session_store,
                thread_store,
                "review action",
                thread_store
                    .get::<RecordingMetrics>()
                    .map(|metrics| metrics as Arc<dyn ExtensionMetrics>),
            )
            .await,
        Some(ReviewDecision::Approved)
    );

    let samples = initial_metrics.0.lock().unwrap();
    let classification_duration_ms = match &samples[9] {
        RecordedMetric::Histogram(name, duration_ms, _)
            if name == CLASSIFICATION_DURATION_METRIC =>
        {
            *duration_ms
        }
        sample => panic!("expected classification duration metric, got {sample:?}"),
    };
    assert_eq!(
        *samples,
        [
            ("total", 150),
            ("input", 120),
            ("cached_input", 40),
            ("cache_write_input", 20),
            ("non_cached_input", 80),
            ("output", 30),
            ("reasoning_output", 10),
        ]
        .into_iter()
        .map(|(token_type, value)| {
            RecordedMetric::Histogram(
                CLASSIFICATION_TOKEN_USAGE_METRIC.to_owned(),
                value,
                vec![("token_type".to_owned(), token_type.to_owned())],
            )
        })
        .chain([
            RecordedMetric::Counter(
                CLASSIFICATION_RISK_METRIC.to_owned(),
                1,
                vec![("risk_level".to_owned(), "high".to_owned())],
            ),
            RecordedMetric::Counter(
                CLASSIFICATION_METRIC.to_owned(),
                1,
                vec![("outcome".to_owned(), "success".to_owned())],
            ),
            RecordedMetric::Histogram(
                CLASSIFICATION_DURATION_METRIC.to_owned(),
                classification_duration_ms,
                vec![("outcome".to_owned(), "success".to_owned())],
            ),
            RecordedMetric::Counter(
                CLASSIFICATION_TRUNCATION_METRIC.to_owned(),
                1,
                vec![
                    ("component".to_owned(), "action".to_owned()),
                    ("disposition".to_owned(), "truncated".to_owned()),
                ],
            ),
        ])
        .chain(
            [
                ("original", original_action_bytes),
                ("retained", retained_action_bytes),
                ("omitted", original_action_bytes - retained_action_bytes),
            ]
            .into_iter()
            .map(|(measurement, bytes)| {
                RecordedMetric::Histogram(
                    CLASSIFICATION_TRUNCATION_BYTES_METRIC.to_owned(),
                    bytes,
                    vec![
                        ("component".to_owned(), "action".to_owned()),
                        ("disposition".to_owned(), "truncated".to_owned()),
                        ("measurement".to_owned(), measurement.to_owned()),
                    ],
                )
            }),
        )
        .chain([
            RecordedMetric::Histogram(TOOL_CALL_LAG_METRIC.to_owned(), 0, vec![]),
            fast_decision_metric("deferred", "elevated_risk"),
            RecordedMetric::Histogram(TOOL_CALL_LAG_METRIC.to_owned(), 0, vec![]),
            fast_decision_metric("approved", "low_risk"),
            RecordedMetric::Histogram(TOOL_CALL_LAG_METRIC.to_owned(), 2, vec![]),
            fast_decision_metric("approved", "low_risk"),
        ])
        .collect::<Vec<_>>()
    );
    let metrics = thread_store.get::<RecordingMetrics>().unwrap();
    assert_eq!(
        *metrics.0.lock().unwrap(),
        vec![
            RecordedMetric::Histogram(TOOL_CALL_LAG_METRIC.to_owned(), 3, vec![]),
            RecordedMetric::Counter(
                REVIEW_FALLBACK_METRIC.to_owned(),
                1,
                vec![("fallback_reason".to_owned(), "score_lag".to_owned())],
            ),
            fast_decision_metric("deferred", "stale_score"),
            RecordedMetric::Histogram(TOOL_CALL_LAG_METRIC.to_owned(), 2, vec![]),
            fast_decision_metric("approved", "low_risk"),
        ]
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contributor_includes_transcript_images_by_default() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let user_image = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGPgEpEDAABoAD1UCKP3AAAAAElFTkSuQmCC";
    let tool_image = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGOQE+ECAACQAD304kFaAAAAAElFTkSuQmCC";
    let history = vec![
        ResponseItem::Message {
            id: None,
            role: "user".to_owned(),
            content: vec![
                ContentItem::InputText {
                    text: "Review what is shown on screen.".to_owned(),
                },
                ContentItem::InputImage {
                    image_url: user_image.to_owned(),
                    detail: Some(ImageDetail::High),
                },
            ],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::FunctionCall {
            id: None,
            name: "screenshot".to_owned(),
            namespace: None,
            arguments: "{}".to_owned(),
            encrypted_function_args: None,
            call_id: "previous-call".to_owned(),
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::FunctionCallOutput {
            id: None,
            call_id: Some("previous-call".to_owned()),
            name: None,
            namespace: None,
            output: FunctionCallOutputPayload::from_content_items(vec![
                FunctionCallOutputContentItem::InputText {
                    text: "Screenshot captured.".to_owned(),
                },
                FunctionCallOutputContentItem::InputImage {
                    image_url: tool_image.to_owned(),
                    detail: Some(ImageDetail::High),
                },
            ]),
            internal_chat_message_metadata_passthrough: None,
        },
    ];
    let configuration = r#"
[features.guardianv2]
enabled = true
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
                "image_url": user_image,
            }),
            json!({
                "type": "input_image",
                "image_url": tool_image,
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
        max_tool_call_lag: Some(2),
        reasoning_effort: Some(ReasoningEffort::Minimal),
        transcript: Some(GuardianV2TranscriptModelConfig {
            sources: Some(vec!["reasoning".to_owned()]),
            include_images: Some(true),
            max_message_entry_tokens: Some(128),
            max_message_transcript_tokens: Some(256),
            ..Default::default()
        }),
        max_action_tokens: Some(128),
        max_classifier_instruction_tokens: Some(256),
        reuse_parent_compaction: Some(false),
        max_parent_compaction_tokens: Some(384),
    };
    let conversation_history = vec![
        user_instruction("Review the pending action."),
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
            "id": request["input"][1]["id"],
            "role": "developer",
            "content": [{
                "type": "input_text",
                "text": format!(
                    "Use the experimental model-owned prompt.\n\n# Security Policy\n{TEST_GUARDIAN_POLICY}\n\n{CLASSIFICATION_OUTPUT_INSTRUCTIONS}"
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
    let guardian_config = thread_store
        .get::<crate::async_scorer::config::GuardianV2Config>()
        .expect("Guardian v2 configuration should be installed");
    assert_eq!(
        (
            guardian_config.max_tool_call_lag,
            guardian_config.reuse_parent_compaction,
            guardian_config.max_parent_compaction_tokens,
            guardian_config.transcript.include_images,
        ),
        (2, false, 384, true)
    );
    assert!(thread_store.get::<NodeReplReviewEvidence>().is_some());
    let score = tokio::time::timeout(ASYNC_TEST_TIMEOUT, async {
        loop {
            if let Some(score) = thread_store.get::<SecurityRiskScore>() {
                return score;
            }
            tokio::task::yield_now().await;
        }
    })
    .await?;
    assert_eq!(
        score.action,
        Some(serde_json::from_str::<serde_json::Value>(action)?)
    );
    thread_store.insert(SecurityRiskScore {
        scores: BTreeMap::from([("action_risk".to_owned(), 0.65)]),
        call_id: None,
        action: None,
        sampled_at: None,
    });
    assert_eq!(
        registry
            .fast_approval_decision(
                &session_store,
                thread_store,
                "review action",
                /*extension_metrics*/ None,
            )
            .await,
        Some(ReviewDecision::Approved)
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contributor_samples_tool_calls_with_the_existing_luna_pool() -> Result<()> {
    assert_luna_pool_context(/*thread_context_enabled*/ true).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_contributor_samples_tool_calls_with_the_existing_luna_pool() -> Result<()> {
    assert_luna_pool_context(/*thread_context_enabled*/ false).await
}

async fn assert_luna_pool_context(thread_context_enabled: bool) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let conversation_history = vec![
        user_instruction("Inspect the repository guidelines."),
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
            call_id: Some("previous-call".to_owned()),
            name: None,
            namespace: None,
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
    let (request, test, registry) = sample_configured_conversation_history(
        conversation_history,
        r#"{"path":"README.md"}"#,
        Some(TEST_GUARDIAN_POLICY),
        &format!("[features]\nguardian_thread_context = {thread_context_enabled}\n"),
        /*model_defaults*/ None,
    )
    .await?;
    let thread_id = test.session_configured.thread_id;
    let session_store = ExtensionData::new("session-1");
    let thread_store = test.codex.thread_extension_data();
    assert_eq!(request["model"], "gpt-5.6-luna");
    let classifier_thread_id = request["client_metadata"]["thread_id"]
        .as_str()
        .expect("classifier thread ID");
    assert_ne!(ThreadId::from_string(classifier_thread_id)?, thread_id);
    let classifier_turn_id = request["client_metadata"]["turn_id"]
        .as_str()
        .expect("classifier turn ID");
    let turn_metadata: serde_json::Value = serde_json::from_str(
        request["client_metadata"]["x-codex-turn-metadata"]
            .as_str()
            .expect("serialized turn metadata"),
    )?;
    assert_eq!(
        turn_metadata,
        json!({
            "session_id": request["client_metadata"]["session_id"],
            "thread_id": classifier_thread_id,
            "guardian_classifier_source_thread_id": thread_id.to_string(),
            "turn_id": classifier_turn_id,
            "parent_turn_id": "turn-1",
            "root_turn_id": "root-turn",
            "thread_source": "guardian_classifier",
        })
    );
    assert_eq!(request["client_metadata"]["x-openai-subagent"], "guardian");
    assert_eq!(
        request["client_metadata"]["x-codex-window-id"],
        format!("{classifier_thread_id}:0")
    );
    assert_eq!(request["client_metadata"]["parent_turn_id"], "turn-1");
    assert_eq!(request["client_metadata"]["root_turn_id"], "root-turn");
    assert_eq!(request["reasoning"]["effort"], "low");
    assert_eq!(request["reasoning"]["context"], "all_turns");
    assert!(request.get("text").is_none());
    assert_eq!(
        request["input"][1],
        json!({
            "type": "message",
            "id": request["input"][1]["id"],
            "role": "developer",
            "content": [{
                "type": "input_text",
                "text": crate::async_scorer::config::DEFAULT_CLASSIFIER_INSTRUCTIONS.replace(
                    "{{ tenant_policy_config }}",
                    TEST_GUARDIAN_POLICY,
                ),
            }],
        })
    );
    let mut expected_content = json!([
        {"type": "input_text", "text": ">>> RETAINED USER INSTRUCTIONS START\nHost: Retained source order labels across instructions and verified answers reflect original acceptance, not section order. Later instructions may revoke earlier grants.\n"},
        {"type": "input_text", "text": "Retained source order: 0\nuser: Inspect the repository guidelines.\n"},
        {"type": "input_text", "text": ">>> RETAINED USER INSTRUCTIONS END\n"},
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
    ]);
    if !thread_context_enabled {
        expected_content
            .as_array_mut()
            .expect("content array")
            .drain(..3);
    }
    assert_eq!(request["input"][2]["content"], expected_content);
    let score = tokio::time::timeout(ASYNC_TEST_TIMEOUT, async {
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
            scores: BTreeMap::from([("action_risk".to_string(), 1.0)]),
            call_id: Some("call-1".to_owned()),
            action: Some(json!({"path": "README.md", "tool": "read_file"})),
            sampled_at: score.sampled_at,
        }
    );
    assert!(score.sampled_at.is_some());
    test.codex.ensure_rollout_materialized().await;
    assert!(
        !test
            .codex
            .load_history(/*include_archived*/ false)
            .await?
            .items
            .into_iter()
            .any(|item| matches!(item, RolloutItem::SecurityRiskScore(_))),
        "risk scores should not be persisted unless explicitly enabled"
    );
    assert_eq!(
        registry
            .fast_approval_decision(
                &session_store,
                thread_store,
                "review action",
                /*extension_metrics*/ None,
            )
            .await,
        None
    );
    thread_store.insert(SecurityRiskScore {
        scores: BTreeMap::from([("action_risk".to_string(), 0.5)]),
        call_id: None,
        action: None,
        sampled_at: None,
    });
    assert_eq!(
        registry
            .fast_approval_decision(
                &session_store,
                thread_store,
                "review action",
                /*extension_metrics*/ None,
            )
            .await,
        None
    );

    thread_store.insert(SecurityRiskScore {
        scores: BTreeMap::from([("action_risk".to_string(), 0.49)]),
        call_id: None,
        action: None,
        sampled_at: None,
    });
    assert_eq!(
        registry
            .fast_approval_decision(
                &session_store,
                thread_store,
                "review action",
                /*extension_metrics*/ None,
            )
            .await,
        Some(ReviewDecision::Approved)
    );

    let disabled_thread_store = ExtensionData::new("disabled-thread");
    disabled_thread_store.insert(SecurityRiskScore {
        scores: BTreeMap::from([("action_risk".to_string(), 0.25)]),
        call_id: None,
        action: None,
        sampled_at: None,
    });
    assert_eq!(
        registry
            .fast_approval_decision(
                &session_store,
                &disabled_thread_store,
                "review action",
                /*extension_metrics*/ None
            )
            .await,
        None
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contributor_persists_nested_code_mode_action_with_score() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let (_request, test, _registry) = sample_configured_conversation_history_with_source(
        Vec::new(),
        r#"{"path":"README.md"}"#,
        Some(TEST_GUARDIAN_POLICY),
        "[features.guardianv2]\nenabled = true\npersist_scores = true\n",
        /*model_defaults*/ None,
        ToolCallSource::CodeMode {
            cell_id: "cell-1".to_owned(),
            runtime_tool_call_id: "nested-1".to_owned(),
        },
    )
    .await?;
    test.codex.ensure_rollout_materialized().await;

    let score = tokio::time::timeout(ASYNC_TEST_TIMEOUT, async {
        loop {
            if let Some(score) = test
                .codex
                .load_history(/*include_archived*/ false)
                .await?
                .items
                .into_iter()
                .find_map(|item| match item {
                    RolloutItem::SecurityRiskScore(score) => Some(score),
                    _ => None,
                })
            {
                return Ok::<_, anyhow::Error>(score);
            }
            tokio::task::yield_now().await;
        }
    })
    .await??;

    assert_eq!(
        score,
        SecurityRiskScore {
            scores: BTreeMap::from([("action_risk".to_owned(), 1.0)]),
            call_id: Some("call-1".to_owned()),
            action: Some(json!({"path": "README.md", "tool": "read_file"})),
            sampled_at: score.sampled_at,
        }
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contributor_skips_required_models_in_standard_scope() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let thread_server = responses::start_mock_server().await;
    let initial = test_codex().build_with_auto_env(&thread_server).await?;
    std::fs::write(
        initial.home.path().join("requirements.toml"),
        "[auto_review]\nrequired_on_models = [\"protected-model\"]\n",
    )?;
    let config_layer_stack = ConfigBuilder::default()
        .codex_home(initial.home.path().to_path_buf())
        .loader_overrides(LoaderOverrides::with_managed_config_path_for_tests(
            initial.home.path().join("managed_config.toml"),
        ))
        .build()
        .await?
        .config_layer_stack;
    let test = test_codex()
        .with_home(Arc::clone(&initial.home))
        .with_config(move |config| {
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            config.config_layer_stack = config_layer_stack;
            config
                .features
                .enable(Feature::GuardianV2)
                .expect("Guardian v2 should remain globally enabled");
        })
        .build_with_auto_env(&thread_server)
        .await?;

    let server =
        responses::start_websocket_server(vec![Vec::new(); INITIAL_WEBSOCKET_CONNECTIONS]).await;
    let provider_info = ModelProviderInfo::create_openai_provider(Some(format!(
        "http://{}/v1",
        server.uri().trim_start_matches("ws://")
    )));
    let auth_manager = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("test-api-key"));
    let mut config = test.config.clone();
    config.model_provider = provider_info;
    let mut builder = ExtensionRegistryBuilder::new();
    super::install(
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

    let mut guardian_config = thread_store
        .get::<crate::async_scorer::config::GuardianV2Config>()
        .expect("Guardian v2 should have initialized")
        .as_ref()
        .clone();
    guardian_config.policy = GuardianPolicy::from_legacy(Some(&GuardianV2ReviewScopeConfigToml {
        computer_use_only: Some(false),
        sandboxed_exec_commands: Some(false),
    }));
    thread_store.insert(guardian_config);

    let mut model_info = test
        .thread_manager
        .get_models_manager()
        .get_model_info("gpt-5.5", &config.to_models_manager_config())
        .await;
    model_info.slug = "protected-model".to_owned();
    thread_store.insert(model_info);
    let authorization = super::ScoreAuthorization::current(&test.codex).await;
    let progress = thread_store
        .get::<GuardianV2ScoreProgress>()
        .expect("Guardian v2 should track score progress per thread");
    *progress.authorization.lock().unwrap() = Some(authorization);
    thread_store.insert(SecurityRiskScore {
        scores: BTreeMap::from([("action_risk".to_owned(), 0.25)]),
        call_id: None,
        action: None,
        sampled_at: None,
    });
    assert_eq!(
        registry
            .fast_approval_decision(
                &session_store,
                thread_store,
                r#"{"tool":"mcp_tool_call","server":"node_repl"}"#,
                /*extension_metrics*/ None,
            )
            .await,
        None
    );

    let turn_store = ExtensionData::new("turn-1");
    let tool_name = ToolName::namespaced("mcp__node_repl__", "js");
    let payload = ToolPayload::Function {
        arguments: json!({ "path": "protected.md" }).to_string(),
    };
    let oversized_compaction = ResponseItem::Compaction {
        id: Some(ResponseItemId::from_server("cmp_oversized".to_owned())),
        encrypted_content: "a"
            .repeat(TruncationPolicy::Tokens(DEFAULT_PARENT_COMPACTION_TOKENS).byte_budget()),
        internal_chat_message_metadata_passthrough: None,
    };
    registry.tool_lifecycle_contributors()[0]
        .on_tool_start(ToolStartInput {
            session_store: &session_store,
            thread_store,
            turn_store: &turn_store,
            turn_id: "turn-1",
            root_turn_id: None,
            call_id: "protected.md",
            tool_name: &tool_name,
            mcp_tool: None,
            payload: &payload,
            conversation_history: Arc::new(TestConversationHistory(vec![oversized_compaction])),
            source: ToolCallSource::Direct,
        })
        .await;

    assert!(
        thread_store.get::<SecurityRiskScore>().is_none(),
        "protected models must not receive Guardian v2 fail-closed scores"
    );
    assert!(
        server.connections().iter().all(Vec::is_empty),
        "protected models must not spawn Guardian v2 classifiers"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cached_score_survives_compaction_and_internal_context_but_not_user_input() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let (_, test, registry) = sample_configured_conversation_history(
        Vec::new(),
        r#"{"path":"README.md"}"#,
        Some(TEST_GUARDIAN_POLICY),
        "[features]\ntoken_budget = true\n[features.guardianv2]\nenabled = true\n",
        /*model_defaults*/ None,
    )
    .await?;
    let session_store = ExtensionData::new("session-1");
    let thread_store = test.codex.thread_extension_data();
    let progress = thread_store.get::<GuardianV2ScoreProgress>().unwrap();
    tokio::time::timeout(ASYNC_TEST_TIMEOUT, async {
        while progress.latest_scored_tool_call.load(Ordering::Acquire) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await?;
    thread_store.insert(SecurityRiskScore {
        scores: BTreeMap::from([("action_risk".to_owned(), 0.25)]),
        call_id: None,
        action: None,
        sampled_at: None,
    });

    test.codex
        .inject_response_items(vec![ContextualUserFragment::into(
            InternalModelContextFragment::new(
                InternalContextSource::from_static("goal"),
                "Continue inspecting the repository.",
            ),
        )])
        .await?;
    test.codex.submit(Op::Compact).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    assert_eq!(
        registry
            .fast_approval_decision(
                &session_store,
                thread_store,
                "review action",
                /*extension_metrics*/ None,
            )
            .await,
        Some(ReviewDecision::Approved),
    );

    test.codex
        .inject_response_items(vec![ResponseItem::Message {
            id: None,
            role: "user".to_owned(),
            content: vec![ContentItem::InputText {
                text: "Stop. Do not change any files.".to_owned(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        }])
        .await?;
    assert_eq!(
        registry
            .fast_approval_decision(
                &session_store,
                thread_store,
                "review action",
                /*extension_metrics*/ None
            )
            .await,
        None,
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn incompatible_compaction_blocks_cached_score_and_initial_cua_allowance() -> Result<()> {
    assert_compaction_approval_policy(/*thread_context_enabled*/ true).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_incompatible_compaction_preserves_cached_score_and_initial_cua_allowance()
-> Result<()> {
    assert_compaction_approval_policy(/*thread_context_enabled*/ false).await
}

async fn assert_compaction_approval_policy(thread_context_enabled: bool) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let fixture = GuardianFailureFixture::with_config(&format!(
        "[features]\nguardian_thread_context = {thread_context_enabled}\n"
    ))
    .await?;
    let thread_store = fixture.test.codex.thread_extension_data();
    thread_store.insert(SecurityRiskScore {
        scores: BTreeMap::from([("action_risk".to_owned(), 0.0)]),
        call_id: None,
        action: None,
        sampled_at: None,
    });
    assert_eq!(
        fixture
            .registry
            .fast_approval_decision(
                &fixture.session_store,
                thread_store,
                "review action",
                /*extension_metrics*/ None
            )
            .await,
        Some(ReviewDecision::Approved)
    );
    let authorization = fixture.test.codex.guardian_authorization_version().await;
    fixture
        .test
        .codex
        .inject_response_items(vec![ResponseItem::Compaction {
            id: Some(ResponseItemId::from_server(
                "incompatible-checkpoint".to_owned(),
            )),
            encrypted_content: "opaque parent summary".to_owned(),
            internal_chat_message_metadata_passthrough: None,
        }])
        .await?;
    let mut model = (*thread_store
        .get::<ModelInfo>()
        .expect("parent model metadata"))
    .clone();
    model.comp_hash = Some("incompatible-parent".to_owned());
    model.node_repl_auto_review_required = true;
    thread_store.insert(model);
    assert_eq!(
        fixture.test.codex.guardian_authorization_version().await,
        authorization
    );
    let score_authorization = super::ScoreAuthorization::current(&fixture.test.codex).await;
    *thread_store
        .get::<GuardianV2ScoreProgress>()
        .expect("score progress")
        .authorization
        .lock()
        .unwrap() = Some(score_authorization);
    // No new sample runs: only the enabled path rejects cached and initial-call approvals.
    for (computer_use_only, prompt) in [
        (false, "review action"),
        (
            true,
            r#"{"tool":"mcp_tool_call","server":"node_repl","connector_id":"node_repl","tool_name":"js"}"#,
        ),
    ] {
        let mut config = (*thread_store
            .get::<GuardianV2Config>()
            .expect("Guardian configuration"))
        .clone();
        config.policy = GuardianPolicy::from_legacy(Some(&GuardianV2ReviewScopeConfigToml {
            computer_use_only: Some(computer_use_only),
            sandboxed_exec_commands: Some(true),
        }));
        thread_store.insert(config);
        thread_store
            .get::<GuardianV2ScoreProgress>()
            .expect("score progress")
            .js_executions
            .store(/*val*/ 1, Ordering::Release);
        assert_eq!(
            fixture
                .registry
                .fast_approval_decision(
                    &fixture.session_store,
                    thread_store,
                    prompt,
                    /*extension_metrics*/ None
                )
                .await,
            (!thread_context_enabled).then_some(ReviewDecision::Approved)
        );
        assert_eq!(
            thread_store.remove::<StrictReviewReason>().as_deref(),
            thread_context_enabled.then_some(&StrictReviewReason::IncompatibleCompaction)
        );
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contributor_counts_failed_thread_lookups_toward_score_lag() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let (_, test, registry) = sample_configured_conversation_history(
        Vec::new(),
        r#"{"path":"README.md"}"#,
        Some(TEST_GUARDIAN_POLICY),
        "[features.guardianv2]\nenabled = true\nmax_tool_call_lag = 0\n",
        /*model_defaults*/ None,
    )
    .await?;
    let session_store = ExtensionData::new("session-1");
    let thread_store = test.codex.thread_extension_data();
    let score_progress = thread_store
        .get::<GuardianV2ScoreProgress>()
        .expect("Guardian v2 should track score progress per thread");
    tokio::time::timeout(ASYNC_TEST_TIMEOUT, async {
        while score_progress
            .latest_scored_tool_call
            .load(Ordering::Acquire)
            == 0
        {
            tokio::task::yield_now().await;
        }
    })
    .await?;
    thread_store.insert(SecurityRiskScore {
        scores: BTreeMap::from([("action_risk".to_owned(), 0.25)]),
        call_id: None,
        action: None,
        sampled_at: None,
    });
    assert_eq!(
        registry
            .fast_approval_decision(
                &session_store,
                thread_store,
                "review action",
                /*extension_metrics*/ None,
            )
            .await,
        Some(ReviewDecision::Approved)
    );

    test.thread_manager
        .remove_thread(&test.session_configured.thread_id)
        .await
        .expect("the test thread should exist before simulating a failed lookup");
    let turn_store = ExtensionData::new("turn-1");
    let tool_name = ToolName::plain("read_file");
    let payload = ToolPayload::Function {
        arguments: r#"{"path":"missing.md"}"#.to_owned(),
    };
    registry.tool_lifecycle_contributors()[0]
        .on_tool_start(ToolStartInput {
            session_store: &session_store,
            thread_store,
            turn_store: &turn_store,
            turn_id: "turn-1",
            root_turn_id: None,
            call_id: "missing.md",
            tool_name: &tool_name,
            mcp_tool: None,
            payload: &payload,
            conversation_history: Arc::new(TestConversationHistory(Vec::new())),
            source: ToolCallSource::Direct,
        })
        .await;

    assert_eq!(score_progress.latest_tool_call.load(Ordering::Acquire), 2);
    assert_eq!(
        registry
            .fast_approval_decision(
                &session_store,
                thread_store,
                "review action",
                /*extension_metrics*/ None,
            )
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
            "id": request["input"][1]["id"],
            "role": "developer",
            "content": [{
                "type": "input_text",
                "text": crate::async_scorer::config::DEFAULT_CLASSIFIER_INSTRUCTIONS.replace(
                    "{{ tenant_policy_config }}",
                    TEST_CATALOG_GUARDIAN_POLICY,
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
async fn contributor_preserves_uncapped_classifier_instructions() -> Result<()> {
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

    assert_eq!(
        request["input"][1],
        json!({
            "type": "message",
            "id": request["input"][1]["id"],
            "role": "developer",
            "content": [{
                "type": "input_text",
                "text": crate::async_scorer::config::DEFAULT_CLASSIFIER_INSTRUCTIONS
                    .replace("{{ tenant_policy_config }}", &guardian_policy),
            }],
        })
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
    let (request, _test, _registry) = sample_configured_conversation_history(
        Vec::new(),
        r#"{"path":"README.md"}"#,
        Some(&guardian_policy),
        "[features.guardianv2]\nenabled = true\nmax_classifier_instruction_tokens = 10000\n",
        /*model_defaults*/ None,
    )
    .await?;
    let instructions = request["input"][1]["content"][0]["text"]
        .as_str()
        .expect("Luna request should contain developer instructions");

    let (prefix, suffix) = crate::async_scorer::config::DEFAULT_CLASSIFIER_INSTRUCTIONS
        .split_once("{{ tenant_policy_config }}")
        .expect("default classifier prompt should contain the policy placeholder");
    assert!(instructions.starts_with(&format!("{prefix}Reject unsafe uploads.")));
    assert!(instructions.contains("<truncated omitted_approx_tokens="));
    assert!(instructions.contains("Require explicit approval."));
    assert!(instructions.ends_with(suffix));
    assert!(
        instructions.len()
            <= TruncationPolicy::Tokens(
                crate::async_scorer::config::DEFAULT_MODEL_CONTEXT_ITEM_TOKENS,
            )
            .byte_budget()
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contributor_preserves_final_assistant_messages_after_tool_eviction() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let mut history = vec![
        responses::user_message_item("Find a flight to New York."),
        ResponseItem::Message {
            id: None,
            role: "assistant".to_owned(),
            content: vec![ContentItem::OutputText {
                text: "I found a $450 flight. Should I book it?".to_owned(),
            }],
            phase: Some(MessagePhase::FinalAnswer),
            internal_chat_message_metadata_passthrough: None,
        },
        responses::user_message_item("Yes."),
        ResponseItem::Message {
            id: None,
            role: "assistant".to_owned(),
            content: vec![ContentItem::OutputText {
                text: "Searching airline websites.".to_owned(),
            }],
            phase: Some(MessagePhase::Commentary),
            internal_chat_message_metadata_passthrough: None,
        },
    ];
    history.extend((0..6).map(|index| ResponseItem::FunctionCall {
        id: None,
        name: "exec_command".to_owned(),
        namespace: None,
        arguments: format!("booking step {index}"),
        encrypted_function_args: None,
        call_id: format!("call-{index}"),
        internal_chat_message_metadata_passthrough: None,
    }));
    let configuration = "[features.guardianv2]\nenabled = true\n\n[features.guardianv2.transcript]\nmax_recent_non_user_entries = 4\n";

    let (request, _test, _registry) = sample_configured_conversation_history(
        history,
        r#"{"path":"README.md"}"#,
        Some(TEST_GUARDIAN_POLICY),
        configuration,
        /*model_defaults*/ None,
    )
    .await?;
    let entries = request["input"][2]["content"]
        .as_array()
        .expect("Luna request should contain separate transcript text items")
        .iter()
        .filter_map(|entry| entry["text"].as_str())
        .filter(|entry| entry.starts_with('['))
        .collect::<Vec<_>>();

    assert_eq!(
        entries,
        vec![
            "[1] user: Find a flight to New York.\n",
            "[2] assistant: I found a $450 flight. Should I book it?\n",
            "[3] user: Yes.\n",
            "[8] tool exec_command call: booking step 3\n",
            "[9] tool exec_command call: booking step 4\n",
            "[10] tool exec_command call: booking step 5\n",
        ]
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
                call_id: Some(call_id),
                name: None,
                namespace: None,
                output: FunctionCallOutputPayload::from_text(format!(
                    "result evidence {index}: {}",
                    "signal ".repeat(1_000)
                )),
                internal_chat_message_metadata_passthrough: None,
            },
        ]
    }));
    history.extend(
        [
            (None, None, "unattributed anonymous output"),
            (Some("missing-call"), None, "unattributed orphaned output"),
            (
                Some("missing-named-call"),
                Some("notifications"),
                "named orphaned output",
            ),
            (None, Some("notifications"), "attributed notification"),
        ]
        .map(|(call_id, name, text)| ResponseItem::FunctionCallOutput {
            id: None,
            call_id: call_id.map(str::to_string),
            name: name.map(str::to_string),
            namespace: Some("slack".to_string()),
            output: FunctionCallOutputPayload::from_text(text.to_string()),
            internal_chat_message_metadata_passthrough: None,
        }),
    );
    history.push(ResponseItem::CustomToolCallOutput {
        id: None,
        call_id: "missing-custom-call".to_string(),
        name: None,
        output: FunctionCallOutputPayload::from_text("unattributed custom output".to_string()),
        internal_chat_message_metadata_passthrough: None,
    });
    history.extend([
        ResponseItem::LocalShellCall {
            id: None,
            call_id: Some("shell-1".to_string()),
            status: LocalShellStatus::Completed,
            action: LocalShellAction::Exec(LocalShellExecAction {
                command: vec!["echo".to_string(), "hello".to_string()],
                timeout_ms: None,
                working_directory: None,
                env: None,
                user: None,
            }),
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::FunctionCallOutput {
            id: None,
            call_id: Some("shell-1".to_string()),
            name: None,
            namespace: None,
            output: FunctionCallOutputPayload::from_text("local shell evidence".to_string()),
            internal_chat_message_metadata_passthrough: None,
        },
    ]);

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
    assert!(
        !entries
            .iter()
            .any(|entry| entry.contains("unattributed anonymous output"))
    );
    for output in [
        "tool result: unattributed orphaned output",
        "tool result: named orphaned output",
        "tool result: unattributed custom output",
        "tool result: local shell evidence",
    ] {
        assert!(
            entries.iter().any(|entry| entry.contains(output)),
            "missing {output}"
        );
    }
    assert!(
        entries
            .iter()
            .any(|entry| entry.contains("tool shell call:"))
    );
    assert!(entries.iter().any(|entry| {
        entry.contains("tool slack.notifications result: attributed notification")
    }));

    for entry in entries.into_iter().filter(|entry| entry.starts_with('[')) {
        let (label, text) = entry.split_once(": ").expect("numbered transcript entry");
        let max_tokens = if label.contains("tool ") {
            MAX_TOOL_ENTRY_TOKENS
        } else {
            MAX_MESSAGE_ENTRY_TOKENS
        };
        assert!(
            text.trim_end_matches('\n').len() <= TruncationPolicy::Tokens(max_tokens).byte_budget()
        );
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contributor_reuses_the_latest_compatible_parent_compaction() -> Result<()> {
    assert_parent_compaction_reuse(/*thread_context_enabled*/ true).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_contributor_reuses_the_latest_compatible_parent_compaction() -> Result<()> {
    assert_parent_compaction_reuse(/*thread_context_enabled*/ false).await
}

async fn assert_parent_compaction_reuse(thread_context_enabled: bool) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let thread_server = responses::start_mock_server().await;
    let test = test_codex()
        .with_config(move |config| {
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            config
                .features
                .set_enabled(Feature::GuardianThreadContext, thread_context_enabled)
                .expect("test context mode");
        })
        .with_pre_build_hook(|home| {
            std::fs::write(
                home.join("config.toml"),
                "[features.guardianv2]\nenabled = true\nmax_parent_compaction_tokens = 256\n\n[features.guardianv2.review_scope]\ncomputer_use_only = false\n",
            )
            .expect("Guardian v2 parent compaction configuration should be written");
        })
        .build_with_auto_env(&thread_server)
        .await?;
    let events = vec![
        ev_assistant_message("sample", "low"),
        ev_completed("response-1"),
    ];
    let mut connections = vec![Vec::new(); INITIAL_WEBSOCKET_CONNECTIONS - 1];
    connections.push(vec![events]);
    let server = responses::start_websocket_server(connections).await;
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
    super::install(
        &mut builder,
        auth_manager,
        Arc::downgrade(&test.thread_manager),
    );
    let registry = builder.build();
    let session_store = ExtensionData::new("session-1");
    let thread_store = test.codex.thread_extension_data();
    let metrics = Arc::new(RecordingMetrics::default());
    registry.thread_lifecycle_contributors()[0]
        .on_thread_start(ThreadStartInput {
            config: &config,
            session_source: &SessionSource::Exec,
            persistent_thread_state_available: false,
            environments: &[],
            mcp_resource_client: None,
            extension_metrics: Some(metrics.clone()),
            session_store: &session_store,
            thread_store,
        })
        .await;
    thread_store
        .get::<LunaSampler>()
        .expect("Guardian v2 should initialize")
        .wait_for_prewarm(PREWARM_TIMEOUT)
        .await?;
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
        user_instruction("Inspect the repository guidelines."),
    ]);

    Box::pin(
        test.codex
            .inject_response_items(conversation_history.0.clone()),
    )
    .await?;
    let mut retained: Vec<ResponseItem> = serde_json::from_value(json!([
        {"type":"function_call", "name":"check_repository", "arguments":"{}", "call_id":"before"},
        {"type":"function_call_output", "call_id":"before", "output":"repository is private"}
    ]))?;
    retained.extend(conversation_history.0.clone());
    let conversation_history = TestRetainedHistory {
        retained,
        current: conversation_history,
        compaction_model_hash: parent_model.comp_hash.clone(),
    };
    thread_store.insert(parent_model);

    registry.tool_lifecycle_contributors()[0]
        .on_tool_start(ToolStartInput {
            session_store: &session_store,
            thread_store,
            turn_store: &turn_store,
            turn_id: "turn-1",
            root_turn_id: None,
            call_id: "call-1",
            tool_name: &tool_name,
            mcp_tool: None,
            payload: &tool_payload,
            conversation_history: Arc::new(conversation_history),
            source: ToolCallSource::Direct,
        })
        .await;

    let request = tokio::time::timeout(
        ASYNC_TEST_TIMEOUT,
        server.wait_for_request(
            /*connection_index*/ INITIAL_WEBSOCKET_CONNECTIONS - 1,
            /*request_index*/ 0,
        ),
    )
    .await?
    .body_json();
    assert_eq!(request["input"][0]["type"], "additional_tools");
    let developer_message = &request["input"][1];
    assert_eq!(developer_message["role"], "developer");
    let (prefix, _) = crate::async_scorer::config::DEFAULT_CLASSIFIER_INSTRUCTIONS
        .split_once("{{ tenant_policy_config }}")
        .expect("default classifier prompt should contain the policy placeholder");
    assert!(
        developer_message["content"][0]["text"]
            .as_str()
            .expect("Luna request should contain developer instructions")
            .replace("\r\n", "\n")
            .starts_with(&format!("{prefix}## Environment Profile\n").replace("\r\n", "\n"))
    );
    assert_eq!(
        request["input"][2],
        serde_json::to_value(&latest_compaction)?
    );
    assert_eq!(request["input"][3]["role"], "user");
    let transcript = serde_json::to_string(&request["input"][3])?;
    assert!(transcript.contains("tool check_repository call"));
    assert!(transcript.contains("tool check_repository result: repository is private"));

    let previous_score = tokio::time::timeout(ASYNC_TEST_TIMEOUT, async {
        loop {
            if let Some(score) = thread_store.get::<SecurityRiskScore>() {
                return score;
            }
            tokio::task::yield_now().await;
        }
    })
    .await?;
    assert_eq!(previous_score.scores.get("action_risk"), Some(&0.0));
    // Raw injection did not attach producer provenance to the live checkpoint.
    // The sample's mock snapshot cannot make that live checkpoint safe for approval.
    assert_eq!(
        registry
            .fast_approval_decision(
                &session_store,
                thread_store,
                "review action",
                /*extension_metrics*/ None,
            )
            .await,
        (!thread_context_enabled).then_some(ReviewDecision::Approved),
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
            root_turn_id: None,
            call_id: "call-2",
            tool_name: &tool_name,
            mcp_tool: None,
            payload: &tool_payload,
            conversation_history: Arc::new(TestRetainedHistory {
                current: TestConversationHistory(vec![latest_compaction, oversized_compaction]),
                retained: Vec::new(),
                compaction_model_hash: thread_store
                    .get::<ModelInfo>()
                    .and_then(|model| model.comp_hash.clone()),
            }),
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
            call_id: None,
            action: None,
            sampled_at: fail_closed_score.sampled_at,
        }
    );
    assert_eq!(
        registry
            .fast_approval_decision(
                &session_store,
                thread_store,
                "review action",
                /*extension_metrics*/ None,
            )
            .await,
        None
    );
    assert_eq!(
        server.connections().iter().map(Vec::len).sum::<usize>(),
        1,
        "an oversized latest compaction must bypass Luna rather than reuse stale context"
    );
    assert!(metrics.0.lock().unwrap().iter().any(|sample| {
        matches!(
            sample,
            RecordedMetric::Counter(name, 1, tags)
                if name == CLASSIFICATION_METRIC
                    && tags.contains(&("outcome".to_owned(), "failure".to_owned()))
        )
    }));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_contributor_can_disable_parent_compaction_reuse() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let oversized_compaction = ResponseItem::Compaction {
        id: Some(ResponseItemId::from_server("cmp_oversized".to_owned())),
        encrypted_content: "a".repeat(TruncationPolicy::Tokens(/*limit*/ 256).byte_budget()),
        internal_chat_message_metadata_passthrough: None,
    };
    let conversation_history = vec![
        oversized_compaction,
        user_instruction("Inspect the repository guidelines."),
    ];
    let configuration = "[features]\nguardian_thread_context = false\n\n[features.guardianv2]\nenabled = true\nreuse_parent_compaction = false\nmax_parent_compaction_tokens = 256\n";
    let (request, test, _registry) = sample_configured_conversation_history(
        conversation_history,
        r#"{"path":"README.md"}"#,
        Some(TEST_GUARDIAN_POLICY),
        configuration,
        /*model_defaults*/ None,
    )
    .await?;

    let input = request["input"]
        .as_array()
        .expect("Luna request input should be an array");
    assert_eq!(input.len(), 3);
    assert_eq!(input[2]["role"], "user");
    assert!(
        input
            .iter()
            .all(|item| item["type"] != "compaction" && item["type"] != "context_compaction")
    );

    let thread_store = test.codex.thread_extension_data();
    let score = tokio::time::timeout(ASYNC_TEST_TIMEOUT, async {
        loop {
            if let Some(score) = thread_store.get::<SecurityRiskScore>() {
                return score;
            }
            tokio::task::yield_now().await;
        }
    })
    .await?;
    assert_eq!(score.scores.get("action_risk"), Some(&1.0));

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
