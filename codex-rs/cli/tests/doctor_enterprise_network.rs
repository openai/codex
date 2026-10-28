#[cfg(target_os = "macos")]
use std::path::Path;
use std::process::Command;
use std::process::Stdio;

use anyhow::Context as _;
use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::write_chatgpt_auth;
use codex_config::types::AuthCredentialsStoreMode;
use codex_state::SqliteConfig;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use regex_lite::Regex;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

async fn local_provider() -> Result<(MockServer, TempDir)> {
    let server = MockServer::start().await;
    for (request_method, request_path, status) in
        [("HEAD", "/v1/responses", 200), ("GET", "/v1/models", 401)]
    {
        Mock::given(method(request_method))
            .and(path(request_path))
            .respond_with(ResponseTemplate::new(status).insert_header("x-request-id", "fixture-id"))
            .mount(&server)
            .await;
    }
    let home = TempDir::new()?;
    std::fs::write(
        home.path().join("config.toml"),
        format!(
            "cli_auth_credentials_store = \"ephemeral\"\ncheck_for_update_on_startup = false\nmodel_provider = \"local\"\n[model_providers.local]\nname = \"local\"\nbase_url = \"{}/v1\"\nwire_api = \"responses\"\n",
            server.uri()
        ),
    )?;
    Ok((server, home))
}

