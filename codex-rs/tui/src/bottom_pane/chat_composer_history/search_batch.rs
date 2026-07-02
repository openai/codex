use super::ChatComposerHistory;
use super::HistoryEntry;
use super::HistorySearchDirection;
use super::HistorySearchResult;
use super::MAX_BATCH_READ_RETRIES;
use super::PendingHistorySearch;
use crate::app_event::AppEvent;
use crate::app_event::HistoryBatchEntryResponse;
use crate::app_event_sender::AppEventSender;
use codex_message_history::HistoryBatchCursor;

impl ChatComposerHistory {
    /// Applies a query-independent batch to the persistent cache and, when still applicable,
    /// resumes the active reverse search.
    pub(crate) fn on_batch_response(
        &mut self,
        log_id: u64,
        cursor: HistoryBatchCursor,
        entries: Vec<HistoryBatchEntryResponse>,
        next_older_cursor: Option<HistoryBatchCursor>,
        app_event_tx: &AppEventSender,
    ) -> Option<HistorySearchResult> {
        if self.persistent_log_id != Some(log_id) {
            return None;
        }

        let entries: Vec<_> = entries
            .into_iter()
            .map(|response| {
                let entry = response.entry.map(|text| {
                    HistoryEntry::new_with_at_mentions(text, self.at_mention_restore_enabled)
                });
                if entry.is_some() {
                    self.fetched_history.insert(response.offset, entry.clone());
                } else {
                    self.fetched_history.entry(response.offset).or_insert(None);
                }
                (response.offset, entry)
            })
            .collect();

        let (boundary_if_exhausted, _) = self.pending_batch(cursor)?;
        if let Some(search) = self.search.as_mut() {
            search.next_older_cursor = next_older_cursor;
        }

        for (offset, entry) in entries {
            if let Some(entry) = entry
                && self.search_matches(&entry)
                && self.search_result_is_unique(&entry)
            {
                return Some(self.search_match(offset, entry));
            }
        }

        let result = if let Some(next_cursor) = next_older_cursor {
            self.advance_older_search_with_batches_from(
                next_cursor,
                boundary_if_exhausted,
                app_event_tx,
            )
        } else {
            self.exhausted_search_result(HistorySearchDirection::Older, boundary_if_exhausted)
        };
        Some(result)
    }

    /// Retries a failed batch lookup up to a fixed limit without treating the failure as history
    /// exhaustion.
    pub(crate) fn on_batch_error(
        &mut self,
        log_id: u64,
        cursor: HistoryBatchCursor,
        app_event_tx: &AppEventSender,
    ) -> Option<HistorySearchResult> {
        if self.persistent_log_id != Some(log_id) {
            return None;
        }

        let (boundary_if_exhausted, read_failures) = self.pending_batch(cursor)?;

        if read_failures < MAX_BATCH_READ_RETRIES
            && let Some(thread_id) = self.thread_id
        {
            if let Some(search) = self.search.as_mut() {
                search.awaiting = Some(PendingHistorySearch::Batch {
                    cursor,
                    boundary_if_exhausted,
                    read_failures: read_failures + 1,
                });
            }
            app_event_tx.send(AppEvent::LookupMessageHistoryBatch {
                thread_id,
                cursor,
                log_id,
            });
            return Some(HistorySearchResult::Pending);
        }

        Some(if boundary_if_exhausted {
            if let Some(search) = self.search.as_mut() {
                search.awaiting = None;
            }
            HistorySearchResult::AtBoundary
        } else {
            self.search = None;
            HistorySearchResult::Unavailable
        })
    }

    fn pending_batch(&self, cursor: HistoryBatchCursor) -> Option<(bool, u8)> {
        let Some(PendingHistorySearch::Batch {
            cursor: awaited_cursor,
            boundary_if_exhausted,
            read_failures,
        }) = self.search.as_ref().and_then(|search| search.awaiting)
        else {
            return None;
        };
        (awaited_cursor == cursor).then_some((boundary_if_exhausted, read_failures))
    }

    /// Switches an older search from the single newest-entry probe to bounded batch lookups.
    ///
    /// Keeping the first probe on the single-entry path avoids fetching a batch when the newest
    /// persistent entry matches, while every later miss is amortized across a bounded batch.
    pub(super) fn advance_older_search_after_entry_miss(
        &mut self,
        offset: usize,
        boundary_if_exhausted: bool,
        app_event_tx: &AppEventSender,
    ) -> HistorySearchResult {
        let Some(next_offset) = offset.checked_sub(1) else {
            return self
                .exhausted_search_result(HistorySearchDirection::Older, boundary_if_exhausted);
        };
        self.advance_older_search_with_batches_from(
            HistoryBatchCursor::new(next_offset),
            boundary_if_exhausted,
            app_event_tx,
        )
    }

    /// Scans cached offsets from a batch cursor and requests the first uncached older range.
    ///
    /// The byte anchor remains usable only while scanning begins at its exact `end_offset`; moving
    /// past cached entries creates an offset-only cursor because no validated byte boundary exists
    /// for those intermediate positions.
    fn advance_older_search_with_batches_from(
        &mut self,
        mut cursor: HistoryBatchCursor,
        boundary_if_exhausted: bool,
        app_event_tx: &AppEventSender,
    ) -> HistorySearchResult {
        let mut offset = cursor.end_offset();
        loop {
            if let Some(entry) = self.entry_at_cached_offset(offset) {
                if self.search_matches(&entry) && self.search_result_is_unique(&entry) {
                    return self.search_match(offset, entry);
                }
            } else if !self.fetched_history.contains_key(&offset)
                && offset < self.persistent_entry_count
            {
                return self.request_older_search_batch(
                    cursor,
                    boundary_if_exhausted,
                    app_event_tx,
                );
            }

            let Some(next_offset) = offset.checked_sub(1) else {
                return self
                    .exhausted_search_result(HistorySearchDirection::Older, boundary_if_exhausted);
            };
            offset = next_offset;
            cursor = HistoryBatchCursor::new(offset);
        }
    }

    pub(super) fn request_older_search_batch(
        &mut self,
        cursor: HistoryBatchCursor,
        boundary_if_exhausted: bool,
        app_event_tx: &AppEventSender,
    ) -> HistorySearchResult {
        let (Some(thread_id), Some(log_id)) = (self.thread_id, self.persistent_log_id) else {
            return self
                .exhausted_search_result(HistorySearchDirection::Older, boundary_if_exhausted);
        };
        if let Some(search) = self.search.as_mut() {
            search.awaiting = Some(PendingHistorySearch::Batch {
                cursor,
                boundary_if_exhausted,
                read_failures: 0,
            });
        }
        app_event_tx.send(AppEvent::LookupMessageHistoryBatch {
            thread_id,
            cursor,
            log_id,
        });
        HistorySearchResult::Pending
    }
}
