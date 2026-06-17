#[cfg(not(target_os = "windows"))]
use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use anyhow::bail;
use base64::Engine;
use codex_config::types::ShellEnvironmentPolicyToml;
use codex_protocol::permissions::ReadDenyMatcher;
use codex_workload_identity::CredentialSourceConfig;
use codex_workload_identity::WorkloadIdentityConfig;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodexBuilder;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde::Serialize;
use serde_json::json;
use tempfile::TempDir;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

const ACCESS_TOKEN_TYPE: &str = "urn:ietf:params:oauth:token-type:access_token";

#[derive(Serialize)]
struct TestConfigToml {
    workload_identity: WorkloadIdentityConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    shell_environment_policy: Option<ShellEnvironmentPolicyToml>,
}

fn workload_identity_config_toml(server: &MockServer, token_path: PathBuf) -> Result<String> {
    workload_identity_config_toml_with_source(
        server,
        CredentialSourceConfig::File { path: token_path },
        /*shell_environment_policy*/ None,
    )
}

fn workload_identity_config_toml_with_source(
    server: &MockServer,
    credential_source: CredentialSourceConfig,
    shell_environment_policy: Option<ShellEnvironmentPolicyToml>,
) -> Result<String> {
    Ok(toml::to_string(&TestConfigToml {
        workload_identity: WorkloadIdentityConfig {
            identity_provider_id: "idp_test".to_string(),
            identity_provider_mapping_id: "idpm_test".to_string(),
            audience: "api://codex-test".to_string(),
            token_url: format!("{}/oauth/token", server.uri()),
            credential_source,
        },
        shell_environment_policy,
    })?)
}

fn test_codex_with_workload_identity(config_toml: String) -> TestCodexBuilder {
    test_codex()
        .with_auth_manager_from_config()
        .with_pre_build_hook(move |codex_home| {
            std::fs::write(codex_home.join("config.toml"), config_toml)
                .expect("write workload identity test config");
        })
}

fn test_access_token() -> String {
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
        json!({
            "email": "workload@example.com",
            "https://api.openai.com/auth": {
                "chatgpt_account_id": "workspace_test",
                "chatgpt_plan_type": "enterprise",
                "chatgpt_user_id": "user_test",
            }
        })
        .to_string(),
    );
    format!("header.{payload}.signature")
}

fn exchange_response(access_token: &str) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({
        "access_token": access_token,
        "issued_token_type": ACCESS_TOKEN_TYPE,
        "token_type": "Bearer",
        "expires_in": 3600,
        "user_id": "user_test",
        "chatgpt_account_id": "workspace_test",
        "chatgpt_plan_type": "enterprise",
    }))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn configured_workload_identity_authenticates_responses_turn() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = MockServer::start().await;
    let credential_dir = TempDir::new()?;
    let token_path = credential_dir.path().join("subject-token");
    std::fs::write(&token_path, "external-subject-token\n")?;
    let access_token = test_access_token();
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(exchange_response(&access_token))
        .expect(1)
        .mount(&server)
        .await;
    let response_mock = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_assistant_message("msg-1", "authenticated"),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;

    let config_toml = workload_identity_config_toml(&server, token_path.clone())?;
    let mut builder = test_codex_with_workload_identity(config_toml);
    let test = builder.build(&server).await?;
    test.submit_turn("authenticate with workload identity")
        .await?;

    let file_system_policy = test.config.permissions.file_system_sandbox_policy();
    let matcher = ReadDenyMatcher::new(&file_system_policy, test.config.cwd.as_path())
        .expect("workload credential should install a deny-read matcher");
    assert!(
        matcher.is_read_denied(&token_path),
        "the workload credential must remain denied by the runtime sandbox"
    );
    let request = response_mock.single_request();
    assert_eq!(
        request.header("authorization"),
        Some(format!("Bearer {access_token}"))
    );
    assert!(
        !request
            .body_json()
            .to_string()
            .contains(token_path.to_string_lossy().as_ref()),
        "the model-visible request must not reveal the workload credential path"
    );
    Ok(())
}

