#![cfg(test)]

use super::Freeze;
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
use codex_message_board::PageRequest;
use codex_message_board::PostQuery;
use codex_protocol::AgentPath;
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
use std::num::NonZeroU32;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use tokio::sync::Notify;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::oneshot;

#[path = "../tests/common/board_host.rs"]
mod board_host;

#[tokio::test]
async fn skips_a_partial_delivery_once_across_wakes_and_freezes() -> anyhow::Result<()> {
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
    evidence.enqueue(/*reviewer*/ None).await?;
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
    evidence.enqueue(/*reviewer*/ None).await?;
    let routed = writer.try_recv()?.snapshot;
    let [next] = routed.records() else {
        anyhow::bail!("expected only the complete delivery")
    };
    let skipped = first.snapshot.cutoff().end_ordinal_exclusive;
    assert_eq!(routed.skipped_incomplete_deliveries, [skipped]);
    assert_eq!(next.id.ordinal, skipped + 1);
    warnings.next().await?;

    evidence.enqueue(/*reviewer*/ None).await?;
    assert!(matches!(writer.try_recv(), Err(TryRecvError::Empty)));

    // A review at that very same position still needs a response from the writer.
    let freeze = evidence.freeze(root, AgentPath::root());
    tokio::pin!(freeze);
    let review = tokio::select! {
        _ = &mut freeze => anyhow::bail!("freeze completed before the writer replied"),
        request = writer.recv() => request.context("writer closed")?,
    };
    assert!(futures::poll!(&mut freeze).is_pending());
    assert_eq!(review.snapshot.cutoff(), routed.cutoff());
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
    let unavailable = evidence.enqueue(/*reviewer*/ None).await;
    tokio::fs::rename(hidden, path).await?;
    assert!(unavailable.is_err());
    evidence.enqueue(/*reviewer*/ None).await?;

    history
        .append_items(AppendThreadItemsParams {
            thread_id: root,
            items: vec![RolloutItem::ResponseItem(ResponseItemEnvelope::new(
                user_message_item("Missing provenance"),
            ))],
        })
        .await?;
    let rejected = evidence
        .enqueue(/*reviewer*/ None)
        .await
        .err()
        .context("expected rejected evidence")?;
    let reason = "user evidence has no host acceptance or context provenance";
    assert_eq!(rejected.to_string(), reason);
    test.codex.shutdown_and_wait().await?;
    // A fresh flush would now fail. The review must still get the original rejection.
    assert!(history.flush_thread(root).await.is_err());
    let error = evidence
        .freeze(root, AgentPath::root())
        .await
        .err()
        .context("expected a rejected cutoff")?;
    assert_eq!(error.to_string(), reason);
    Ok(())
}

#[tokio::test]
async fn overlapping_reviews_only_expose_a_revocation_in_the_second_view() -> anyhow::Result<()> {
    use codex_extension_api::ExtensionData;
    use codex_extension_api::ExtensionRegistryBuilder;
    use codex_extension_api::ThreadReadyInput;
    use codex_extension_api::ThreadStopInput;
    use codex_protocol::protocol::SessionSource;

    core_test_support::skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let test = test_codex()
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
        pause_next: AtomicBool::new(true),
        started: Notify::new(),
        release: Notify::new(),
    });
    let mut registry = ExtensionRegistryBuilder::default();
    crate::install(
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
    let evidence = thread.get::<RootEvidence>().context("root intake")?;
    let _first_route = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message(
                "route1",
                r#"{"routes":[["release"]],"descriptions":{"release":"Release policy"}}"#,
            ),
            ev_completed("route1"),
        ]),
    )
    .await;
    let append_user_message = async |order, text: &str| {
        test.thread_store
            .append_items(AppendThreadItemsParams {
                thread_id: root,
                items: vec![RolloutItem::ResponseItem(ResponseItemEnvelope {
                    item: user_message_item(text),
                    metadata: Some(CodexHarnessMetadata {
                        user_input_order: Some(order),
                        ..Default::default()
                    }),
                })],
            })
            .await
    };
    append_user_message(1, "Stop the release.").await?;
    // The first review has captured its history once its board write reaches the gate.
    let wait = Duration::from_secs(15);
    let first = evidence.freeze(root, AgentPath::root());
    tokio::pin!(first);
    tokio::select! {
        result = &mut first => {
            result?;
            anyhow::bail!("first review finished before its board write")
        }
        started = tokio::time::timeout(wait, host.started.notified()) => started?,
    }

    let _second_route = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message(
                "route2",
                r#"{"routes":[["revocations"]],"descriptions":{"revocations":"Updated authorization"}}"#,
            ),
            ev_completed("route2"),
        ]),
    )
    .await;
    append_user_message(2, "New revocation.").await?;
    // Queue the second review behind the first write before allowing either view to open.
    let (reply, second) = oneshot::channel();
    let enqueue_second = evidence.enqueue(Some(Freeze {
        caller: root,
        path: AgentPath::root(),
        reply,
    }));
    tokio::time::timeout(wait, enqueue_second)
        .await
        .context("second review never entered the writer queue")??;
    host.release.notify_one();
    let first = first.await?;
    let second = second.await??;
    assert!(second.source.end_ordinal_exclusive > first.source.end_ordinal_exclusive);
    for (cutoff, query, expected) in [
        (&first, "Stop the release", 1),
        (&first, "New revocation", 0),
        (&second, "Stop the release", 1),
        (&second, "New revocation", 1),
    ] {
        let posts = cutoff
            .subscriptions
            .view()
            .search_posts(
                root,
                PostQuery {
                    channel_name: None,
                    query: Some(query.into()),
                    author: None,
                    after_message_id: None,
                    page: PageRequest::default(),
                    max_chars_per_post: NonZeroU32::MAX,
                },
            )
            .await?;
        assert_eq!(posts.results.len(), expected);
    }
    lifecycle
        .on_thread_stop(ThreadStopInput {
            thread_store: thread,
            session_store: &session,
        })
        .await;
    test.codex.shutdown_and_wait().await?;
    Ok(())
}
