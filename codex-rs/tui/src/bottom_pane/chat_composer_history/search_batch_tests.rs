use pretty_assertions::assert_eq;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::unbounded_channel;

use super::*;
use crate::app_event::AppEvent;
use crate::app_event::HistoryBatchEntryResponse;

fn thread_id(value: u8) -> ThreadId {
    ThreadId::from_string(&format!("00000000-0000-0000-0000-{value:012}"))
        .expect("thread id should parse")
}

fn batch_entry(offset: usize, entry: &str) -> HistoryBatchEntryResponse {
    HistoryBatchEntryResponse {
        offset,
        entry: Some(entry.to_string()),
    }
}

fn found(entry: &str) -> HistorySearchResult {
    HistorySearchResult::Found(HistoryEntry::new(entry.to_string()))
}

fn history(
    entry_count: usize,
) -> (
    ChatComposerHistory,
    AppEventSender,
    UnboundedReceiver<AppEvent>,
) {
    let (tx, rx) = unbounded_channel();
    let tx = AppEventSender::new(tx);
    let mut history = ChatComposerHistory::new();
    history.set_metadata(thread_id(/*value*/ 1), /*log_id*/ 42, entry_count);
    (history, tx, rx)
}

fn recv_batch_request(rx: &mut UnboundedReceiver<AppEvent>) -> (HistoryBatchCursor, u64) {
    let AppEvent::LookupMessageHistoryBatch { cursor, log_id, .. } =
        rx.try_recv().expect("batch request")
    else {
        panic!("expected batch request");
    };
    (cursor, log_id)
}

fn recv_entry_request(rx: &mut UnboundedReceiver<AppEvent>) -> (usize, u64) {
    let AppEvent::LookupMessageHistoryEntry { offset, log_id, .. } =
        rx.try_recv().expect("entry request")
    else {
        panic!("expected entry request");
    };
    (offset, log_id)
}

fn start_older_search(
    history: &mut ChatComposerHistory,
    tx: &AppEventSender,
    rx: &mut UnboundedReceiver<AppEvent>,
    query: &str,
    newest_offset: usize,
) -> HistoryBatchCursor {
    assert_eq!(
        history.search(
            query,
            HistorySearchDirection::Older,
            /*restart*/ true,
            tx
        ),
        HistorySearchResult::Pending
    );
    let (offset, log_id) = recv_entry_request(rx);
    assert_eq!((offset, log_id), (newest_offset, 42));
    assert_eq!(
        history.on_entry_response(log_id, offset, Some("unrelated entry".to_string()), tx),
        HistoryEntryResponse::Search(HistorySearchResult::Pending)
    );
    let (cursor, _) = recv_batch_request(rx);
    assert_eq!(cursor.end_offset(), newest_offset - 1);
    cursor
}

#[test]
fn search_batch_late_data_is_cache_only_after_cancel_or_query_edit() {
    let (mut cancelled, tx, mut rx) = history(/*entry_count*/ 5);
    start_older_search(
        &mut cancelled,
        &tx,
        &mut rx,
        "cached",
        /*newest_offset*/ 4,
    );
    cancelled.reset_search();
    assert_eq!(
        cancelled.on_batch_response(
            /*log_id*/ 42,
            HistoryBatchCursor::new(/*end_offset*/ 3),
            vec![batch_entry(/*offset*/ 3, "cached match")],
            Some(HistoryBatchCursor::new(/*end_offset*/ 2)),
            &tx,
        ),
        None
    );
    assert_eq!(
        cancelled.search(
            "cached",
            HistorySearchDirection::Older,
            /*restart*/ true,
            &tx
        ),
        found("cached match")
    );
    assert!(rx.try_recv().is_err());

    let (mut edited, tx, mut rx) = history(/*entry_count*/ 5);
    start_older_search(&mut edited, &tx, &mut rx, "old", /*newest_offset*/ 4);
    assert_eq!(
        edited.search(
            "new",
            HistorySearchDirection::Older,
            /*restart*/ true,
            &tx
        ),
        HistorySearchResult::Pending
    );
    let (offset, _) = recv_entry_request(&mut rx);
    assert_eq!(offset, 3);
    assert_eq!(
        edited.on_batch_response(
            /*log_id*/ 42,
            HistoryBatchCursor::new(/*end_offset*/ 3),
            vec![batch_entry(/*offset*/ 3, "old data")],
            Some(HistoryBatchCursor::new(/*end_offset*/ 2)),
            &tx,
        ),
        None
    );
    assert_eq!(
        edited.on_entry_response(
            /*log_id*/ 42,
            /*offset*/ 3,
            Some("new current match".to_string()),
            &tx,
        ),
        HistoryEntryResponse::Search(found("new current match"))
    );
}

