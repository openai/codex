use super::*;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

const STATE_N: &str = "ois1.a.b.c";
const STATE_N_PLUS_ONE: &str = "ois1.d.e.f";

#[test]
fn maps_app_server_client_names_to_bounded_surfaces() {
    assert_eq!(
        [
            "codex-tui",
            "codex_exec",
            "codex_vscode",
            "codex_desktop",
            "codex_desktop_ssh",
            "codex_remote_control",
            "third_party_client",
        ]
        .map(NativeIntegritySurface::from_app_server_client_name),
        [
            NativeIntegritySurface::CodexTui,
            NativeIntegritySurface::CodexExec,
            NativeIntegritySurface::CodexVscode,
            NativeIntegritySurface::CodexDesktop,
            NativeIntegritySurface::CodexDesktopSsh,
            NativeIntegritySurface::CodexRemoteControl,
            NativeIntegritySurface::CodexCli,
        ]
    );
}

#[test]
fn stores_state_in_one_file_per_surface() {
    let codex_home = TempDir::new().expect("tempdir");
    let store = NativeIntegrityStateStore::new(codex_home.path().to_path_buf());

    store
        .replace(NativeIntegritySurface::CodexCli, STATE_N.to_string())
        .expect("CLI state should store");
    store
        .replace(
            NativeIntegritySurface::CodexDesktop,
            STATE_N_PLUS_ONE.to_string(),
        )
        .expect("desktop state should store");

    assert_eq!(
        store
            .load(NativeIntegritySurface::CodexCli)
            .expect("CLI state should load")
            .expect("CLI state should exist"),
        NativeIntegrityStateFile {
            state: STATE_N.to_string(),
        }
    );
    assert_eq!(
        store
            .load(NativeIntegritySurface::CodexDesktop)
            .expect("desktop state should load")
            .expect("desktop state should exist"),
        NativeIntegrityStateFile {
            state: STATE_N_PLUS_ONE.to_string(),
        }
    );
    assert_eq!(
        store.state_path(NativeIntegritySurface::CodexCli),
        codex_home.path().join("state/codex_cli.json")
    );
    store
        .clear(NativeIntegritySurface::CodexDesktop)
        .expect("desktop state should clear");
    assert_eq!(
        store
            .load(NativeIntegritySurface::CodexDesktop)
            .expect("desktop state should load"),
        None
    );
}

#[test]
fn compare_and_store_rejects_a_stale_prior_value() {
    let codex_home = TempDir::new().expect("tempdir");
    let store = NativeIntegrityStateStore::new(codex_home.path().to_path_buf());
    store
        .replace(NativeIntegritySurface::CodexCli, STATE_N.to_string())
        .expect("state should store");

    assert!(
        !store
            .compare_and_store(
                NativeIntegritySurface::CodexCli,
                "ois1.stale.state.value",
                STATE_N_PLUS_ONE.to_string(),
            )
            .expect("compare should succeed")
    );
    assert!(
        store
            .compare_and_store(
                NativeIntegritySurface::CodexCli,
                STATE_N,
                STATE_N_PLUS_ONE.to_string(),
            )
            .expect("compare should succeed")
    );
}

#[test]
fn validates_integrity_state_envelopes() {
    assert!(is_valid_integrity_state_envelope(STATE_N));
    assert!(!is_valid_integrity_state_envelope(""));
    assert!(!is_valid_integrity_state_envelope("state"));
    assert!(!is_valid_integrity_state_envelope(&format!(" {STATE_N}")));
    assert!(!is_valid_integrity_state_envelope("ois1.a.b"));
    assert!(!is_valid_integrity_state_envelope(&format!(
        "ois1.a.b.{}",
        "c".repeat(MAX_INTEGRITY_STATE_ENVELOPE_BYTES + 1)
    )));
}
