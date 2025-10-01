#![cfg(not(target_os = "windows"))]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use core_test_support::test_codex_exec::test_codex_exec;
use std::fs;
use std::io::Write;

fn write_script(dir: &std::path::Path, name: &str, body: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    let mut f = fs::File::create(&path).expect("create script");
    writeln!(f, "#!/usr/bin/env bash").unwrap();
    writeln!(f, "{body}").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut p = fs::metadata(&path).unwrap().permissions();
        p.set_mode(0o755);
        fs::set_permissions(&path, p).unwrap();
    }
    path
}

#[tokio::test(flavor = "current_thread")]
async fn prehook_script_deny_exits_10() {
    let test = test_codex_exec();
    let script = write_script(
        test.cwd_path(),
        "deny.sh",
        "cat >/dev/null; printf '%s\n' '{\"decision\":\"deny\",\"reason\":\"nope\"}'",
    );

    test.cmd()
        .arg("--skip-git-repo-check")
        .arg("--prehook-enabled")
        .arg("--prehook-backend")
        .arg("script")
        .arg("--prehook-script")
        .arg(script)
        .arg("hello")
        .assert()
        .code(10);
}

#[tokio::test(flavor = "current_thread")]
async fn prehook_script_rate_limit_exits_14() {
    let test = test_codex_exec();
    let script = write_script(
        test.cwd_path(),
        "rl.sh",
        "cat >/dev/null; printf '%s\n' '{\"decision\":\"rate_limit\",\"retry_after_ms\":1234}'",
    );

    test.cmd()
        .arg("--skip-git-repo-check")
        .arg("--prehook-enabled")
        .arg("--prehook-backend")
        .arg("script")
        .arg("--prehook-script")
        .arg(script)
        .arg("hello")
        .assert()
        .code(14);
}

#[tokio::test(flavor = "current_thread")]
async fn prehook_script_defer_exits_13_in_exec() {
    let test = test_codex_exec();
    let script = write_script(
        test.cwd_path(),
        "defer.sh",
        "cat >/dev/null; printf '%s\n' '{\"decision\":\"defer\"}'",
    );

    test.cmd()
        .arg("--skip-git-repo-check")
        .arg("--prehook-enabled")
        .arg("--prehook-backend")
        .arg("script")
        .arg("--prehook-script")
        .arg(script)
        .arg("hello")
        .assert()
        .code(13);
}
