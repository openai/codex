use std::future::pending;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use codex_app_server_protocol::SessionSource as ApiSessionSource;
use codex_app_server_protocol::Thread;
use codex_app_server_protocol::ThreadListCwdFilter;
use codex_app_server_protocol::ThreadSourceKind;
use codex_protocol::ThreadId;
use codex_utils_absolute_path::test_support::PathBufExt;
use codex_utils_absolute_path::test_support::test_path_buf;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tempfile::tempdir;

use super::*;
use crate::resume_picker::FrameRequester;
use crate::resume_picker::LoadTrigger;
use crate::resume_picker::SessionPickerAction;
use crate::resume_picker::SessionPickerViewPersistence;

fn page(
    rows: Vec<Row>,
    next_cursor: Option<&str>,
    num_scanned_files: usize,
    reached_scan_cap: bool,
) -> PickerPage {
    PickerPage {
        rows,
        next_cursor: next_cursor.map(|cursor| PageCursor::AppServer(cursor.to_string())),
        num_scanned_files,
        reached_scan_cap,
    }
}

fn page_only_loader(loader: impl Fn(PageLoadRequest) + Send + Sync + 'static) -> PickerLoader {
    Arc::new(move |request| {
        if let PickerLoadRequest::Page(request) = request {
            loader(request);
        }
    })
}

fn page_request(request_token: usize) -> PageLoadRequest {
    PageLoadRequest {
        cursor: None,
        request_token,
        search_token: None,
        cwd_filter: None,
        provider_filter: ProviderFilter::Any,
        sort_key: ThreadSortKey::UpdatedAt,
        seed_from_state_db: true,
    }
}

fn recording_picker_state() -> (PickerState, Arc<Mutex<Vec<PageLoadRequest>>>) {
    let recorded_requests = Arc::new(Mutex::new(Vec::new()));
    let request_sink = recorded_requests.clone();
    let loader = page_only_loader(move |request| {
        request_sink.lock().unwrap().push(request);
    });
    let mut state = PickerState::new(
        FrameRequester::test_dummy(),
        loader,
        ProviderFilter::MatchDefault(String::from("openai")),
        /*show_all*/ true,
        /*filter_cwd*/ None,
        SessionPickerAction::Resume,
    );
    state.initial_page_load = InitialPageLoad::state_db_first();
    (state, recorded_requests)
}

fn make_row(path: &str, ts: &str, preview: &str) -> Row {
    let timestamp = parse_timestamp_str(ts).expect("timestamp should parse");
    Row {
        path: Some(PathBuf::from(path)),
        preview: preview.to_string(),
        thread_id: None,
        thread_name: None,
        created_at: Some(timestamp),
        updated_at: Some(timestamp),
        cwd: None,
        git_branch: None,
    }
}

fn make_thread(thread_id: ThreadId) -> Thread {
    Thread {
        id: thread_id.to_string(),
        session_id: thread_id.to_string(),
        forked_from_id: None,
        parent_thread_id: None,
        preview: String::new(),
        ephemeral: false,
        model_provider: String::from("openai"),
        created_at: 1,
        updated_at: 2,
        status: codex_app_server_protocol::ThreadStatus::Idle,
        path: None,
        cwd: test_path_buf("/tmp").abs(),
        cli_version: String::from("0.0.0"),
        source: ApiSessionSource::Cli,
        thread_source: None,
        agent_nickname: None,
        agent_role: None,
        git_info: None,
        name: None,
        turns: Vec::new(),
    }
}

struct RolloutFixture {
    _temp_dir: TempDir,
    codex_home: PathBuf,
    path: PathBuf,
    thread_id: ThreadId,
}

