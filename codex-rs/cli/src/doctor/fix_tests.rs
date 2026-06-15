use std::ffi::OsStr;
use std::ffi::OsString;

use super::*;

fn args(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

#[test]
fn recognizes_cli_and_standalone_app_servers() {
    assert!(is_app_server_process(
        OsStr::new("codex"),
        Some(Path::new("/usr/bin/codex")),
        &args(&["codex", "app-server", "--listen", "stdio://"]),
    ));
    assert!(is_app_server_process(
        OsStr::new("codex-app-server"),
        Some(Path::new("/usr/bin/codex-app-server")),
        &args(&["codex-app-server"]),
    ));
}

#[test]
fn excludes_daemon_commands_and_unrelated_codex_processes() {
    assert!(!is_app_server_process(
        OsStr::new("codex"),
        Some(Path::new("/usr/bin/codex")),
        &args(&["codex", "app-server", "daemon", "pid-update-loop"]),
    ));
    assert!(!is_app_server_process(
        OsStr::new("codex"),
        Some(Path::new("/usr/bin/codex")),
        &args(&["codex", "doctor", "--fix"]),
    ));
}

#[tokio::test]
async fn no_incompatible_database_skips_interactive_repair() {
    let temp = tempfile::tempdir().expect("tempdir");
    let config = codex_core::config::ConfigBuilder::default()
        .codex_home(temp.path().to_path_buf())
        .build()
        .await
        .expect("config");

    let repaired = run_migration_fix(&config, &[])
        .await
        .expect("repair result");

    assert!(!repaired);
}
