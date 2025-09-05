use assert_cmd::prelude::*;
use serde_json::Value;
use std::fs;
use std::process::Command;

fn write(path: &std::path::Path, contents: &str) {
    fs::write(path, contents).unwrap();
}

#[test]
fn get_returns_winning_scope() {
    let codex_home = tempfile::tempdir().unwrap();
    write(
        &codex_home.path().join("config.toml"),
        r#"[mcp_servers.svc]
command = "user-cmd"
"#,
    );

    let project_dir = tempfile::tempdir().unwrap();
    write(&project_dir.path().join(".git"), "gitdir: nowhere");
    write(
        &project_dir.path().join(".mcp.toml"),
        r#"[mcp_servers.svc]
command = "project-cmd"
"#,
    );
    write(
        &project_dir.path().join(".mcp.local.toml"),
        r#"[mcp_servers.svc]
command = "local-cmd"
"#,
    );

    let assert = Command::cargo_bin("codex")
        .unwrap()
        .current_dir(project_dir.path())
        .env("CODEX_HOME", codex_home.path())
        .args(["mcp", "get", "svc", "--json"])
        .assert()
        .success();
    let out = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v.get("name").and_then(|x| x.as_str()), Some("svc"));
    assert_eq!(v.get("scope").and_then(|x| x.as_str()), Some("local"));
    assert_eq!(
        v.get("config")
            .and_then(|c| c.get("command"))
            .and_then(|x| x.as_str()),
        Some("local-cmd")
    );
}
