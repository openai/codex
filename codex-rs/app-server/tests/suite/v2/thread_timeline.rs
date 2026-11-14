use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_mock_responses_server_repeating_assistant;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::SortDirection;
use codex_app_server_protocol::ThreadItemPayload;
use codex_app_server_protocol::ThreadItemsListParams;
use codex_app_server_protocol::ThreadItemsListResponse;
use codex_app_server_protocol::ThreadRealtimeItemContent;
use codex_app_server_protocol::ThreadTimelineEntry;
use codex_app_server_protocol::ThreadTimelineListParams;
use codex_app_server_protocol::ThreadTimelineListResponse;
use codex_protocol::ThreadId;
use codex_protocol::items::TurnItem;
use codex_protocol::items::UserMessageItem;
use codex_protocol::models::BaseInstructions;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ItemCompletedEvent;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_protocol::protocol::ThreadMemoryMode;
use codex_protocol::protocol::TurnCompleteEvent;
use codex_protocol::protocol::TurnStartedEvent;
use codex_protocol::realtime::RealtimeItem;
use codex_protocol::realtime::RealtimeItemContent;
use codex_protocol::realtime::RealtimeTranscriptRole;
use codex_protocol::user_input::UserInput;
use codex_rollout::RolloutItem;
use codex_thread_store::AppendThreadItemsParams;
use codex_thread_store::CreateThreadParams;
use codex_thread_store::ForkBoundary;
use codex_thread_store::LiveThread;
use codex_thread_store::LocalThreadStore;
use codex_thread_store::LocalThreadStoreConfig;
use codex_thread_store::PersistContext;
use codex_thread_store::PrepareForkParams;
use codex_thread_store::ThreadPersistenceMetadata;
use codex_thread_store::ThreadStore;
use codex_utils_absolute_path::test_support::PathExt;
use pretty_assertions::assert_eq;
use std::path::Path;
use std::sync::Arc;
use tempfile::TempDir;

async fn local_store(codex_home: &Path) -> Result<LocalThreadStore> {
    let sqlite = codex_state::SqliteConfig::new_for_testing(codex_home.abs());
    let state_db =
        codex_state::StateRuntime::init(sqlite.clone(), "mock_provider".to_string()).await?;
    Ok(LocalThreadStore::new(
        LocalThreadStoreConfig {
            codex_home: codex_home.to_path_buf(),
            sqlite,
            default_model_provider_id: "mock_provider".to_string(),
        },
        Some(state_db),
    ))
}

fn create_params(codex_home: &Path, thread_id: ThreadId) -> CreateThreadParams {
    CreateThreadParams {
        attached_thread: false,
        creator_user_id: None,
        creator_account_id: None,
        session_id: thread_id.into(),
        thread_id,
        extra_config: None,
        forked_from_id: None,
        parent_thread_id: None,
        source: SessionSource::Cli,
        thread_source: None,
        originator: "test_originator".to_string(),
        base_instructions: BaseInstructions::default(),
        dynamic_tools: Vec::new(),
        selected_capability_roots: Vec::new(),
        multi_agent_version: None,
        history_mode: ThreadHistoryMode::Paginated,
        history_base: None,
        subagent_history_start_ordinal: None,
        initial_window_id: codex_protocol::ContextWindowId::new_uuid_v7(),
        runtime_workspace_roots: None,
        metadata: ThreadPersistenceMetadata {
            cwd: Some(codex_home.to_path_buf()),
            model_provider: "mock_provider".to_string(),
            memory_mode: ThreadMemoryMode::Enabled,
        },
    }
}

