//! Delegated tasks are discoverable through database listing before any user message.

use super::*;
use crate::SortDirection;
use crate::ThreadSortKey;
use codex_protocol::items::FunctionCallOutputItem;
use codex_protocol::models::FunctionCallOutputBody;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn delegated_thread_is_listed_before_user_followup() {
    for history_mode in [ThreadHistoryMode::Legacy, ThreadHistoryMode::Paginated] {
        let home = TempDir::new().expect("temp dir");
        let config = test_config(home.path());
        let runtime = codex_state::StateRuntime::init(
            config.sqlite.clone(),
            config.default_model_provider_id.clone(),
        )
        .await
        .expect("initialize state database");
        let store = Arc::new(LocalThreadStore::new(config, Some(runtime)));
        let thread_id = ThreadId::new();
        let mut params = create_thread_params(thread_id);
        params.history_mode = history_mode;
        let live_thread = LiveThread::create(store.clone(), params)
            .await
            .expect("create live thread");
        let preview = "Inspect <main> & report findings.";
        live_thread
            .append_items(&[RolloutItem::EventMsg(EventMsg::ItemCompleted(
                ItemCompletedEvent {
                    thread_id,
                    turn_id: "delegated-turn".to_string(),
                    item: TurnItem::FunctionCallOutput(FunctionCallOutputItem {
                        id: "delegated-output".to_string(),
                        name: "create_thread".to_string(),
                        namespace: Some("codex_app".to_string()),
                        output: FunctionCallOutputBody::Text(
                            "<codex_delegation>\n  <source_thread_id>source</source_thread_id>\n  <input>Inspect &lt;main&gt; &amp; report findings.</input>\n</codex_delegation>".to_string(),
                        ),
                    }),
                    started_at_ms: Some(0),
                    completed_at_ms: 0,
                },
            ))])
            .await
            .expect("append delegated input");
        live_thread.flush().await.expect("persist delegated input");

        let listed = store
            .list_threads(ListThreadsParams {
                page_size: 10,
                cursor: None,
                sort_key: ThreadSortKey::CreatedAt,
                sort_direction: SortDirection::Desc,
                allowed_sources: Vec::new(),
                model_providers: None,
                cwd_filters: None,
                section: None,
                project_id: None,
                archived: false,
                search_term: None,
                relation_filter: None,
                use_state_db_only: true,
            })
            .await
            .expect("list database threads");
        assert_eq!(
            listed
                .items
                .iter()
                .map(|thread| (
                    thread.thread_id,
                    thread.preview.as_str(),
                    thread.first_user_message.as_deref(),
                ))
                .collect::<Vec<_>>(),
            vec![(thread_id, preview, None)],
            "history mode: {history_mode:?}",
        );
        live_thread.shutdown().await.expect("shutdown live thread");
    }
}
