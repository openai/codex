use super::super::PickerLoadRequest;
use super::super::PickerLoader;
use super::super::PickerState;
use super::super::ProviderFilter;
use super::super::Row;
use super::super::SessionPickerAction;
use super::super::SessionPickerLaunchContext;
use super::super::SessionSelection;
use super::super::SessionStatus;
use super::super::SessionTarget;
use super::ArchiveState;
use crate::key_hint::KeyBinding;
use crate::tui::FrameRequester;
use codex_app_server_client::TypedRequestError;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::ThreadSortKey;
use codex_protocol::ThreadId;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

fn archive_picker_state() -> (PickerState, Arc<Mutex<Vec<ThreadId>>>) {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let request_sink = Arc::clone(&requests);
    let picker_loader: PickerLoader = Arc::new(move |request| {
        if let PickerLoadRequest::Archive { thread_id }
        | PickerLoadRequest::Unarchive { thread_id } = request
        {
            request_sink
                .lock()
                .expect("archive request sink")
                .push(thread_id);
        }
    });
    let state = PickerState::new(
        FrameRequester::test_dummy(),
        picker_loader,
        ProviderFilter::Any,
        /*show_all*/ true,
        /*filter_cwd*/ None,
        SessionPickerAction::Resume,
    );
    (state, requests)
}

fn set_selected_session(state: &mut PickerState, thread_id: ThreadId) {
    state.all_rows = vec![Row {
        path: None,
        preview: String::from("Selected session"),
        thread_id: Some(thread_id),
        thread_name: None,
        created_at: None,
        updated_at: None,
        cwd: None,
        git_branch: None,
    }];
    state.apply_filter();
}

#[tokio::test]
async fn archive_shortcut_archives_selected_session_once() {
    let (mut state, requests) = archive_picker_state();
    let thread_id = ThreadId::new();
    set_selected_session(&mut state, thread_id);
    let shortcut = KeyEvent::new(KeyCode::Char('\u{0001}'), KeyModifiers::NONE);

    assert!(state.handle_key(shortcut).await.unwrap().is_none());
    assert!(state.handle_key(shortcut).await.unwrap().is_none());
    assert_eq!(state.archive_state, ArchiveState::Pending { thread_id });
    assert_eq!(*requests.lock().unwrap(), vec![thread_id]);
    assert!(
        state
            .handle_key(KeyEvent::from(KeyCode::Enter))
            .await
            .unwrap()
            .is_none()
    );

    state.handle_archive_result(thread_id, Ok(()));

    assert_eq!(state.archive_state, ArchiveState::Idle);
    assert!(state.filtered_rows.is_empty());
}

#[test]
fn archive_request_rejects_current_session() {
    let (mut state, requests) = archive_picker_state();
    let thread_id = ThreadId::new();
    state.launch_context = SessionPickerLaunchContext::ExistingSession {
        current_thread_id: Some(thread_id),
    };
    set_selected_session(&mut state, thread_id);

    state.request_archive_for_selected_session();

    assert_eq!(state.archive_state, ArchiveState::Idle);
    assert_eq!(
        state.inline_error.as_deref(),
        Some("Use /archive to archive the current session and exit.")
    );
    assert!(requests.lock().unwrap().is_empty());
}

#[test]
fn archive_failure_preserves_session_and_reports_server_error() {
    let (mut state, requests) = archive_picker_state();
    let thread_id = ThreadId::new();
    set_selected_session(&mut state, thread_id);
    state.request_archive_for_selected_session();
    let error = TypedRequestError::Server {
        method: String::from("thread/archive"),
        source: JSONRPCErrorError {
            code: -32600,
            message: String::from("thread already has an active writer"),
            data: None,
        },
    };

    state.handle_archive_result(thread_id, Err(std::io::Error::other(error)));

    assert_eq!(state.archive_state, ArchiveState::Idle);
    assert_eq!(
        state.inline_error.as_deref(),
        Some(
            "Failed to archive session: thread/archive failed: thread already has an active writer (code -32600)"
        )
    );
    assert_eq!(state.filtered_rows.len(), 1);

    state.request_archive_for_selected_session();

    assert_eq!(*requests.lock().unwrap(), vec![thread_id, thread_id]);
}

#[test]
fn archive_shortcut_preserves_configured_list_binding() {
    let (mut state, _requests) = archive_picker_state();
    state.list_keymap.move_up.push(KeyBinding::new(
        KeyCode::Char('\u{0001}'),
        KeyModifiers::NONE,
    ));

    assert!(!state.archive_shortcut_available());
}

