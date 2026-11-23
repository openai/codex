#![cfg(test)]

use super::RootEvidence;
use crate::EvidenceCursor;
use anyhow::Context;
use board_host::PausedClockHost;
use board_host::board_host;
use codex_history::CodexHarnessMetadata;
use codex_history::HistoryInitialization;
use codex_history::ResponseItemEnvelope;
use codex_history::RetainedContextEvent;
use codex_history::RetainedUserMessage;
use codex_history::RolloutItem;
use codex_history::UserInputOrigin;
use codex_message_board::SubscriptionChange;
use codex_message_board::UpsertChannelRequest;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_thread_store::AppendThreadItemsParams;
use codex_thread_store::PersistContext;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::responses::user_message_item;
use core_test_support::test_codex::test_codex;
use core_test_support::tracing::NamedTraceEvents;
use futures::FutureExt;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use tokio::sync::Notify;
use tokio::sync::mpsc::error::TryRecvError;

#[path = "../tests/common/board_host.rs"]
mod board_host;

#[tokio::test]
async fn skips_a_partial_delivery_once_across_wakes_and_catch_up() -> anyhow::Result<()> {
    core_test_support::skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let test = test_codex()
        .with_history_mode(ThreadHistoryMode::Paginated)
        .build_with_auto_env(&server)
        .await?;
    let root = test.session_configured.thread_id;
    let history = &test.thread_store;
    let cursor = EvidenceCursor::at_thread_ready(
        history.as_ref(),
        root,
        ThreadHistoryMode::Paginated,
        HistoryInitialization::New,
    )
    .await?;
    let (evidence, mut writer) = RootEvidence::new(Default::default(), history.clone(), cursor);

    // The initial rollout header advances the writer even without evidence to route.
    history
        .persist_thread(root, PersistContext::Standard)
        .await?;
    evidence.enqueue(/*cutoff*/ None).await?;
    let first = writer.try_recv()?;
    assert!(first.snapshot.records().is_empty());

    let items = [(1, false, "partial"), (2, true, "next complete message")]
        .into_iter()
        .map(|(acceptance_order, complete, text)| {
            RolloutItem::RetainedContext(RetainedContextEvent::DeliveredAssistantMessage {
                message: RetainedUserMessage {
                    origin: UserInputOrigin::User,
                    turn_id: "turn-1".into(),
                    message_id: None,
                    text: text.into(),
                    complete,
                    phase: None,
                },
                acceptance_order,
                retained_source: None,
            })
        })
        .collect();
    history
        .append_items(AppendThreadItemsParams {
            thread_id: root,
            items,
        })
        .await?;
    // A request can go to a human after binding. Capturing its source position alone
    // must leave the worker idle, even when there are unread originals.
    let bound = evidence.capture_cutoff().await?;
    assert!(matches!(writer.try_recv(), Err(TryRecvError::Empty)));
    history
        .append_items(AppendThreadItemsParams {
            thread_id: root,
            items: vec![RolloutItem::ResponseItem(ResponseItemEnvelope {
                item: user_message_item("Later authorization"),
                metadata: Some(CodexHarnessMetadata {
                    user_input_order: Some(3),
                    ..Default::default()
                }),
            })],
        })
        .await?;
    history.flush_thread(root).await?;
    let mut warnings = NamedTraceEvents::subscribe("guardian_v3.evidence_skipped");
    evidence.enqueue(Some(bound)).await?;
    let capture = writer.try_recv()?;
    let routed = &capture.snapshot;
    let [next] = routed.records() else {
        anyhow::bail!("expected only the complete delivery")
    };
    let skipped = first.snapshot.cutoff().end_ordinal_exclusive;
    assert_eq!(routed.skipped_incomplete_deliveries, [skipped]);
    assert_eq!(next.id.ordinal, skipped + 1);
    warnings.next().await?;

    // A second waiter at the same cutoff shares the writer's progress, even though
    // a newer original is already durable.
    let ready = evidence.request_through(Some(bound)).await?;
    assert!(matches!(writer.try_recv(), Err(TryRecvError::Empty)));
    tokio::pin!(ready);
    assert!(futures::poll!(&mut ready).is_pending());
    let cutoff = routed.cutoff();
    capture.finish();
    assert_eq!(ready.await?, cutoff);
    let latest = evidence.request_catch_up().await?;
    let later = writer.try_recv()?;
    assert_eq!(later.snapshot.records().len(), 1);
    assert!(later.snapshot.cutoff().end_ordinal_exclusive > cutoff.end_ordinal_exclusive);
    let end = later.snapshot.cutoff();
    {
        // A later enqueue may hold the cursor while the queue is full.
        let _guard = evidence.enqueued.try_lock()?;
        let through = async |target| evidence.request_through(Some(target)).await?.await;
        assert_eq!(
            through(bound).now_or_never().context("already routed")??,
            bound
        );
        let waiting = through(end);
        tokio::pin!(waiting);
        assert!(waiting.as_mut().now_or_never().is_none());
        later.finish();
        assert_eq!(
            waiting.now_or_never().context("blocked after routing")??,
            end
        );
    }
    assert_eq!(latest.await?, end);
    assert!(warnings.next().now_or_never().is_none());
    test.codex.shutdown_and_wait().await?;
    Ok(())
}

