//! GCP setup validates before saving and shares provider configuration with subsequent requests.

use anyhow::Context;
use anyhow::Result;
use app_test_support::TestAppServer;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::GcpSetupParams;
use codex_app_server_protocol::GcpSetupResponse;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::ModelListParams;
use codex_app_server_protocol::ModelListResponse;
use codex_app_server_protocol::RequestId;
use codex_protocol::openai_models::ModelVisibility;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

const CLEARED_AUTH_ENV: &[(&str, Option<&str>)] = &[
    ("OPENAI_API_KEY", None),
    ("CODEX_API_KEY", None),
    ("OPENAI_FEDERATION_RULE_ID", None),
    ("OPENAI_IDENTITY_TOKEN_FILE", None),
    ("OPENAI_WORKLOAD_IDENTITY_CONTEXT", None),
];

const READ_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 60);

fn write_fake_gcloud(home: &Path) -> Result<PathBuf> {
    #[cfg(unix)]
    let script = home.join("gcloud");
    #[cfg(unix)]
    codex_utils_cargo_bin::write_executable(
        &script,
        r#"#!/bin/sh
if [ -f fail ]; then exit 1; fi
printf '%s\n' "$GOOGLE_APPLICATION_CREDENTIALS" >> credential-log
printf '%s\n' setup-token
"#,
    )?;
    #[cfg(windows)]
    let script = home.join("gcloud.cmd");
    #[cfg(windows)]
    std::fs::write(
        &script,
        "@echo off\r\nif exist fail exit /b 1\r\nsetlocal EnableDelayedExpansion\r\necho(!GOOGLE_APPLICATION_CREDENTIALS!>>credential-log\r\nendlocal\r\necho setup-token\r\n",
    )?;
    Ok(script)
}

async fn gcp_app_server(
    home: &Path,
    env_overrides: &[(&str, Option<&str>)],
) -> Result<TestAppServer> {
    let existing_path = std::env::var_os("PATH").unwrap_or_default();
    let child_path = std::env::join_paths(
        std::iter::once(home.to_path_buf()).chain(std::env::split_paths(&existing_path)),
    )?;
    TestAppServer::builder()
        .with_codex_home(home)
        .with_env_overrides(&[
            (
                "PATH",
                Some(child_path.to_str().context("child PATH must be UTF-8")?),
            ),
            ("GOOGLE_APPLICATION_CREDENTIALS", None),
            ("GOOGLE_CLOUD_PROJECT", Some("ambient-project")),
            ("GOOGLE_CLOUD_LOCATION", Some("ambient-location")),
        ])
        .with_env_overrides(CLEARED_AUTH_ENV)
        .with_env_overrides(env_overrides)
        .build_initialized_with_timeout(READ_TIMEOUT)
        .await
}

async fn setup_error(app_server: &mut TestAppServer, params: Value) -> Result<JSONRPCErrorError> {
    let id = app_server
        .send_raw_request("account/gcp/setup", Some(params))
        .await?;
    let error = timeout(
        READ_TIMEOUT,
        app_server.read_stream_until_error_message(RequestId::Integer(id)),
    )
    .await??;
    assert_eq!(error.error.code, -32600);
    Ok(error.error)
}