#[tokio::test]
async fn timeline_pages_mix_items_and_resolve_the_opening_realtime_session() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri()).write(codex_home.path())?;

    let thread_id = ThreadId::default();
    let store = local_store(codex_home.path()).await?;
    store
        .create_thread(create_params(codex_home.path(), thread_id))
        .await?;
    store
        .persist_thread(thread_id, PersistContext::Standard)
        .await?;
    store
        .append_items(AppendThreadItemsParams {
            thread_id,
            items: vec![
                RolloutItem::RealtimeItem(RealtimeItem {
                    id: "voice-1:started".to_string(),
                    realtime_session_id: "voice-1".to_string(),
                    observed_at_ms: Some(1_000),
                    content: RealtimeItemContent::RealtimeSessionStarted,
                }),
                RolloutItem::EventMsg(EventMsg::TurnStarted(TurnStartedEvent {
                    turn_attribution: None,
                    turn_id: "turn-1".to_string(),
                    root_turn_id: None,
                    trace_id: None,
                    started_at: Some(10),
                    model_context_window: None,
                    collaboration_mode_kind: Default::default(),
                })),
                RolloutItem::EventMsg(EventMsg::ItemCompleted(ItemCompletedEvent {
                    content_token_count: None,
                    thread_id,
                    turn_id: "turn-1".to_string(),
                    item: TurnItem::UserMessage(UserMessageItem {
                        id: "user-1".to_string(),
                        client_id: None,
                        content: vec![UserInput::Text {
                            text: "check staging".to_string(),
                            text_elements: Vec::new(),
                        }],
                    }),
                    started_at_ms: Some(0),
                    completed_at_ms: 1,
                })),
                RolloutItem::RealtimeItem(RealtimeItem {
                    id: "voice-1:transcript-1".to_string(),
                    realtime_session_id: "voice-1".to_string(),
                    observed_at_ms: Some(2_100),
                    content: RealtimeItemContent::Frame {
                        channels: codex_protocol::realtime::RealtimeFrameChannels {
                            voice_transcript: codex_protocol::realtime::RealtimeVoiceTranscript {
                                role: RealtimeTranscriptRole::Assistant,
                                text: "Checking staging".to_string(),
                            },
                        },
                    },
                }),
                RolloutItem::EventMsg(EventMsg::TurnComplete(TurnCompleteEvent {
                    root_turn_id: None,
                    turn_id: "turn-1".to_string(),
                    last_agent_message: None,
                    error: None,
                    started_at: Some(10),
                    completed_at: Some(12),
                    duration_ms: Some(2000),
                    time_to_first_token_ms: None,
                })),
            ],
        })
        .await?;
    store.shutdown_thread(thread_id).await?;

    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized()
        .await?;
    let realtime: codex_app_server_protocol::ThreadRealtimeListResponse = app_server
        .request(|request_id| ClientRequest::ThreadRealtimeList {
            request_id,
            params: codex_app_server_protocol::ThreadRealtimeListParams {
                thread_id: thread_id.to_string(),
                session_id: Some("voice-1".into()),
                from_observed_at_ms: Some(1_000),
                to_observed_at_ms: Some(2_000),
                order: None,
                after_position: None,
                event_id: None,
                cursor: None,
                limit: Some(10),
                snapshot_end: None,
                required_through_position: None,
            },
        })
        .await?;
    assert_eq!(
        realtime.coverage,
        codex_app_server_protocol::ThreadRealtimeCoverage::Ready
    );
    assert_eq!(realtime.data.len(), 1);
    assert_eq!(realtime.data[0].item.id, "voice-1:started");
    assert_eq!(realtime.data[0].item.observed_at_ms, Some(1_000));

    let by_id: codex_app_server_protocol::ThreadRealtimeListResponse = app_server
        .request(|request_id| ClientRequest::ThreadRealtimeList {
            request_id,
            params: codex_app_server_protocol::ThreadRealtimeListParams {
                thread_id: thread_id.to_string(),
                event_id: Some("voice-1:transcript-1".into()),
                ..Default::default()
            },
        })
        .await?;
    let [entry] = by_id.data.as_slice() else {
        panic!("expected the persisted voice frame");
    };
    assert_eq!(entry.item.observed_at_ms, Some(2_100));
    assert!(matches!(
        &entry.item.content,
        ThreadRealtimeItemContent::Frame { channels }
            if channels.voice_transcript.text == "Checking staging"
    ));

    let page: ThreadTimelineListResponse = app_server
        .request(|request_id| ClientRequest::ThreadTimelineList {
            request_id,
            params: ThreadTimelineListParams {
                thread_id: thread_id.to_string(),
                cursor: None,
                limit: Some(3),
            },
        })
        .await?;
    assert_eq!(page.data.len(), 3);
    assert!(matches!(
        &page.data[0],
        ThreadTimelineEntry::Item { turn_id, item, .. }
            if turn_id == "turn-1"
                && matches!(&item.payload, ThreadItemPayload::UserMessage { .. })
                && item.started_at_ms == Some(0)
                && item.completed_at_ms == Some(1)
    ));
    assert!(matches!(
        &page.data[1],
        ThreadTimelineEntry::Realtime { item, .. }
            if matches!(
                &item.content,
                ThreadRealtimeItemContent::TranscriptSegment { text, .. }
                    if text == "Checking staging"
            )
    ));
    assert_eq!(
        page.active_realtime_session_at_page_start.as_deref(),
        Some("voice-1")
    );
    assert!(matches!(
        &page.data[2],
        ThreadTimelineEntry::TurnCompleted { turn_id, duration_ms: Some(2000), .. }
            if turn_id == "turn-1"
    ));

    let older: ThreadTimelineListResponse = app_server
        .request(|request_id| ClientRequest::ThreadTimelineList {
            request_id,
            params: ThreadTimelineListParams {
                thread_id: thread_id.to_string(),
                cursor: page.next_cursor,
                limit: Some(2),
            },
        })
        .await?;
    assert_eq!(older.data.len(), 2);
    assert!(matches!(
        &older.data[0],
        ThreadTimelineEntry::Realtime { item, .. }
            if matches!(
                item.content,
                ThreadRealtimeItemContent::RealtimeSessionStarted
            )
    ));
    assert!(matches!(
        &older.data[1],
        ThreadTimelineEntry::TurnStarted { turn_id, started_at: Some(10), .. }
            if turn_id == "turn-1"
    ));
    assert_eq!(older.active_realtime_session_at_page_start, None);
    assert_eq!(older.next_cursor, None);

    let ordinary: ThreadItemsListResponse = app_server
        .request(|request_id| ClientRequest::ThreadItemsList {
            request_id,
            params: ThreadItemsListParams {
                thread_id: thread_id.to_string(),
                turn_id: None,
                cursor: None,
                limit: Some(10),
                sort_direction: Some(SortDirection::Asc),
                defer_output: None,
            },
        })
        .await?;
    assert_eq!(ordinary.data.len(), 1);
    assert_eq!(ordinary.data[0].turn_id, "turn-1");

    Ok(())
}

