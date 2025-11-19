use std::fs;
use std::path::Path;

use anyhow::Result;
use assert_cmd::Command;
use pretty_assertions::assert_eq;
use serde_json::Value as JsonValue;
use serde_json::json;
use tempfile::TempDir;

fn codex_command(codex_home: &Path) -> Result<Command> {
    let mut cmd = Command::cargo_bin("codex")?;
    cmd.env("CODEX_HOME", codex_home);
    Ok(cmd)
}

#[test]
fn execpolicycheck_evaluates_command() -> Result<()> {
    let codex_home = TempDir::new()?;
    let policy_path = codex_home.path().join("policy.codexpolicy");
    fs::write(
        &policy_path,
        r#"
prefix_rule(
    pattern = ["echo"],
    decision = "forbidden",
)
"#,
    )?;

    let mut cmd = codex_command(codex_home.path())?;
    let output = cmd
        .args([
            "execpolicycheck",
            "--policy",
            policy_path
                .to_str()
                .expect("policy path should be valid UTF-8"),
            "--pretty",
            "echo",
            "hello",
        ])
        .output()?;

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;
    let parsed: JsonValue = serde_json::from_str(&stdout)?;
    let matched = parsed
        .get("match")
        .cloned()
        .expect("match result should be present");
    assert_eq!(matched["decision"], "forbidden");
    assert_eq!(
        matched["matchedRules"][0]["prefixRuleMatch"]["matchedPrefix"],
        json!(["echo"])
    );

    Ok(())
}
