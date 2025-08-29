use assert_cmd::prelude::*;
use std::fs;
use std::process::Command;

fn write(path: &std::path::Path, contents: &str) {
    fs::write(path, contents).unwrap();
}

#[test]
fn add_and_remove_user_scope() {
    let codex_home = tempfile::tempdir().unwrap();
    // Pre-create CODEX_HOME for canonicalization logic
    let config_path = codex_home.path().join("config.toml");

    let project_dir = tempfile::tempdir().unwrap();
    write(&project_dir.path().join(".git"), "gitdir: nowhere");

    // Add
    Command::cargo_bin("codex")
        .unwrap()
        .current_dir(project_dir.path())
        .env("CODEX_HOME", codex_home.path())
        .args([
            "mcp", "add", "svc", "--scope", "user", "--", "tool", "--flag",
        ])
        .assert()
        .success();

    let config = fs::read_to_string(&config_path).unwrap();
    assert!(config.contains("[mcp_servers.svc]"));
    assert!(config.contains("command = \"tool\""));

    // Remove
    Command::cargo_bin("codex")
        .unwrap()
        .current_dir(project_dir.path())
        .env("CODEX_HOME", codex_home.path())
        .args(["mcp", "remove", "svc", "--scope", "user"])
        .assert()
        .success();

    let config_after = fs::read_to_string(&config_path).unwrap();
    assert!(!config_after.contains("[mcp_servers.svc]"));
}

#[test]
fn add_local_and_project_scopes() {
    let codex_home = tempfile::tempdir().unwrap();
    let project_dir = tempfile::tempdir().unwrap();
    write(&project_dir.path().join(".git"), "gitdir: nowhere");

    // Add project
    Command::cargo_bin("codex")
        .unwrap()
        .current_dir(project_dir.path())
        .env("CODEX_HOME", codex_home.path())
        .args(["mcp", "add", "svc", "--scope", "project", "--", "toolp"])
        .assert()
        .success();
    let proj = fs::read_to_string(project_dir.path().join(".mcp.toml")).unwrap();
    assert!(proj.contains("[mcp_servers.svc]"));
    assert!(proj.contains("toolp"));

    // Add local (override in precedence for merged view)
    Command::cargo_bin("codex")
        .unwrap()
        .current_dir(project_dir.path())
        .env("CODEX_HOME", codex_home.path())
        .args(["mcp", "add", "svc", "--scope", "local", "--", "tooll"])
        .assert()
        .success();
    let local = fs::read_to_string(project_dir.path().join(".mcp.local.toml")).unwrap();
    assert!(local.contains("tooll"));

    // Remove all
    Command::cargo_bin("codex")
        .unwrap()
        .current_dir(project_dir.path())
        .env("CODEX_HOME", codex_home.path())
        .args(["mcp", "remove", "svc", "--all"])
        .assert()
        .success();

    let proj_after = fs::read_to_string(project_dir.path().join(".mcp.toml")).unwrap();
    assert!(!proj_after.contains("[mcp_servers.svc]"));
    let local_after = fs::read_to_string(project_dir.path().join(".mcp.local.toml")).unwrap();
    assert!(!local_after.contains("[mcp_servers.svc]"));
}
