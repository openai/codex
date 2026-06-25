use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use serde_json::Value;
use serde_json::json;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::process::Output;
use std::process::Stdio;

#[test]
fn compatible_schema_passes_end_to_end() -> Result<()> {
    let output = run_lint(
        "before.json",
        "compatible-after.json",
        "empty-known-breakages.toml",
        "empty-known-breakages.toml",
    )?;

    ensure!(
        output.status.success(),
        "compatible schema failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    ensure!(
        String::from_utf8_lossy(&output.stdout)
            .contains("request schema does not narrow the baseline")
    );
    Ok(())
}

#[test]
fn breaking_schema_prints_the_known_breakage_to_append() -> Result<()> {
    let output = run_lint(
        "before.json",
        "breaking-after.json",
        "empty-known-breakages.toml",
        "empty-known-breakages.toml",
    )?;

    ensure!(!output.status.success(), "unrecorded breakage should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    for expected in [
        "RequiredPropertyAdded",
        "params.value",
        "[[breakages]]",
        "id = 1",
    ] {
        ensure!(
            stderr.contains(expected),
            "missing {expected:?} in {stderr}"
        );
    }
    Ok(())
}

#[test]
fn matching_new_known_breakage_still_fails_and_history_cannot_be_deleted() -> Result<()> {
    let recorded = run_lint(
        "before.json",
        "breaking-after.json",
        "empty-known-breakages.toml",
        "known-required-property.toml",
    )?;
    ensure!(
        !recorded.status.success(),
        "detected breakage should return a failing status"
    );
    ensure!(
        String::from_utf8_lossy(&recorded.stdout)
            .contains("1 request schema breakage(s) recorded in the known-breakage log")
    );

    let deleted = run_lint(
        "before.json",
        "before.json",
        "known-required-property.toml",
        "empty-known-breakages.toml",
    )?;
    ensure!(!deleted.status.success(), "deleting history should fail");
    ensure!(
        String::from_utf8_lossy(&deleted.stderr)
            .contains("known breakage 1 was deleted; existing entries are append-only")
    );
    Ok(())
}

fn run_lint(
    before_schema: &str,
    after_schema: &str,
    before_known_breakages: &str,
    after_known_breakages: &str,
) -> Result<Output> {
    let input = json!({
        "before": read_json(before_schema)?,
        "after": read_json(after_schema)?,
        "beforeKnownBreakages": read_fixture(before_known_breakages)?,
        "afterKnownBreakages": read_fixture(after_known_breakages)?,
    });
    let mut child = Command::new(codex_utils_cargo_bin::cargo_bin("codex-schema-evolution")?)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("spawn schema-evolution CLI")?;
    child
        .stdin
        .take()
        .context("schema-evolution CLI stdin should be piped")?
        .write_all(&serde_json::to_vec(&input)?)?;
    child.wait_with_output().context("run schema-evolution CLI")
}

fn read_json(name: &str) -> Result<Value> {
    serde_json::from_str(&read_fixture(name)?).with_context(|| format!("parse fixture {name}"))
}

fn read_fixture(name: &str) -> Result<String> {
    let path = fixture_path(name)?;
    std::fs::read_to_string(&path).with_context(|| format!("read fixture {}", path.display()))
}

fn fixture_path(name: &str) -> Result<PathBuf> {
    let relative_path = format!("tests/fixtures/{name}");
    codex_utils_cargo_bin::find_resource!(relative_path).map_err(Into::into)
}
