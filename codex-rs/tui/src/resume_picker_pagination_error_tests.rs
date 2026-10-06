//! Keeps loaded sessions selectable when background pagination fails.

use super::*;
use insta::assert_snapshot;
use pretty_assertions::assert_eq;
use std::sync::Mutex;

#[tokio::test]
async fn later_page_error_preserves_loaded_sessions() {
    for action in [SessionPickerAction::Resume, SessionPickerAction::Fork] {
        for query in ["", "unloaded session"] {
            let requests = Arc::new(Mutex::new(Vec::new()));
            let request_sink = Arc::clone(&requests);
            let loader: PickerLoader = Arc::new(move |request| {
                if let PickerLoadRequest::Page(request) = request {
                    request_sink.lock().unwrap().push(request);
                }
            });
            let mut state = PickerState::new(
                FrameRequester::test_dummy(),
                loader,
                ProviderFilter::Any,
                /*show_all*/ true,
                /*filter_cwd*/ None,
                action,
            );
            state.initial_page_mode = PageLoadMode::StateDbOnly;
            state.start_initial_load();
            let request = requests.lock().unwrap()[0].clone();
            let thread_id = ThreadId::from_string("019dabc1-0ef5-7431-b81c-03037f51f62c").unwrap();
            state
                .handle_background_event(BackgroundEvent::Page {
                    request_token: request.request_token,
                    search_token: request.search_token,
                    page: Ok(PickerPage {
                        rows: vec![Row {
                            path: None,
                            preview: "Loaded session".to_string(),
                            thread_id: Some(thread_id),
                            thread_name: None,
                            created_at: None,
                            updated_at: None,
                            cwd: None,
                            git_branch: None,
                        }],
                        history_modes: HashMap::new(),
                        next_cursor: Some(PageCursor::AppServer("next-page".to_string())),
                        num_scanned_files: 1,
                        reached_scan_cap: false,
                    }),
                })
                .await
                .unwrap();
            if query.is_empty() {
                state
                    .handle_key(KeyEvent::from(KeyCode::PageDown))
                    .await
                    .unwrap();
            } else {
                state.set_query(query.to_string());
            }
            let request = requests.lock().unwrap()[1].clone();
            state
                .handle_background_event(BackgroundEvent::Page {
                    request_token: request.request_token,
                    search_token: request.search_token,
                    page: Err(std::io::Error::other(
                        "failed to list threads from state database",
                    )),
                })
                .await
                .expect("later page errors must not close the picker");
            assert_eq!(state.all_rows.len(), 1);
            assert!(!state.pagination.is_loading());
            assert!(!state.search_state.is_active());
            assert!(state.pending_page_down_target.is_none());
            assert!(state.frozen_footer_percent.is_none());
            assert!(state.pagination.next_cursor.is_none());
            state.set_query(String::new());
            state.maybe_load_more_for_scroll();
            assert_eq!(requests.lock().unwrap().len(), 2);

            if matches!(action, SessionPickerAction::Resume) && query.is_empty() {
                assert_snapshot!(
                    "resume_picker_pagination_error",
                    search_line(&state, /*width*/ 80).to_string()
                );
            }
            let selection = state
                .handle_key(KeyEvent::from(KeyCode::Enter))
                .await
                .unwrap();
            let target = match (action, selection) {
                (SessionPickerAction::Resume, Some(SessionSelection::Resume(target)))
                | (SessionPickerAction::Fork, Some(SessionSelection::Fork(target))) => target,
                other => panic!("loaded session was not selectable: {other:?}"),
            };
            assert_eq!(target.thread_id, thread_id);
        }
    }
}