fn local_command(home: &TempDir) -> Result<Command> {
    let mut command = Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
    command
        .current_dir(home.path())
        .env("CODEX_HOME", home.path())
        .env("NO_PROXY", "127.0.0.1,localhost")
        .env("no_proxy", "127.0.0.1,localhost")
        .stdin(Stdio::null());
    for name in [
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "http_proxy",
        "https_proxy",
        "all_proxy",
        "CODEX_ACCESS_TOKEN",
        "CODEX_API_KEY",
        "OPENAI_API_KEY",
        "CODEX_CA_CERTIFICATE",
        "SSL_CERT_FILE",
        "OPENAI_WORKLOAD_IDENTITY_CONTEXT",
        "OPENAI_IDENTITY_TOKEN_FILE",
        "OPENAI_FEDERATION_RULE_ID",
    ] {
        command.env_remove(name);
    }
    Ok(command)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn network_only_selects_checks_without_database_or_mcp_scans() -> Result<()> {
    let (server, home) = local_provider().await?;
    Mock::given(path("/unrelated-mcp"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    let sqlite =
        SqliteConfig::new_for_testing(AbsolutePathBuf::try_from(home.path().to_path_buf())?);
    let database = sqlite.logs_db_path();
    std::fs::write(&database, "not a SQLite database")?;
    let mcp_url = format!("{}/unrelated-mcp", server.uri());
    let output = local_command(&home)?
        .args(["doctor", "--network-only", "--json", "--all", "--feedback"])
        .arg("-c")
        .arg(format!("mcp_servers.unrelated.url={mcp_url:?}"))
        .output()?;
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert!(output.status.success(), "{report:#}");
    let checks = report["checks"].as_object().unwrap();
    assert_eq!(
        checks.keys().map(String::as_str).collect::<Vec<_>>(),
        vec![
            "config.load",
            "network.env",
            "network.provider_reachability",
            "network.websocket_reachability",
        ]
    );
    assert_eq!(std::fs::read_to_string(database)?, "not a SQLite database");
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn network_only_reports_recoverable_config_warnings_and_effective_config() -> Result<()> {
    let (_server, home) = local_provider().await?;
    let baseline_output = local_command(&home)?
        .args(["doctor", "--network-only", "--json"])
        .output()?;
    let baseline_report: Value = serde_json::from_slice(&baseline_output.stdout)?;
    assert!(baseline_output.status.success(), "{baseline_report:#}");
    let baseline_warning_count =
        baseline_report["checks"]["config.load"]["details"]["startup warnings"]
            .as_str()
            .unwrap_or("0")
            .parse::<usize>()?;
    let config_path = home.path().join("config.toml");
    let config = std::fs::read_to_string(&config_path)?;
    std::fs::write(
        config_path,
        format!("{config}\n[otel.span_attributes]\n\"\" = \"missing-key\"\n"),
    )?;
    let output = local_command(&home)?
        .args(["doctor", "--network-only", "--json"])
        .output()?;
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert!(output.status.success(), "{report:#}");
    let config_check = &report["checks"]["config.load"];
    let warning_count = config_check["details"]["startup warnings"]
        .as_str()
        .context("missing startup-warning count")?
        .parse::<usize>()?;
    let warnings = &config_check["details"]["startup warning"];
    let startup_warning = std::iter::once(warnings)
        .chain(warnings.as_array().into_iter().flatten())
        .filter_map(Value::as_str)
        .find(|warning| warning.starts_with("Ignoring invalid `otel.span_attributes` config"))
        .context("missing recoverable OTEL config warning")?;
    let check_statuses = report["checks"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(id, check)| (id.clone(), check["status"].clone()))
        .collect::<serde_json::Map<_, _>>();
    assert_eq!(
        json!({
            "overallStatus": report["overallStatus"],
            "checkStatuses": check_statuses,
            "config": {
                "summary": config_check["summary"],
                "scope": config_check["details"]["configuration scope"],
                "provider": config_check["details"]["model provider"],
                "startupWarnings": warning_count,
                "startupWarning": startup_warning,
            },
        }),
        json!({
            "overallStatus": "warning",
            "checkStatuses": {
                "config.load": "warning",
                "network.env": "ok",
                "network.provider_reachability": "ok",
                "network.websocket_reachability": "ok",
            },
            "config": {
                "summary": "config loaded",
                "scope": "invocation config, including cloud-managed policy",
                "provider": "local",
                "startupWarnings": baseline_warning_count + 1,
                "startupWarning": "Ignoring invalid `otel.span_attributes` config: configured span attribute key must not be empty",
            },
        })
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn network_only_config_failure_does_not_guess_probe_targets() -> Result<()> {
    let proxy = MockServer::start().await;
    let home = TempDir::new()?;
    std::fs::write(
        home.path().join("config.toml"),
        "model_provider = sk-config-credential-fixture\n",
    )?;
    let output = local_command(&home)?
        .args(["doctor", "--network-only", "--json"])
        .env("HTTP_PROXY", proxy.uri())
        .env("HTTPS_PROXY", proxy.uri())
        .output()?;
    assert_eq!(output.status.code(), Some(1));
    assert!(!String::from_utf8_lossy(&output.stdout).contains("sk-config-credential-fixture"));
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(report["checks"].as_object().unwrap().len(), 1);
    assert_eq!(report["checks"]["config.load"]["status"], "fail");
    assert_eq!(
        report["checks"]["config.load"]["details"]["network diagnostics"],
        "effective network configuration is unavailable"
    );
    assert!(proxy.received_requests().await.unwrap().is_empty());
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn network_only_human_output() -> Result<()> {
    let (_server, home) = local_provider().await?;
    let output = local_command(&home)?
        .args([
            "doctor",
            "--network-only",
            "--summary",
            "--ascii",
            "--no-color",
        ])
        .output()?;
    let stdout = String::from_utf8(output.stdout)?;
    assert!(output.status.success(), "{stdout}");
    insta::assert_snapshot!("network_only_human_output", stdout);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn network_only_config_failure_human_output() -> Result<()> {
    let home = TempDir::new()?;
    std::fs::write(
        home.path().join("config.toml"),
        "model_provider = sk-config-credential-fixture\n",
    )?;
    let output = local_command(&home)?
        .args(["doctor", "--network-only", "--all", "--ascii", "--no-color"])
        .output()?;
    let stdout = String::from_utf8(output.stdout)?;
    assert_eq!(output.status.code(), Some(1));
    assert!(!stdout.contains("sk-config-credential-fixture"));
    // Config loading may canonicalize the directory, rendering may shorten it,
    // and Windows uses backslashes.
    // Normalize the rendered config-file row while retaining its label and filename.
    let file_row = Regex::new(r"(?m)^(      file +).+[\\/]config\.toml$")?;
    assert!(file_row.is_match(&stdout), "{stdout}");
    let stdout = file_row.replace_all(&stdout, "${1}<CODEX_HOME>/config.toml");
    insta::assert_snapshot!("network_only_config_failure_human_output", stdout);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn doctor_reports_cloud_filesystem_policy_and_rejects_invalid_requirements() -> Result<()> {
    for valid_requirements in [true, false] {
        let server = MockServer::start().await;
        let codex_home = TempDir::new()?;
        let workspace = TempDir::new()?;
        let workspace_key = serde_json::to_string(workspace.path())?;
        let private_path = workspace.path().join("private-doctor-control");
        let glob = format!("{}/**/*.doctor-secret", workspace.path().display());
        let requirements = if valid_requirements {
            format!("[permissions.filesystem]\ndeny_read = [{private_path:?}, {glob:?}]\n")
        } else {
            "[permissions.filesystem]\ndeny_read = false\n".to_string()
        };
        std::fs::write(
            codex_home.path().join("config.toml"),
            format!(
                r#"
cli_auth_credentials_store = "ephemeral"
chatgpt_base_url = "{}/backend-api"
model_provider = "local"
[model_providers.local]
name = "local"
base_url = "{}/v1"
wire_api = "responses"
[windows]
sandbox = "elevated"
[projects.{workspace_key}]
trust_level = "trusted"
"#,
                server.uri(),
                server.uri(),
            ),
        )?;
        // Cloud authentication must use the project selected by --cd.
        std::fs::create_dir(workspace.path().join(".codex"))?;
        std::fs::write(
            workspace.path().join(".codex/config.toml"),
            "cli_auth_credentials_store = \"file\"\n",
        )?;
        write_chatgpt_auth(
            codex_home.path(),
            ChatGptAuthFixture::new("doctor-test-token")
                .account_id("doctor-workspace")
                .chatgpt_account_id("doctor-workspace")
                .chatgpt_user_id("doctor-user")
                .plan_type("enterprise"),
            AuthCredentialsStoreMode::File,
        )?;
        Mock::given(method("GET"))
            .and(path("/backend-api/wham/config/bundle"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "requirements_toml": {
                    "enterprise_managed": [{
                        "id": "doctor-policy",
                        "name": "Doctor policy fixture",
                        "contents": requirements,
                    }],
                },
            })))
            .expect(1)
            .mount(&server)
            .await;

        let output = Command::new(codex_utils_cargo_bin::cargo_bin("codex")?)
            .current_dir(codex_home.path())
            .env("CODEX_HOME", codex_home.path())
            .env("NO_PROXY", "127.0.0.1,localhost")
            .env("no_proxy", "127.0.0.1,localhost")
            .env_remove("CODEX_ACCESS_TOKEN")
            .env_remove("CODEX_API_KEY")
            .env_remove("OPENAI_API_KEY")
            .arg("--cd")
            .arg(workspace.path())
            .args(["doctor", "--json"])
            .stdin(Stdio::null())
            .output()?;
        let report: Value = serde_json::from_slice(&output.stdout)?;
        if valid_requirements {
            let config = &report["checks"]["config.load"];
            let sandbox = &report["checks"]["sandbox.helpers"]["details"];
            assert_eq!(config["status"], "ok", "{config:#}");
            assert_eq!(
                config["details"]["cwd"],
                workspace.path().display().to_string()
            );
            insta::assert_snapshot!(
                serde_json::to_string_pretty(&json!({
                    "scope": config["details"]["configuration scope"],
                    "activeThreadOverrides": config["details"]["active thread overrides"],
                    "denyRules": sandbox["denied-read rules"],
                    "denyGlobs": sandbox["denied-read glob rules"],
                    "scanDepth": sandbox["glob scan max depth"],
                    "managedFilesystemSource": sandbox["managed filesystem source"],
                }))?,
                @r#"
                {
                  "scope": "invocation config, including cloud-managed policy",
                  "activeThreadOverrides": "not inspected",
                  "denyRules": "2",
                  "denyGlobs": "1",
                  "scanDepth": "unbounded",
                  "managedFilesystemSource": "cloud"
                }
                "#
            );
            let stdout = String::from_utf8(output.stdout)?;
            assert!(!stdout.contains("doctor-secret"));
            assert!(!stdout.contains("private-doctor-control"));
        } else {
            assert_eq!(report["checks"]["config.load"]["status"], "fail");
            assert!(report["checks"].get("sandbox.helpers").is_none());
        }
        server.verify().await;
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invalid_custom_ca_falls_back_to_system_roots() -> Result<()> {
    let server = MockServer::start().await;
    for (request_method, request_path) in [("HEAD", "/v1/responses"), ("GET", "/v1/models")] {
        Mock::given(method(request_method))
            .and(path(request_path))
            .respond_with(ResponseTemplate::new(200))
            .expect(2)
            .mount(&server)
            .await;
    }

    let codex_home = TempDir::new()?;
    let certificate = codex_home.path().join("invalid-ca.pem");
    std::fs::write(&certificate, "not a certificate")?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        format!(
            "model_provider = \"local\"\n[model_providers.local]\nname = \"local\"\nbase_url = \"{}/v1\"\nwire_api = \"responses\"\n",
            server.uri()
        ),
    )?;
    for sandbox in [None, Some("seatbelt")] {
        let mut command = Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
        command
            .args(["doctor", "--json"])
            .env("CODEX_HOME", codex_home.path())
            .env("CODEX_CA_CERTIFICATE", &certificate)
            .stdin(Stdio::null());
        if let Some(sandbox) = sandbox {
            command
                .env("CODEX_SANDBOX", sandbox)
                .env("HTTP_PROXY", "http://127.0.0.1:1")
                .env("http_proxy", "http://127.0.0.1:1")
                .env("HTTPS_PROXY", "http://127.0.0.1:1")
                .env("https_proxy", "http://127.0.0.1:1")
                .env("NO_PROXY", "")
                .env("no_proxy", "");
        } else {
            command
                .env("NO_PROXY", "127.0.0.1,localhost")
                .env("no_proxy", "127.0.0.1,localhost");
        }
        let output = command
            .output()
            .context("failed to run the doctor with an invalid custom CA")?;
        let report: Value = serde_json::from_slice(&output.stdout)?;

        assert!(
            report["checks"]["network.provider_reachability"]["details"]["local API inference URL"]
                .as_str()
                .is_some_and(|detail| detail.ends_with("reachable (HTTP 200)"))
        );
    }
    server.verify().await;

    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn doctor_reports_macos_system_proxy_configuration_and_policy() -> Result<()> {
    let codex_home = TempDir::new()?;
    let report = doctor_report(codex_home.path())?;
    let details = &report["checks"]["network.env"]["details"];

    assert_eq!(details["respect system proxy"], json!("disabled"));
    assert!(matches!(
        details["system proxy"].as_str(),
        Some("automatic (PAC)" | "manual" | "direct" | "unavailable")
    ));

    std::fs::write(
        codex_home.path().join("config.toml"),
        "[features]\nrespect_system_proxy = true\n",
    )?;
    let report = doctor_report(codex_home.path())?;
    assert_eq!(
        report["checks"]["network.env"]["details"]["respect system proxy"],
        json!("enabled")
    );

    Ok(())
}

#[cfg(target_os = "macos")]
fn doctor_report(codex_home: &Path) -> Result<Value> {
    let output = Command::new(codex_utils_cargo_bin::cargo_bin("codex")?)
        .args(["doctor", "--json"])
        .env("CODEX_HOME", codex_home)
        .stdin(Stdio::null())
        .output()
        .context("failed to run the doctor")?;

    serde_json::from_slice(&output.stdout).context("doctor did not emit a valid json report")
}
