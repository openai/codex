use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::write_chatgpt_auth;
use codex_config::ConfigLoadOptions;
use codex_config::types::AuthCredentialsStoreMode;
use codex_core::config::load_config_toml_with_layer_stack;
use codex_utils_absolute_path::AbsolutePathBuf;
use predicates::str::contains;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

fn codex_command(codex_home: &Path) -> Result<assert_cmd::Command> {
    let mut cmd = assert_cmd::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
    cmd.env("CODEX_HOME", codex_home);
    Ok(cmd)
}

#[test]
fn strict_config_rejects_unknown_config_override() -> Result<()> {
    let codex_home = TempDir::new()?;

    let mut cmd = codex_command(codex_home.path())?;
    cmd.args(["--strict-config", "-c", "foo=bar", "exec", "hello"])
        .assert()
        .failure()
        .stderr(contains("unknown configuration field"));

    Ok(())
}

#[test]
fn interactive_validates_config_before_requiring_terminal() -> Result<()> {
    let cases: &[(&[&str], &str, &str, &str)] = &[
        (&[], "config.toml", "model = [", "Error loading config.toml"),
        (
            &["--strict-config"],
            "config.toml",
            "unknown_key = true",
            "unknown configuration field",
        ),
        (
            &["--strict-config", "-c", "foo=bar"],
            "config.toml",
            "",
            "unknown configuration field",
        ),
        (
            &["--profile", "work"],
            "work.config.toml",
            "model = [",
            "work.config.toml",
        ),
        (
            &["-c", "model_provider=\"missing\""],
            "config.toml",
            "",
            "Model provider `missing` not found",
        ),
        (&[], "config.toml", "", "stdin is not a terminal"),
    ];

    for &(args, config_file, contents, expected_error) in cases {
        let codex_home = TempDir::new()?;
        std::fs::write(codex_home.path().join(config_file), contents)?;

        let mut cmd = codex_command(codex_home.path())?;
        cmd.env("TERM", "xterm-256color")
            .current_dir(codex_home.path())
            .args(args)
            .assert()
            .failure()
            .stderr(contains(expected_error));
    }

    Ok(())
}

#[test]
fn interactive_remote_default_preserves_remote_working_directory_before_requiring_terminal()
-> Result<()> {
    let codex_home = TempDir::new()?;
    std::fs::write(codex_home.path().join("config.toml"), "")?;
    std::fs::write(
        codex_home.path().join("environments.toml"),
        r#"default = "remote"
include_local = false

[[environments]]
id = "remote"
url = "ws://127.0.0.1:4512"
"#,
    )?;
    let remote_only_cwd = codex_home.path().join("remote-only-working-directory");

    let mut cmd = codex_command(codex_home.path())?;
    cmd.env("TERM", "xterm-256color")
        .current_dir(codex_home.path())
        .arg("--cd")
        .arg(remote_only_cwd)
        .assert()
        .failure()
        .stderr(contains("stdin is not a terminal"));

    Ok(())
}

#[test]
fn strict_config_is_not_supported_for_cloud_command() -> Result<()> {
    let codex_home = TempDir::new()?;

    let mut cmd = codex_command(codex_home.path())?;
    cmd.args(["--strict-config", "-c", "foo=bar", "cloud", "list"])
        .assert()
        .failure()
        .stderr(contains(
            "`--strict-config` is not supported for `codex cloud`",
        ));

    Ok(())
}

#[tokio::test]
async fn features_enable_writes_feature_flag_to_config() -> Result<()> {
    let codex_home = TempDir::new()?;

    for feature in ["unified_exec", "shell_tool"] {
        let mut cmd = codex_command(codex_home.path())?;
        cmd.args(["features", "enable", feature])
            .assert()
            .success()
            .stdout(contains(format!(
                "Enabled feature `{feature}` in config.toml."
            )));

        let config = std::fs::read_to_string(codex_home.path().join("config.toml"))?;
        assert!(config.contains("[features]"));
        assert!(config.contains(&format!("{feature} = true")));
    }

    Ok(())
}

#[tokio::test]
async fn features_disable_writes_feature_flag_to_config() -> Result<()> {
    let codex_home = TempDir::new()?;

    let mut cmd = codex_command(codex_home.path())?;
    cmd.args(["features", "disable", "shell_tool"])
        .assert()
        .success()
        .stdout(contains("Disabled feature `shell_tool` in config.toml."));

    let config = std::fs::read_to_string(codex_home.path().join("config.toml"))?;
    assert!(config.contains("[features]"));
    assert!(config.contains("shell_tool = false"));

    Ok(())
}

#[tokio::test]
async fn features_enable_under_development_feature_prints_warning() -> Result<()> {
    let codex_home = TempDir::new()?;

    let mut cmd = codex_command(codex_home.path())?;
    cmd.args(["features", "enable", "runtime_metrics"])
        .assert()
        .success()
        .stderr(contains(
            "Under-development features enabled: runtime_metrics.",
        ));

    Ok(())
}

