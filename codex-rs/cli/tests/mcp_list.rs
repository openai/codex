use assert_cmd::prelude::*;
use serde_json::Value;
use std::fs;
use std::process::Command;

fn write(path: &std::path::Path, contents: &str) {
    fs::write(path, contents).unwrap();
}

#[test]
fn list_shows_scopes_for_user_project_local() {
    let codex_home = tempfile::tempdir().unwrap();
    write(
        &codex_home.path().join("config.toml"),
        r#"[mcp_servers.user_svc]
command = "user-cmd"
"#,
    );

    let project_dir = tempfile::tempdir().unwrap();
    // Mark git root for nicer parity with real use
    write(&project_dir.path().join(".git"), "gitdir: nowhere");
    write(
        &project_dir.path().join(".mcp.toml"),
        r#"[mcp_servers.proj_svc]
command = "proj-cmd"
"#,
    );
    write(
        &project_dir.path().join(".mcp.local.toml"),
        r#"[mcp_servers.local_svc]
command = "local-cmd"
"#,
    );

    let assert = Command::cargo_bin("codex")
        .unwrap()
        .current_dir(project_dir.path())
        .env("CODEX_HOME", codex_home.path())
        .args(["mcp", "list", "--json"])
        .assert()
        .success();
    let out = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    let arr = v.as_array().unwrap();

    let mut found = (false, false, false);
    for e in arr {
        let name = e.get("name").and_then(|x| x.as_str()).unwrap();
        let scope = e.get("scope").and_then(|x| x.as_str()).unwrap();
        match name {
            "user_svc" => {
                assert_eq!(scope, "user");
                found.0 = true;
            }
            "proj_svc" => {
                assert_eq!(scope, "project");
                found.1 = true;
            }
            "local_svc" => {
                assert_eq!(scope, "local");
                found.2 = true;
            }
            _ => {}
        }
    }
    assert!(
        found.0 && found.1 && found.2,
        "expected three entries across scopes"
    );
}
