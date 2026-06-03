use super::sandbox_setup_is_complete;
use crate::setup::SETUP_VERSION;
use crate::setup::SandboxUserRecord;
use crate::setup::SandboxUsersFile;
use crate::setup::SetupMarker;
use crate::setup::sandbox_users_path;
use crate::setup::setup_marker_path;
use crate::setup_error::prepare_setup_completion_report;
use crate::setup_error::write_setup_completion_report;
use std::fs;
use tempfile::TempDir;

#[test]
fn sandbox_setup_requires_completion_report() {
    let codex_home = TempDir::new().expect("tempdir");
    let marker = SetupMarker {
        version: SETUP_VERSION,
        offline_username: "offline".to_string(),
        online_username: "online".to_string(),
        created_at: None,
        proxy_ports: Vec::new(),
        allow_local_binding: false,
    };
    let users = SandboxUsersFile {
        version: SETUP_VERSION,
        offline: SandboxUserRecord {
            username: "offline".to_string(),
            password: "offline-password".to_string(),
        },
        online: SandboxUserRecord {
            username: "online".to_string(),
            password: "online-password".to_string(),
        },
    };
    let marker_path = setup_marker_path(codex_home.path());
    let users_path = sandbox_users_path(codex_home.path());
    fs::create_dir_all(marker_path.parent().expect("marker parent")).expect("create marker parent");
    fs::create_dir_all(users_path.parent().expect("users parent")).expect("create users parent");
    fs::write(
        marker_path,
        serde_json::to_vec_pretty(&marker).expect("serialize marker"),
    )
    .expect("write marker");
    fs::write(
        users_path,
        serde_json::to_vec_pretty(&users).expect("serialize users"),
    )
    .expect("write users");

    assert!(!sandbox_setup_is_complete(codex_home.path()));

    prepare_setup_completion_report(codex_home.path(), "completed")
        .expect("prepare completion report");
    assert!(!sandbox_setup_is_complete(codex_home.path()));

    write_setup_completion_report(codex_home.path(), "completed").expect("write completion report");

    assert!(sandbox_setup_is_complete(codex_home.path()));
}