fn write_rollout(cwd: &Path, model_provider: &str, source: &str, preview: &str) -> RolloutFixture {
    let temp_dir = tempdir().expect("tmpdir");
    let codex_home = temp_dir.path().to_path_buf();
    let day_dir = codex_home.join("sessions/2025/01/01");
    std::fs::create_dir_all(&day_dir).expect("sessions dir");
    let thread_id = ThreadId::new();
    let path = day_dir.join(format!("rollout-2025-01-01T00-00-00-{thread_id}.jsonl"));
    let session_meta = serde_json::json!({
        "timestamp": "2025-01-01T00:00:00Z",
        "type": "session_meta",
        "payload": {
            "id": thread_id,
            "timestamp": "2025-01-01T00:00:00Z",
            "cwd": cwd,
            "originator": "test",
            "cli_version": "test",
            "source": source,
            "model_provider": model_provider,
            "git": {
                "branch": "main"
            }
        }
    });
    let user_event = serde_json::json!({
        "timestamp": "2025-01-01T00:00:01Z",
        "type": "event_msg",
        "payload": {
            "type": "user_message",
            "message": preview,
            "kind": "plain"
        }
    });
    std::fs::write(&path, format!("{session_meta}\n{user_event}\n")).expect("write rollout");
    RolloutFixture {
        _temp_dir: temp_dir,
        codex_home,
        path,
        thread_id,
    }
}

fn validation(fixture: &RolloutFixture) -> SelectionValidation {
    SelectionValidation {
        path: fixture.path.clone(),
        thread_id: fixture.thread_id,
        thread_name: None,
        git_branch: None,
        codex_home: fixture.codex_home.clone(),
        cwd_filter: Some(PathBuf::from("/tmp/current")),
        provider_filter: ProviderFilter::MatchDefault(String::from("openai")),
        include_non_interactive: false,
        query: String::new(),
    }
}

#[test]
fn initial_page_load_tracks_one_time_seed_and_reconciliation() {
    let mut state = InitialPageLoad::state_db_first();

    assert!(state.begin_load());
    assert!(!state.begin_load());
    assert!(!state.is_provisional());

    state.mark_seeded();
    assert!(state.is_provisional());
    assert!(state.finish_reconciliation());
    assert!(!state.is_provisional());
    assert!(!state.finish_reconciliation());
}

#[tokio::test]
async fn loader_does_not_block_followup_requests_and_cancels_on_close() {
    struct SetOnDrop(Arc<AtomicBool>);

    impl Drop for SetOnDrop {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    let (request_tx, request_rx) = mpsc::unbounded_channel();
    let (signal_tx, mut signal_rx) = mpsc::unbounded_channel();
    let page_dropped = Arc::new(AtomicBool::new(false));
    let worker_page_dropped = page_dropped.clone();
    let worker = tokio::spawn(run_picker_loader(request_rx, move |request| {
        let signal_tx = signal_tx.clone();
        let page_dropped = worker_page_dropped.clone();
        async move {
            match request {
                PickerLoadRequest::Page(_) => {
                    let _drop_guard = SetOnDrop(page_dropped);
                    let _ = signal_tx.send("page");
                    pending::<()>().await;
                }
                PickerLoadRequest::Transcript { .. } => {
                    let _ = signal_tx.send("transcript");
                }
                PickerLoadRequest::Preview { .. } => {}
            }
        }
    }));

    request_tx
        .send(PickerLoadRequest::Page(page_request(
            /*request_token*/ 1,
        )))
        .expect("send page request");
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(1), signal_rx.recv())
            .await
            .expect("page request should start"),
        Some("page")
    );

    request_tx
        .send(PickerLoadRequest::Transcript {
            thread_id: ThreadId::new(),
        })
        .expect("send transcript request");
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(1), signal_rx.recv())
            .await
            .expect("transcript request should not wait for page load"),
        Some("transcript")
    );

    drop(request_tx);
    tokio::time::timeout(Duration::from_secs(1), worker)
        .await
        .expect("loader should stop promptly")
        .expect("loader task should not panic");
    assert!(page_dropped.load(Ordering::SeqCst));
}

