//! Coordinates independently paginated sources and tags rows with their backend.
//! Resetting a listing invalidates pending pages without mixing source cursors.

use super::PageCursor;
use super::PageLoadMode;
use super::PickerPage;
use crate::backend_id::BackendId;
use std::collections::HashMap;
use std::collections::HashSet;

#[derive(Default)]
pub(super) struct ThreadCatalog {
    sources: HashMap<BackendId, Source>,
    next_request: u64,
}

#[derive(Default)]
struct Source {
    cursor: Option<PageCursor>,
    mode: Option<PageLoadMode>,
    pending: Option<u64>,
    exhausted: bool,
}

pub(super) struct PageRequest {
    backend: BackendId,
    token: u64,
    pub(super) cursor: Option<PageCursor>,
    pub(super) mode: PageLoadMode,
}

impl ThreadCatalog {
    pub(super) fn has_more(&self, excluded: &HashSet<BackendId>) -> bool {
        self.sources
            .iter()
            .any(|(id, source)| !source.exhausted && !excluded.contains(id))
    }

    pub(super) fn reset(&mut self) {
        self.sources.clear();
    }

    /// A slow or exhausted source does not prevent another source from loading.
    pub(super) fn begin_page(
        &mut self,
        backend: BackendId,
        initial_mode: PageLoadMode,
    ) -> Option<PageRequest> {
        let source = self.sources.entry(backend).or_default();
        if source.pending.is_some() || source.exhausted {
            return None;
        }
        let mode = *source.mode.get_or_insert(initial_mode);
        let token = self.next_request;
        self.next_request += 1;
        source.pending = Some(token);
        Some(PageRequest {
            backend,
            token,
            cursor: source.cursor.clone(),
            mode,
        })
    }

    /// Errors retain the cursor for an explicit retry and never become empty pages.
    pub(super) fn finish_page(
        &mut self,
        request: PageRequest,
        page: std::io::Result<PickerPage>,
    ) -> Option<std::io::Result<PickerPage>> {
        let source = self.sources.get_mut(&request.backend)?;
        if source.pending != Some(request.token) {
            return None;
        }
        source.pending = None;
        source.mode = Some(request.mode);
        Some(page.map(|mut page| {
            source.cursor = page.next_cursor.clone();
            source.exhausted = source.cursor.is_none();
            for row in &mut page.rows {
                row.backend_id = Some(request.backend);
            }
            page
        }))
    }
}

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;