#[test]
fn search_batch_rejects_stale_thread_and_log_metadata() {
    let (mut history, tx, mut rx) = history(/*entry_count*/ 5);
    start_older_search(
        &mut history,
        &tx,
        &mut rx,
        "stale",
        /*newest_offset*/ 4,
    );
    history.set_metadata(
        thread_id(/*value*/ 2),
        /*log_id*/ 43,
        /*entry_count*/ 5,
    );
    assert_eq!(
        history.on_batch_response(
            /*log_id*/ 42,
            HistoryBatchCursor::new(/*end_offset*/ 3),
            vec![batch_entry(/*offset*/ 3, "stale match")],
            Some(HistoryBatchCursor::new(/*end_offset*/ 2)),
            &tx,
        ),
        None
    );
    assert!(history.fetched_history.is_empty());
}

#[test]
fn search_batch_read_failure_stops_after_bounded_retries() {
    let (mut history, tx, mut rx) = history(/*entry_count*/ 5);
    let cursor = start_older_search(
        &mut history,
        &tx,
        &mut rx,
        "retry",
        /*newest_offset*/ 4,
    );

    for _ in 0..MAX_BATCH_READ_RETRIES {
        assert_eq!(
            history.on_batch_error(/*log_id*/ 42, cursor, &tx),
            Some(HistorySearchResult::Pending)
        );
        let (retry_cursor, log_id) = recv_batch_request(&mut rx);
        assert_eq!((retry_cursor, log_id), (cursor, 42));
    }

    assert_eq!(
        history.on_batch_error(/*log_id*/ 42, cursor, &tx),
        Some(HistorySearchResult::Unavailable)
    );
    assert!(rx.try_recv().is_err());
    assert!(history.search.is_none());
    assert_eq!(
        history.search(
            "retry",
            HistorySearchDirection::Older,
            /*restart*/ false,
            &tx,
        ),
        HistorySearchResult::Pending
    );
    assert_eq!(recv_entry_request(&mut rx), (3, 42));
}

#[test]
fn search_batch_match_preserves_cursor_for_next_older_search() {
    let (mut history, tx, mut rx) = history(/*entry_count*/ 7);
    let cursor = start_older_search(
        &mut history,
        &tx,
        &mut rx,
        "needle",
        /*newest_offset*/ 6,
    );
    let continuation = HistoryBatchCursor::new(/*end_offset*/ 2);

    assert_eq!(
        history.on_batch_response(
            /*log_id*/ 42,
            cursor,
            vec![
                batch_entry(/*offset*/ 5, "unrelated newer"),
                batch_entry(/*offset*/ 4, "needle first"),
                batch_entry(/*offset*/ 3, "unrelated older"),
            ],
            Some(continuation),
            &tx,
        ),
        Some(found("needle first"))
    );

    assert_eq!(
        history.search(
            "needle",
            HistorySearchDirection::Older,
            /*restart*/ false,
            &tx,
        ),
        HistorySearchResult::Pending
    );
    let (requested_cursor, _) = recv_batch_request(&mut rx);
    assert_eq!(requested_cursor, continuation);
    assert!(rx.try_recv().is_err());

    assert_eq!(
        history.on_batch_response(
            /*log_id*/ 42,
            continuation,
            vec![batch_entry(/*offset*/ 2, "needle second")],
            /*next_older_cursor*/ None,
            &tx,
        ),
        Some(found("needle second"))
    );
}

#[test]
fn search_batch_absent_1024_uses_one_single_and_eight_batches() {
    let (mut history, tx, mut rx) = history(/*entry_count*/ 1_024);
    let mut cursor = start_older_search(
        &mut history,
        &tx,
        &mut rx,
        "absent",
        /*newest_offset*/ 1_023,
    );
    let mut batches = 0;

    loop {
        let end_offset = cursor.end_offset();
        let start_offset = end_offset.saturating_sub(127);
        let entries = (start_offset..=end_offset)
            .rev()
            .map(|offset| batch_entry(offset, "unrelated entry"))
            .collect();
        let next_older_cursor = start_offset.checked_sub(1).map(HistoryBatchCursor::new);
        let expected = if next_older_cursor.is_some() {
            HistorySearchResult::Pending
        } else {
            HistorySearchResult::NotFound
        };
        assert_eq!(
            history.on_batch_response(/*log_id*/ 42, cursor, entries, next_older_cursor, &tx,),
            Some(expected)
        );
        batches += 1;
        let Some(expected_cursor) = next_older_cursor else {
            break;
        };
        let (next, _) = recv_batch_request(&mut rx);
        assert_eq!(next, expected_cursor);
        cursor = next;
    }

    assert_eq!((1 + batches, batches), (9, 8));
    assert!(rx.try_recv().is_err());
}
