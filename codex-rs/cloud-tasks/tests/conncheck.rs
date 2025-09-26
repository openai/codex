use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn conncheck_mock_mode_succeeds() {
    let mut cmd = Command::cargo_bin("conncheck").unwrap();
    cmd.env("CODEX_CLOUD_TASKS_MODE", "mock")
        // base URL is ignored in mock mode but provided for completeness.
        .env(
            "CODEX_CLOUD_TASKS_BASE_URL",
            "https://chatgpt.com/backend-api",
        )
        .assert()
        .success()
        .stdout(predicate::str::contains("mode: mock"))
        .stdout(predicate::str::contains("ok: received"));
}
