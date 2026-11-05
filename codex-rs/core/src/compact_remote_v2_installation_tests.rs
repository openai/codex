//! Stale prefixes are skipped or replaced with a fresh request according to caller intent.

use super::*;
use crate::AgentRef;
use crate::AgentSessionReader;
use crate::compact_remote_v2::RemoteCompactV2Attempt;
use crate::context::world_state::EnvironmentsState;
use crate::context::world_state::WorldState;
use crate::session::CompactionPrefix;
use codex_login::CodexAuth;
use codex_protocol::models::ResponseItem;
use core_test_support::responses;
use futures::future::BoxFuture;
use pretty_assertions::assert_eq;
use test_case::test_case;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Copy)]
enum HistoryChange {
    Replacement,
    Account,
}

#[derive(Default)]
struct FailingRosterRead {
    started: Notify,
    release: Notify,
}

impl AgentSessionReader for FailingRosterRead {
    fn root(&self) -> BoxFuture<'_, CodexResult<AgentRef>> {
        unreachable!("roster rendering reads children")
    }

    fn parent(&self) -> BoxFuture<'_, CodexResult<Option<AgentRef>>> {
        unreachable!("roster rendering reads children")
    }

    fn children(&self) -> BoxFuture<'_, CodexResult<Vec<AgentRef>>> {
        Box::pin(async {
            self.started.notify_one();
            self.release.notified().await;
            Err(CodexErr::InvalidRequest("roster unavailable".into()))
        })
    }
}

#[test_case(HistoryChange::Replacement, CompactionFallback::Skip; "replacement optional")]
#[test_case(HistoryChange::Account, CompactionFallback::Skip; "account optional")]
#[test_case(HistoryChange::Replacement, CompactionFallback::Foreground; "replacement required")]
#[test_case(HistoryChange::Account, CompactionFallback::Foreground; "account required")]
#[tokio::test]
async fn prepared_prefix_rejection_respects_caller_intent(
    change: HistoryChange,
    fallback: CompactionFallback,
) {
    let server = responses::start_mock_server().await;
    let mut provider =
        codex_model_provider_info::built_in_model_providers(/*openai_base_url*/ None)["openai"]
            .clone();
    provider.base_url = Some(format!("{}/v1", server.uri()));
    provider.supports_websockets = false;
    let server_uri = server.uri();
    let (sess, turn, _events) =
        crate::session::tests::make_session_and_context_with_auth_and_config_and_rx(
            CodexAuth::create_dummy_chatgpt_auth_for_testing(),
            Vec::new(),
            move |config| {
                config.model = Some("gpt-5.2".to_string());
                config.model_provider = provider;
                config.chatgpt_base_url = server_uri;
                let _ = config.features.disable(Feature::TokenBudget);
            },
        )
        .await;
    let requests = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            serde_json::json!({"type":"response.output_item.done", "item": {
                "type":"compaction", "encrypted_content":"fresh-summary"
            }}),
            serde_json::json!({"type":"response.completed", "response":{"id":"foreground"}}),
        ]),
    )
    .await;
    let step = sess
        .capture_step_context(turn, &CancellationToken::new())
        .await
        .expect("capture step");
    let original: ResponseItem = serde_json::from_value(serde_json::json!({
        "type": "message", "role": "user",
        "content": [{"type": "input_text", "text": "original turn"}]
    }))
    .expect("user message");
    sess.replace_history(vec![original.clone()], /*reference_context_item*/ None)
        .await;
    let history = sess.clone_history().await;
    let auth = sess.services.model_client.auth_owner_generation();
    let prefix = CompactionPrefix {
        generation: history.history_version(),
        len: history.annotated_items().len(),
        auth_owner_generation: match change {
            HistoryChange::Replacement => auth,
            HistoryChange::Account => Some(auth.unwrap_or_default().wrapping_add(1)),
        },
    };
    let roster = Arc::new(FailingRosterRead::default());
    let mut cutoff_world_state = WorldState::default();
    if matches!(change, HistoryChange::Replacement) {
        cutoff_world_state.add_section(
            EnvironmentsState::from_turn_context_with_environments(
                &step.turn,
                &step.environments,
                /*current_date*/ Some("2026-01-01".into()),
            )
            .await,
        );
        cutoff_world_state.host_roster_reader = Some(roster.clone());
    }
    let candidate = BackgroundCompactionCandidate {
        prefix,
        model_info: Arc::clone(&step.settings.model_info),
        cutoff_world_state: Arc::new(cutoff_world_state),
        attempt: RemoteCompactV2Attempt {
            rejected_audio_file_ids: Default::default(),
            input_goal_ids: Default::default(),
            trace_input_history: None,
            prompt_input: vec![original.clone()],
            prompt_input_metadata: vec![None],
            compaction_output: ResponseItem::Compaction {
                id: None,
                encrypted_content: "stale-summary".to_string(),
                internal_chat_message_metadata_passthrough: None,
            },
            compaction_response_id: "prepared".to_string(),
            token_usage: Some(TokenUsage {
                output_tokens: 777,
                cached_input_tokens: 9,
                cache_write_input_tokens: 5,
                ..Default::default()
            }),
            owned_client_session: None,
        },
    };
    let world_state = WorldState::default();
    let mut analytics = CompactionAnalyticsDetails::default();
    let mut compaction = Box::pin(run_compaction(
        &sess,
        &step,
        &step,
        /*client_session*/ None,
        &world_state,
        CompactionTurnMetadata::new(
            CompactionTrigger::Auto,
            CompactionReason::ContextLimit,
            CompactionImplementation::ResponsesCompactionV2,
            CompactionPhase::PreTurn,
        ),
        &mut analytics,
        /*started_item*/ None,
        Some(candidate),
        fallback,
    ));
    if matches!(change, HistoryChange::Replacement) {
        tokio::select! {
            _ = roster.started.notified() => {}
            result = &mut compaction => panic!("expected a pending roster read, got {result:?}"),
        }
        sess.replace_history(
            vec![original.clone(), original],
            /*reference_context_item*/ None,
        )
        .await;
    }
    // The replacement still fits the captured length, but changes the generation.
    let before = sess.clone_history().await;
    let window_before = sess.current_window().await;
    assert!(before.annotated_items().len() >= prefix.len);
    if matches!(change, HistoryChange::Replacement) {
        assert_ne!(before.history_version(), prefix.generation);
        roster.release.notify_one();
    }
    let installed = compaction
        .await
        .expect("reject stale result without failing compaction");
    // The foreground response has no usage; stale candidate statistics must not leak.
    assert_eq!(
        (
            analytics.compaction_summary_tokens,
            analytics.cached_input_tokens,
            analytics.cache_write_input_tokens
        ),
        (None, None, None),
    );
    let after = sess.clone_history().await;
    match fallback {
        CompactionFallback::Skip => {
            assert!(!installed);
            assert_eq!(analytics.retained_image_count, None);
            assert_eq!(requests.requests().len(), 0);
            assert_eq!(after.annotated_items(), before.annotated_items());
            assert_eq!(sess.current_window().await, window_before);
            assert_eq!(
                after.world_state_checkpoint(),
                before.world_state_checkpoint()
            );
        }
        CompactionFallback::Foreground => {
            assert!(installed);
            assert_eq!(requests.requests().len(), 1);
            let summaries = after
                .raw_items()
                .filter_map(|item| match item {
                    ResponseItem::Compaction {
                        encrypted_content, ..
                    } => Some(encrypted_content.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(summaries, ["fresh-summary"]);
        }
    }
}
