#![cfg(test)]

use crate::EvidenceCursor;
use anyhow::Context;
use board_host::PausedClockHost;
use board_host::board_host;
use codex_core::guardian_review::PreserveReviewWrites;
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
use pretty_assertions::assert_eq;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use tokio::sync::Notify;

#[path = "../tests/common/board_host.rs"]
mod board_host;

#[tokio::test]
async fn skips_partial_deliveries_and_advances_past_their_ordinals() -> anyhow::Result<()> {
    core_test_support::skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let test = test_codex()
        .with_history_mode(ThreadHistoryMode::Paginated)
        .build_with_auto_env(&server)
        .await?;
    let root = test.session_configured.thread_id;
    let history = &test.thread_store;
    let mut cursor = EvidenceCursor::at_thread_ready(
        history.as_ref(),
        root,
        ThreadHistoryMode::Paginated,
        HistoryInitialization::New,
    )
    .await?;
    // The initial rollout header advances the writer even without evidence to route.
    history
        .persist_thread(root, PersistContext::Standard)
        .await?;
    history.flush_thread(root).await?;
    let first = cursor.capture(history.as_ref()).await?;
    assert!(first.records().is_empty());
    cursor.acknowledge(&first, /*count*/ 0)?;

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
    history.flush_thread(root).await?;
    let routed = cursor.capture(history.as_ref()).await?;
    let [next] = routed.records() else {
        anyhow::bail!("expected only the complete delivery")
    };
    let skipped = first.cutoff().end_ordinal_exclusive;
    assert_eq!(routed.skipped_incomplete_deliveries, [skipped]);
    assert_eq!(next.id.ordinal, skipped + 1);
    cursor.acknowledge(&routed, /*count*/ 1)?;
    assert!(!cursor.capture(history.as_ref()).await?.source_advanced());
    test.codex.shutdown_and_wait().await?;
    Ok(())
}

#[test_case::test_case(false; "on_disk")]
#[test_case::test_case(true; "in_memory")]
#[tokio::test]
async fn activity_routes_committed_posts_in_source_order(in_memory: bool) -> anyhow::Result<()> {
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
        .with_thread_extension(PreserveReviewWrites)
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
        assert!(thread.get::<crate::host::RootWorker>().is_none());
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
    let mut routed = NamedTraceEvents::subscribe("guardian_v3.housekeeping_routed");
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
    append_user_message(/*order*/ 1, "Set up the release channels.").await?;
    super::wake_worker(thread);
    routed.next().await?;
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
    let first_original = append_user_message(/*order*/ 2, "Stop the release.").await?;
    let wait = Duration::from_secs(15);
    super::wake_worker(thread);
    tokio::time::timeout(wait, host.started.notified()).await?;
    assert_eq!(inbox.len(), 0);

    let _second_route = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("route2", r#"{"routes":[["release"]],"descriptions":[]}"#),
            ev_completed("route2"),
        ]),
    )
    .await;
    let second_original = append_user_message(/*order*/ 3, "New revocation.").await?;
    super::wake_worker(thread);
    host.release.notify_one();
    routed.next().await?;
    routed.next().await?;
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

    // Reject unattributed user-role records without advancing the surviving tree's cursor.
    let published = inbox.delivery.cursor();
    test.thread_store
        .append_items(AppendThreadItemsParams {
            thread_id: root,
            items: vec![RolloutItem::ResponseItem(ResponseItemEnvelope::new(
                user_message_item("Missing provenance"),
            ))],
        })
        .await?;
    super::wake_worker(thread);
    tokio::time::timeout(wait, worker.wake.closed()).await?;
    assert_eq!(inbox.delivery.cursor(), published);
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