#[tokio::test]
async fn page_loader_cancels_and_coalesces_obsolete_requests() {
    struct ActiveLoad(Arc<AtomicUsize>);

    impl ActiveLoad {
        fn start(active_loads: Arc<AtomicUsize>) -> Self {
            assert_eq!(active_loads.fetch_add(1, Ordering::SeqCst), 0);
            Self(active_loads)
        }
    }

    impl Drop for ActiveLoad {
        fn drop(&mut self) {
            self.0.fetch_sub(1, Ordering::SeqCst);
        }
    }

    let (request_tx, request_rx) = mpsc::unbounded_channel();
    let (started_tx, mut started_rx) = mpsc::unbounded_channel();
    let release = Arc::new(tokio::sync::Semaphore::new(0));
    let worker_release = release.clone();
    let active_loads = Arc::new(AtomicUsize::new(0));
    let worker_active_loads = active_loads.clone();
    let worker = tokio::spawn(run_page_loader(request_rx, move |request| {
        let started_tx = started_tx.clone();
        let release = worker_release.clone();
        let active_loads = worker_active_loads.clone();
        async move {
            let _active_load = ActiveLoad::start(active_loads);
            let _ = started_tx.send(request.request_token);
            let _permit = release
                .acquire_owned()
                .await
                .expect("release semaphore should remain open");
        }
    }));

    request_tx
        .send(page_request(/*request_token*/ 1))
        .expect("send first page");
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(1), started_rx.recv())
            .await
            .expect("first page should start"),
        Some(1)
    );
    request_tx
        .send(page_request(/*request_token*/ 2))
        .expect("send second page");
    request_tx
        .send(page_request(/*request_token*/ 3))
        .expect("send third page");
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(1), started_rx.recv())
            .await
            .expect("latest page should supersede the active load"),
        Some(3)
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(50), started_rx.recv())
            .await
            .is_err()
    );

    drop(request_tx);
    release.add_permits(1);
    tokio::time::timeout(Duration::from_secs(1), worker)
        .await
        .expect("page loader should stop")
        .expect("page loader should not panic");
    assert_eq!(active_loads.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn loader_bounds_concurrent_preview_reads() {
    let (request_tx, request_rx) = mpsc::unbounded_channel();
    let (started_tx, mut started_rx) = mpsc::unbounded_channel();
    let release = Arc::new(tokio::sync::Semaphore::new(0));
    let worker_release = release.clone();
    let worker = tokio::spawn(run_picker_loader(request_rx, move |request| {
        let started_tx = started_tx.clone();
        let release = worker_release.clone();
        async move {
            if let PickerLoadRequest::Preview { thread_id } = request {
                let _ = started_tx.send(thread_id);
                let _permit = release
                    .acquire_owned()
                    .await
                    .expect("release semaphore should remain open");
            }
        }
    }));

    for _ in 0..=MAX_CONCURRENT_PREVIEW_READS {
        request_tx
            .send(PickerLoadRequest::Preview {
                thread_id: ThreadId::new(),
            })
            .expect("send preview request");
    }
    for _ in 0..MAX_CONCURRENT_PREVIEW_READS {
        tokio::time::timeout(Duration::from_secs(1), started_rx.recv())
            .await
            .expect("preview should start")
            .expect("preview signal channel should remain open");
    }
    assert!(
        tokio::time::timeout(Duration::from_millis(50), started_rx.recv())
            .await
            .is_err()
    );

    release.add_permits(MAX_CONCURRENT_PREVIEW_READS + 1);
    tokio::time::timeout(Duration::from_secs(1), started_rx.recv())
        .await
        .expect("queued preview should start")
        .expect("preview signal channel should remain open");

    drop(request_tx);
    tokio::time::timeout(Duration::from_secs(1), worker)
        .await
        .expect("loader should stop")
        .expect("loader should not panic");
}

#[test]
fn rollout_file_name_requires_timestamp_and_thread_id() {
    let thread_id = ThreadId::new();
    let file_name = format!("rollout-2025-01-01T00-00-00-{thread_id}.jsonl");

    assert!(is_rollout_file_name(&file_name));
    assert!(is_rollout_file_name(&format!("{file_name}.zst")));
    assert!(!is_rollout_file_name(&format!("rollout-{thread_id}.jsonl")));
}

#[test]
fn state_db_page_params_honor_cwd_filter() {
    let params = thread_list_params(
        Some(String::from("cursor-1")),
        Some(Path::new("/tmp/project")),
        ProviderFilter::MatchDefault(String::from("openai")),
        ThreadSortKey::UpdatedAt,
        /*include_non_interactive*/ false,
        ThreadListLookupMode::StateDbOnly,
    );

    assert_eq!(
        params.cwd,
        Some(ThreadListCwdFilter::One(String::from("/tmp/project")))
    );
    assert!(params.use_state_db_only);

    let params = thread_list_params(
        /*cursor*/ None,
        /*cwd_filter*/ None,
        ProviderFilter::MatchDefault(String::from("openai")),
        ThreadSortKey::UpdatedAt,
        /*include_non_interactive*/ false,
        ThreadListLookupMode::StateDbOnly,
    );
    assert_eq!(params.cwd, None);
}

#[test]
fn remote_thread_list_params_omit_provider_filter() {
    let params = thread_list_params(
        Some(String::from("cursor-1")),
        Some(Path::new("repo/on/server")),
        ProviderFilter::Any,
        ThreadSortKey::UpdatedAt,
        /*include_non_interactive*/ false,
        ThreadListLookupMode::ScanAndRepair,
    );

    assert_eq!(params.cursor, Some(String::from("cursor-1")));
    assert_eq!(params.model_providers, None);
    assert_eq!(
        params.source_kinds,
        Some(vec![ThreadSourceKind::Cli, ThreadSourceKind::VsCode])
    );
    assert_eq!(
        params.cwd,
        Some(ThreadListCwdFilter::One(String::from("repo/on/server")))
    );
    assert!(!params.use_state_db_only);
}

#[test]
fn remote_thread_list_params_can_include_non_interactive_sources() {
    let params = thread_list_params(
        Some(String::from("cursor-1")),
        /*cwd_filter*/ None,
        ProviderFilter::Any,
        ThreadSortKey::UpdatedAt,
        /*include_non_interactive*/ true,
        ThreadListLookupMode::ScanAndRepair,
    );

    assert_eq!(params.cursor, Some(String::from("cursor-1")));
    assert_eq!(params.model_providers, None);
    let source_kinds = crate::resume_source_kinds(/*include_non_interactive*/ true);
    assert_eq!(params.source_kinds, Some(source_kinds));
}

#[test]
fn app_server_row_keeps_pathless_threads() {
    let thread_id = ThreadId::new();
    let mut thread = make_thread(thread_id);
    thread.preview = String::from("remote thread");
    thread.name = Some(String::from("Named thread"));

    let row = row_from_app_server_thread(thread).expect("row should be preserved");

    assert_eq!(row.path, None);
    assert_eq!(row.thread_id, Some(thread_id));
    assert_eq!(row.thread_name, Some(String::from("Named thread")));
}

#[tokio::test]
async fn local_picker_replaces_seed_page_and_preserves_selection() {
    let (mut state, recorded_requests) = recording_picker_state();
    let first_thread_id = ThreadId::new();
    let selected_thread_id = ThreadId::new();
    let replacement_thread_id = ThreadId::new();

    state.start_initial_load();

    let request = recorded_requests.lock().unwrap()[0].clone();
    assert!(request.seed_from_state_db);
    let mut first_row = make_row("/tmp/a.jsonl", "2025-01-03T00:00:00Z", "a");
    first_row.thread_id = Some(first_thread_id);
    let mut selected_row = make_row("/tmp/stale-b.jsonl", "2025-01-02T00:00:00Z", "b");
    selected_row.thread_id = Some(selected_thread_id);
    state
        .handle_background_event(BackgroundEvent::SeedPage {
            request_token: request.request_token,
            page: page(
                vec![first_row, selected_row],
                Some("db-cursor"),
                /*num_scanned_files*/ 2,
                /*reached_scan_cap*/ false,
            ),
        })
        .await
        .expect("State DB page should seed the picker");

    assert!(state.pagination.loading.is_pending());
    assert!(state.initial_page_load.is_provisional());
    state.selected = 1;
    state.load_more_if_needed(LoadTrigger::Scroll);
    assert_eq!(recorded_requests.lock().unwrap().len(), 1);
    let mut repaired_selected_row = make_row(
        "/tmp/repaired-b.jsonl",
        "2025-01-02T00:00:00Z",
        "b repaired",
    );
    repaired_selected_row.thread_id = Some(selected_thread_id);
    let mut replacement_row = make_row("/tmp/c.jsonl", "2025-01-01T00:00:00Z", "c");
    replacement_row.thread_id = Some(replacement_thread_id);

    state
        .handle_background_event(BackgroundEvent::Page {
            request_token: request.request_token,
            search_token: request.search_token,
            page: Ok(page(
                vec![repaired_selected_row, replacement_row],
                Some("scan-cursor"),
                /*num_scanned_files*/ 3,
                /*reached_scan_cap*/ false,
            )),
        })
        .await
        .expect("reconciled page should load");

    assert!(!state.pagination.loading.is_pending());
    assert!(!state.initial_page_load.is_provisional());
    assert_eq!(state.selected, 0);
    assert_eq!(
        state.filtered_rows[state.selected].thread_id,
        Some(selected_thread_id)
    );
    assert_eq!(
        state
            .filtered_rows
            .iter()
            .map(|row| row.preview.as_str())
            .collect::<Vec<_>>(),
        vec!["b repaired", "c"]
    );
    assert!(matches!(
        state.pagination.next_cursor.as_ref(),
        Some(PageCursor::AppServer(cursor)) if cursor == "scan-cursor"
    ));

    state.load_more_if_needed(LoadTrigger::Scroll);
    let requests = recorded_requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(!requests[1].seed_from_state_db);
    assert!(matches!(
        requests[1].cursor.as_ref(),
        Some(PageCursor::AppServer(cursor)) if cursor == "scan-cursor"
    ));
}

#[tokio::test]
async fn local_picker_keeps_seed_page_when_reconciliation_fails() {
    let (mut state, recorded_requests) = recording_picker_state();
    let thread_id = ThreadId::new();

    state.start_initial_load();

    let request = recorded_requests.lock().unwrap()[0].clone();
    assert!(request.seed_from_state_db);
    let mut provisional_row = make_row("/tmp/a.jsonl", "2025-01-03T00:00:00Z", "a");
    provisional_row.thread_id = Some(thread_id);
    state
        .handle_background_event(BackgroundEvent::SeedPage {
            request_token: request.request_token,
            page: page(
                vec![provisional_row],
                Some("db-cursor"),
                /*num_scanned_files*/ 1,
                /*reached_scan_cap*/ false,
            ),
        })
        .await
        .expect("State DB page should seed the picker");
    state
        .handle_background_event(BackgroundEvent::Page {
            request_token: request.request_token,
            search_token: request.search_token,
            page: Err(io::Error::other("scan failed")),
        })
        .await
        .expect("fast page should remain usable");

    assert!(!state.pagination.loading.is_pending());
    assert!(state.initial_page_load.is_provisional());
    assert_eq!(
        state
            .filtered_rows
            .iter()
            .map(|row| row.preview.as_str())
            .collect::<Vec<_>>(),
        vec!["a"]
    );
    assert!(state.pagination.next_cursor.is_none());
    assert_eq!(
        state.inline_error,
        Some(String::from(
            "Could not refresh sessions; showing the first page of indexed results"
        ))
    );

    state.load_more_if_needed(LoadTrigger::Scroll);
    assert_eq!(recorded_requests.lock().unwrap().len(), 1);
}

#[test]
fn reloads_do_not_reuse_initial_state_db_seed() {
    let (mut state, recorded_requests) = recording_picker_state();
    state.start_initial_load();
    state.initial_page_load.mark_seeded();
    state.replace_with_page(page(
        vec![make_row(
            "/tmp/provisional.jsonl",
            "2025-01-03T00:00:00Z",
            "provisional",
        )],
        /*next_cursor*/ None,
        /*num_scanned_files*/ 1,
        /*reached_scan_cap*/ false,
    ));

    state.toggle_sort_key();

    assert!(!state.initial_page_load.is_provisional());
    assert!(state.all_rows.is_empty());
    let requests = recorded_requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(requests[0].seed_from_state_db);
    assert!(!requests[1].seed_from_state_db);
    assert_eq!(requests[1].sort_key, ThreadSortKey::CreatedAt);
}

async fn assert_reconciliation_restarts_search(provisional_preview: &str) {
    let (mut state, recorded_requests) = recording_picker_state();

    state.start_initial_load();

    let initial_request = recorded_requests.lock().unwrap()[0].clone();
    state
        .handle_background_event(BackgroundEvent::SeedPage {
            request_token: initial_request.request_token,
            page: page(
                vec![make_row(
                    "/tmp/provisional.jsonl",
                    "2025-01-03T00:00:00Z",
                    provisional_preview,
                )],
                /*next_cursor*/ None,
                /*num_scanned_files*/ 1,
                /*reached_scan_cap*/ false,
            ),
        })
        .await
        .expect("State DB page should seed the picker");
    state.set_query(String::from("target"));
    assert!(!state.search_state.is_active());

    state
        .handle_background_event(BackgroundEvent::Page {
            request_token: initial_request.request_token,
            search_token: initial_request.search_token,
            page: Ok(page(
                vec![make_row(
                    "/tmp/reconciled.jsonl",
                    "2025-01-02T00:00:00Z",
                    "other",
                )],
                Some("scan-cursor"),
                /*num_scanned_files*/ 2,
                /*reached_scan_cap*/ false,
            )),
        })
        .await
        .expect("reconciled page should load");

    assert!(state.filtered_rows.is_empty());
    assert!(state.search_state.is_active());
    let requests = recorded_requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(requests[1].search_token.is_some());
    assert!(matches!(
        requests[1].cursor.as_ref(),
        Some(PageCursor::AppServer(cursor)) if cursor == "scan-cursor"
    ));
}

#[tokio::test]
async fn reconciliation_restarts_search_when_provisional_match_disappears() {
    assert_reconciliation_restarts_search("target").await;
}

#[tokio::test]
async fn reconciliation_restarts_search_when_authoritative_page_adds_cursor() {
    assert_reconciliation_restarts_search("other").await;
}

#[tokio::test]
async fn provisional_accept_rechecks_stale_cwd_from_rollout() {
    let fixture = write_rollout(Path::new("/tmp/on-disk"), "openai", "cli", "target preview");
    let loader = page_only_loader(|_| {});
    let mut state = PickerState::new(
        FrameRequester::test_dummy(),
        loader,
        ProviderFilter::MatchDefault(String::from("openai")),
        /*show_all*/ false,
        Some(PathBuf::from("/tmp/current")),
        SessionPickerAction::Resume,
    );
    state.view_persistence = Some(SessionPickerViewPersistence {
        codex_home: fixture.codex_home.clone(),
    });
    state.initial_page_load = InitialPageLoad::Provisional;
    let row = Row {
        path: Some(fixture.path),
        preview: String::from("target preview"),
        thread_id: Some(fixture.thread_id),
        thread_name: None,
        created_at: None,
        updated_at: None,
        cwd: Some(PathBuf::from("/tmp/current")),
        git_branch: None,
    };
    state.all_rows = vec![row.clone()];
    state.filtered_rows = vec![row];

    let selection = state
        .handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        .await
        .expect("validation failure should not abort the picker");

    assert!(selection.is_none());
    assert_eq!(
        state.inline_error,
        Some(String::from("Selected session is no longer available"))
    );
}

#[tokio::test]
async fn provisional_selection_validates_authoritative_filters() {
    let fixture = write_rollout(Path::new("/tmp/current"), "openai", "cli", "target preview");

    let target = validate_provisional_session_target(validation(&fixture))
        .await
        .expect("matching rollout should validate");
    assert_eq!(target.thread_id, fixture.thread_id);
    assert_eq!(target.path, Some(fixture.path.clone()));

    let mut stale_cwd = validation(&fixture);
    stale_cwd.cwd_filter = Some(PathBuf::from("/tmp/other"));
    assert_eq!(
        validate_provisional_session_target(stale_cwd)
            .await
            .expect_err("stale cwd should fail")
            .kind(),
        io::ErrorKind::NotFound
    );

    let mut stale_provider = validation(&fixture);
    stale_provider.provider_filter = ProviderFilter::MatchDefault(String::from("other"));
    assert_eq!(
        validate_provisional_session_target(stale_provider)
            .await
            .expect_err("stale provider should fail")
            .kind(),
        io::ErrorKind::NotFound
    );

    let mut stale_query = validation(&fixture);
    stale_query.query = String::from("missing");
    assert_eq!(
        validate_provisional_session_target(stale_query)
            .await
            .expect_err("stale query should fail")
            .kind(),
        io::ErrorKind::NotFound
    );

    let mut named_query = validation(&fixture);
    named_query.thread_name = Some(String::from("saved title"));
    named_query.query = String::from("saved title");
    validate_provisional_session_target(named_query)
        .await
        .expect("authoritative list reattaches the selected thread name");

    let mut stale_head_branch = validation(&fixture);
    stale_head_branch.git_branch = Some(String::from("db-branch"));
    stale_head_branch.query = String::from("main");
    assert_eq!(
        validate_provisional_session_target(stale_head_branch)
            .await
            .expect_err("DB branch should replace the rollout-head branch")
            .kind(),
        io::ErrorKind::NotFound
    );

    let mut db_branch = validation(&fixture);
    db_branch.git_branch = Some(String::from("db-branch"));
    db_branch.query = String::from("db-branch");
    validate_provisional_session_target(db_branch)
        .await
        .expect("query should match the branch retained by reconciliation");
}

#[tokio::test]
async fn provisional_selection_rechecks_source_and_thread_id() {
    let exec_fixture = write_rollout(
        Path::new("/tmp/current"),
        "openai",
        "exec",
        "target preview",
    );
    assert_eq!(
        validate_provisional_session_target(validation(&exec_fixture))
            .await
            .expect_err("interactive-only picker should reject exec sessions")
            .kind(),
        io::ErrorKind::NotFound
    );

    let mut include_non_interactive = validation(&exec_fixture);
    include_non_interactive.include_non_interactive = true;
    validate_provisional_session_target(include_non_interactive)
        .await
        .expect("all-source picker should accept exec sessions");

    let cli_fixture = write_rollout(Path::new("/tmp/current"), "openai", "cli", "target preview");
    let mut stale_id = validation(&cli_fixture);
    stale_id.thread_id = ThreadId::new();
    assert_eq!(
        validate_provisional_session_target(stale_id)
            .await
            .expect_err("stale thread id should fail")
            .kind(),
        io::ErrorKind::NotFound
    );
}

#[tokio::test]
async fn provisional_selection_rejects_non_discoverable_rollout_path() {
    let fixture = write_rollout(Path::new("/tmp/current"), "openai", "cli", "target preview");
    let outside_path = fixture.codex_home.join("rollout.jsonl");
    std::fs::copy(&fixture.path, &outside_path).expect("copy rollout");
    let mut outside = validation(&fixture);
    outside.path = outside_path;

    assert_eq!(
        validate_provisional_session_target(outside)
            .await
            .expect_err("rollout outside active sessions should fail")
            .kind(),
        io::ErrorKind::NotFound
    );

    let invalid_layout = fixture.codex_home.join("sessions/orphan.jsonl");
    std::fs::copy(&fixture.path, &invalid_layout).expect("copy rollout");
    let mut undiscoverable = validation(&fixture);
    undiscoverable.path = invalid_layout;
    assert_eq!(
        validate_provisional_session_target(undiscoverable)
            .await
            .expect_err("filesystem scan should not discover invalid layout")
            .kind(),
        io::ErrorKind::NotFound
    );

    let invalid_file_name = fixture.codex_home.join(format!(
        "sessions/2025/01/01/rollout-{}.jsonl",
        fixture.thread_id
    ));
    std::fs::copy(&fixture.path, &invalid_file_name).expect("copy rollout");
    let mut undiscoverable = validation(&fixture);
    undiscoverable.path = invalid_file_name;
    assert_eq!(
        validate_provisional_session_target(undiscoverable)
            .await
            .expect_err("filesystem scan should skip invalid rollout filename")
            .kind(),
        io::ErrorKind::NotFound
    );
}
