#![cfg(not(target_os = "windows"))]
#![allow(clippy::unwrap_used)]

use std::path::Path;

use anyhow::Result;
use assert_cmd::prelude::*;
use predicates::prelude::*;
use tempfile::TempDir;

fn codex_cmd(codex_home: &Path) -> Result<assert_cmd::Command> {
    let mut cmd = assert_cmd::Command::cargo_bin("codex")?;
    cmd.env("CODEX_HOME", codex_home);
    Ok(cmd)
}

#[test]
fn apply_prehook_mcp_timeout_exits_10() -> Result<()> {
    let codex_home = TempDir::new()?;

    let mut cmd = codex_cmd(codex_home.path())?;
    // Point to a long-running stdio command so the MCP client times out.
    cmd.env("PREHOOK_APPLY_MCP", "stdio:/bin/sleep 10")
        .args(["apply", "TASK-123"])
        .assert()
        .code(10)
        .stderr(
            predicate::str::contains("Prehook error (apply): prehook MCP timed out").or(
                predicate::str::contains("Prehook error (apply): request timed out"),
            ),
        );

    Ok(())
}
