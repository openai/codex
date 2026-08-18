use codex_arg0::Arg0DispatchPaths;
use codex_core::config::ConfigBuilder;
use pretty_assertions::assert_eq;

use super::CheckStatus;
use super::sandbox_check;

#[tokio::test]
async fn reports_missing_linux_sandbox_helper() {
    let home = tempfile::tempdir().expect("create Codex home");
    let config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .build()
        .await
        .expect("load sandbox config");
    let arg0_paths = Arg0DispatchPaths {
        codex_linux_sandbox_exe: Some(home.path().join("missing-linux-helper")),
        ..Default::default()
    };

    let check = sandbox_check(&config, &arg0_paths);

    assert_eq!(check.status, CheckStatus::Warning);
    assert_eq!(check.summary, "Linux sandbox helper path does not exist");
}

#[cfg(target_os = "windows")]
#[tokio::test]
async fn reports_refresh_failure_after_setup_completed() {
    let home = tempfile::tempdir().expect("create Codex home");
    let version = codex_windows_sandbox::SETUP_VERSION;
    for (path, contents) in [
        (
            ".sandbox/setup_marker.json",
            serde_json::json!({
                "version": version,
                "offline_username": codex_windows_sandbox::OFFLINE_USERNAME,
                "online_username": codex_windows_sandbox::ONLINE_USERNAME,
            }),
        ),
        (
            ".sandbox-secrets/sandbox_users.json",
            serde_json::json!({
                "version": version,
                "offline": {
                    "username": codex_windows_sandbox::OFFLINE_USERNAME,
                    "password": "",
                },
                "online": {
                    "username": codex_windows_sandbox::ONLINE_USERNAME,
                    "password": "",
                },
            }),
        ),
    ] {
        let path = home.path().join(path);
        std::fs::create_dir_all(path.parent().expect("fixture parent"))
            .expect("create fixture directory");
        std::fs::write(path, contents.to_string()).expect("write sandbox fixture");
    }
    assert!(codex_windows_sandbox::sandbox_setup_is_complete(
        home.path()
    ));
    codex_windows_sandbox::write_setup_error_report(
        home.path(),
        &codex_windows_sandbox::SetupErrorReport {
            code: codex_windows_sandbox::SetupErrorCode::HelperFirewallPolicyIneffective,
            message: "firewall policy rejected the refresh".to_string(),
        },
    )
    .expect("write refresh failure");
    std::fs::write(
        home.path().join("config.toml"),
        "[windows]\nsandbox = \"elevated\"\n",
    )
    .expect("write elevated sandbox config");
    let config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .build()
        .await
        .expect("load sandbox config");

    let check = sandbox_check(&config, &Arg0DispatchPaths::default());

    assert_eq!(check.status, CheckStatus::Fail);
    assert_eq!(
        check.issues[0].measured.as_deref(),
        Some("helper_firewall_policy_ineffective")
    );
}

#[cfg(target_os = "windows")]
#[tokio::test]
async fn rejects_oversized_setup_failure_reports() {
    let home = tempfile::tempdir().expect("create Codex home");
    codex_windows_sandbox::write_setup_error_report(
        home.path(),
        &codex_windows_sandbox::SetupErrorReport {
            code: codex_windows_sandbox::SetupErrorCode::HelperFirewallPolicyIneffective,
            message: "x".repeat(64 * 1024),
        },
    )
    .expect("write oversized setup failure");
    std::fs::write(
        home.path().join("config.toml"),
        "[windows]\nsandbox = \"elevated\"\n",
    )
    .expect("write elevated sandbox config");
    let config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .build()
        .await
        .expect("load sandbox config");

    let check = sandbox_check(&config, &Arg0DispatchPaths::default());

    assert_eq!(check.status, CheckStatus::Warning);
    assert!(
        check
            .details
            .contains(&"setup failure report: unreadable".to_string())
    );
    assert_eq!(
        check.issues[0].cause,
        "Windows sandbox setup failure report could not be read"
    );
    assert_eq!(
        check.issues[0].measured.as_deref(),
        Some("larger than 64 KiB")
    );
}

#[cfg(target_os = "windows")]
#[tokio::test]
async fn reports_malformed_setup_failure_reports() {
    let home = tempfile::tempdir().expect("create Codex home");
    let path = codex_windows_sandbox::setup_error_path(home.path());
    std::fs::create_dir_all(path.parent().expect("setup failure report parent"))
        .expect("create sandbox directory");
    std::fs::write(path, "{").expect("write malformed setup failure report");
    std::fs::write(
        home.path().join("config.toml"),
        "[windows]\nsandbox = \"elevated\"\n",
    )
    .expect("write elevated sandbox config");
    let config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .build()
        .await
        .expect("load sandbox config");

    let check = sandbox_check(&config, &Arg0DispatchPaths::default());

    assert_eq!(check.status, CheckStatus::Warning);
    assert_eq!(
        check.issues[0].cause,
        "Windows sandbox setup failure report could not be read"
    );
    assert!(check.issues[0].measured.is_some());
}
