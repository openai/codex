#![cfg(test)]
//! Source pagination, stale completions, and row-specific source routing.

use super::*;
use crate::resume_picker::tests::make_row;
use crate::resume_picker::tests::page;
use codex_protocol::ThreadId;
use pretty_assertions::assert_eq;

fn listed(thread: ThreadId, cursor: Option<&str>) -> PickerPage {
    let mut row = make_row(
        &format!("/tmp/{thread}.jsonl"),
        "2025-01-01T00:00:00Z",
        "conversation",
    );
    row.thread_id = Some(thread);
    page(
        vec![row],
        cursor,
        /*num_scanned_files*/ 1,
        /*reached_scan_cap*/ false,
    )
}

fn cursor(request: &PageRequest) -> Option<&str> {
    request.cursor.as_ref().map(|cursor| match cursor {
        PageCursor::AppServer(value) => value.as_str(),
        PageCursor::Catalog => panic!("unexpected catalog cursor"),
    })
}

fn begin_page(catalog: &mut ThreadCatalog, backend: BackendId) -> Option<PageRequest> {
    catalog.begin_page(backend, PageLoadMode::StoreDefault)
}

#[test]
fn sources_progress_independently_and_retry_their_own_cursor() {
    let mut catalog = ThreadCatalog::default();
    let slow = BackendId::new();
    let failed = BackendId::new();
    let healthy = BackendId::new();
    let slow_request = begin_page(&mut catalog, slow).unwrap();
    let first = begin_page(&mut catalog, failed).unwrap();
    catalog
        .finish_page(first, Ok(listed(ThreadId::new(), Some("failed-next"))))
        .unwrap()
        .unwrap();
    let failing_request = begin_page(&mut catalog, failed).unwrap();
    assert_eq!(cursor(&failing_request), Some("failed-next"));
    let error = catalog
        .finish_page(failing_request, Err(std::io::Error::other("unavailable")))
        .unwrap()
        .err()
        .unwrap();
    assert_eq!(error.to_string(), "unavailable");
    let healthy_request = begin_page(&mut catalog, healthy).unwrap();
    let thread = ThreadId::new();
    let result = catalog
        .finish_page(healthy_request, Ok(listed(thread, /*cursor*/ None)))
        .unwrap()
        .unwrap();
    assert_eq!(result.rows[0].backend_id, Some(healthy));
    assert!(begin_page(&mut catalog, healthy).is_none());
    assert!(begin_page(&mut catalog, slow).is_none());
    let retry = begin_page(&mut catalog, failed).unwrap();
    assert_eq!(cursor(&retry), Some("failed-next"));
    catalog
        .finish_page(retry, Ok(listed(ThreadId::new(), /*cursor*/ None)))
        .unwrap()
        .unwrap();
    catalog
        .finish_page(slow_request, Ok(listed(ThreadId::new(), /*cursor*/ None)))
        .unwrap()
        .unwrap();
    assert!(begin_page(&mut catalog, failed).is_none());
    assert!(begin_page(&mut catalog, slow).is_none());
}

#[test]
fn reset_rejects_old_completions_without_finishing_new_requests() {
    let mut catalog = ThreadCatalog::default();
    let backend = BackendId::new();
    let old = begin_page(&mut catalog, backend).unwrap();
    catalog.reset();
    let current = begin_page(&mut catalog, backend).unwrap();
    assert!(
        catalog
            .finish_page(old, Ok(listed(ThreadId::new(), /*cursor*/ None)))
            .is_none()
    );
    assert!(begin_page(&mut catalog, backend).is_none());
    let thread = ThreadId::new();
    let result = catalog
        .finish_page(current, Ok(listed(thread, /*cursor*/ None)))
        .unwrap()
        .unwrap();
    assert_eq!(result.rows[0].backend_id, Some(backend));
}

#[tokio::test]
async fn picker_selections_retain_each_rows_source() {
    use crate::resume_picker::SessionSelection;
    use crate::resume_picker::tests::local_db_first_state;
    use crossterm::event::KeyCode;
    use crossterm::event::KeyEvent;
    use crossterm::event::KeyModifiers;

    let (mut picker, _) = local_db_first_state();
    let owner = BackendId::new();
    let thread = ThreadId::new();
    let mut catalog = ThreadCatalog::default();
    let request = begin_page(&mut catalog, owner).unwrap();
    let page = catalog
        .finish_page(request, Ok(listed(thread, /*cursor*/ None)))
        .unwrap()
        .unwrap();
    picker.ingest_page(page);
    let other = BackendId::new();
    let other_thread = ThreadId::new();
    let other_request = begin_page(&mut catalog, other).unwrap();
    let other_page = catalog
        .finish_page(other_request, Ok(listed(other_thread, /*cursor*/ None)))
        .unwrap()
        .unwrap();
    picker.ingest_page(other_page);
    let selection = picker
        .handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        .await
        .unwrap();
    let Some(SessionSelection::Resume(target)) = selection else {
        panic!("expected selection")
    };
    assert_eq!((target.thread_id, target.backend_id), (thread, owner));
    picker
        .handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE))
        .await
        .unwrap();
    let selection = picker
        .handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        .await
        .unwrap();
    let Some(SessionSelection::Resume(target)) = selection else {
        panic!("expected selection")
    };
    assert_eq!((target.thread_id, target.backend_id), (other_thread, other));
}
