//! Reviewer tools and model-driven selection share a fixed Guardian catalog.

use codex_analytics::GuardianReviewSessionKind;
use codex_core::guardian_review::prepare_review_prewarm;
use codex_extension_api::SessionIsolation;
use codex_extension_api::ToolPolicy;
use codex_guardian_reviewer::ReviewDeadline;
use codex_guardian_reviewer::SessionDisposition;
use codex_guardian_v3::CHANNEL_SELECTION_INSTRUCTIONS;
use codex_guardian_v3::DeliveryState;
use codex_guardian_v3::RetrievalBinding;
use codex_guardian_v3::ReviewerSubscriptions;
use codex_guardian_v3::local_board;
use codex_message_board::AgentMessageBoard;
use codex_message_board::AppendRequest;
use codex_message_board::LocalBoard;
use codex_message_board::PostDestination;
use codex_message_board::SubscriptionChange::Subscribe;
use codex_message_board::SubscriptionRequest;
use codex_message_board::SubscriptionState;
use codex_message_board::SubscriptionTarget;
use codex_message_board::UpsertChannelRequest;
use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::openai_models::TruncationPolicyConfig;
use codex_protocol::protocol::HistoryPosition;
use codex_protocol::protocol::InternalSessionSource;
use codex_protocol::protocol::SessionSource;
use codex_state::SqliteConfig;
use codex_tools::ToolCall;
use codex_tools::ToolCallSource;
use codex_tools::ToolName;
use codex_tools::ToolPayload;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::context_snapshot::SnapshotEntry;
use core_test_support::responses::*;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

use crate::retrieval_host;