#[tokio::test]
async fn features_list_is_sorted_alphabetically_by_feature_name() -> Result<()> {
    let codex_home = TempDir::new()?;

    let mut cmd = codex_command(codex_home.path())?;
    let output = cmd
        .args(["features", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output)?;

    let actual_names = stdout
        .lines()
        .map(|line| {
            line.split_once("  ")
                .map(|(name, _)| name.trim_end().to_string())
                .expect("feature list output should contain aligned columns")
        })
        .collect::<Vec<_>>();
    let mut expected_names = actual_names.clone();
    expected_names.sort();

    assert_eq!(actual_names, expected_names);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn features_list_honors_cloud_managed_feature_requirements() -> Result<()> {
    let server = MockServer::start().await;
    let chatgpt_base_url = format!("{}/backend-api", server.uri());
    let codex_home = TempDir::new()?;
    let user_config = format!(
        "cli_auth_credentials_store = \"file\"\nchatgpt_base_url = \"{chatgpt_base_url}\"\n\n[features]\nfast_mode = true\n"
    );
    std::fs::write(codex_home.path().join("config.toml"), &user_config)?;

    let bootstrap_config = load_config_toml_with_layer_stack(
        codex_home.path(),
        Some(&AbsolutePathBuf::from_absolute_path(codex_home.path())?),
        Vec::new(),
        ConfigLoadOptions::default(),
    )
    .await?;
    if bootstrap_config.config_toml.cli_auth_credentials_store
        != Some(AuthCredentialsStoreMode::File)
        || bootstrap_config.config_toml.chatgpt_base_url.as_deref()
            != Some(chatgpt_base_url.as_str())
    {
        eprintln!(
            "skipping cloud-managed feature subprocess: host-managed authentication or backend routing prevents isolated mock credentials"
        );
        return Ok(());
    }

    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("workspace-123")
            .chatgpt_account_id("workspace-123")
            .chatgpt_user_id("user-123")
            .plan_type("enterprise"),
        AuthCredentialsStoreMode::File,
    )?;
    Mock::given(method("GET"))
        .and(path("/backend-api/wham/config/bundle"))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "workspace-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "requirements_toml": {
                "enterprise_managed": [{
                    "id": "managed-feature-requirements",
                    "name": "Managed feature requirements",
                    "contents": "[features]\nfast_mode = false\n",
                }],
            },
        })))
        .expect(1)
        .mount(&server)
        .await;

    let mut cmd = codex_command(codex_home.path())?;
    let output = cmd
        .current_dir(codex_home.path())
        .env("NO_PROXY", "127.0.0.1,localhost")
        .env("no_proxy", "127.0.0.1,localhost")
        .env_remove("CODEX_ACCESS_TOKEN")
        .env_remove("CODEX_API_KEY")
        .env_remove("OPENAI_API_KEY")
        .args(["features", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output)?;
    let fast_mode = stdout
        .lines()
        .find(|line| line.starts_with("fast_mode "))
        .context("feature list should include fast_mode")?;

    assert_eq!(
        fast_mode.split_whitespace().collect::<Vec<_>>(),
        ["fast_mode", "stable", "false"]
    );
    assert_eq!(
        std::fs::read_to_string(codex_home.path().join("config.toml"))?,
        user_config
    );
    server.verify().await;

    Ok(())
}

#[test]
fn remote_start_rejects_add_dir_before_connecting() -> Result<()> {
    let home = TempDir::new()?;
    let output = codex_command(home.path())?
        .env("TERM", "xterm-256color")
        .args(["--remote", "ws://127.0.0.1:1", "--add-dir", "remote-extra"])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    insta::assert_snapshot!(String::from_utf8(output)?);
    Ok(())
}

#[test]
fn remote_start_rejects_writable_root_overrides_before_connecting() -> Result<()> {
    for config_override in [
        "sandbox_workspace_write.writable_roots=[\"./extra\"]",
        "sandbox_workspace_write={writable_roots=[\"./extra\"],network_access=true}",
    ] {
        let home = TempDir::new()?;
        let output = codex_command(home.path())?
            .env("TERM", "xterm-256color")
            .args(["--remote", "ws://127.0.0.1:1", "-c", config_override])
            .assert()
            .failure()
            .get_output()
            .stderr
            .clone();
        let output = String::from_utf8(output)?;
        insta::allow_duplicates! {
            insta::assert_snapshot!(output, @"Error: sandbox_workspace_write.writable_roots overrides are not supported with --remote. Configure additional workspace roots on the server.");
        }
    }
    Ok(())
}

#[test]
fn remote_start_allows_network_access_overrides_before_requiring_terminal() -> Result<()> {
    for config_override in [
        "sandbox_workspace_write.network_access=true",
        "sandbox_workspace_write={network_access=true}",
    ] {
        let home = TempDir::new()?;
        codex_command(home.path())?
            .env("TERM", "xterm-256color")
            .args(["--remote", "ws://127.0.0.1:1", "-c", config_override])
            .assert()
            .failure()
            .stderr(contains("stdin is not a terminal"));
    }
    Ok(())
}

#[test]
fn no_daemon_rejects_agents_and_explicit_remote_targets() -> Result<()> {
    for args in [
        "--no-daemon agents",
        "agents --no-daemon",
        "--no-daemon queue --thread example --message hello",
        "--no-daemon --remote ws://localhost:9999 agents",
        "--no-daemon --remote ws://localhost:9999",
        "--no-daemon --remote ws://localhost:9999 archive example",
        "--no-daemon --remote ws://localhost:9999 queue --thread example --message hello",
        "--remote ws://localhost:9999 resume --no-daemon --last",
        "--no-daemon fork --remote ws://localhost:9999 session-name",
    ] {
        let args = args.split_whitespace().collect::<Vec<_>>();
        let home = TempDir::new()?;
        let expected = if args.contains(&"agents") {
            "--no-daemon cannot be used with codex agents."
        } else if args.contains(&"queue") && !args.contains(&"--remote") {
            "--no-daemon cannot be used with codex queue."
        } else {
            "--no-daemon cannot be used with --remote."
        };
        codex_command(home.path())?
            .args(args)
            .assert()
            .failure()
            .stderr(contains(expected));
        assert!(!home.path().join("app-server-daemon").exists());
    }
    Ok(())
}
