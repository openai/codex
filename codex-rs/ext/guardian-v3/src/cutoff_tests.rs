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
    evidence.enqueue(/*routed*/ None).await?;
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
    let mut warnings = NamedTraceEvents::subscribe("guardian_v3.evidence_skipped");
    evidence.enqueue(/*routed*/ None).await?;
    let routed = writer.try_recv()?.snapshot;
    let [next] = routed.records() else {
        anyhow::bail!("expected only the complete delivery")
    };
    let skipped = first.snapshot.cutoff().end_ordinal_exclusive;
    assert_eq!(routed.skipped_incomplete_deliveries, [skipped]);
    assert_eq!(next.id.ordinal, skipped + 1);
    warnings.next().await?;

    evidence.enqueue(/*routed*/ None).await?;
    assert!(matches!(writer.try_recv(), Err(TryRecvError::Empty)));

    // A review at that very same position still needs a response from the writer.
    let mut ready = evidence.request_catch_up().await?;
    assert!(futures::poll!(&mut ready).is_pending());
    writer.try_recv()?.finish();
    assert_eq!(ready.await?, routed.cutoff());
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
    let (evidence, _writer) = RootEvidence::new(Default::default(), history.clone(), cursor);

    let path = test
        .session_configured
        .rollout_path
        .as_deref()
        .context("rollout path")?;
    let hidden = path.with_extension("unavailable");
    tokio::fs::rename(path, &hidden).await?;
    let unavailable = evidence.enqueue(/*routed*/ None).await;
    tokio::fs::rename(hidden, path).await?;
    assert!(unavailable.is_err());
    evidence.enqueue(/*routed*/ None).await?;

    history
        .append_items(AppendThreadItemsParams {
            thread_id: root,
            items: vec![RolloutItem::ResponseItem(ResponseItemEnvelope::new(
                user_message_item("Missing provenance"),
            ))],
        })
        .await?;
    let rejected = evidence
        .enqueue(/*routed*/ None)
        .await
        .err()
        .context("expected rejected evidence")?;
    let reason = "user evidence has no host acceptance or context provenance";
    assert_eq!(rejected.to_string(), reason);
    test.codex.shutdown_and_wait().await?;
    // A fresh flush would now fail. The review must still get the original rejection.
    assert!(history.flush_thread(root).await.is_err());
    let error = evidence
        .request_catch_up()
        .await
        .err()
        .context("expected a rejected cutoff")?;
    assert_eq!(error.to_string(), reason);
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
        .binding(Arc::clone(board), &evidence.delivery)?;
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
    let mut first = evidence.request_catch_up().await?;
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
async fn pending_posts_belong_to_the_original_subscriber_inbox() -> anyhow::Result<()> {
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
    writer
        .upsert_channel(
            root,
            UpsertChannelRequest {
                channel_name: "release".into(),
                description: String::new().try_into()?,
            },
        )
        .await?;
    let delivery = Arc::new(LiveDelivery::new(root));
    let inbox = delivery.register(root);
    let subscribe = async |inbox: &Arc<ActorInbox>| {
        inbox
            .set_subscription("release".into(), SubscriptionChange::Subscribe)
            .await
    };
    let cancellation = CancellationToken::new();
    let append = async |ordinal, text: &str| {
        let mut recipients = crate::inbox::Recipients::default();
        delivery
            .append(
                &writer,
                AppendRequest {
                    request_id: format!("source-{ordinal}"),
                    destination: PostDestination::Channel("release".into()),
                    text: serde_json::to_string(
                        &json!({"id":{"ordinal":ordinal}, "original":text}),
                    )?,
                },
                &mut recipients,
                ReviewDeadline::Unlimited,
                &cancellation,
            )
            .await?;
        anyhow::Ok(recipients)
    };

    let before = append(1, "before subscription").await?;
    subscribe(&inbox).await?;
    before.publish(EvidenceId { ordinal: 1 })?;
    let large = "é".repeat(20_010);
    append(2, &large)
        .await?
        .publish(EvidenceId { ordinal: 2 })?;
    let through = inbox.len();
    assert_eq!(through, 1);
    let mut wake = inbox.subscribe();
    let child_inbox = delivery.register(subscriber);
    subscribe(&child_inbox).await?;
    let later = append(3, "later").await?;
    inbox
        .set_subscription("release".into(), SubscriptionChange::Unsubscribe)
        .await?;
    later.publish(EvidenceId { ordinal: 3 })?;
    wake.changed().await?;
    assert_eq!(*wake.borrow_and_update(), 2);
    assert_eq!(child_inbox.len(), 1);

    // Pause after the append chooses the child, then reload the same child ID.
    let pending = append(4, "during eviction").await?;
    child_inbox.close();
    let reloaded_inbox = delivery.register(subscriber);
    assert!(
        reloaded_inbox
            .subscribed_channels(&Default::default())?
            .results
            .is_empty()
    );
    subscribe(&reloaded_inbox).await?;
    pending.publish(EvidenceId { ordinal: 4 })?;
    assert_eq!(child_inbox.len(), 0);
    assert_eq!(reloaded_inbox.len(), 0);
    append(5, "after reload")
        .await?
        .publish(EvidenceId { ordinal: 5 })?;
    assert_eq!(reloaded_inbox.len(), 1);
    assert!(host.notifications.lock().unwrap().is_empty());

    let mut delivered = 0;
    let policy = TruncationPolicy::Bytes(64 * 1024);
    assert!(
        inbox
            .deliver(root, &reader, &mut delivered, through, policy, |_| ready(
                Err(anyhow::anyhow!("reviewer busy"))
            ))
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
            .deliver(root, &reader, &mut delivered, target, policy, &mut accept)
            .await?;
    }
    let mut replacement = 0;
    inbox
        .deliver(
            root,
            &reader,
            &mut replacement,
            inbox.len(),
            policy,
            &mut accept,
        )
        .await?;
    let first = json!({"id":{"ordinal":2}, "original":large});
    let next = json!({"id":{"ordinal":3}, "original":"later"});
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
