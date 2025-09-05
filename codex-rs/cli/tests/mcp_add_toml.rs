use assert_cmd::prelude::*;
use serde_json::Value;
use std::fs;
use std::process::Command;

fn write(path: &std::path::Path, contents: &str) {
    fs::write(path, contents).unwrap();
}

#[test]
fn add_toml_local_filters_non_stdio_and_lists() {
    let codex_home = tempfile::tempdir().unwrap();
    let project_dir = tempfile::tempdir().unwrap();
    write(&project_dir.path().join(".git"), "gitdir: nowhere");

    let import = tempfile::NamedTempFile::new().unwrap();
    write(
        import.path(),
        r#"[mcp_servers.ok]
type = "stdio"
command = "tool"
args = ["--x"]
env = { K = "V" }

[mcp_servers.bad]
type = "http"
url = "https://example.invalid/mcp"

[mcp_servers.missing]
type = "stdio"
"#,
    );

    // Import into local scope
    Command::cargo_bin("codex")
        .unwrap()
        .current_dir(project_dir.path())
        .env("CODEX_HOME", codex_home.path())
        .args([
            "mcp",
            "add-toml",
            "--scope",
            "local",
            import.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    // Verify file contents
    let local_contents = fs::read_to_string(project_dir.path().join(".mcp.local.toml")).unwrap();
    assert!(local_contents.contains("[mcp_servers.ok]"));
    assert!(local_contents.contains("command = \"tool\""));
    assert!(!local_contents.contains("[mcp_servers.bad]"));
    assert!(!local_contents.contains("[mcp_servers.missing]"));

    // And list shows only the accepted entry, with local scope
    let out = Command::cargo_bin("codex")
        .unwrap()
        .current_dir(project_dir.path())
        .env("CODEX_HOME", codex_home.path())
        .args(["mcp", "list", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: Value = serde_json::from_slice(&out).unwrap();
    let arr = v.as_array().unwrap();
    let mut seen_ok = false;
    for e in arr {
        if e.get("name").and_then(|x| x.as_str()) == Some("ok") {
            assert_eq!(e.get("scope").and_then(|x| x.as_str()), Some("local"));
            seen_ok = true;
        }
        assert_ne!(e.get("name").and_then(|x| x.as_str()), Some("bad"));
        assert_ne!(e.get("name").and_then(|x| x.as_str()), Some("missing"));
    }
    assert!(
        seen_ok,
        "expected to find imported 'ok' entry in list output"
    );
}

#[test]
fn add_toml_user_and_get() {
    let codex_home = tempfile::tempdir().unwrap();
    let project_dir = tempfile::tempdir().unwrap();
    write(&project_dir.path().join(".git"), "gitdir: nowhere");

    let import = tempfile::NamedTempFile::new().unwrap();
    write(
        import.path(),
        r#"[mcp_servers.userok]
type = "stdio"
command = "utool"
"#,
    );

    // Import into user scope
    Command::cargo_bin("codex")
        .unwrap()
        .current_dir(project_dir.path())
        .env("CODEX_HOME", codex_home.path())
        .args([
            "mcp",
            "add-toml",
            "--scope",
            "user",
            import.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    // Get shows the user scope
    let out = Command::cargo_bin("codex")
        .unwrap()
        .current_dir(project_dir.path())
        .env("CODEX_HOME", codex_home.path())
        .args(["mcp", "get", "userok", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v.get("scope").and_then(|x| x.as_str()), Some("user"));
    assert_eq!(
        v.get("config")
            .and_then(|c| c.get("command"))
            .and_then(|x| x.as_str()),
        Some("utool")
    );
}
