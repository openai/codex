use super::*;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

#[test]
fn replace_after_login_sets_and_clears_surface_state() {
    let codex_home = TempDir::new().expect("tempdir");
    let store = HttpStateStore::new(codex_home.path().to_path_buf());
    store
        .set(HttpStateSurface::CodexCli, "stale-cli-state".to_string())
        .expect("CLI state should store");

    replace_after_login(
        codex_home.path(),
        HttpStateSurface::CodexDesktop,
        Some("minted-state".to_string()),
    );
    assert_eq!(
        store
            .get(HttpStateSurface::CodexCli)
            .expect("CLI state should load"),
        None,
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

#[test]
fn replace_after_refresh_preserves_state_without_replacement_and_sets_returned_state() {
    let codex_home = TempDir::new().expect("tempdir");
    let store = HttpStateStore::new(codex_home.path().to_path_buf());
    store
        .set(HttpStateSurface::CodexDesktop, "old-state".to_string())
        .expect("state should store");

    replace_after_refresh(codex_home.path(), HttpStateSurface::CodexDesktop, None);
    assert_eq!(
        store
            .get(HttpStateSurface::CodexDesktop)
            .expect("state should load"),
        Some("old-state".to_string()),
    );

    replace_after_refresh(
        codex_home.path(),
        HttpStateSurface::CodexDesktop,
        Some("new-state".to_string()),
    );
    assert_eq!(
        store
            .get(HttpStateSurface::CodexDesktop)
            .expect("state should load"),
        Some("new-state".to_string()),
    );
}
