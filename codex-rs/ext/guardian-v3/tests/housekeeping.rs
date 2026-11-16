//! Original evidence is routed as data through an isolated runtime into a real board.

#![allow(clippy::expect_used)]

use chrono::DateTime;
use chrono::Utc;
use codex_analytics::GuardianReviewSessionKind;
use codex_core::guardian_review::prepare_review_prewarm;
use codex_extension_api::SessionIsolation;
use codex_extension_api::ToolPolicy;
use codex_guardian_reviewer::SessionDisposition;
use codex_guardian_v3::EvidenceCursor;
use codex_guardian_v3::HOUSEKEEPER_INSTRUCTIONS;
use codex_guardian_v3::Housekeeper;
use codex_guardian_v3::local_board;
use codex_history::CodexHarnessMetadata;
use codex_history::ResponseItemEnvelope;
use codex_history::RolloutItem;
use codex_message_board::AgentMessageBoard;
use codex_message_board::ChannelQuery;
use codex_message_board::MessageBoardHost;
use codex_message_board::NotificationDelivery;
use codex_message_board::PageRequest;
use codex_message_board::PostPreview;
use codex_message_board::PostQuery;
use codex_message_board::ReadPostRequest;
use codex_message_board::SortDirection;
use codex_message_board::UpsertChannelRequest;
use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::error::Result as CodexResult;
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
use futures::future::BoxFuture;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

#[path = "../../../message-board/tests/common/mod.rs"]
mod board_host;

enum RoutingCase {
    Success,
    WriteFailure,
    MissingNewDescription,
    UnroutedDescription,
    UpsertDeadline,
    InvalidResponse,
}

