use std::path::PathBuf;

use anyhow::Result;
use anyhow::bail;
use base64::Engine;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
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
}

fn workload_identity_config_toml(server: &MockServer, token_path: PathBuf) -> Result<String> {
    Ok(toml::to_string(&TestConfigToml {
        workload_identity: WorkloadIdentityConfig {
            identity_provider_id: "idp_test".to_string(),
            identity_provider_mapping_id: "idpm_test".to_string(),
            audience: "api://codex-test".to_string(),
            token_url: format!("{}/oauth/token", server.uri()),
            credential_source: CredentialSourceConfig::File { path: token_path },
        },
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

    assert!(
        test.config
            .permissions
            .file_system_sandbox_policy()
            .entries
            .iter()
            .any(|entry| {
                entry.access == FileSystemAccessMode::Deny
                    && matches!(
                        &entry.path,
                        FileSystemPath::Path { path } if path.as_path() == token_path.as_path()
                    )
            }),
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
