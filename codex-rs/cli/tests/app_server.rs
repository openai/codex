use std::path::Path;

use anyhow::Result;
use app_test_support::app_server_json_shutdown_event;
use predicates::str::contains;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;

fn codex_command(codex_home: &Path) -> Result<assert_cmd::Command> {
    let mut cmd = assert_cmd::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
    cmd.env("CODEX_HOME", codex_home)
        .env_remove("CODEX_ACCESS_TOKEN")
        .env_remove("CODEX_API_KEY")
        .env_remove("OPENAI_API_KEY")
        .env_remove("OPENAI_FEDERATION_RULE_ID")
        .env_remove("OPENAI_IDENTITY_TOKEN_FILE")
        .env_remove("OPENAI_WORKLOAD_IDENTITY_CONTEXT");
    Ok(cmd)
}

#[test]
fn strict_config_rejects_unknown_config_fields_for_app_server() -> Result<()> {
    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        r#"
foo = "bar"
"#,
    )?;

    let mut cmd = codex_command(codex_home.path())?;
    cmd.args(["app-server", "--strict-config", "--listen", "off"])
        .assert()
        .failure()
        .stderr(contains("unknown configuration field"));

    Ok(())
}

#[test]
fn app_server_preserves_config_overrides_across_subcommand() -> Result<()> {
    let home = TempDir::new()?;
    codex_command(home.path())?
        .args([
            "-c",
            "missing-equals",
            "app-server",
            "--print-startup-config",
            "-c",
            "model=example",
        ])
        .assert()
        .failure()
        .stdout(contains("Invalid override (missing '='): missing-equals"));
    Ok(())
}

#[test]
fn agents_accept_interactive_configuration_overrides() -> Result<()> {
    let codex_home = TempDir::new()?;

    for args in [
        ["-c", "features.multi_agent_mode=true", "agents"].as_slice(),
        ["--enable", "multi_agent_mode", "agents"].as_slice(),
        ["--yolo", "agents"].as_slice(),
        ["--search", "agents"].as_slice(),
        ["--model", "gpt-5", "agents"].as_slice(),
        ["--approve-for-me", "agents"].as_slice(),
        ["--cd", ".", "agents"].as_slice(),
    ] {
        let mut cmd = codex_command(codex_home.path())?;
        cmd.env("TERM", "xterm-256color").args(args);
        #[cfg(not(any(unix, windows)))]
        cmd.args(["--remote", "ws://127.0.0.1:4512"]);

        cmd.assert()
            .failure()
            .stderr(contains("stdin is not a terminal"));
    }

    Ok(())
}

#[test]
fn agents_reject_inputs_that_cannot_be_applied() -> Result<()> {
    let codex_home = TempDir::new()?;

    for (args, expected_error) in [
        (
            ["--image=image.png", "agents"].as_slice(),
            "does not accept an initial prompt or images",
        ),
        (
            ["--oss", "agents", "--remote", "ws://127.0.0.1:4512"].as_slice(),
            "cannot apply local provider or additional-directory overrides",
        ),
        (
            [
                "--add-dir",
                ".",
                "agents",
                "--remote",
                "ws://127.0.0.1:4512",
            ]
            .as_slice(),
            "cannot apply local provider or additional-directory overrides",
        ),
        (
            [
                "-c",
                "sandbox_workspace_write.writable_roots=[\"../shared\"]",
                "agents",
                "--remote",
                "ws://127.0.0.1:4512",
            ]
            .as_slice(),
            "cannot apply local provider or additional-directory overrides",
        ),
    ] {
        let mut cmd = codex_command(codex_home.path())?;
        cmd.args(args)
            .assert()
            .failure()
            .stderr(contains(expected_error));
    }

    Ok(())
}

#[test]
fn app_server_emits_json_info_events() -> Result<()> {
    let codex_home = TempDir::new()?;
    let event = app_server_json_shutdown_event("codex", &["app-server"], codex_home.path())?;

    assert_eq!(
        event,
        json!({
            "level": "INFO",
            "fields": {
                "message": "processor task exited",
                "exit_reason": "stdio_connection_closed",
                "remaining_connection_count": 0,
                "shutdown_forced": false,
            },
            "target": "codex_app_server",
        })
    );

    Ok(())
}

