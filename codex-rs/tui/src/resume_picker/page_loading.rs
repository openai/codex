use super::PageCursor;

/// Selects whether thread listing trusts indexed metadata or uses the store's normal behavior.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PageLoadMode {
    /// Return only the store's indexed metadata without repairing it from rollout files.
    StateDbOnly,
    /// Use the store's normal listing behavior, which may include repair for local stores.
    StoreDefault,
}

/// Tracks the current request and the cursor policy inherited by its next page.
pub(super) struct PaginationState {
    pub(super) next_cursor: Option<PageCursor>,
    next_page_mode: PageLoadMode,
    pub(super) num_scanned_files: usize,
    pub(super) reached_scan_cap: bool,
    loading: LoadingState,
}

#[derive(Clone, Copy, Debug)]
enum LoadingState {
    Idle,
    Pending(PendingLoad),
}

#[derive(Clone, Copy, Debug)]
pub(super) struct PendingLoad {
    request_token: usize,
    pub(super) search_token: Option<usize>,
    pub(super) mode: PageLoadMode,
}

impl PaginationState {
    pub(super) fn new() -> Self {
        Self {
            next_cursor: None,
            next_page_mode: PageLoadMode::StoreDefault,
            num_scanned_files: 0,
            reached_scan_cap: false,
            loading: LoadingState::Idle,
        }
    }

    pub(super) fn reset(&mut self) {
        *self = Self::new();
    }

    pub(super) fn start_load(
        &mut self,
        request_token: usize,
        search_token: Option<usize>,
        mode: PageLoadMode,
    ) {
        self.loading = LoadingState::Pending(PendingLoad {
            request_token,
            search_token,
            mode,
        });
    }

    /// Completes only the matching in-flight request, leaving newer requests untouched.
    pub(super) fn finish_load(&mut self, request_token: usize) -> Option<PendingLoad> {
        let LoadingState::Pending(pending) = self.loading else {
            return None;
        };
        if pending.request_token != request_token {
            return None;
        }
        self.loading = LoadingState::Idle;
        self.next_page_mode = pending.mode;
        Some(pending)
    }

    /// Records a page cursor and accumulates scan progress across the current listing.
    pub(super) fn complete_page(
        &mut self,
        next_cursor: Option<PageCursor>,
        num_scanned_files: usize,
        reached_scan_cap: bool,
    ) {
        self.next_cursor = next_cursor;
        self.num_scanned_files = self.num_scanned_files.saturating_add(num_scanned_files);
        self.reached_scan_cap |= reached_scan_cap;
    }

    /// Returns the next cursor together with the listing mode that produced it.
    pub(super) fn next_page(&self) -> Option<(PageCursor, PageLoadMode)> {
        if self.is_loading() {
            return None;
        }
        self.next_cursor
            .clone()
            .map(|cursor| (cursor, self.next_page_mode))
    }

    pub(super) fn is_loading(&self) -> bool {
        matches!(self.loading, LoadingState::Pending(_))
    }
}