#[cfg(not(target_os = "windows"))]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn workload_identity_credentials_are_unavailable_to_shell_tools() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = MockServer::start().await;
    let credential_dir = TempDir::new()?;
    let token_path = credential_dir.path().join("azure-subject-token");
    std::fs::write(&token_path, "external-subject-token\n")?;
    let access_token = test_access_token();
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(exchange_response(&access_token))
        .expect(1)
        .mount(&server)
        .await;

    let shell_call_id = "wif-isolation-shell";
    let token_path_string = token_path.to_string_lossy();
    assert!(!token_path_string.contains('\''));
    let command = format!(
        "if [ -z \"${{AZURE_FEDERATED_TOKEN_FILE+x}}\" ]; then printf 'env-hidden\\n'; else printf 'env-visible:%s\\n' \"$AZURE_FEDERATED_TOKEN_FILE\"; fi; if cat '{token_path_string}' >/dev/null 2>&1; then printf 'file-readable\\n'; else printf 'file-denied\\n'; fi"
    );
    let shell_arguments = serde_json::to_string(&json!({
        "command": command,
        "login": false,
        "timeout_ms": 5_000,
    }))?;
    let response_mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("resp-1"),
                responses::ev_function_call(shell_call_id, "shell_command", &shell_arguments),
                responses::ev_completed("resp-1"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("msg-1", "isolated"),
                responses::ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let config_toml = workload_identity_config_toml_with_source(
        &server,
        CredentialSourceConfig::Azure {
            token_file: Some(token_path.clone()),
        },
        Some(ShellEnvironmentPolicyToml {
            r#set: Some(HashMap::from([(
                "AZURE_FEDERATED_TOKEN_FILE".to_string(),
                "credential-env-must-not-leak".to_string(),
            )])),
            ..Default::default()
        }),
    )?;
    let mut builder = test_codex_with_workload_identity(config_toml);
    let test = builder.build(&server).await?;
    test.submit_turn_with_permission_profile(
        "verify workload credentials are isolated",
        test.config.permissions.permission_profile().clone(),
    )
    .await?;

    let output = response_mock
        .function_call_output_text(shell_call_id)
        .expect("shell output present");
    assert!(
        output.contains("env-hidden"),
        "unexpected shell output: {output}"
    );
    assert!(
        output.contains("file-denied"),
        "unexpected shell output: {output}"
    );
    assert!(!output.contains("credential-env-must-not-leak"));
    assert!(!output.contains("external-subject-token"));
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn required_workload_identity_failure_blocks_session_startup() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = MockServer::start().await;
    let credential_dir = TempDir::new()?;
    let missing_token_path = credential_dir.path().join("missing-subject-token");
    let config_toml = workload_identity_config_toml(&server, missing_token_path)?;
    let mut builder = test_codex_with_workload_identity(config_toml);

    let error = match builder.build(&server).await {
        Ok(_) => bail!("required workload identity should block session startup"),
        Err(error) => error,
    };
    assert!(
        format!("{error:#}").contains("file credential source could not read"),
        "unexpected startup error: {error:#}"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unauthenticated_provider_skips_unavailable_workload_identity() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = MockServer::start().await;
    let credential_dir = TempDir::new()?;
    let missing_token_path = credential_dir.path().join("missing-subject-token");
    let config_toml = workload_identity_config_toml(&server, missing_token_path)?;
    let response_mock = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_assistant_message("msg-1", "unauthenticated"),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;
    let mut builder = test_codex_with_workload_identity(config_toml).with_config(|config| {
        config.model_provider.requires_openai_auth = false;
    });
    let test = builder.build(&server).await?;
    test.submit_turn("use the unauthenticated provider").await?;

    assert_eq!(response_mock.single_request().header("authorization"), None);
    Ok(())
}
