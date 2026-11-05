//! Original evidence is routed as data through an isolated runtime into a real board.

#![allow(clippy::expect_used)]

use codex_analytics::GuardianReviewSessionKind;
use codex_core::guardian_review::prepare_review_prewarm;
use codex_extension_api::SessionIsolation;
use codex_extension_api::ToolPolicy;
use codex_guardian_reviewer::ReviewDeadline;
use codex_guardian_reviewer::SessionDisposition;
use codex_guardian_v3::EvidenceCursor;
use codex_guardian_v3::HOUSEKEEPER_INSTRUCTIONS;
use codex_guardian_v3::HOUSEKEEPER_TIMEOUT;
use codex_guardian_v3::Housekeeper;
use codex_guardian_v3::local_board;
use codex_history::CodexHarnessMetadata;
use codex_history::ResponseItemEnvelope;
use codex_history::RolloutItem;
use codex_message_board::AgentMessageBoard;
use codex_message_board::ChannelQuery;
use codex_message_board::PageRequest;
use codex_message_board::PostQuery;
use codex_message_board::ReadPostRequest;
use codex_message_board::SortDirection;
use codex_message_board::UpsertChannelRequest;
use codex_protocol::AgentPath;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::openai_models::TruncationPolicyConfig;
use codex_protocol::protocol::InternalSessionSource;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_state::SqliteConfig;
use codex_thread_store::AppendThreadItemsParams;
use codex_thread_store::PersistContext;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::responses::*;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

#[path = "../../../message-board/tests/common/mod.rs"]
mod board_host;