#[tokio::test]
async fn retries_unavailable_history_and_remembers_rejected_evidence() -> anyhow::Result<()> {
    core_test_support::skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let test = test_codex()
        .with_history_mode(ThreadHistoryMode::Paginated)
        .build_with_auto_env(&server)
        .await?;
    let root = test.session_configured.thread_id;
    let history = &test.thread_store;
    history
        .persist_thread(root, PersistContext::Standard)
        .await?;
    let cursor = EvidenceCursor::from_now(history.as_ref(), root).await?;
    let delivered = cursor.position();
    let (evidence, _writer) = RootEvidence::new(Default::default(), history.clone(), cursor);

    let path = test
        .session_configured
        .rollout_path
        .as_deref()
        .context("rollout path")?;
    let hidden = path.with_extension("unavailable");
    tokio::fs::rename(path, &hidden).await?;
    let unavailable = evidence.enqueue(/*cutoff*/ None).await;
    tokio::fs::rename(hidden, path).await?;
    assert!(unavailable.is_err());
    evidence.enqueue(/*cutoff*/ None).await?;

    history
        .append_items(AppendThreadItemsParams {
            thread_id: root,
            items: vec![RolloutItem::ResponseItem(ResponseItemEnvelope::new(
                user_message_item("Missing provenance"),
            ))],
        })
        .await?;
    let rejected = evidence
        .enqueue(/*cutoff*/ None)
        .await
        .err()
        .context("expected rejected evidence")?;
    let reason = "user evidence has no host acceptance or context provenance";
    assert_eq!(rejected.to_string(), reason);
    test.codex.shutdown_and_wait().await?;
    // A fresh flush would now fail. The review must still get the original rejection.
    assert!(history.flush_thread(root).await.is_err());
    let error = evidence
        .capture_cutoff()
        .await
        .err()
        .context("expected a rejected cutoff")?;
    assert_eq!(error.to_string(), reason);
    for cutoff in [None, Some(delivered)] {
        let error = evidence
            .request_through(cutoff)
            .await
            .err()
            .context("expected a rejected cutoff")?;
        assert_eq!(error.to_string(), reason);
    }
    Ok(())
}