#[test]
fn app_server_keeps_sqlite_logs_in_overridden_sqlite_home() -> Result<()> {
    let home = TempDir::new()?;
    let sqlite_home = TempDir::new()?;
    std::fs::write(
        home.path().join("config.toml"),
        "[features]\nplugins = false\n",
    )?;

    codex_command(home.path())?
        .env("CODEX_SQLITE_HOME", sqlite_home.path())
        .env(
            "CODEX_APP_SERVER_MANAGED_CONFIG_PATH",
            home.path().join("managed_config.toml"),
        )
        .arg("app-server")
        .write_stdin("")
        .assert()
        .success();

    assert!(sqlite_home.path().join("state_5.sqlite").is_file());
    assert!(std::fs::metadata(sqlite_home.path().join("logs_2.sqlite"))?.len() > 0);
    assert!(!home.path().join("logs_2.sqlite").exists());
    assert!(!home.path().join("log/logs_2.sqlite").exists());
    Ok(())
}

#[cfg(unix)]
#[test]
fn app_server_preserves_the_existing_sqlite_logs_symlink() -> Result<()> {
    let home = TempDir::new()?;
    let external_logs = TempDir::new()?;
    let target = external_logs.path().join("logs_2.sqlite");
    let link = home.path().join("logs_2.sqlite");
    std::os::unix::fs::symlink(&target, &link)?;
    std::fs::write(
        home.path().join("config.toml"),
        "[features]\nplugins = false\n",
    )?;

    codex_command(home.path())?
        .env_remove("CODEX_SQLITE_HOME")
        .env(
            "CODEX_APP_SERVER_MANAGED_CONFIG_PATH",
            home.path().join("managed_config.toml"),
        )
        .arg("app-server")
        .write_stdin("")
        .assert()
        .success();

    assert_eq!(std::fs::read_link(&link)?, target);
    assert!(std::fs::metadata(&target)?.len() > 0);
    assert!(home.path().join("state_5.sqlite").is_file());
    assert!(!home.path().join("log/logs_2.sqlite").exists());
    Ok(())
}

#[test]
fn print_startup_config_preserves_project_login_restrictions() -> Result<()> {
    let home = TempDir::new()?;
    let project = TempDir::new()?;
    let project_key =
        serde_json::to_string(&codex_config::loader::project_trust_key(project.path()))?;
    std::fs::write(
        home.path().join("config.toml"),
        format!("[projects.{project_key}]\ntrust_level = \"trusted\"\n"),
    )?;
    std::fs::create_dir(project.path().join(".git"))?;
    std::fs::create_dir(project.path().join(".codex"))?;
    for method in ["chatgpt", "api"] {
        std::fs::write(
            project.path().join(".codex/config.toml"),
            format!("forced_login_method = '{method}'\n"),
        )?;
        for sign_out in [None, Some("--startup-sign-out")] {
            let mut command = codex_command(home.path())?;
            command
                .current_dir(project.path())
                .args(["app-server", "--print-startup-config"])
                .args(sign_out);
            let output = command.assert().success();
            let value: serde_json::Value = serde_json::from_slice(&output.get_output().stdout)?;
            assert_eq!(
                value["configRequirements"]["requirements"]["allowedLoginMethods"],
                json!([method])
            );
        }
    }
    Ok(())
}

#[test]
fn print_startup_config_rejects_sign_out_with_environment_access_token() -> Result<()> {
    let home = TempDir::new()?;
    let auth_path = home.path().join("auth.json");
    let saved_auth = br#"{"OPENAI_API_KEY":"saved-test-key"}"#;
    std::fs::write(&auth_path, saved_auth)?;
    for token in ["at-test-token", "header.payload.signature"] {
        let output = codex_command(home.path())?
            .env("CODEX_ACCESS_TOKEN", token)
            .args(["app-server", "--print-startup-config", "--startup-sign-out"])
            .assert()
            .failure();
        let value: serde_json::Value = serde_json::from_slice(&output.get_output().stdout)?;
        assert_eq!(
            value["error"]["message"],
            "Configured external authentication is owned by the app-server host and cannot be changed during startup recovery."
        );
        assert_eq!(std::fs::read(&auth_path)?, saved_auth);
    }
    Ok(())
}

