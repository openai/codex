use super::*;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

const STATE_N: &str = "state-n";
const STATE_N_PLUS_ONE: &str = "state-n-plus-one";

#[test]
fn maps_app_server_client_names_to_bounded_surfaces() {
    assert_eq!(
        [
            "codex_cli",
            "codex-tui",
            "codex_exec",
            "codex_vscode",
            "codex_desktop",
            "codex_desktop_ssh",
            "codex_remote_control",
            "third_party_client",
        ]
        .map(HttpStateSurface::from_app_server_client_name),
        [
            HttpStateSurface::CodexCli,
            HttpStateSurface::CodexTui,
            HttpStateSurface::CodexExec,
            HttpStateSurface::CodexVscode,
            HttpStateSurface::CodexDesktop,
            HttpStateSurface::CodexDesktopSsh,
            HttpStateSurface::CodexRemoteControl,
            HttpStateSurface::CodexCli,
        ]
    );
    assert_eq!(
        HttpStateSurface::try_from_app_server_client_name("third_party_client"),
        None
    );
}

#[test]
fn stores_state_in_one_file_per_surface() {
    let codex_home = TempDir::new().expect("tempdir");
    let store = HttpStateStore::new(codex_home.path().to_path_buf());

    store
        .set(HttpStateSurface::CodexCli, STATE_N.to_string())
        .expect("CLI state should store");
    store
        .set(HttpStateSurface::CodexDesktop, STATE_N_PLUS_ONE.to_string())
        .expect("desktop state should store");

    assert_eq!(
        store
            .get(HttpStateSurface::CodexCli)
            .expect("CLI state should load")
            .expect("CLI state should exist"),
        STATE_N
    );
    assert_eq!(
        store
            .get(HttpStateSurface::CodexDesktop)
            .expect("desktop state should load")
            .expect("desktop state should exist"),
        STATE_N_PLUS_ONE
    );
    assert_eq!(
        store.state_path(HttpStateSurface::CodexCli),
        codex_home.path().join("state/codex_cli.json")
    );
    store
        .clear(HttpStateSurface::CodexDesktop)
        .expect("desktop state should clear");
    assert_eq!(
        store
            .get(HttpStateSurface::CodexDesktop)
            .expect("desktop state should load"),
        None
    );
}

#[test]
fn compare_and_set_rejects_a_stale_prior_value() {
    let codex_home = TempDir::new().expect("tempdir");
    let store = HttpStateStore::new(codex_home.path().to_path_buf());
    store
        .set(HttpStateSurface::CodexCli, STATE_N.to_string())
        .expect("state should store");

    assert!(
        !store
            .compare_and_set(
                HttpStateSurface::CodexCli,
                "stale-state",
                STATE_N_PLUS_ONE.to_string(),
            )
            .expect("compare should succeed")
    );
    assert!(
        store
            .compare_and_set(
                HttpStateSurface::CodexCli,
                STATE_N,
                STATE_N_PLUS_ONE.to_string(),
            )
            .expect("compare should succeed")
    );
}
