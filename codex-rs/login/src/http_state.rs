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
    let result = match state {
        Some(state) => store.set(surface, state),
        None => store.clear(surface),
    };
    if let Err(err) = result {
        warn!(%surface, "failed to reset HTTP state after login: {err}");
    }
}

pub(crate) fn clear(codex_home: &Path, surface: HttpStateSurface) {
    if let Err(err) = HttpStateStore::new(codex_home.to_path_buf()).clear(surface) {
        warn!(%surface, "failed to clear HTTP state: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn replace_after_login_sets_and_clears_surface_state() {
        let codex_home = TempDir::new().expect("tempdir");
        let store = HttpStateStore::new(codex_home.path().to_path_buf());

        replace_after_login(
            codex_home.path(),
            HttpStateSurface::CodexDesktop,
            Some("minted-state".to_string()),
        );
        assert_eq!(
            store
                .get(HttpStateSurface::CodexDesktop)
                .expect("state should load"),
            Some("minted-state".to_string()),
        );

        replace_after_login(codex_home.path(), HttpStateSurface::CodexDesktop, None);
        assert_eq!(
            store
                .get(HttpStateSurface::CodexDesktop)
                .expect("state should load"),
            None,
        );
    }
}