#[test]
fn print_startup_config_validates_configuration() -> Result<()> {
    let home = TempDir::new()?;
    let config_path = home.path().join("config.toml");
    for (config, strict_arg, error) in [
        ("[broken\n", None, Some("failed to load configuration")),
        (
            "unknown_future_setting = true\n",
            Some("--strict-config"),
            Some("unknown configuration field"),
        ),
        ("unknown_future_setting = true\n", None, None),
    ] {
        std::fs::write(&config_path, config)?;
        let output = codex_command(home.path())?
            .args(["app-server", "--print-startup-config"])
            .args(strict_arg)
            .assert();
        let value: serde_json::Value = serde_json::from_slice(&output.get_output().stdout)?;
        if let Some(error) = error {
            output.failure().stdout(contains(error));
            assert_eq!(value["error"]["data"]["reason"], "configLoad");
            assert_eq!(
                Path::new(value["error"]["data"]["filePath"].as_str().unwrap()).canonicalize()?,
                config_path.canonicalize()?
            );
            assert!(value["error"]["data"]["line"].as_u64().is_some());
            assert!(value.get("configRequirements").is_none());
        } else {
            output.success();
            assert_eq!(value["appServerVersion"], env!("CARGO_PKG_VERSION"));
            assert!(value["configRequirements"].get("requirements").is_some());
            assert!(value.get("config").is_none());
        }
    }
    Ok(())
}

#[tokio::test]
async fn print_startup_config_is_independent_of_a_locked_history_database() -> Result<()> {
    use sqlx::Executor;
    use std::time::Duration;
    use tokio::process::Command;
    use tokio::time::timeout;

    let home = TempDir::new()?;
    let sqlite = TempDir::new()?;
    let binary = codex_utils_cargo_bin::cargo_bin("codex")?;
    let mut command = Command::new(&binary);
    command
        .env("CODEX_HOME", home.path())
        .env("CODEX_SQLITE_HOME", sqlite.path())
        .env_remove("CODEX_ACCESS_TOKEN")
        .env_remove("CODEX_API_KEY")
        .env_remove("OPENAI_API_KEY")
        .env_remove("OPENAI_FEDERATION_RULE_ID")
        .env_remove("OPENAI_IDENTITY_TOKEN_FILE")
        .env_remove("OPENAI_WORKLOAD_IDENTITY_CONTEXT")
        .args(["app-server", "--print-startup-config"])
        .kill_on_drop(true);
    let baseline = timeout(Duration::from_secs(10), command.output()).await??;
    assert!(baseline.status.success());
    let path = sqlite.path().join("state_5.sqlite");
    let sqlite_config = codex_state::SqliteConfig::new_for_testing(
        codex_utils_absolute_path::AbsolutePathBuf::from_absolute_path(sqlite.path())?,
    );
    let pool = sqlite_config.open_read_write_pool(&path).await?;
    let mut database = pool.acquire().await?;
    database
        .execute("CREATE TABLE synthetic_history (id INTEGER)")
        .await?;
    database.execute("PRAGMA journal_mode=DELETE").await?;
    let readers = sqlite_config
        .open_read_only_pool(&path, Some(Duration::ZERO))
        .await?;
    let mut reader = readers.acquire().await?;
    database.execute("BEGIN EXCLUSIVE").await?;
    assert!(
        reader
            .execute("SELECT * FROM synthetic_history")
            .await
            .is_err()
    );
    let before = std::fs::read(&path)?;
    let output = timeout(Duration::from_secs(10), command.output()).await??;
    assert!(output.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output.stdout)?,
        serde_json::from_slice::<serde_json::Value>(&baseline.stdout)?
    );
    assert_eq!(std::fs::read(&path)?, before);

    database.execute("ROLLBACK").await?;
    Ok(())
}
