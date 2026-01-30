#![allow(clippy::expect_used, clippy::unwrap_used)]

use codex_core::auth::CODEX_API_KEY_ENV_VAR;
use codex_utils_cargo_bin::cargo_bin;
use codex_utils_cargo_bin::find_resource;
use core_test_support::test_codex_exec::test_codex_exec;
use pretty_assertions::assert_eq;
use std::io::BufRead as _;
use std::io::BufReader;
use std::process::Command;
use std::process::Stdio;

#[test]
fn json_streaming_exits_cleanly_when_stdout_closes() -> anyhow::Result<()> {
    let test = test_codex_exec();
    let fixture = find_resource!("tests/fixtures/cli_responses_fixture.sse")?;
    let repo_root = codex_utils_cargo_bin::repo_root()?;
    let bin = cargo_bin("codex-exec")?;

    let mut cmd = Command::new(bin);
    cmd.current_dir(test.cwd_path())
        .env("CODEX_HOME", test.home_path())
        .env(CODEX_API_KEY_ENV_VAR, "dummy")
        .env("CODEX_RS_SSE_FIXTURE", &fixture)
        .env("OPENAI_BASE_URL", "http://unused.local")
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(&repo_root)
        .arg("--json")
        .arg("echo broken pipe handling");
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn()?;

    let stdout = child.stdout.take().expect("stdout missing");
    let mut reader = BufReader::new(stdout);
    let mut first_line = String::new();
    // Read the first line, then drop the reader to close the pipe.
    let _bytes = reader.read_line(&mut first_line)?;
    drop(reader);

    let status = child.wait()?;
    assert_eq!(status.code(), Some(0));
    Ok(())
}