#[test_case::test_case(false; "on_disk")]
#[test_case::test_case(true; "in_memory")]
#[tokio::test]
async fn catch_up_waits_for_committed_delivery_and_allows_later_posts(
    in_memory: bool,
) -> anyhow::Result<()> {
    use codex_extension_api::ExtensionData;
    use codex_extension_api::ExtensionRegistryBuilder;
    use codex_extension_api::ThreadReadyInput;
    use codex_extension_api::ThreadStopInput;
    use codex_message_board::ReadPostRequest;
    use codex_protocol::error::CodexErrorDetails;
    use codex_protocol::protocol::SessionSource;
    use std::num::NonZeroU32;

    core_test_support::skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let home = Arc::new(tempfile::tempdir()?);
    let settings = if in_memory {
        "[features.guardianv3]\nenabled = true\nmessage_board_in_memory = true"
    } else {
        "[features.guardianv3]\nenabled = true"
    };
    std::fs::write(home.path().join("config.toml"), settings)?;
    let test = test_codex()
        .with_home(home)
        .with_history_mode(ThreadHistoryMode::Paginated)
        .with_model_info_override("gpt-5.6-luna", |_| {})
        .with_config(|config| {
            config.approvals_reviewer = ApprovalsReviewer::User;
        })
        .build_with_auto_env(&server)
        .await?;
    let root = test.session_configured.thread_id;
    let host = Arc::new(PausedClockHost {
        inner: board_host(root),
        pause_next: AtomicBool::new(false),
        started: Notify::new(),
        release: Notify::new(),
    });
    let mut registry = ExtensionRegistryBuilder::default();
    crate::install(
        &mut registry,
        Arc::downgrade(&test.thread_manager),
        &test.config,
        {
            let host = host.clone();
            move |_| host.clone()
        },
    );
    let registry = registry.build();
    let session = ExtensionData::new(root.to_string());
    let thread = test.codex.thread_extension_data();
    let lifecycle = &registry.thread_lifecycle_contributors()[0];
    let ready = |history_initialization| ThreadReadyInput {
        config: &test.config,
        session_source: &SessionSource::Exec,
        history_initialization,
        history_mode: ThreadHistoryMode::Paginated,
        session_store: &session,
        thread_store: thread,
    };
    if in_memory {
        lifecycle
            .on_thread_ready(ready(HistoryInitialization::ColdResume))
            .await;
        assert!(thread.get::<RootEvidence>().is_none());
        assert!(
            thread
                .get::<Arc<dyn codex_core::guardian_review::AttachedReviewer>>()
                .is_some()
        );
        lifecycle
            .on_thread_stop(ThreadStopInput {
                thread_store: thread,
                session_store: &session,
            })
            .await;
    }
    lifecycle
        .on_thread_ready(ready(HistoryInitialization::New))
        .await;
    let evidence = thread.get::<RootEvidence>().context("root intake")?;
    let worker = thread
        .get::<crate::host::RootWorker>()
        .context("root worker")?;
    let board = &worker.reader;
    let denied = board
        .upsert_channel(
            root,
            UpsertChannelRequest {
                channel_name: "reviews".into(),
                description: "Read-only".to_owned().try_into()?,
            },
        )
        .await
        .err()
        .context("reviewer unexpectedly updated a channel")?;
    assert!(
        matches!(denied.details(), CodexErrorDetails::Io(error) if error.kind() == std::io::ErrorKind::PermissionDenied)
    );
    let append_user_message = async |order, text: &str| -> anyhow::Result<serde_json::Value> {
        let item = RolloutItem::ResponseItem(ResponseItemEnvelope {
            item: user_message_item(text),
            metadata: Some(CodexHarnessMetadata {
                user_input_order: Some(order),
                ..Default::default()
            }),
        });
        let original = serde_json::to_value(&item)?;
        test.thread_store
            .append_items(AppendThreadItemsParams {
                thread_id: root,
                items: vec![item],
            })
            .await?;
        Ok(original)
    };
    // Create the channels through the housekeeper and subscribe the live reviewer.
    let _seed_route = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("seed", r#"{"routes":[["release","releases-copy"]],"descriptions":[{"channel":"release","description":"Release policy"},{"channel":"releases-copy","description":"Release copy"}]}"#),
            ev_completed("seed"),
        ]),
    ).await;
    append_user_message(1, "Set up the release channels.").await?;
    evidence.request_catch_up().await?.await?;
    let database = test
        .config
        .sqlite_config()
        .home()
        .join("message_board_guardian_1.sqlite");
    assert_eq!(tokio::fs::try_exists(database).await?, !in_memory);
    let binding = thread
        .get::<crate::ActorReviewer>()
        .context("attached reviewer")?
        .binding(Arc::clone(board))?;
    let inbox = &binding.inbox;
    for channel in ["release", "releases-copy"] {
        inbox
            .set_subscription(channel.into(), SubscriptionChange::Subscribe)
            .await?;
    }
    let _first_route = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message(
                "route1",
                r#"{"routes":[["release","releases-copy"]],"descriptions":[]}"#,
            ),
            ev_completed("route1"),
        ]),
    )
    .await;
    host.pause_next
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let first_original = append_user_message(2, "Stop the release.").await?;
    let wait = Duration::from_secs(15);
    let first = evidence.request_catch_up().await?;
    tokio::pin!(first);
    tokio::time::timeout(wait, host.started.notified()).await?;
    assert!(futures::poll!(&mut first).is_pending());
    assert_eq!(inbox.len(), 0);

    let _second_route = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("route2", r#"{"routes":[["release"]],"descriptions":[]}"#),
            ev_completed("route2"),
        ]),
    )
    .await;
    let second_original = append_user_message(3, "New revocation.").await?;
    let second = tokio::time::timeout(wait, evidence.request_catch_up()).await??;
    host.release.notify_one();
    let first_position = first.await?;
    let second_position = second.await?;
    assert!(second_position.end_ordinal_exclusive > first_position.end_ordinal_exclusive);
    // Either channel's copy of the first original is valid. Resolve the IDs the worker selected.
    let mut queued_originals = Vec::new();
    for (ordinal, message_id) in inbox.queued_posts() {
        let post = board
            .read_post(
                root,
                ReadPostRequest {
                    message_id,
                    offset_chars: 0,
                    limit_chars: NonZeroU32::MAX,
                },
            )
            .await?;
        let mut record: serde_json::Value = serde_json::from_str(&post.text)?;
        assert_eq!(record["id"]["ordinal"], ordinal);
        queued_originals.push(record["original"].take());
    }
    assert_eq!(queued_originals, [first_original, second_original]);
    lifecycle
        .on_thread_stop(ThreadStopInput {
            thread_store: thread,
            session_store: &session,
        })
        .await;
    test.codex.shutdown_and_wait().await?;
    Ok(())
}

