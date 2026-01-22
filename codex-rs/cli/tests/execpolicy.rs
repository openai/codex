use std::fs;

use assert_cmd::Command;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;

#[test]
fn execpolicy_check_matches_expected_json() -> Result<(), Box<dyn std::error::Error>> {
    let codex_home = TempDir::new()?;
    let policy_path = codex_home.path().join("rules").join("policy.rules");
    fs::create_dir_all(
        policy_path
            .parent()
            .expect("policy path should have a parent"),
    )?;
    fs::write(
        &policy_path,
        r#"
prefix_rule(
    pattern = ["git", "push"],
    decision = "forbidden",
)
"#,
    )?;

    let output = Command::cargo_bin("codex")?
        .env("CODEX_HOME", codex_home.path())
        .args([
            "execpolicy",
            "check",
            "--rules",
            policy_path
                .to_str()
                .expect("policy path should be valid UTF-8"),
            "git",
            "push",
            "origin",
            "main",
        ])
        .output()?;

    assert!(output.status.success());
    let result: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        result,
        json!({
            "decision": "forbidden",
            "matchedRules": [
                {
                    "prefixRuleMatch": {
                        "matchedPrefix": ["git", "push"],
                        "decision": "forbidden"
                    }
                }
            ]
        })
    );

    Ok(())
}
