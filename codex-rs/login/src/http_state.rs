use std::path::Path;

use codex_http_state::HttpStateStore;
use codex_http_state::HttpStateSurface;
use tracing::warn;

pub(crate) fn replace_after_login(
    codex_home: &Path,
    surface: HttpStateSurface,
    state: Option<String>,
) {
    let store = HttpStateStore::new(codex_home.to_path_buf());
    clear_all(codex_home);
    if let Some(state) = state
        && let Err(err) = store.set(surface, state)
    {
        warn!(%surface, "failed to reset HTTP state after login: {err}");
    }
}

pub(crate) fn replace_after_refresh(
    codex_home: &Path,
    surface: HttpStateSurface,
    state: Option<String>,
) {
    let Some(state) = state else {
        return;
    };
    if let Err(err) = HttpStateStore::new(codex_home.to_path_buf()).set(surface, state) {
        warn!(%surface, "failed to reset HTTP state after refresh: {err}");
    }
}

pub(crate) fn clear_all(codex_home: &Path) {
    let store = HttpStateStore::new(codex_home.to_path_buf());
    for surface in HttpStateSurface::ALL {
        if let Err(err) = store.clear(surface) {
            warn!(%surface, "failed to clear HTTP state: {err}");
        }
    }
}

#[cfg(test)]
#[path = "http_state_tests.rs"]
mod tests;