#[tokio::test]
async fn realtime_list_reads_only_frames_sealed_on_the_paginated_fork() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri()).write(codex_home.path())?;
    let store = Arc::new(local_store(codex_home.path()).await?);
    let parent_id = ThreadId::default();
    let child_id = ThreadId::default();
    let frame = |id: &str, session: &str| RealtimeItem {
        id: id.to_owned(),
        realtime_session_id: session.to_owned(),
        observed_at_ms: Some(1_000),
        content: RealtimeItemContent::Frame {
            channels: codex_protocol::realtime::RealtimeFrameChannels {
                voice_transcript: codex_protocol::realtime::RealtimeVoiceTranscript {
                    role: RealtimeTranscriptRole::User,
                    text: session.to_owned(),
                },
            },
        },
    };
    let parent =
        LiveThread::create(store.clone(), create_params(codex_home.path(), parent_id)).await?;
    parent.persist(PersistContext::Standard).await?;
    parent
        .append_items(&[RolloutItem::RealtimeItem(frame(
            "parent-frame",
            "parent-call",
        ))])
        .await?;
    parent.flush().await?;
    parent.shutdown().await?;

    let fork = store
        .prepare_fork(PrepareForkParams {
            thread_id: parent_id,
            boundary: ForkBoundary::Latest,
        })
        .await?;
    let child = LiveThread::create(
        store,
        CreateThreadParams {
            forked_from_id: Some(fork.source_thread_id),
            history_base: fork.history_base,
            ..create_params(codex_home.path(), child_id)
        },
    )
    .await?;
    child.persist(PersistContext::Standard).await?;
    child
        .append_items(&[RolloutItem::RealtimeItem(frame(
            "child-frame",
            "child-call",
        ))])
        .await?;
    child.flush().await?;
    child.shutdown().await?;
    drop(fork);

    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized()
        .await?;
    for event_id in [None, Some("child-frame".to_owned())] {
        let page: codex_app_server_protocol::ThreadRealtimeListResponse = app_server
            .request(|request_id| ClientRequest::ThreadRealtimeList {
                request_id,
                params: codex_app_server_protocol::ThreadRealtimeListParams {
                    thread_id: child_id.to_string(),
                    event_id,
                    ..Default::default()
                },
            })
            .await?;
        assert_eq!(
            page.coverage,
            codex_app_server_protocol::ThreadRealtimeCoverage::Ready
        );
        assert_eq!(
            page.data
                .iter()
                .map(|entry| entry.item.id.as_str())
                .collect::<Vec<_>>(),
            ["child-frame"],
        );
    }
    Ok(())
}
