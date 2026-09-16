//! WSLg runtime coverage for the duplicate-view mask and procfs fallback.

use super::NETWORK_TIMEOUT_MS;
use super::codex_linux_sandbox_exe;
use super::should_skip_bwrap_tests;
use codex_protocol::models::PermissionProfile;
use pretty_assertions::assert_eq;
use std::os::unix::fs::MetadataExt as _;
use std::time::Duration;

#[tokio::test]
async fn wslg_duplicate_view_is_masked_with_and_without_fresh_procfs() {
    let root = std::fs::metadata("/").expect("inspect root");
    let has_wslg_root = std::fs::symlink_metadata("/mnt/wslg/distro").is_ok_and(|metadata| {
        metadata.is_dir() && (metadata.dev(), metadata.ino()) == (root.dev(), root.ino())
    });
    if !has_wslg_root || should_skip_bwrap_tests().await {
        eprintln!("skipping WSLg test: WSLg or bwrap prerequisites are unavailable");
        return;
    }

    let workspace = tempfile::tempdir().expect("create workspace");
    let allowed = workspace.path().join("allowed.txt");
    std::fs::write(&allowed, "allowed\n").expect("write allowed fixture");
    let profile = serde_json::to_string(&PermissionProfile::read_only())
        .expect("serialize read-only profile");

    for no_proc in [false, true] {
        let mut command = tokio::process::Command::new(codex_linux_sandbox_exe());
        command
            .arg("--sandbox-policy-cwd")
            .arg(workspace.path())
            .args(["--permission-profile", &profile]);
        if no_proc {
            command.arg("--no-proc");
        }
        // Inspect the protection itself, without accessing data through the alias.
        let script = if no_proc {
            "stat -c %a /mnt/wslg/distro && stat -f -c %T /mnt/wslg/distro && cat \"$1\" && test ! -e /proc/1"
        } else {
            "stat -c %a /mnt/wslg/distro && stat -f -c %T /mnt/wslg/distro && cat \"$1\""
        };
        let output = tokio::time::timeout(
            Duration::from_millis(NETWORK_TIMEOUT_MS),
            command
                .args(["--", "/bin/sh", "-c", script, "sh"])
                .arg(&allowed)
                .kill_on_drop(true)
                .output(),
        )
        .await
        .expect("WSLg sandbox check should finish")
        .expect("sandbox helper should start");
        assert!(
            output.status.success(),
            "WSLg mask check failed (no_proc={no_proc}): {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"0\ntmpfs\nallowed\n");
    }
    // Exercise the public helper boundary: production wraps the requested
    // executable in another helper command before constructing bwrap args.
    let output = tokio::process::Command::new(codex_linux_sandbox_exe())
        .arg("--sandbox-policy-cwd")
        .arg(workspace.path())
        .args([
            "--permission-profile",
            &profile,
            "--",
            "/mnt/wslg/distro/bin/true",
        ])
        .output()
        .await
        .expect("sandbox helper should start");
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(output.stderr).expect("UTF-8 diagnostic"),
        "error building bubblewrap command: Fatal error: restricted sandboxes do not support paths under /mnt/wslg/distro: /mnt/wslg/distro/bin/true; use the corresponding path under the primary filesystem root instead\n"
    );
}
