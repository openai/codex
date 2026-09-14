//! Requirements reads report the authentication policy enforced by the running server.

use anyhow::Result;
use app_test_support::TestAppServer;
use codex_app_server_protocol::Account;
use codex_app_server_protocol::GetAccountParams;
use codex_app_server_protocol::GetAccountResponse;
use codex_app_server_protocol::RequestId;
use codex_protocol::config_types::ForcedLoginMethod;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::time::Duration;
use tempfile::TempDir;
use test_case::test_case;
use tokio::time::timeout;
use wiremock::MockServer;

const READ_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 60);

async fn start_server(
    config: &str,
    requirements: Option<&str>,
) -> Result<(TempDir, TestAppServer)> {
    let home = TempDir::new()?;
    std::fs::write(home.path().join("config.toml"), config)?;
    if let Some(requirements) = requirements {
        std::fs::write(home.path().join("requirements.toml"), requirements)?;
    }
    let server = TestAppServer::builder()
        .with_codex_home(home.path())
        .build_initialized_with_timeout(READ_TIMEOUT)
        .await?;
    Ok((home, server))
}

async fn read_requirements(server: &mut TestAppServer) -> Result<Value> {
    let id = server.send_config_requirements_read_request().await?;
    timeout(READ_TIMEOUT, server.read_response(id)).await?
}

#[test_case(""; "no_requirements")]
#[test_case("forced_chatgpt_workspace_id = []"; "empty_forced_workspaces_are_unrestricted")]
#[test_case("forced_chatgpt_workspace_id = ['managed']"; "forced_workspace_does_not_exclude_api")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn config_requirements_read_preserves_unrestricted_default(config: &str) -> Result<()> {
    let (_home, mut server) = start_server(config, /*requirements*/ None).await?;
    assert_eq!(
        read_requirements(&mut server).await?,
        json!({"requirements": null})
    );
    Ok(())
}

#[test_case("allow_remote_control = false", "", &[ForcedLoginMethod::Api, ForcedLoginMethod::Chatgpt]; "unrestricted_with_other_requirements")]
#[test_case("allowed_login_methods = ['api']", "", &[ForcedLoginMethod::Api]; "managed_api")]
#[test_case("allowed_login_methods = ['chatgpt']", "", &[ForcedLoginMethod::Chatgpt]; "managed_chatgpt")]
#[test_case("allowed_login_methods = ['chatgpt', 'api', 'api']", "", &[ForcedLoginMethod::Api, ForcedLoginMethod::Chatgpt]; "both_normalized")]
#[test_case("", "forced_login_method = 'api'", &[ForcedLoginMethod::Api]; "forced_api_without_requirements")]
#[test_case("", "forced_login_method = 'chatgpt'", &[ForcedLoginMethod::Chatgpt]; "forced_chatgpt_without_requirements")]
#[test_case("allowed_login_methods = ['chatgpt', 'api']", "forced_login_method = 'api'", &[ForcedLoginMethod::Api]; "forced_narrows_managed")]
#[test_case("allowed_chatgpt_workspaces = []", "", &[ForcedLoginMethod::Api]; "empty_managed_workspaces")]
#[test_case("allowed_chatgpt_workspaces = ['managed']", "forced_chatgpt_workspace_id = ['other']", &[ForcedLoginMethod::Api]; "disjoint_workspaces")]
#[test_case("allowed_chatgpt_workspaces = ['managed']", "forced_chatgpt_workspace_id = ['other', 'managed']", &[ForcedLoginMethod::Api, ForcedLoginMethod::Chatgpt]; "overlapping_workspaces")]
#[test_case("allowed_login_methods = ['api']\nallowed_chatgpt_workspaces = ['managed']", "forced_chatgpt_workspace_id = ['other']", &[ForcedLoginMethod::Api]; "api_only_ignores_workspace_mismatch")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn config_requirements_read_exposes_effective_login_methods(
    requirements: &str,
    config: &str,
    expected: &[ForcedLoginMethod],
) -> Result<()> {
    let (_home, mut server) = start_server(config, Some(requirements)).await?;
    let wire = read_requirements(&mut server).await?;
    assert_eq!(wire["requirements"]["allowedLoginMethods"], json!(expected));
    if !expected.contains(&ForcedLoginMethod::Chatgpt) {
        let id = server.send_login_account_chatgpt_request().await?;
        let error = timeout(
            READ_TIMEOUT,
            server.read_stream_until_error_message(RequestId::Integer(id)),
        )
        .await??;
        assert!(error.error.message.contains("disabled"), "{error:?}");
    }
    if !expected.contains(&ForcedLoginMethod::Api) {
        let id = server.send_login_account_api_key_request("sk-test").await?;
        let error = timeout(
            READ_TIMEOUT,
            server.read_stream_until_error_message(RequestId::Integer(id)),
        )
        .await??;
        assert!(error.error.message.contains("disabled"), "{error:?}");
    }
    Ok(())
}

#[test_case(""; "removed")]
#[test_case("allowed_login_methods = ['chatgpt']"; "changed")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn config_requirements_read_uses_running_auth_policy(refreshed: &str) -> Result<()> {
    let (home, mut server) = start_server("", Some("allowed_login_methods = ['api']")).await?;
    std::fs::write(home.path().join("requirements.toml"), refreshed)?;
    assert_eq!(
        read_requirements(&mut server).await?["requirements"]["allowedLoginMethods"],
        json!(["api"])
    );
    let id = server.send_login_account_chatgpt_request().await?;
    let error = timeout(
        READ_TIMEOUT,
        server.read_stream_until_error_message(RequestId::Integer(id)),
    )
    .await??;
    assert!(error.error.message.contains("disabled"), "{error:?}");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn config_requirements_read_rejects_invalid_login_method() -> Result<()> {
    let (home, mut server) = start_server("", /*requirements*/ None).await?;
    std::fs::write(
        home.path().join("requirements.toml"),
        "allowed_login_methods = ['saml']",
    )?;
    let id = server.send_config_requirements_read_request().await?;
    let error = timeout(
        READ_TIMEOUT,
        server.read_stream_until_error_message(RequestId::Integer(id)),
    )
    .await??;
    assert!(
        error.error.message.contains("allowed_login_methods"),
        "{error:?}"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn config_requirements_read_preserves_api_only_bedrock_without_chatgpt_requests() -> Result<()>
{
    let backend = MockServer::start().await;
    let (_home, mut server) = start_server(
        &format!(
            r#"
forced_login_method = "api"
chatgpt_base_url = "{}/backend-api"
model_provider = "amazon-bedrock"
[model_providers.amazon-bedrock]
base_url = "https://bedrock.example.com/v1"
[model_providers.amazon-bedrock.auth]
command = "print-token"
"#,
            backend.uri()
        ),
        /*requirements*/ None,
    )
    .await?;
    assert_eq!(
        read_requirements(&mut server).await?["requirements"]["allowedLoginMethods"],
        json!(["api"])
    );
    let id = server
        .send_get_account_request(GetAccountParams {
            refresh_token: false,
        })
        .await?;
    let account: GetAccountResponse = timeout(READ_TIMEOUT, server.read_response(id)).await??;
    assert_eq!(
        account,
        GetAccountResponse {
            account: Some(Account::AmazonBedrock {
                uses_codex_managed_credentials: false
            }),
            requires_openai_auth: false,
        }
    );
    assert!(
        backend
            .received_requests()
            .await
            .expect("recorded requests")
            .is_empty()
    );
    Ok(())
}
