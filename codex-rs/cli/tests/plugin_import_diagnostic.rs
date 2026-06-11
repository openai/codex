use anyhow::Result;
use serde_json::Value;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn write_file(path: &Path, contents: &str) -> Result<()> {
    let Some(parent) = path.parent() else {
        anyhow::bail!("fixture file should have a parent: {}", path.display());
    };
    fs::create_dir_all(parent)?;
    fs::write(path, contents)?;
    Ok(())
}

#[test]
fn diagnostic_reports_every_entry_and_continues_after_failures() -> Result<()> {
    let fixture = TempDir::new()?;
    let marketplace = fixture.path();
    write_file(
        &marketplace.join(".claude-plugin/marketplace.json"),
        r#"{
  "name": "diagnostic-fixture",
  "plugins": [
    { "name": "good", "source": "./plugins/good" },
    { "name": "missing-manifest", "source": "./plugins/missing-manifest" },
    { "name": "missing-source", "source": "./plugins/missing-source" },
    { "name": "unsupported", "source": { "source": "npm", "package": "private-package" } },
    { "name": "bad-path", "source": "plugins/bad-path" }
  ]
}"#,
    )?;
    write_file(
        &marketplace.join("plugins/good/.claude-plugin/plugin.json"),
        r#"{
  "name": "good",
  "version": "1.0.0",
  "commands": "./commands",
  "hooks": "./hooks/missing.json"
}"#,
    )?;
    write_file(
        &marketplace.join("plugins/good/skills/demo/SKILL.md"),
        r#"---
name: demo
description: Diagnostic fixture skill.
---
Use this skill only in the diagnostic fixture.
"#,
    )?;
    write_file(
        &marketplace.join("plugins/good/skills/broken/SKILL.md"),
        "This skill is missing YAML frontmatter.",
    )?;
    write_file(
        &marketplace.join("plugins/good/.mcp.json"),
        r#"{ "mcpServers": { "broken": { "command": 42 } } }"#,
    )?;
    write_file(
        &marketplace.join("plugins/good/.app.json"),
        r#"{ "apps": { "broken": { "id": "" } } }"#,
    )?;
    fs::create_dir_all(marketplace.join("plugins/missing-manifest"))?;

    let output_dir = TempDir::new()?;
    let output = output_dir.path().join("plugin-diagnostic.jsonl");
    let mut command = assert_cmd::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
    command
        .arg("debug")
        .arg("marketplace-import-diagnostic")
        .arg(marketplace)
        .arg("--output")
        .arg(&output)
        .arg("--attempt-install")
        .assert()
        .success();

    let report = fs::read_to_string(&output)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(fs::metadata(&output)?.permissions().mode() & 0o077, 0);
    }
    assert!(!report.contains(&marketplace.display().to_string()));
    assert!(!report.contains("private-package"));
    let records = report
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(records.len(), 19);
    assert_eq!(records[0]["attemptInstall"], true);

    let started = records
        .iter()
        .filter(|record| record["event"] == "plugin_started")
        .collect::<Vec<_>>();
    assert_eq!(started.len(), 5);
    assert_eq!(started[0]["pluginName"], "good");

    let stages = records
        .iter()
        .filter(|record| record["event"] == "plugin_stage_started")
        .collect::<Vec<_>>();
    assert_eq!(stages.len(), 6);
    assert_eq!(stages[0]["stage"], "install");
    assert_eq!(stages[1]["stage"], "capabilities");

    let plugins = records
        .iter()
        .filter(|record| record["event"] == "plugin_diagnostic")
        .collect::<Vec<_>>();
    assert_eq!(plugins.len(), 5);
    assert_eq!(plugins[0]["pluginName"], "good");
    assert_eq!(plugins[0]["outcome"], "installed");
    assert_eq!(
        plugins[0]["capabilities"]["manifest"]["unsupportedCapabilityFields"],
        serde_json::json!(["commands"])
    );
    assert_eq!(
        plugins[0]["capabilities"]["skills"]["detected"][0]["name"],
        "good:demo"
    );
    assert!(
        plugins[0]["capabilities"]["skills"]["errors"]
            .as_array()
            .is_some_and(|errors| !errors.is_empty())
    );
    assert_eq!(
        plugins[0]["capabilities"]["mcp"]["serverNames"],
        serde_json::json!([])
    );
    assert!(
        plugins[0]["capabilities"]["mcp"]["issues"]
            .as_array()
            .is_some_and(|issues| !issues.is_empty())
    );
    assert!(
        plugins[0]["capabilities"]["hooks"]["issues"]
            .as_array()
            .is_some_and(|issues| !issues.is_empty())
    );
    assert!(
        plugins[0]["capabilities"]["apps"]["issues"]
            .as_array()
            .is_some_and(|issues| !issues.is_empty())
    );
    assert_eq!(plugins[1]["pluginName"], "missing-manifest");
    assert_eq!(plugins[1]["outcome"], "install_failed");
    assert_eq!(plugins[1]["install"]["code"], "install_failed");
    assert_eq!(plugins[2]["pluginName"], "missing-source");
    assert_eq!(plugins[2]["outcome"], "install_failed");
    assert_eq!(plugins[2]["install"]["code"], "source_path_not_found");
    assert_eq!(plugins[3]["detection"]["code"], "unsupported_source");
    assert_eq!(plugins[4]["detection"]["code"], "invalid_source");

    let completed = records.last().expect("completion record");
    assert_eq!(completed["event"], "run_completed");
    assert_eq!(completed["summary"]["rawEntries"], 5);
    assert_eq!(completed["summary"]["detectedEntries"], 3);
    assert_eq!(completed["summary"]["detectionFailures"], 2);
    assert_eq!(completed["summary"]["installedEntries"], 1);
    assert_eq!(completed["summary"]["installFailures"], 2);
    Ok(())
}

#[test]
fn diagnostic_writes_preflight_failures_to_the_report() -> Result<()> {
    let fixture = TempDir::new()?;
    let missing_marketplace = fixture.path().join("missing-marketplace");
    let output = fixture.path().join("preflight-diagnostic.jsonl");
    let mut command = assert_cmd::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
    command
        .arg("debug")
        .arg("marketplace-import-diagnostic")
        .arg(&missing_marketplace)
        .arg("--output")
        .arg(&output)
        .assert()
        .success();

    let records = fs::read_to_string(&output)?
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(records.len(), 3);
    assert_eq!(records[0]["event"], "run_started");
    assert_eq!(records[1]["event"], "marketplace_diagnostic");
    assert_eq!(records[1]["code"], "marketplace_root_unavailable");
    assert_eq!(records[2]["status"], "report_complete_preflight_failed");
    Ok(())
}

#[test]
fn diagnostic_does_not_overwrite_an_existing_report() -> Result<()> {
    let fixture = TempDir::new()?;
    let output = fixture.path().join("existing.jsonl");
    fs::write(&output, "keep-me")?;
    let mut command = assert_cmd::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
    command
        .arg("debug")
        .arg("marketplace-import-diagnostic")
        .arg(fixture.path())
        .arg("--output")
        .arg(&output)
        .assert()
        .failure();

    assert_eq!(fs::read_to_string(output)?, "keep-me");
    Ok(())
}
