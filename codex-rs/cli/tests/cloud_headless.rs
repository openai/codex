use std::process::Command;

use assert_cmd::prelude::*;
use serde_json::Value;

fn codex_cmd() -> Command {
    let mut cmd = Command::cargo_bin("codex")
        .unwrap_or_else(|err| panic!("failed to locate codex binary: {err}"));
    cmd.env("CODEX_CLOUD_TASKS_MODE", "mock");
    cmd
}

#[test]
fn list_json_outputs_tasks() {
    let assert = codex_cmd()
        .args(["cloud", "list", "--json"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8");
    let value: Value = serde_json::from_str(&stdout).expect("json");
    assert!(
        value
            .get("tasks")
            .and_then(Value::as_array)
            .map(|a| !a.is_empty())
            .unwrap_or(false)
    );
}

#[test]
fn show_json_includes_variants() {
    let assert = codex_cmd()
        .args(["cloud", "show", "T-1000", "--json"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8");
    let value: Value = serde_json::from_str(&stdout).expect("json");
    let variants = value
        .get("variants")
        .and_then(Value::as_array)
        .expect("variants array");
    assert!(!variants.is_empty());
}

#[test]
fn export_writes_files() {
    let temp = tempfile::tempdir().expect("tempdir");
    codex_cmd()
        .args([
            "cloud",
            "export",
            "T-1000",
            "--dir",
            temp.path().to_str().unwrap(),
        ])
        .assert()
        .success();
    let patch = std::fs::read_to_string(temp.path().join("var1/patch.diff")).expect("patch");
    assert!(patch.contains("diff --git"));
}

#[test]
fn new_accepts_env_id() {
    let assert = codex_cmd()
        .args([
            "cloud",
            "new",
            "--env",
            "env_abc123",
            "--prompt",
            "Test env id",
        ])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8");
    assert!(stdout.contains("Created task"));
}

#[test]
fn new_resolves_label_to_id() {
    let assert = codex_cmd()
        .args([
            "cloud",
            "new",
            "--env",
            "L1nuxOne/ade",
            "--prompt",
            "Test env label",
        ])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8");
    assert!(stdout.contains("Created task"));
}

#[test]
fn new_rejects_ambiguous_label() {
    let assert = codex_cmd()
        .args(["cloud", "new", "--env", "prod", "--prompt", "Test env amb"])
        .assert()
        .failure();
    let stderr = String::from_utf8(assert.get_output().stderr.clone()).expect("utf8");
    assert!(stderr.contains("Ambiguous environment label 'prod'"));
    assert!(stderr.contains("OrgA/prod (env_abc123)"));
    assert!(stderr.contains("OrgB/prod (env_prod999)"));
}