#[tokio::test]
async fn setup_gcp_saves_overrides_restores_adc_and_refreshes_catalog() -> Result<()> {
    let home = TempDir::new()?;
    let home = home.path();
    write_fake_gcloud(home)?;
    let mut catalog = codex_models_manager::bundled_models_response()?;
    catalog.models.retain(|model| model.slug == "gpt-6.1-sol");
    let model = catalog
        .models
        .first_mut()
        .context("bundled catalog must include a supported Vertex model")?;
    model.visibility = ModelVisibility::List;
    model.supported_in_api = true;
    let mut unsupported_model = model.clone();
    unsupported_model.slug = "test-non-vertex-model".to_string();
    catalog.models.push(unsupported_model);
    let catalog_path = home.join("models.json");
    std::fs::write(&catalog_path, serde_json::to_vec(&catalog)?)?;
    let config_path = home.join("config.toml");
    let mut expected_config = json!({
        "model_catalog_json": catalog_path,
        "model_providers": {"gcp": {"http_headers": {"X-Existing": "preserved"}}}
    });
    std::fs::write(&config_path, toml::to_string(&expected_config)?)?;
    let selected = home.join("selected.json");
    let ambient = home.join("ambient.json");
    for file in [&selected, &ambient] {
        std::fs::write(file, r#"{"type":"external_account"}"#)?;
    }
    let mut app_server = gcp_app_server(
        home,
        &[(
            "GOOGLE_APPLICATION_CREDENTIALS",
            Some(ambient.to_str().context("non-UTF-8 ADC path")?),
        )],
    )
    .await?;
    let before: ModelListResponse = app_server
        .request(|request_id| ClientRequest::ModelList {
            request_id,
            params: ModelListParams::default(),
        })
        .await?;
    assert_eq!(before.data.len(), 2);

    let _: GcpSetupResponse = app_server
        .request(|request_id| ClientRequest::GcpSetup {
            request_id,
            params: GcpSetupParams {
                project_id: " selected-project ".to_string(),
                location: Some(" us-central1 ".to_string()),
                credential_file: Some(selected.to_string_lossy().into_owned()),
            },
        })
        .await?;
    expected_config["model_provider"] = json!("gcp");
    expected_config["model_providers"]["gcp"]["project_id"] = json!("selected-project");
    expected_config["model_providers"]["gcp"]["location"] = json!("us-central1");
    expected_config["model_providers"]["gcp"]["credential_file"] = json!(selected);
    assert_eq!(
        toml::from_str::<Value>(&std::fs::read_to_string(&config_path)?)?,
        expected_config
    );

    let after: ModelListResponse = app_server
        .request(|request_id| ClientRequest::ModelList {
            request_id,
            params: ModelListParams::default(),
        })
        .await?;
    assert_eq!(after.data.len(), 1);
    assert_eq!(after.data[0].model, "gpt-6.1-sol");

    // A managed GCP selection takes precedence without replacing the user's preference.
    expected_config["model_provider"] = json!("openai");
    std::fs::write(&config_path, toml::to_string(&expected_config)?)?;
    std::fs::write(home.join("requirements.toml"), "model_provider = 'gcp'")?;
    let request_id = app_server
        .send_raw_request(
            "account/gcp/setup",
            Some(json!({"projectId": "other-project"})),
        )
        .await?;
    let _: GcpSetupResponse = app_server.read_response(request_id).await?;
    expected_config["model_providers"]["gcp"]["project_id"] = json!("other-project");
    expected_config["model_providers"]["gcp"]["location"] = json!("global");
    expected_config["model_providers"]["gcp"]
        .as_object_mut()
        .unwrap()
        .remove("credential_file");
    assert_eq!(
        toml::from_str::<Value>(&std::fs::read_to_string(&config_path)?)?,
        expected_config
    );
    assert_eq!(
        std::fs::read_to_string(home.join("credential-log"))?
            .lines()
            .map(Path::new)
            .collect::<Vec<_>>(),
        [selected.as_path(), ambient.as_path()]
    );
    Ok(())
}

#[tokio::test]
async fn setup_gcp_rejects_invalid_inputs_and_credentials_before_saving() -> Result<()> {
    let home = TempDir::new()?;
    let home = home.path();
    let config_path = home.join("config.toml");
    std::fs::write(&config_path, "")?;
    let invalid = home.join("invalid.json");
    std::fs::write(&invalid, r#"["authorized_user"]"#)?;
    let mut server = gcp_app_server(
        home,
        &[("PATH", Some(home.to_str().context("non-UTF-8 PATH")?))],
    )
    .await?;
    let params = json!({"projectId": "project"});
    for (request, reason) in [
        (json!({"projectId": " "}), None),
        (params.clone(), Some("gcloudNotInstalled")),
        (
            json!({"projectId": "project", "credentialFile": invalid}),
            Some("credentialFileInvalid"),
        ),
    ] {
        let error = setup_error(&mut server, request).await?;
        assert_eq!(error.data, reason.map(|reason| json!({ "reason": reason })));
    }

    write_fake_gcloud(home)?;
    std::fs::write(home.join("fail"), "")?;
    let error = setup_error(&mut server, params).await?;
    assert_eq!(error.data, Some(json!({"reason": "authCommandFailed"})));
    assert_eq!(std::fs::read_to_string(config_path)?, "");
    Ok(())
}

#[tokio::test]
async fn setup_gcp_validates_the_auth_override_relative_to_the_startup_directory() -> Result<()> {
    let home = TempDir::new()?;
    let startup = TempDir::new()?;
    let script = write_fake_gcloud(home.path())?;
    let auth = toml::Value::try_from(json!({"command": script, "cwd": "."}))?;
    let config_path = home.path().join("config.toml");
    std::fs::write(&config_path, "")?;
    // The same command succeeds in CODEX_HOME but fails in the catalog's startup context.
    std::fs::write(startup.path().join("fail"), "")?;
    let mut server = TestAppServer::builder()
        .with_codex_home(startup.path())
        .with_env_overrides(CLEARED_AUTH_ENV)
        .with_env_overrides(&[(
            "CODEX_HOME",
            Some(home.path().to_str().context("non-UTF-8 CODEX_HOME")?),
        )])
        .with_args(&["-c", &format!("model_providers.gcp.auth={auth}")])
        .build_initialized_with_timeout(READ_TIMEOUT)
        .await?;

    let error = setup_error(&mut server, json!({"projectId": "project"})).await?;
    assert_eq!(error.data, Some(json!({"reason": "authCommandFailed"})));
    assert_eq!(std::fs::read_to_string(config_path)?, "");
    Ok(())
}

#[tokio::test]
async fn setup_gcp_rejects_policy_and_endpoint_conflicts_before_saving() -> Result<()> {
    for (file, policy, expected_message) in [
        (
            "managed_config.toml",
            "forced_login_method = 'chatgpt'",
            "Google Cloud setup is disabled. Use ChatGPT login instead.",
        ),
        (
            "managed_config.toml",
            "[model_providers.gcp]\nproject_id = 'managed-project'",
            "Cannot configure `model_providers.gcp.project_id`: Overridden by legacy managed_config.toml",
        ),
        (
            "managed_config.toml",
            "[model_providers.gcp]\nbase_url = 'https://vertex.example.com/v1'",
            "Google Cloud has a custom `base_url`",
        ),
        (
            "requirements.toml",
            "model_provider = 'openai'",
            "requirements set `model_provider` to `openai`",
        ),
        (
            "requirements.toml",
            "[application.network.domains]\n'oauth2.googleapis.com' = 'allow'",
            "disabled by application network restrictions",
        ),
    ] {
        let home = TempDir::new()?;
        let home = home.path();
        write_fake_gcloud(home)?;
        std::fs::write(home.join(file), policy)?;
        let config_path = home.join("config.toml");
        std::fs::write(&config_path, "")?;
        let mut app_server = gcp_app_server(home, &[]).await?;
        let error = setup_error(
            &mut app_server,
            json!({"projectId": "project", "credentialFile": home.join("missing.json")}),
        )
        .await?;
        assert!(
            error.message.contains(expected_message),
            "expected {expected_message:?}, got {:?}",
            error.message
        );
        assert_eq!(std::fs::read_to_string(config_path)?, "");
        assert!(!home.join("credential-log").exists());
    }
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn setup_gcp_stops_the_credential_command_when_network_permission_is_revoked() -> Result<()> {
    use core_test_support::process::wait_for_pid_file;
    use core_test_support::process::wait_for_process_exit;

    let home = TempDir::new()?;
    let config_path = home.path().join("config.toml");
    let initial_config = r#"
[model_providers.gcp.auth]
command = "sh"
args = ["-c", "echo $$ > refresh.pid; exec sleep 120"]
timeout_ms = 120000
"#;
    std::fs::write(&config_path, initial_config)?;
    let mut app_server = gcp_app_server(home.path(), &[]).await?;
    let request_id = app_server
        .send_raw_request("account/gcp/setup", Some(json!({"projectId": "project"})))
        .await?;
    let pid = wait_for_pid_file(&home.path().join("refresh.pid")).await?;

    std::fs::write(
        home.path().join("requirements.toml"),
        "[application.network]",
    )?;
    app_server
        .send_raw_request("config/read", Some(json!({})))
        .await?;
    let error = timeout(
        READ_TIMEOUT,
        app_server.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(
        error.error.message,
        "application network permission was revoked"
    );
    wait_for_process_exit(&pid).await?;
    assert_eq!(std::fs::read_to_string(config_path)?, initial_config);
    Ok(())
}
