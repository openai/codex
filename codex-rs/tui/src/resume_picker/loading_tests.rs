use std::future::pending;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use codex_app_server_protocol::Thread;
use codex_app_server_protocol::ThreadListCwdFilter;
use codex_app_server_protocol::ThreadSortKey;
use codex_app_server_protocol::ThreadSourceKind;
use codex_protocol::ThreadId;
use codex_utils_absolute_path::test_support::PathBufExt;
use codex_utils_absolute_path::test_support::test_path_buf;
use pretty_assertions::assert_eq;

use super::*;
use crate::resume_picker::picker_cwd_filter;

fn page_request(request_token: usize) -> PageLoadRequest {
    PageLoadRequest {
        cursor: None,
        request_token,
        search_token: None,
        cwd_filter: None,
        provider_filter: ProviderFilter::Any,
        sort_key: ThreadSortKey::UpdatedAt,
    }
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
        .expect("loader task should not panic");
}

#[test]
fn local_picker_thread_list_params_include_cwd_filter() {
    let cwd_filter = picker_cwd_filter(
        Path::new("/tmp/project"),
        /*show_all*/ false,
        /*uses_remote_workspace*/ false,
        /*remote_cwd_override*/ None,
    );
    let params = thread_list_params(
        Some(String::from("cursor-1")),
        cwd_filter.as_deref(),
        ProviderFilter::MatchDefault(String::from("openai")),
        ThreadSortKey::UpdatedAt,
        /*include_non_interactive*/ false,
    );

    assert_eq!(
        params.cwd,
        Some(ThreadListCwdFilter::One(String::from("/tmp/project")))
    );
}

#[test]
fn remote_thread_list_params_omit_provider_filter() {
    let params = thread_list_params(
        Some(String::from("cursor-1")),
        Some(Path::new("repo/on/server")),
        ProviderFilter::Any,
        ThreadSortKey::UpdatedAt,
        /*include_non_interactive*/ false,
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
}

#[test]
fn remote_thread_list_params_can_include_non_interactive_sources() {
    let params = thread_list_params(
        Some(String::from("cursor-1")),
        /*cwd_filter*/ None,
        ProviderFilter::Any,
        ThreadSortKey::UpdatedAt,
        /*include_non_interactive*/ true,
    );

    assert_eq!(params.cursor, Some(String::from("cursor-1")));
    assert_eq!(params.model_providers, None);
    let source_kinds = crate::resume_source_kinds(/*include_non_interactive*/ true);
    assert_eq!(params.source_kinds, Some(source_kinds));
}

#[test]
fn app_server_row_keeps_pathless_threads() {
    let thread_id = ThreadId::new();
    let thread = Thread {
        id: thread_id.to_string(),
        session_id: thread_id.to_string(),
        forked_from_id: None,
        parent_thread_id: None,
        preview: String::from("remote thread"),
        ephemeral: false,
        model_provider: String::from("openai"),
        created_at: 1,
        updated_at: 2,
        status: codex_app_server_protocol::ThreadStatus::Idle,
        path: None,
        cwd: test_path_buf("/tmp").abs(),
        cli_version: String::from("0.0.0"),
        source: codex_app_server_protocol::SessionSource::Cli,
        thread_source: None,
        agent_nickname: None,
        agent_role: None,
        git_info: None,
        name: Some(String::from("Named thread")),
        turns: Vec::new(),
    };

    let row = row_from_app_server_thread(thread).expect("row should be preserved");

    assert_eq!(row.path, None);
    assert_eq!(row.thread_id, Some(thread_id));
    assert_eq!(row.thread_name, Some(String::from("Named thread")));
}