#[tokio::test]
async fn subscription_tools_only_change_the_caller_and_return_captured_post_ids()
-> anyhow::Result<()> {
    let policy = TruncationPolicyConfig::bytes(/*limit*/ 1024);
    let owner = ThreadId::new();
    let reviewer = ThreadId::new();
    let path = AgentPath::root()
        .join("reviewer")
        .map_err(anyhow::Error::msg)?;
    let host = retrieval_host([(owner, AgentPath::root()), (reviewer, path.clone())]);
    let dir = tempfile::tempdir()?;
    let sqlite = SqliteConfig::new_for_testing(dir.path().to_path_buf().try_into()?);
    let agent_board = LocalBoard::new(sqlite.clone(), owner.into())
        .read_only()
        .open(host.clone())
        .await?;
    // All three reviewer entry points must reject the ordinary board, even for the same root.
    let agent_view = agent_board.read_view(reviewer).await?;
    assert!(RetrievalBinding::new(agent_view.as_ref(), reviewer, path.clone()).is_err());
    let cutoff = HistoryPosition {
        thread_id: owner,
        end_ordinal_exclusive: 0,
        end_byte_offset: 0,
    };
    assert!(
        DeliveryState::new(reviewer, owner.into())
            .plan(agent_view.as_ref(), &BTreeSet::new(), cutoff)
            .await
            .is_err()
    );
    assert!(
        ReviewerSubscriptions::capture(Arc::new(agent_board), reviewer, path.clone())
            .await
            .is_err()
    );
    let storage = local_board(sqlite, owner.into());
    let writer = storage.read_write().open(host.clone()).await?;
    let original = writer
        .append(
            owner,
            AppendRequest {
                request_id: "original".into(),
                destination: PostDestination::NewChannel("release".into()),
                text: "Original".into(),
            },
        )
        .await?;
    let board: Arc<dyn AgentMessageBoard> = Arc::new(storage.read_only().open(host.clone()).await?);
    board
        .set_subscription(
            owner,
            SubscriptionRequest {
                target: SubscriptionTarget::Channel("release".into()),
                change: Subscribe,
                target_agent: None,
            },
        )
        .await?;
    let subscriptions =
        Arc::new(ReviewerSubscriptions::capture(Arc::clone(&board), reviewer, path.clone()).await?);
    writer
        .upsert_channel(
            owner,
            UpsertChannelRequest {
                channel_name: "later".into(),
                description: "Later channel".to_owned().try_into()?,
            },
        )
        .await?;
    let later = writer
        .append(
            owner,
            AppendRequest {
                request_id: "later".into(),
                destination: PostDestination::Channel("release".into()),
                text: "Later evidence".into(),
            },
        )
        .await?;
    assert_ne!(original.message_id, later.message_id);
    // Tool calls reuse the real backend but return only the captured catalog boundary.
    let tools = subscriptions.tools(Some("guardian"), "Guardian evidence");
    for (name, arguments, succeeds) in [
        ("subscribe", json!({"channel_name":"release"}), true),
        ("unsubscribe", json!({"channel_name":"release"}), true),
        ("subscribe", json!({"channel_name":"release"}), true),
        ("subscribe", json!({"channel_name":"later"}), false),
        (
            "unsubscribe",
            json!({"channel_name":"release","target_agent":"/"}),
            false,
        ),
    ] {
        let tool = tools
            .iter()
            .find(|tool| tool.tool_name().name == name && tool.exposure().is_direct())
            .ok_or_else(|| anyhow::anyhow!("missing direct subscription tool {name}"))?;
        let call = ToolCall {
            turn_id: "review".into(),
            call_id: name.into(),
            tool_name: ToolName::namespaced("guardian", name),
            model: "test".into(),
            codex_turn_metadata: None,
            truncation_policy: policy.into(),
            source: ToolCallSource::Direct,
            conversation_history: Default::default(),
            turn_item_emitter: Arc::new(codex_tools::NoopTurnItemEmitter),
            attachment_store: Arc::new(codex_attachment_store::InlineAttachmentStore),
            ephemeral: false,
            environments: Vec::new(),
            payload: ToolPayload::Function {
                arguments: arguments.to_string(),
            },
        };
        let result = tool.handle(call.clone()).await;
        if succeeds {
            let result = result?;
            let response = result.code_mode_result(&call.payload);
            assert_eq!(result.log_output(), response.to_string());
            let output: SubscriptionState = serde_json::from_value(response)?;
            assert_eq!(
                output,
                SubscriptionState {
                    channel_name: "release".into(),
                    thread_id: None,
                    target_agent: path.clone(),
                    enabled: name == "subscribe",
                    last_message_id: Some(original.message_id),
                }
            );
            if name == "unsubscribe" {
                assert!(subscriptions.channels().await?.is_empty());
            }
        } else {
            assert!(result.is_err());
        }
    }
    assert_eq!(
        subscriptions.channels().await?,
        BTreeSet::from(["release".into()])
    );
    assert_eq!(
        board
            .subscribed_channels(owner, Default::default())
            .await?
            .results,
        vec!["release".to_owned()]
    );
    assert!(
        host.notifications
            .lock()
            .map_err(|_| anyhow::anyhow!("notifications poisoned"))?
            .is_empty()
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn preparation_commits_only_valid_selections_from_the_captured_catalog() -> anyhow::Result<()>
{
    core_test_support::skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let policy = TruncationPolicyConfig::bytes(/*limit*/ 1024);
    let test = test_codex()
        .with_model_info_override("test-gpt-5.1-codex", move |model| {
            model.auto_review_model_override = Some(model.slug.clone());
            model.truncation_policy = policy;
        })
        .with_config(|config| {
            config.approvals_reviewer = ApprovalsReviewer::User;
            config.include_environment_context = false;
        })
        .build_with_auto_env(&server)
        .await?;
    let prepared = prepare_review_prewarm(&test.codex).await?;
    let key = prepared.reuse_key(/*previous*/ None);
    let (mut options, state) = prepared
        .thread_options(GuardianReviewSessionKind::TrunkNew, /*snapshot*/ None)
        .await;
    options.config.base_instructions = Some(CHANNEL_SELECTION_INSTRUCTIONS.into());
    options.session_source = Some(SessionSource::Internal(InternalSessionSource::Guardian));
    options
        .thread_extension_init
        .insert(SessionIsolation::Isolated);
    options.thread_extension_init.insert(ToolPolicy {
        allowed_tools: Some(Vec::new()),
        ..Default::default()
    });
    let spawned = test.thread_manager.start_thread(options).await?;
    let cancellation = CancellationToken::new();
    let runtime = prepared
        .bind_thread(&spawned.thread, key, state, cancellation.clone())
        .await;
    let owner = test.session_configured.thread_id;
    let reviewer = spawned.thread_id;
    let path = AgentPath::root()
        .join("reviewer")
        .map_err(anyhow::Error::msg)?;
    let host = retrieval_host([(owner, AgentPath::root()), (reviewer, path.clone())]);
    let dir = tempfile::tempdir()?;
    let sqlite = SqliteConfig::new_for_testing(dir.path().to_path_buf().try_into()?);
    let storage = local_board(sqlite, owner.into());
    let writer = storage.read_write().open(host.clone()).await?;
    for (name, description) in [("build", "Build"), ("release", "Release policy")] {
        writer
            .upsert_channel(
                owner,
                UpsertChannelRequest {
                    channel_name: name.into(),
                    description: description.to_owned().try_into()?,
                },
            )
            .await?;
    }
    let board: Arc<dyn AgentMessageBoard> = Arc::new(storage.read_only().open(host.clone()).await?);
    writer
        .set_subscriptions(owner, &BTreeMap::from([("release", Subscribe)]))
        .await?;
    board
        .set_subscriptions(reviewer, &BTreeMap::from([("build", Subscribe)]))
        .await?;
    let subscriptions =
        Arc::new(ReviewerSubscriptions::capture(Arc::clone(&board), reviewer, path.clone()).await?);
    // A newer channel can gain a subscription after capture, before this selection commits.
    writer
        .upsert_channel(
            owner,
            UpsertChannelRequest {
                channel_name: "later".into(),
                description: "Later channel".to_owned().try_into()?,
            },
        )
        .await?;
    board
        .set_subscriptions(reviewer, &BTreeMap::from([("later", Subscribe)]))
        .await?;
    let response = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("selection", r#"{"channels":["release"]}"#),
            ev_completed("selection"),
        ]),
    )
    .await;
    let selected = subscriptions
        .prepare(
            &runtime,
            "Publish?",
            policy.into(),
            ReviewDeadline::Unlimited,
            &cancellation,
        )
        .await;
    assert_eq!(selected.disposition, SessionDisposition::Reusable);
    let expected = BTreeSet::from(["release".into()]);
    assert_eq!(selected.result?, expected);
    assert_eq!(
        board
            .subscribed_channels(reviewer, Default::default())
            .await?
            .results,
        vec!["later".to_owned(), "release".to_owned()]
    );
    assert_eq!(
        board
            .subscribed_channels(owner, Default::default())
            .await?
            .results,
        vec!["release".to_owned()]
    );
    assert_eq!(subscriptions.channels().await?, expected);
    let mut request = response.single_request().body_json();
    for item in request["input"].as_array_mut().expect("request input") {
        if let Some(Value::String(output)) = item.get_mut("output") {
            *output = context_snapshot::normalize_json_lines(output);
        }
    }
    insta::assert_snapshot!(context_snapshot::format_context_snapshot(
        "Reviewer replaces a previous subscription with the relevant channel.",
        &[SnapshotEntry::body(&request)], &ContextSnapshotOptions::default().rewrite_known_segments(),
    ), @r###"
    Scenario: Reviewer replaces a previous subscription with the relevant channel.

    ## Window 1
    -- request 1 (request) --
    00:message/developer:
        <MODEL_INSTRUCTIONS>
    01:message/developer:
        <PERMISSIONS_INSTRUCTIONS>
    02:function_call_output/guardian.guardian_channel_selection_input:{"catalog":{"build":"Build","release":"Release policy"},"subscriptions":["build"],"task":"Publish?"}
    "###);
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    let stopped = subscriptions
        .prepare(
            &runtime,
            "Publish?",
            policy.into(),
            ReviewDeadline::Unlimited,
            &cancelled,
        )
        .await;
    assert!(stopped.result.is_err());
    assert_eq!(stopped.disposition, SessionDisposition::Reusable);
    assert_eq!(response.requests().len(), 1);
    assert_eq!(subscriptions.channels().await?, expected);
    let invalid = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("invalid", r#"{"channels":["build","later"]}"#),
            ev_completed("invalid"),
        ]),
    )
    .await;
    let selected = subscriptions
        .prepare(
            &runtime,
            "Publish?",
            policy.into(),
            ReviewDeadline::Unlimited,
            &cancellation,
        )
        .await;
    assert_eq!(selected.disposition, SessionDisposition::Rollback);
    assert!(selected.result.is_err());
    assert_eq!(subscriptions.channels().await?, expected);
    assert_eq!(invalid.requests().len(), 1);
    spawned.thread.shutdown_and_wait().await?;
    test.codex.shutdown_and_wait().await?;
    Ok(())
}