#[test_case::test_case(false; "successful_writes")]
#[test_case::test_case(true; "write_failure")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn routes_captured_originals_and_updates_descriptions(
    fail_writes: bool,
) -> anyhow::Result<()> {
    core_test_support::skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let input_policy = TruncationPolicyConfig::bytes(/*limit*/ 1024);
    let test = test_codex()
        .with_history_mode(ThreadHistoryMode::Paginated)
        .with_model_info_override("test-gpt-5.1-codex", move |model| {
            model.auto_review_model_override = Some(model.slug.clone());
            model.truncation_policy = input_policy;
        })
        .with_config(|config| {
            config.approvals_reviewer = ApprovalsReviewer::User;
            config.include_environment_context = false;
        })
        .build_with_auto_env(&server)
        .await?;
    let owner = test.session_configured.thread_id;
    test.thread_store
        .persist_thread(owner, PersistContext::Standard)
        .await?;
    test.thread_store.flush_thread(owner).await?;
    let cursor = EvidenceCursor::from_now(test.thread_store.as_ref(), owner).await?;
    let initial_cursor = cursor.clone();
    let original = ResponseItemEnvelope {
        item: serde_json::from_value(json!({"type":"message","id":"msg-policy","role":"user",
            "content":[{"type":"input_text","text":"Do not publish the release."}]}))?,
        metadata: Some(CodexHarnessMetadata {
            user_input_order: Some(1),
            ..Default::default()
        }),
    };
    test.thread_store
        .append_items(AppendThreadItemsParams {
            thread_id: owner,
            items: vec![RolloutItem::ResponseItem(original.clone())],
        })
        .await?;
    test.thread_store
        .persist_thread(owner, PersistContext::Standard)
        .await?;
    test.thread_store.flush_thread(owner).await?;
    let snapshot = cursor.capture(test.thread_store.as_ref()).await?;
    assert_eq!(snapshot.records().len(), 1);
    for user_input_order in 2..=3 {
        let later = ResponseItemEnvelope {
            item: serde_json::from_value(
                json!({"type":"message","id":format!("msg-later-{user_input_order}"),"role":"user",
                "content":[{"type":"input_text","text":"The later message is outside this capture.".repeat(8)}]}),
            )?,
            metadata: Some(CodexHarnessMetadata {
                user_input_order: Some(user_input_order),
                ..Default::default()
            }),
        };
        test.thread_store
            .append_items(AppendThreadItemsParams {
                thread_id: owner,
                items: vec![RolloutItem::ResponseItem(later)],
            })
            .await?;
    }
    test.thread_store.flush_thread(owner).await?;
    let dir = tempfile::tempdir()?;
    let host = Arc::new(board_host::Host {
        clock: AtomicI64::default(),
        fail_clock: AtomicBool::new(false),
        agent_path_calls: AtomicUsize::default(),
        members: [(owner, AgentPath::root())].into(),
        active: AtomicBool::new(true),
        fail_notifications: AtomicBool::new(false),
        notifications: Mutex::default(),
    });
    let board = Arc::new(
        local_board(
            SqliteConfig::new_for_testing(dir.path().to_path_buf().try_into()?),
            owner.into(),
        )
        .read_write()
        .open(host.clone())
        .await?,
    );
    board
        .upsert_channel(
            owner,
            UpsertChannelRequest {
                channel_name: "release".into(),
                description: "Release procedure".to_owned().try_into()?,
            },
        )
        .await?;
    let board_handle: Arc<dyn AgentMessageBoard> = board.clone();
    let mut housekeeper = Housekeeper::new(board_handle, cursor, input_policy.into())?;
    let prepared = prepare_review_prewarm(&test.codex).await?;
    let key = prepared.reuse_key(/*previous*/ None);
    let (mut options, state) = prepared
        .thread_options(GuardianReviewSessionKind::TrunkNew, /*snapshot*/ None)
        .await;
    options.config.base_instructions = Some(HOUSEKEEPER_INSTRUCTIONS.into());
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
    let response = mount_sse_once(&server, sse(vec![
        ev_assistant_message("routing", r#"{"routes":[["release","_audit"]],"descriptions":{"release":"Release policy","_audit":""}}"#),
        ev_completed("routing"),
    ])).await;
    let deadline = ReviewDeadline::At(Instant::now() + HOUSEKEEPER_TIMEOUT);
    host.fail_clock.store(fail_writes, Ordering::SeqCst);
    let routed = housekeeper
        .route_next(&snapshot, &runtime, deadline, &cancellation)
        .await;
    insta::assert_snapshot!(
        "guardian_housekeeping_originals",
        context_snapshot::format_request_history_snapshot(
            "Housekeeper routes captured originals and updates discovery descriptions.",
            &response.requests(),
            &ContextSnapshotOptions::default().rewrite_known_segments(),
        )
    );
    assert_eq!(routed.disposition, SessionDisposition::Reusable);
    if fail_writes {
        assert!(routed.result.is_err());
        assert_eq!(housekeeper.cursor(), &initial_cursor);
        assert_eq!(housekeeper.cursor().pending(&snapshot)?.len(), 1);
        spawned.thread.shutdown_and_wait().await?;
        test.codex.shutdown_and_wait().await?;
        return Ok(());
    }
    assert_eq!(routed.result?, 1);
    assert_eq!(
        housekeeper
            .route_next(&snapshot, &runtime, deadline, &cancellation)
            .await
            .result?,
        0
    );
    assert!(housekeeper.cursor().pending(&snapshot)?.is_empty());
    let next = housekeeper
        .cursor()
        .capture(test.thread_store.as_ref())
        .await?;
    assert_eq!(next.records().len(), 2);
    let request = response.single_request();
    let input = request.inputs_of_type("function_call_output");
    assert_eq!(input.len(), 1);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(
            input[0]["output"].as_str().expect("operation input")
        )?,
        json!({"catalog":{"release":"Release procedure"},"sources":[serde_json::to_value(&snapshot.records()[0])?]})
    );
    assert!(
        request
            .message_input_texts("developer")
            .iter()
            .any(|text| text.contains(HOUSEKEEPER_INSTRUCTIONS))
    );
    assert!(request.message_input_texts("user").is_empty());
    let channels = board
        .list_channels(
            owner,
            ChannelQuery {
                query: None,
                direction: SortDirection::OldestFirst,
                page: PageRequest::default(),
            },
        )
        .await?;
    assert_eq!(
        channels
            .channels
            .results
            .into_iter()
            .map(|channel| (
                channel.channel_name,
                channel.description,
                channel.message_count
            ))
            .collect::<Vec<_>>(),
        vec![
            ("release".into(), Some("Release policy".into()), 1),
            ("_audit".into(), Some("".into()), 1)
        ]
    );
    for channel in ["release", "_audit"] {
        let posts = board
            .search_posts(
                owner,
                PostQuery {
                    channel_name: Some(channel.into()),
                    query: None,
                    after_message_id: None,
                    author: None,
                    page: PageRequest::default(),
                    max_chars_per_post: NonZeroU32::MAX,
                },
            )
            .await?;
        assert_eq!(posts.results.len(), 1);
        let post = board
            .read_post(
                owner,
                ReadPostRequest {
                    message_id: posts.results[0].metadata.message_id,
                    offset_chars: 0,
                    limit_chars: NonZeroU32::MAX,
                },
            )
            .await?;
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&post.text)?,
            serde_json::to_value(&snapshot.records()[0])?
        );
    }
    // The next capture needs two batches; each request retains the full original
    // and current catalog within the selected model's tool-output budget.
    let catalog = json!({"release":"Release policy","_audit":""});
    assert!(
        serde_json::to_string(&json!({"catalog":catalog,"sources":next.records()}))?.len()
            > input_policy.limit as usize
    );
    for (index, source) in next.records().iter().enumerate() {
        let response = mount_sse_once(
            &server,
            sse(vec![
                ev_assistant_message(
                    "routing-later",
                    r#"{"routes":[["release"]],"descriptions":{"release":"Release policy"}}"#,
                ),
                ev_completed("routing-later"),
            ]),
        )
        .await;
        let routed = housekeeper
            .route_next(&next, &runtime, deadline, &cancellation)
            .await;
        assert_eq!(routed.disposition, SessionDisposition::Reusable);
        assert_eq!(routed.result?, 1);
        assert_eq!(
            housekeeper.cursor().pending(&next)?.len(),
            next.records().len() - index - 1
        );
        let request = response.single_request();
        let input = request.inputs_of_type("function_call_output");
        let input = input.last().expect("latest operation input")["output"]
            .as_str()
            .expect("operation input");
        assert!(input.len() <= input_policy.limit as usize);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(input)?,
            json!({"catalog":catalog,"sources":[source]})
        );
    }
    assert!(
        host.notifications
            .lock()
            .expect("notifications lock")
            .is_empty()
    );
    spawned.thread.shutdown_and_wait().await?;
    test.codex.shutdown_and_wait().await?;
    Ok(())
}
