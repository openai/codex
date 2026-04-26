#![cfg(target_os = "linux")]

use anyhow::Context as _;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::test_codex_exec::test_codex_exec;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;

const SMOKE_ENV: &str = "CODEX_LINUX_SANDBOX_SMOKE";

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn linux_sandbox_smoke_codex_exec_uses_distro_bwrap() -> anyhow::Result<()> {
    run_codex_exec_linux_sandbox_smoke(
        /*use_legacy_landlock*/ false,
        "call-bwrap-smoke",
        ".codex-bwrap-smoke",
        BWRAP_PROBE_SCRIPT,
        "smoke.ok=bwrap",
    )
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn linux_sandbox_smoke_codex_exec_uses_legacy_landlock() -> anyhow::Result<()> {
    run_codex_exec_linux_sandbox_smoke(
        /*use_legacy_landlock*/ true,
        "call-legacy-landlock-smoke",
        ".codex-legacy-landlock-smoke",
        LEGACY_LANDLOCK_PROBE_SCRIPT,
        "smoke.ok=legacy-landlock",
    )
    .await
}

async fn run_codex_exec_linux_sandbox_smoke(
    use_legacy_landlock: bool,
    call_id: &str,
    smoke_file: &str,
    script: &str,
    expected_marker: &str,
) -> anyhow::Result<()> {
    if std::env::var_os(SMOKE_ENV).is_none() {
        eprintln!("Skipping Linux sandbox smoke test: set {SMOKE_ENV}=1 to enable.");
        return Ok(());
    }

    let test = test_codex_exec();
    let server = responses::start_mock_server().await;
    let args = json!({
        "command": script,
        "timeout_ms": 10_000_u64,
    });
    mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "shell_command", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    let mut cmd = test.cmd_with_server(&server);
    cmd.arg("--skip-git-repo-check").arg("--full-auto");
    if use_legacy_landlock {
        cmd.arg("-c").arg("use_legacy_landlock=true");
    }
    cmd.arg("run linux sandbox smoke").assert().success();

    let output = results_mock
        .single_request()
        .function_call_output(call_id)
        .get("output")
        .and_then(Value::as_str)
        .context("shell command output should be a string")?
        .to_string();
    assert!(
        output.contains(expected_marker),
        "shell command output missing {expected_marker:?}: {output}"
    );
    assert_eq!(
        std::fs::read_to_string(test.cwd_path().join(smoke_file))?,
        "ok"
    );
    Ok(())
}

const BWRAP_PROBE_SCRIPT: &str = r#"
set -euo pipefail

aa_profile="$(cat /proc/self/attr/current)"
echo "payload.apparmor=$aa_profile"
case "$aa_profile" in
  *unpriv_bwrap*) ;;
  *)
    echo "Expected payload to run under Ubuntu's unprivileged bwrap AppArmor profile." >&2
    exit 1
    ;;
esac

seccomp_mode="$(grep '^Seccomp:' /proc/self/status | tr -s '[:space:]' ' ' | cut -d' ' -f2)"
echo "payload.seccomp=$seccomp_mode"
if [[ "$seccomp_mode" != "2" ]]; then
  echo "Expected Codex to install a seccomp filter in the sandbox payload." >&2
  exit 1
fi

printf ok > .codex-bwrap-smoke
test "$(cat .codex-bwrap-smoke)" = ok
echo "smoke.ok=bwrap"
"#;

const LEGACY_LANDLOCK_PROBE_SCRIPT: &str = r#"
set -euo pipefail

aa_profile="$(cat /proc/self/attr/current)"
echo "payload.apparmor=$aa_profile"
case "$aa_profile" in
  *unpriv_bwrap*)
    echo "Expected legacy Landlock smoke to avoid the bwrap AppArmor profile." >&2
    exit 1
    ;;
esac

seccomp_mode="$(grep '^Seccomp:' /proc/self/status | tr -s '[:space:]' ' ' | cut -d' ' -f2)"
echo "payload.seccomp=$seccomp_mode"
if [[ "$seccomp_mode" != "2" ]]; then
  echo "Expected Codex to install a seccomp filter in the sandbox payload." >&2
  exit 1
fi

printf ok > .codex-legacy-landlock-smoke
test "$(cat .codex-legacy-landlock-smoke)" = ok
echo "smoke.ok=legacy-landlock"
"#;