#[tokio::test]
async fn shadow_intake_enrolls_only_new_opted_in_roots() -> anyhow::Result<()> {
    use codex_core::ConfigRefreshOutcome;
    use codex_core::InheritedSessionId;
    use codex_core::config::Config;
    use codex_core::config::LoaderOverrides;
    use codex_core::guardian_review::AttachedReviewer;
    use codex_core::guardian_review::ShadowReviewer;
    use codex_extension_api::ExtensionData;
    use codex_extension_api::ExtensionRegistryBuilder;
    use codex_extension_api::ThreadReadyInput;
    use codex_extension_api::ThreadStopInput;
    use codex_features::Feature;
    use codex_protocol::AgentPath;
    use codex_protocol::protocol::AskForApproval;
    use codex_protocol::protocol::SessionSource;
    use codex_protocol::protocol::SubAgentSource;
    use core_test_support::CloudConfigBundleFixture;
    use core_test_support::LOCAL_FS;
    use core_test_support::load_guardian_policy_snapshot;
    use futures::FutureExt;

    type ReviewerChange = (ApprovalsReviewer, ApprovalsReviewer);
    struct RecordReviewerChange;

    impl codex_extension_api::ConfigContributor<Config> for RecordReviewerChange {
        fn on_config_changed(
            &self,
            _session: &ExtensionData,
            thread: &ExtensionData,
            previous: &Config,
            current: &Config,
        ) {
            thread.insert((previous.approvals_reviewer, current.approvals_reviewer));
        }
    }

    core_test_support::skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let mut unavailable = ExtensionRegistryBuilder::default();
    crate::install_shadow(&mut unavailable, std::sync::Weak::new(), |root| {
        board_host(root)
    });
    let unavailable = unavailable.build();
    // Drive startup below, but let the session publish policy changes to the real contributor.
    let mut settings = ExtensionRegistryBuilder::default();
    settings.config_contributor(Arc::clone(&unavailable.config_contributors()[0]));
    settings.config_contributor(Arc::new(RecordReviewerChange));
    let test = test_codex()
        .with_extensions(Arc::new(settings.build()))
        .with_config(|config| {
            config.features.enable(Feature::GuardianV2).unwrap();
            config.features.enable(Feature::GuardianV3Shadow).unwrap();
            config.requested_approvals_reviewer = Some(ApprovalsReviewer::AutoReview);
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            config
                .permissions
                .approval_policy
                .set(AskForApproval::OnRequest)
                .unwrap();
        })
        .with_history_mode(ThreadHistoryMode::Paginated)
        .build_with_auto_env(&server)
        .await?;
    let root = test.session_configured.thread_id;
    let session = ExtensionData::new(root.to_string());
    let thread = test.codex.thread_extension_data();
    let mut registry = ExtensionRegistryBuilder::default();
    crate::install_shadow(
        &mut registry,
        Arc::downgrade(&test.thread_manager),
        |root| board_host(root),
    );
    let registry = registry.build();
    let lifecycle = &registry.thread_lifecycle_contributors()[0];
    let mut config = test.config.clone();
    config.features.disable(Feature::GuardianV3Shadow)?;
    let mode = test
        .codex
        .thread_settings_snapshot()
        .await
        .collaboration_mode;
    let turn = ExtensionData::new("retry");
    let start = || codex_extension_api::TurnStartInput {
        turn_id: "retry",
        collaboration_mode: &mode,
        token_usage_at_turn_start: None,
        session_store: &session,
        thread_store: thread,
        turn_store: &turn,
    };
    let turns = &registry.turn_lifecycle_contributors()[0];
    // Force the first lookup to fail, then retry with the live manager on a turn.
    for (enrolled, history, expected) in [
        (false, HistoryInitialization::New, false),
        (true, HistoryInitialization::ColdResume, false),
        (true, HistoryInitialization::WarmFork, false),
        (true, HistoryInitialization::New, true),
    ] {
        if enrolled {
            config.features.enable(Feature::GuardianV3Shadow)?;
        }
        lifecycle
            .on_thread_start(codex_extension_api::ThreadStartInput {
                config: &config,
                session_source: &SessionSource::Exec,
                persistent_thread_state_available: true,
                environments: &[],
                mcp_resource_client: None,
                extension_metrics: None,
                session_store: &session,
                thread_store: thread,
            })
            .await;
        assert!(thread.get::<PreserveReviewWrites>().is_none());
        let ready = unavailable.thread_lifecycle_contributors()[0]
            .on_thread_ready(ThreadReadyInput {
                config: &config,
                session_source: &SessionSource::Exec,
                history_initialization: history,
                history_mode: ThreadHistoryMode::Paginated,
                session_store: &session,
                thread_store: thread,
            })
            .now_or_never();
        assert_eq!(ready, Some(()));
        assert!(thread.get::<ShadowReviewer>().is_none());
        if let Some(enrollment) = thread.get::<super::shadow::Enrollment>() {
            enrollment.drain().await;
            assert!(thread.get::<ShadowReviewer>().is_none());
            assert!(thread.get::<super::RootWorker>().is_none());
            assert_eq!(turns.on_turn_start(start()).now_or_never(), Some(()));
            enrollment.drain().await;
        }
        assert_eq!(thread.get::<ShadowReviewer>().is_some(), expected);
        assert_eq!(thread.get::<crate::host::RootWorker>().is_some(), expected);
        assert!(thread.get::<Arc<dyn AttachedReviewer>>().is_none());
    }
    let old_worker = thread.get::<super::RootWorker>().context("first board")?;
    let old_reviewer = thread.get::<ShadowReviewer>().context("first reviewer")?;
    let mut options = test.start_thread_options().await;
    options
        .thread_extension_init
        .insert(InheritedSessionId(root.into()));
    let child = test.thread_manager.start_thread(options).await?.thread;
    let child_store = child.thread_extension_data();
    let source = SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
        parent_thread_id: root,
        depth: 1,
        agent_path: Some(
            AgentPath::root()
                .join("worker")
                .map_err(anyhow::Error::msg)?,
        ),
        agent_nickname: None,
        agent_role: None,
    });
    lifecycle
        .on_thread_ready(ThreadReadyInput {
            config: &config,
            session_source: &source,
            history_initialization: HistoryInitialization::WarmFork,
            history_mode: ThreadHistoryMode::Paginated,
            session_store: &session,
            thread_store: child_store,
        })
        .await;
    let child_reviewer = child_store
        .get::<ShadowReviewer>()
        .context("child reviewer")?;
    assert!(child_store.get::<super::RootWorker>().is_none());

    let refresh = async |requirements: &str, expected: ApprovalsReviewer| -> anyhow::Result<()> {
        let loader = CloudConfigBundleFixture::loader_with_enterprise_requirement(requirements);
        let policy = load_guardian_policy_snapshot(
            LOCAL_FS.as_ref(),
            test.codex_home_path(),
            &LoaderOverrides::without_managed_config_for_tests(),
            &loader,
            test.codex.config().await.guardian_policy_revision + 1,
        )
        .await?;
        for actor in [&child, &test.codex] {
            let before = actor.thread_settings_snapshot().await.approvals_reviewer;
            let outcome = actor.refresh_guardian_policy(&policy).await;
            let after = actor.thread_settings_snapshot().await.approvals_reviewer;
            assert_ne!(before, expected);
            assert_eq!(outcome, ConfigRefreshOutcome::Published);
            assert_eq!(after, expected);
            let change = actor.thread_extension_data().remove::<ReviewerChange>();
            assert_eq!(change.as_deref(), Some(&(before, after)));
        }
        Ok(())
    };
    refresh(
        "allowed_approvals_reviewers = ['user']",
        ApprovalsReviewer::User,
    )
    .await?;
    assert!(old_reviewer.cancellation.is_cancelled());
    assert!(child_reviewer.cancellation.is_cancelled());
    assert!(thread.get::<ShadowReviewer>().is_none());
    assert!(thread.get::<super::RootWorker>().is_none());

    test.thread_store
        .persist_thread(root, PersistContext::Standard)
        .await?;
    let enrollment = thread
        .get::<super::shadow::Enrollment>()
        .context("enrollment")?;
    let disabled = test.codex.config().await;
    let settings = &registry.config_contributors()[0];
    // A delayed Auto Review notification must not override the root's live settings.
    settings.on_config_changed(&session, thread, &disabled, &config);
    assert_eq!(turns.on_turn_start(start()).now_or_never(), Some(()));
    enrollment.drain().await;
    assert!(thread.get::<ShadowReviewer>().is_none());
    assert!(thread.get::<super::RootWorker>().is_none());

    // Lift the restriction so the requested Auto Review setting takes effect again.
    refresh(
        "allowed_approvals_reviewers = ['user', 'auto_review']",
        ApprovalsReviewer::AutoReview,
    )
    .await?;
    // Keep the root eligible so only cancellation can stop the queued restart.
    assert_eq!(turns.on_turn_start(start()).now_or_never(), Some(()));
    settings.on_config_changed(&session, thread, &config, &disabled);
    enrollment.drain().await;
    assert!(thread.get::<ShadowReviewer>().is_none());
    assert!(thread.get::<super::RootWorker>().is_none());

    settings.on_config_changed(&session, thread, &disabled, &config);
    assert_eq!(turns.on_turn_start(start()).now_or_never(), Some(()));
    enrollment.drain().await;
    let new_worker = thread.get::<super::RootWorker>().context("fresh board")?;
    let new_reviewer = thread.get::<ShadowReviewer>().context("fresh reviewer")?;
    assert!(!Arc::ptr_eq(&old_worker.reader, &new_worker.reader));
    assert!(!Arc::ptr_eq(&old_reviewer.reviewer, &new_reviewer.reviewer));
    assert!(!new_reviewer.cancellation.is_cancelled());
    lifecycle
        .on_thread_stop(ThreadStopInput {
            thread_store: thread,
            session_store: &session,
        })
        .await;
    assert!(thread.get::<crate::host::RootWorker>().is_none());
    assert!(
        !config
            .sqlite_config()
            .home()
            .join("message_board_guardian_1.sqlite")
            .exists()
    );
    child.shutdown_and_wait().await?;
    test.codex.shutdown_and_wait().await?;
    Ok(())
}