#[test]
fn archive_footer_shows_shortcut_for_resume_sessions() {
    let (mut state, _requests) = archive_picker_state();
    set_selected_session(&mut state, ThreadId::new());
    let footer = super::super::footer_hint_lines(&state, /*width*/ 220)
        .into_iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n");

    insta::assert_snapshot!(footer, @r"
     enter resume   ctrl+a archive   esc start new   ctrl+c quit   tab focus sort/filter   ←/→ change option
     ctrl+o dense view   ctrl+t transcript   ctrl+e expand   ↑/↓ browse
    ");
}

#[test]
fn archived_status_preserves_directory_filter_and_hides_archive_shortcut() {
    let requested_filters = Arc::new(Mutex::new(Vec::new()));
    let request_sink = Arc::clone(&requested_filters);
    let picker_loader: PickerLoader = Arc::new(move |request| {
        if let PickerLoadRequest::Page(request) = request {
            request_sink
                .lock()
                .expect("page request sink")
                .push((request.status, request.cwd_filter));
        }
    });
    let mut state = PickerState::new(
        FrameRequester::test_dummy(),
        picker_loader,
        ProviderFilter::Any,
        /*show_all*/ false,
        Some(PathBuf::from("/tmp/project")),
        SessionPickerAction::Resume,
    );

    state.toggle_status();

    assert_eq!(state.status, SessionStatus::Archived);
    let params = super::super::thread_list_params(
        /*cursor*/ None,
        Some(std::path::Path::new("/tmp/project")),
        SessionStatus::Archived,
        ProviderFilter::Any,
        ThreadSortKey::UpdatedAt,
        /*include_non_interactive*/ false,
        /*use_state_db_only*/ false,
    );
    assert_eq!(params.archived, Some(true));

    set_selected_session(&mut state, ThreadId::new());
    assert!(!state.archive_shortcut_available());
    let footer = super::super::footer_hint_lines(&state, /*width*/ 220)
        .into_iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(footer, @r"
     enter restore   esc start new   ctrl+c quit   tab focus sort/filter   ←/→ change option
     ctrl+o dense view   ctrl+t transcript   ctrl+e expand   ↑/↓ browse
    ");
    insta::assert_snapshot!(
        super::super::toolbar_line(&state, /*compact*/ true).to_string(),
        @"Filter:[Cwd] [Archived] Sort:[Updated]"
    );

    state.toggle_filter_mode();

    assert_eq!(
        *requested_filters.lock().unwrap(),
        vec![
            (SessionStatus::Archived, Some(PathBuf::from("/tmp/project")),),
            (SessionStatus::Archived, None),
        ]
    );
}

#[tokio::test]
async fn archived_session_restore_resumes_after_completion() {
    let (mut state, requests) = archive_picker_state();
    let thread_id = ThreadId::new();
    state.status = SessionStatus::Archived;
    set_selected_session(&mut state, thread_id);

    assert!(
        state
            .handle_key(KeyEvent::from(KeyCode::Enter))
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(state.archive_state, ArchiveState::Restoring { thread_id });
    assert_eq!(*requests.lock().unwrap(), vec![thread_id]);

    assert!(
        state
            .handle_key(KeyEvent::from(KeyCode::Enter))
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(*requests.lock().unwrap(), vec![thread_id]);

    let selection = state.handle_unarchive_result(
        thread_id,
        Ok(SessionTarget {
            path: None,
            thread_id,
        }),
    );

    assert!(matches!(
        selection,
        Some(SessionSelection::Resume(SessionTarget {
            path: None,
            thread_id: resumed_thread_id,
        })) if resumed_thread_id == thread_id
    ));
    assert_eq!(state.archive_state, ArchiveState::Idle);
}

#[test]
fn archived_session_restore_failure_preserves_session_and_allows_retry() {
    let (mut state, requests) = archive_picker_state();
    let thread_id = ThreadId::new();
    state.status = SessionStatus::Archived;
    set_selected_session(&mut state, thread_id);
    state.request_unarchive(thread_id);

    let selection =
        state.handle_unarchive_result(thread_id, Err(std::io::Error::other("restore denied")));

    assert!(selection.is_none());
    assert_eq!(state.archive_state, ArchiveState::Idle);
    assert_eq!(
        state.inline_error.as_deref(),
        Some("Failed to restore archived session: restore denied")
    );
    assert_eq!(state.filtered_rows.len(), 1);

    state.request_unarchive(thread_id);

    assert_eq!(*requests.lock().unwrap(), vec![thread_id, thread_id]);
}