#[test_case::test_case(RoutingCase::Success; "successful_writes")]
#[test_case::test_case(RoutingCase::WriteFailure; "write_failure")]
#[test_case::test_case(RoutingCase::MissingNewDescription; "new_destination_needs_description")]
#[test_case::test_case(RoutingCase::UnroutedDescription; "only_routed_channels_can_be_edited")]
#[test_case::test_case(RoutingCase::UpsertDeadline; "stalled_channel_creation")]
#[test_case::test_case(RoutingCase::InvalidResponse; "redacted_model_error")]
#[tokio::test]
async fn routes_captured_originals_and_updates_descriptions(
    case: RoutingCase,
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
    let gated_host = Arc::new(PausedClockHost {
        inner: host.clone(),
        pause_next: AtomicBool::new(false),
        started: Notify::new(),
        release: Notify::new(),
    });
    let board = Arc::new(
        local_board(
            SqliteConfig::new_for_testing(dir.path().to_path_buf().try_into()?),
            owner.into(),
        )
        .read_write()
        .open(gated_host.clone())
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
    let reply = match case {
        RoutingCase::MissingNewDescription => {
            r#"{"routes":[["release","_audit"]],"descriptions":{"release":"Release policy"}}"#
        }
        RoutingCase::UnroutedDescription => {
            r#"{"routes":[["_audit"]],"descriptions":{"_audit":"","release":"Unrelated edit"}}"#
        }
        RoutingCase::InvalidResponse => r#"{"private-evidence-canary":true}"#,
        RoutingCase::Success | RoutingCase::WriteFailure | RoutingCase::UpsertDeadline => {
            r#"{"routes":[["release","_audit"]],"descriptions":{"release":"Release policy","_audit":""}}"#
        }
    };
    let response = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("routing", reply),
            ev_completed("routing"),
        ]),
    )
    .await;
    let calls_before = host.agent_path_calls.load(Ordering::SeqCst);
    host.fail_clock
        .store(matches!(case, RoutingCase::WriteFailure), Ordering::SeqCst);
    let route = housekeeper.route_next(&snapshot, &runtime, &cancellation);
    let routed = if matches!(case, RoutingCase::UpsertDeadline) {
        gated_host.pause_next.store(true, Ordering::SeqCst);
        tokio::pin!(route);
        tokio::select! {
            () = gated_host.started.notified() => {}
            _ = &mut route => anyhow::bail!("routing ended before creating the channel"),
        }
        // The catalog read and model call finished; only the new channel's timestamp is blocked.
        tokio::time::pause();
        tokio::time::advance(std::time::Duration::from_secs(90)).await;
        tokio::time::resume();
        tokio::time::timeout(std::time::Duration::from_secs(5), route).await?
    } else {
        route.await
    };
    insta::assert_snapshot!(
        "guardian_housekeeping_originals",
        context_snapshot::format_request_history_snapshot(
            "Housekeeper routes captured originals and updates discovery descriptions.",
            &response.requests(),
            &ContextSnapshotOptions::default().rewrite_known_segments(),
        )
    );
    if !matches!(case, RoutingCase::Success) {
        let error = routed.result.expect_err("routing should fail");
        if matches!(case, RoutingCase::InvalidResponse) {
            let diagnostic = format!("{error:#}");
            assert!(diagnostic.contains("Data at line 1 column"));
            assert!(!diagnostic.contains("private-evidence-canary"));
        } else if matches!(
            case,
            RoutingCase::MissingNewDescription | RoutingCase::UnroutedDescription
        ) {
            assert_eq!(routed.disposition, SessionDisposition::Rollback);
            // Invalid routes stop after the catalog read, before any board writes.
            assert_eq!(
                host.agent_path_calls.load(Ordering::SeqCst) - calls_before,
                1
            );
        } else {
            assert_eq!(routed.disposition, SessionDisposition::Reusable);
            if matches!(case, RoutingCase::UpsertDeadline) {
                assert!(error.to_string().contains("TimedOut"));
            }
        }
        assert_eq!(housekeeper.cursor(), &initial_cursor);
        assert_eq!(housekeeper.cursor().pending(&snapshot)?.len(), 1);
        spawned.thread.shutdown_and_wait().await?;
        test.codex.shutdown_and_wait().await?;
        return Ok(());
    }
    assert_eq!(routed.disposition, SessionDisposition::Reusable);
    assert_eq!(routed.result?, 1);
    assert_eq!(
        housekeeper
            .route_next(&snapshot, &runtime, &cancellation)
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
        // Omission preserves the description. A redundant echo is harmless too.
        let reply = if index == 0 {
            r#"{"routes":[["release"]],"descriptions":{}}"#
        } else {
            r#"{"routes":[["release"]],"descriptions":{"release":"Release policy"}}"#
        };
        let response = mount_sse_once(
            &server,
            sse(vec![
                ev_assistant_message("routing-later", reply),
                ev_completed("routing-later"),
            ]),
        )
        .await;
        let calls_before = host.agent_path_calls.load(Ordering::SeqCst);
        let routed = housekeeper.route_next(&next, &runtime, &cancellation).await;
        assert_eq!(routed.disposition, SessionDisposition::Reusable);
        assert_eq!(routed.result?, 1);
        // An unchanged description needs only the catalog read and the evidence append.
        assert_eq!(
            host.agent_path_calls.load(Ordering::SeqCst) - calls_before,
            2
        );
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
    let view = board.read_view(owner).await?;
    let release = view
        .channels()
        .iter()
        .find(|channel| channel.channel_name == "release");
    assert_eq!(
        release.map(|channel| (channel.description.as_deref(), channel.message_count)),
        Some((Some("Release policy"), 3))
    );
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

struct PausedClockHost {
    inner: Arc<board_host::Host>,
    pause_next: AtomicBool,
    started: Notify,
    release: Notify,
}

impl MessageBoardHost for PausedClockHost {
    fn agent_path(&self, caller: ThreadId) -> BoxFuture<'_, CodexResult<AgentPath>> {
        self.inner.agent_path(caller)
    }

    fn resolve_agent(&self, path: AgentPath) -> BoxFuture<'_, CodexResult<ThreadId>> {
        self.inner.resolve_agent(path)
    }

    fn current_time(&self, caller: ThreadId) -> BoxFuture<'_, CodexResult<DateTime<Utc>>> {
        Box::pin(async move {
            if self.pause_next.swap(false, Ordering::SeqCst) {
                self.started.notify_one();
                self.release.notified().await;
            }
            self.inner.current_time(caller).await
        })
    }

    fn notify(
        &self,
        recipient: ThreadId,
        post: PostPreview,
    ) -> BoxFuture<'_, CodexResult<NotificationDelivery>> {
        self.inner.notify(recipient, post)
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn root_shutdown_waits_for_the_accepted_evidence_append() -> anyhow::Result<()> {
    use codex_extension_api::ExtensionData;
    use codex_extension_api::ExtensionRegistryBuilder;
    use codex_extension_api::ThreadReadyInput;
    use codex_extension_api::ThreadStopInput;
    use codex_extension_api::TurnAbortInput;
    use codex_protocol::protocol::SessionSource;
    use codex_protocol::protocol::TurnAbortReason;

    core_test_support::skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let test = test_codex()
        .with_history_mode(ThreadHistoryMode::Paginated)
        .with_config(|config| {
            config.approvals_reviewer = ApprovalsReviewer::User;
        })
        .build_with_auto_env(&server)
        .await?;
    let root = test.session_configured.thread_id;
    let host = Arc::new(board_host::Host {
        clock: AtomicI64::default(),
        fail_clock: AtomicBool::new(false),
        agent_path_calls: AtomicUsize::default(),
        members: [(root, AgentPath::root())].into(),
        active: AtomicBool::new(true),
        fail_notifications: AtomicBool::new(false),
        notifications: Mutex::default(),
    });
    let board = local_board(test.config.sqlite_config().clone(), root.into())
        .read_write()
        .open(host.clone())
        .await?;
    board
        .upsert_channel(
            root,
            UpsertChannelRequest {
                channel_name: "release".to_owned(),
                description: "Release policy".to_owned().try_into()?,
            },
        )
        .await?;
    let host = Arc::new(PausedClockHost {
        inner: host,
        pause_next: AtomicBool::new(true),
        started: Notify::new(),
        release: Notify::new(),
    });
    let mut registry = ExtensionRegistryBuilder::default();
    codex_guardian_v3::install(
        &mut registry,
        Arc::downgrade(&test.thread_manager),
        test.config.sqlite_config().clone(),
        {
            let host = host.clone();
            move |_| host.clone()
        },
    );
    let registry = registry.build();
    let session = ExtensionData::new(root.to_string());
    let thread = test.codex.thread_extension_data();
    let lifecycle = &registry.thread_lifecycle_contributors()[0];
    lifecycle
        .on_thread_ready(ThreadReadyInput {
            config: &test.config,
            session_source: &SessionSource::Exec,
            history_initialization: codex_history::HistoryInitialization::New,
            history_mode: ThreadHistoryMode::Paginated,
            session_store: &session,
            thread_store: thread,
        })
        .await;
    assert!(!tokio::fs::try_exists(test.codex.rollout_path().expect("persistent root")).await?);
    let data = ResponseItemEnvelope {
        item: serde_json::from_value(
            json!({"type":"message","role":"user","content":[{"type":"input_text","text":"Stop the release."}]}),
        )?,
        metadata: Some(CodexHarnessMetadata {
            user_input_order: Some(1),
            ..Default::default()
        }),
    };
    test.thread_store
        .append_items(AppendThreadItemsParams {
            thread_id: root,
            items: vec![RolloutItem::ResponseItem(data)],
        })
        .await?;
    let response = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message(
                "route",
                r#"{"routes":[["release"]],"descriptions":{"release":"Release policy"}}"#,
            ),
            ev_completed("route"),
        ]),
    )
    .await;
    registry.turn_lifecycle_contributors()[0]
        .on_turn_abort(TurnAbortInput {
            reason: TurnAbortReason::Interrupted,
            session_store: &session,
            thread_store: thread,
            turn_store: &ExtensionData::new("turn"),
        })
        .await;
    tokio::time::timeout(std::time::Duration::from_secs(15), host.started.notified()).await?;
    let housekeeper_id: ThreadId = serde_json::from_value(
        response.single_request().body_json()["client_metadata"]["thread_id"].take(),
    )?;
    let model = test
        .thread_manager
        .get_thread_with_extension::<SessionIsolation>(housekeeper_id, |isolation| {
            *isolation == SessionIsolation::Isolated
        })
        .await?;
    let shutdown = lifecycle.on_thread_stop(ThreadStopInput {
        thread_store: thread,
        session_store: &session,
    });
    tokio::pin!(shutdown);
    // The private model can stop while the accepted board write is still blocked.
    tokio::select! {
        biased;
        () = &mut shutdown => panic!("root shutdown returned before the append was released"),
        () = model.wait_until_terminated() => {},
    }
    // Allow shutdown to finish all other tracked tasks while the write stays gated.
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), shutdown.as_mut())
            .await
            .is_err(),
        "root shutdown returned while the accepted append was blocked"
    );
    host.release.notify_one();
    shutdown.await;
    let channels = board
        .list_channels(
            root,
            ChannelQuery {
                query: None,
                direction: SortDirection::OldestFirst,
                page: PageRequest::default(),
            },
        )
        .await?;
    assert!(
        channels
            .channels
            .results
            .iter()
            .any(|row| row.channel_name == "release" && row.message_count == 1)
    );
    assert_eq!(
        codex_core::find_thread_path_by_id_str(
            test.home.path(),
            &housekeeper_id.to_string(),
            /*state_db_ctx*/ None
        )
        .await?,
        None,
        "the housekeeper must not leave a private copy of the routed evidence"
    );
    test.codex.shutdown_and_wait().await?;
    Ok(())
}
