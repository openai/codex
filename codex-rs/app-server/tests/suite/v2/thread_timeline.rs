use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_mock_responses_server_repeating_assistant;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::SortDirection;
use codex_app_server_protocol::ThreadItem;
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
use codex_thread_store::LocalThreadStore;
use codex_thread_store::LocalThreadStoreConfig;
use codex_thread_store::PersistContext;
use codex_thread_store::ThreadPersistenceMetadata;
use codex_thread_store::ThreadStore;
use codex_utils_absolute_path::test_support::PathExt;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use uuid::Uuid;

#[tokio::test]
async fn timeline_pages_mix_items_and_resolve_the_opening_realtime_session() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri()).write(codex_home.path())?;

    let thread_id = ThreadId::default();
    let sqlite = codex_state::SqliteConfig::new_for_testing(codex_home.path().abs());
    let state_db =
        codex_state::StateRuntime::init(sqlite.clone(), "mock_provider".to_string()).await?;
    let store = LocalThreadStore::new(
        LocalThreadStoreConfig {
            codex_home: codex_home.path().to_path_buf(),
            sqlite,
            default_model_provider_id: "mock_provider".to_string(),
        },
        Some(state_db),
    );
    store
        .create_thread(CreateThreadParams {
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
            initial_window_id: Uuid::now_v7().to_string(),
            metadata: ThreadPersistenceMetadata {
                cwd: Some(codex_home.path().to_path_buf()),
                model_provider: "mock_provider".to_string(),
                memory_mode: ThreadMemoryMode::Enabled,
            },
        })
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
                    content: RealtimeItemContent::RealtimeSessionStarted,
                }),
                RolloutItem::EventMsg(EventMsg::TurnStarted(TurnStartedEvent {
                    turn_id: "turn-1".to_string(),
                    trace_id: None,
                    started_at: Some(10),
                    model_context_window: None,
                    collaboration_mode_kind: Default::default(),
                })),
                RolloutItem::EventMsg(EventMsg::ItemCompleted(ItemCompletedEvent {
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
                    content: RealtimeItemContent::TranscriptSegment {
                        role: RealtimeTranscriptRole::Assistant,
                        text: "Checking staging".to_string(),
                    },
                }),
                RolloutItem::EventMsg(EventMsg::TurnComplete(TurnCompleteEvent {
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
                && matches!(item.as_ref(), ThreadItem::UserMessage { .. })
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
            },
        })
        .await?;
    assert_eq!(ordinary.data.len(), 1);
    assert_eq!(ordinary.data[0].turn_id, "turn-1");

    Ok(())
}
