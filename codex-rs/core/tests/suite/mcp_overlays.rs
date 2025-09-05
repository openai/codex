use std::fs;
use std::path::PathBuf;

use codex_core::config::Config;
use codex_core::config::ConfigOverrides;

fn write(path: impl Into<PathBuf>, contents: &str) {
    let p: PathBuf = path.into();
    fs::write(&p, contents).unwrap_or_else(|e| panic!("failed writing {}: {e}", p.display()));
}

#[test]
fn test_overlay_precedence_local_over_project_over_user() -> std::io::Result<()> {
    // Set up a fake CODEX_HOME with a user-level MCP server.
    let codex_home = tempfile::tempdir()?;
    std::env::set_var("CODEX_HOME", codex_home.path());
    // Ensure directory exists before canonicalization in find_codex_home().
    let config_toml_path = codex_home.path().join("config.toml");
    write(&config_toml_path, r#"[mcp_servers.svc]
command = "user"
"#);

    // Set up a project directory with overlays.
    let project_dir = tempfile::tempdir()?;
    // Mark as git repo root (enough for resolve_root_git_project_for_trust()).
    write(project_dir.path().join(".git"), "gitdir: nowhere");

    // Project overlay defines the same server name.
    write(
        project_dir.path().join(".mcp.toml"),
        r#"[mcp_servers.svc]
command = "project"
"#,
    );
    // Local overlay should take precedence.
    write(
        project_dir.path().join(".mcp.local.toml"),
        r#"[mcp_servers.svc]
command = "local"
"#,
    );

    let overrides = ConfigOverrides {
        cwd: Some(project_dir.path().to_path_buf()),
        ..Default::default()
    };

    let cfg = Config::load_with_cli_overrides(vec![], overrides)?;
    let svc = cfg
        .mcp_servers
        .get("svc")
        .expect("svc should be present after merge");
    assert_eq!(svc.command, "local");

    Ok(())
}

