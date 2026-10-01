//! GovCloud checks use current login state and refresh managed requirements without a restart.

use anyhow::Result;
use app_test_support::TestAppServer;
use codex_app_server_protocol::BedrockCheckGovCloudRequirementsParams;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::LoginAccountParams;
use codex_app_server_protocol::LoginAccountResponse;
use codex_app_server_protocol::RequestId;
use codex_config::types::AuthCredentialsStoreMode;
use codex_login::AuthKeyringBackendKind;
use codex_login::login_with_bedrock_api_key;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use test_case::test_case;

const BASELINE: &str = r#"
allowed_login_methods = ["api"]
[application.network]
enabled = true
[application.network.domains]
"bedrock-mantle.us-gov-west-1.api.aws" = "allow"
"#;

async fn check(server: &mut TestAppServer) -> Result<Value> {
    server
        .request(
            |request_id| ClientRequest::BedrockCheckGovCloudRequirements {
                request_id,
                params: BedrockCheckGovCloudRequirementsParams {},
            },
        )
        .await
}

#[tokio::test]
async fn checks_current_provider_after_login_without_restart() -> Result<()> {
    let home = TempDir::new()?;
    std::fs::write(home.path().join("config.toml"), "")?;
    login_with_bedrock_api_key(
        home.path(),
        "test-key",
        "us-gov-west-1",
        AuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    )?;
    let mut server = TestAppServer::builder()
        .with_codex_home(home.path())
        .build_initialized()
        .await?;
    assert_eq!(
        check(&mut server).await?,
        json!({"isGovCloud": false, "shouldWarn": false})
    );
    // GovCloud onboarding is enabled separately. Select the provider using the
    // existing login so stale startup configuration produces a different result.
    std::fs::write(
        home.path().join("config.toml"),
        "model_provider = 'amazon-bedrock'\n",
    )?;
    assert_eq!(
        check(&mut server).await?,
        json!({"isGovCloud": true, "shouldWarn": true})
    );
    let _: LoginAccountResponse = server
        .request(|request_id| ClientRequest::LoginAccount {
            request_id,
            params: LoginAccountParams::AmazonBedrock {
                api_key: "test-key".to_string(),
                region: "us-west-2".to_string(),
            },
        })
        .await?;
    assert_eq!(
        check(&mut server).await?,
        json!({"isGovCloud": false, "shouldWarn": false})
    );

    Ok(())
}

#[test_case("amazon-bedrock", "", "bedrock-mantle.us-gov-west-1.api.aws"; "mantle")]
#[test_case("amazon-bedrock-runtime", "", "bedrock-runtime.us-gov-west-1.amazonaws.com"; "runtime")]
#[test_case("amazon-bedrock", "[model_providers.amazon-bedrock]\nbase_url = 'https://bedrock.example.com/openai/v1'", "bedrock.example.com"; "custom_endpoint")]
#[tokio::test]
async fn checks_required_policy_fields_and_endpoint(
    provider: &str,
    provider_config: &str,
    domain: &str,
) -> Result<()> {
    let home = TempDir::new()?;
    std::fs::write(
        home.path().join("config.toml"),
        format!("model_provider = '{provider}'\n{provider_config}\n"),
    )?;
    login_with_bedrock_api_key(
        home.path(),
        "test-key",
        "us-gov-west-1",
        AuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    )?;
    let mut server = TestAppServer::builder()
        .with_codex_home(home.path())
        .build_initialized()
        .await?;
    let baseline = BASELINE.replace("bedrock-mantle.us-gov-west-1.api.aws", domain);
    for (requirements, should_warn) in [
        (
            baseline.replace("[\"api\"]", "[\"api\", \"chatgpt\"]"),
            true,
        ),
        (baseline.replace("enabled = true", "enabled = false"), true),
        (baseline.replace("\"allow\"", "\"deny\""), true),
        (baseline, false),
    ] {
        std::fs::write(home.path().join("requirements.toml"), &requirements)?;
        assert_eq!(
            check(&mut server).await?,
            json!({"isGovCloud": true, "shouldWarn": should_warn}),
            "{requirements}"
        );
    }
    std::fs::write(home.path().join("requirements.toml"), "[invalid toml")?;
    let request_id = server
        .send_request("account/bedrock/checkGovCloudRequirements", Some(json!({})))
        .await?;
    let error = tokio::time::timeout(
        std::time::Duration::from_secs(/*secs*/ 10),
        server.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!((error.error.code, error.error.data), (-32603, None));
    assert!(
        error
            .error
            .message
            .starts_with("failed to load configuration:")
    );
    Ok(())
}

#[tokio::test]
async fn check_preserves_current_login_when_saved_credentials_are_unreadable() -> Result<()> {
    let home = TempDir::new()?;
    std::fs::write(
        home.path().join("config.toml"),
        "model_provider = 'amazon-bedrock'\ncli_auth_credentials_store = 'file'\n",
    )?;
    login_with_bedrock_api_key(
        home.path(),
        "test-key",
        "us-gov-west-1",
        AuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    )?;
    let mut server = TestAppServer::builder()
        .with_codex_home(home.path())
        .build_initialized()
        .await?;

    // The advisory must keep using the running session's credentials even when
    // its persistent credential store becomes unreadable.
    std::fs::write(home.path().join("auth.json"), "invalid json")?;
    assert_eq!(
        check(&mut server).await?,
        json!({"isGovCloud": true, "shouldWarn": true})
    );
    std::fs::write(home.path().join("requirements.toml"), BASELINE)?;
    assert_eq!(
        check(&mut server).await?,
        json!({"isGovCloud": true, "shouldWarn": false})
    );
    Ok(())
}

#[test_case("bedrock-mantle.us-gov-west-1.api.aws", "us-west-2", true; "mantle_url_overrides_commercial_region")]
#[test_case("bedrock-runtime.us-gov-east-1.amazonaws.com", "us-west-2", true; "runtime_url_overrides_commercial_region")]
#[test_case("bedrock-runtime-fips.us-gov-west-1.amazonaws.com", "us-west-2", true; "fips_url_overrides_commercial_region")]
#[test_case("bedrock-mantle.us-west-2.api.aws", "us-gov-west-1", false; "commercial_url_overrides_govcloud_region")]
#[test_case("bedrock-mantle.us-gov-west-1.api.aws.example.com", "us-west-2", false; "lookalike_proxy_uses_region")]
#[tokio::test]
async fn official_endpoint_takes_precedence_over_auth_region(
    domain: &str,
    auth_region: &str,
    is_gov_cloud: bool,
) -> Result<()> {
    let home = TempDir::new()?;
    std::fs::write(
        home.path().join("config.toml"),
        format!(
            "model_provider = 'amazon-bedrock'\n\
             [model_providers.amazon-bedrock]\n\
             base_url = 'https://{domain}/openai/v1'\n"
        ),
    )?;
    login_with_bedrock_api_key(
        home.path(),
        "test-key",
        auth_region,
        AuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    )?;
    let mut server = TestAppServer::builder()
        .with_codex_home(home.path())
        .build_initialized()
        .await?;
    assert_eq!(
        check(&mut server).await?,
        json!({"isGovCloud": is_gov_cloud, "shouldWarn": is_gov_cloud})
    );
    std::fs::write(
        home.path().join("requirements.toml"),
        BASELINE.replace("bedrock-mantle.us-gov-west-1.api.aws", domain),
    )?;
    assert_eq!(
        check(&mut server).await?,
        json!({"isGovCloud": is_gov_cloud, "shouldWarn": false})
    );
    Ok(())
}