#[tokio::test]
async fn committed_posts_reach_current_subscribers_once_across_channel_retries()
-> anyhow::Result<()> {
    use crate::EvidenceId;
    use crate::inbox::ActorInbox;
    use crate::inbox::LiveDelivery;
    use codex_guardian_reviewer::ReviewDeadline;
    use codex_message_board::AppendRequest;
    use codex_message_board::LocalAgentMessageBoard;
    use codex_message_board::PostDestination;
    use codex_protocol::AgentPath;
    use codex_protocol::ThreadId;
    use codex_protocol::protocol::TruncationPolicy;
    use serde_json::json;
    use std::future::ready;
    use std::sync::atomic::Ordering;
    use tokio_util::sync::CancellationToken;

    let root = ThreadId::new();
    let subscriber = ThreadId::new();
    let subscriber_path = AgentPath::root()
        .join("subscriber")
        .map_err(anyhow::Error::msg)?;
    let host = board_host::board_host_with_members([
        (root, AgentPath::root()),
        (subscriber, subscriber_path),
    ]);
    let writer = LocalAgentMessageBoard::open_in_memory(
        &crate::local_board_options(),
        root.into(),
        host.clone(),
    )
    .await?;
    let reader = writer.read_only_handle();
    for channel in ["release", "audit"] {
        writer
            .upsert_channel(
                root,
                UpsertChannelRequest {
                    channel_name: channel.into(),
                    description: String::new().try_into()?,
                },
            )
            .await?;
    }
    let delivery = Arc::new(LiveDelivery::new(root));
    let inbox = delivery.register(subscriber);
    let subscribe = async |inbox: &Arc<ActorInbox>| {
        inbox
            .set_subscription("release".into(), SubscriptionChange::Subscribe)
            .await
    };
    let cancellation = CancellationToken::new();
    let append = async |ordinal, channel: &str, text: &str| {
        let id = EvidenceId { ordinal };
        delivery
            .append(
                &writer,
                AppendRequest {
                    request_id: id.request_id(channel)?,
                    destination: PostDestination::Channel(channel.into()),
                    text: serde_json::to_string(
                        &json!({"id":{"ordinal":ordinal}, "original":text}),
                    )?,
                },
                id,
                ReviewDeadline::Unlimited,
                &cancellation,
            )
            .await
    };

    append(/*ordinal*/ 1, "release", "before subscription").await?;
    subscribe(&inbox).await?;
    let large = "é".repeat(20_010);
    append(/*ordinal*/ 2, "release", &large).await?;
    let through = inbox.len();
    assert_eq!(through, 1);
    let audit_only = delivery.register(ThreadId::new());
    for subscriber in [&inbox, &audit_only] {
        subscriber
            .set_subscription("audit".into(), SubscriptionChange::Subscribe)
            .await?;
    }
    let mut wake = inbox.subscribe();
    let root_inbox = delivery.register(root);
    subscribe(&root_inbox).await?;
    let revoke = async |channel: &str| append(/*ordinal*/ 3, channel, "stop the release").await;
    revoke("release").await?;
    host.fail_clock.store(true, Ordering::SeqCst);
    assert!(revoke("audit").await.is_err());
    assert!(audit_only.queued_posts().is_empty());
    assert_eq!(*wake.borrow_and_update(), 2);
    assert_eq!(root_inbox.len(), 1);
    host.fail_clock.store(false, Ordering::SeqCst);

    // The root subscribes again after reload. Only the surviving child followed
    // release at the first write and still has that inbox when audit succeeds.
    root_inbox.close();
    let reloaded_inbox = delivery.register(root);
    assert!(
        reloaded_inbox
            .subscribed_channels(&Default::default())?
            .results
            .is_empty()
    );
    subscribe(&reloaded_inbox).await?;
    revoke("release").await?;
    assert!(audit_only.queued_posts().is_empty());
    revoke("audit").await?;
    assert!(matches!(audit_only.queued_posts().as_slice(), [(3, _)]));
    assert!(!wake.has_changed()?);
    assert_eq!(root_inbox.len(), 0);
    assert_eq!(reloaded_inbox.len(), 0);
    inbox
        .set_subscription("release".into(), SubscriptionChange::Unsubscribe)
        .await?;
    append(/*ordinal*/ 4, "release", "after reload").await?;
    assert_eq!(reloaded_inbox.len(), 1);
    assert!(host.notifications.lock().unwrap().is_empty());

    let mut delivered = 0;
    let policy = TruncationPolicy::Bytes(64 * 1024);
    assert!(
        inbox
            .deliver(subscriber, &reader, &mut delivered, through, policy, |_| {
                ready(Err(anyhow::anyhow!("reviewer busy")))
            })
            .await
            .is_err()
    );
    assert_eq!(delivered, 0);

    let mut received: Vec<codex_protocol::models::ResponseItem> = Vec::new();
    let mut accept = |items| {
        received.extend(items);
        ready(Ok(()))
    };
    for target in [through, inbox.len(), inbox.len()] {
        inbox
            .deliver(
                subscriber,
                &reader,
                &mut delivered,
                target,
                policy,
                &mut accept,
            )
            .await?;
    }
    let mut replacement = 0;
    inbox
        .deliver(
            subscriber,
            &reader,
            &mut replacement,
            inbox.len(),
            policy,
            &mut accept,
        )
        .await?;
    let first = json!({"id":{"ordinal":2}, "original":large});
    let next = json!({"id":{"ordinal":3}, "original":"stop the release"});
    let expected = |sources| {
        crate::sampling::context_input(
            "guardian_original_evidence",
            &json!({"sources":sources}),
            policy,
        )
    };
    assert_eq!(
        received,
        [
            expected(vec![&first])?,
            expected(vec![&next])?,
            expected(vec![&first, &next])?,
        ]
    );
    Ok(())
}
