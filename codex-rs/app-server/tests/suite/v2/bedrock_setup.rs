use anyhow::Result;
use app_test_support::TestAppServer;
use codex_app_server_protocol::BedrockDiscoverParams;
use codex_app_server_protocol::BedrockDiscoverResponse;
use codex_app_server_protocol::BedrockSetupParams;
use codex_app_server_protocol::BedrockSetupResponse;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::RequestId;
use codex_config::types::AuthCredentialsStoreMode;
use codex_login::AuthKeyringBackendKind;
use codex_login::login_with_bedrock_api_key;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::path::Path;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

const READ_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 60);

async fn bedrock_app_server(
    codex_home: &Path,
    environment: &[(&str, Option<&str>)],
) -> Result<TestAppServer> {
    let aws_config = codex_home.join("aws-config");
    let aws_credentials = codex_home.join("aws-credentials");
    std::fs::write(&aws_config, "[profile engineering]\nregion = us-west-2\n")?;
    std::fs::write(
        &aws_credentials,
        "[engineering]\naws_access_key_id = engineering-id\naws_secret_access_key = engineering-secret\n\
         [finance]\naws_access_key_id = finance-id\naws_secret_access_key = finance-secret\n",
    )?;
    let aws_config_path = aws_config.to_string_lossy();
    let aws_credentials_path = aws_credentials.to_string_lossy();
    TestAppServer::builder()
        .with_codex_home(codex_home)
        .with_env_overrides(&[
            ("AWS_CONFIG_FILE", Some(aws_config_path.as_ref())),
            (
                "AWS_SHARED_CREDENTIALS_FILE",
                Some(aws_credentials_path.as_ref()),
            ),
            ("AWS_PROFILE", None),
            ("AWS_ACCESS_KEY_ID", None),
            ("AWS_SECRET_ACCESS_KEY", None),
            ("AWS_SESSION_TOKEN", None),
            ("AWS_BEARER_TOKEN_BEDROCK", None),
            ("AWS_REGION", None),
            ("AWS_DEFAULT_REGION", None),
            ("AWS_EC2_METADATA_DISABLED", Some("true")),
        ])
        .with_env_overrides(environment)
        .build_initialized_with_timeout(READ_TIMEOUT)
        .await
}

#[tokio::test]
async fn discover_bedrock_profiles_and_environment_credentials() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut app_server = bedrock_app_server(
        codex_home.path(),
        &[
            ("AWS_PROFILE", Some("engineering")),
            ("AWS_ACCESS_KEY_ID", Some("environment-id")),
            ("AWS_SECRET_ACCESS_KEY", Some("environment-secret")),
            ("AWS_SESSION_TOKEN", Some("environment-token")),
            ("AWS_BEARER_TOKEN_BEDROCK", Some("environment-bedrock-key")),
            ("AWS_DEFAULT_REGION", Some("us-west-2")),
        ],
    )
    .await?;

    let response: BedrockDiscoverResponse = app_server
        .request(|request_id| ClientRequest::BedrockDiscover {
            request_id,
            params: BedrockDiscoverParams {},
        })
        .await?;
    assert_eq!(
        serde_json::to_value(response)?,
        json!({
            "profiles": [
                {"name": "engineering", "region": "us-west-2"},
                {"name": "finance", "region": null}
            ],
            "environmentCredentials": [
                {"type": "accessKeys", "region": null},
                {"type": "bedrockApiKey", "region": null}
            ]
        })
    );

    let response: BedrockSetupResponse = app_server
        .request(|request_id| ClientRequest::BedrockSetup {
            request_id,
            params: BedrockSetupParams::Environment {
                region: " us-east-2 ".to_string(),
            },
        })
        .await?;
    assert_eq!(response, BedrockSetupResponse {});

    let config: toml::Value = toml::from_str(&std::fs::read_to_string(
        codex_home.path().join("config.toml"),
    )?)?;
    assert_eq!(
        config,
        toml::toml! {
            model_provider = "amazon-bedrock"
            [model_providers.amazon-bedrock.aws]
            region = "us-east-2"
        }
        .into()
    );
    assert!(!codex_home.path().join(".env").exists());

    Ok(())
}

#[tokio::test]
async fn setup_bedrock_profile_and_environment() -> Result<()> {
    let codex_home = TempDir::new()?;
    let config_path = codex_home.path().join("config.toml");
    let dotenv_path = codex_home.path().join(".env");
    std::fs::write(
        &config_path,
        "[model_providers.amazon-bedrock]\nhttp_headers = { X-Existing = \"preserved\" }\n\
         [model_providers.amazon-bedrock.aws]\nprofile = \"old\"\nregion = \"us-east-1\"\n",
    )?;
    let existing_dotenv = "# existing configuration\nUNRELATED=value\nAWS_ACCESS_KEY_ID=old-id\n\
         export AWS_SECRET_ACCESS_KEY=old-secret\nAWS_SESSION_TOKEN=stale-token\n";
    std::fs::write(&dotenv_path, existing_dotenv)?;
    let mut app_server = bedrock_app_server(
        codex_home.path(),
        &[
            ("AWS_ACCESS_KEY_ID", Some("environment-id")),
            ("AWS_SECRET_ACCESS_KEY", Some("environment-secret")),
            ("AWS_DEFAULT_REGION", Some("us-east-1")),
        ],
    )
    .await?;

    let response: BedrockSetupResponse = app_server
        .request(|request_id| ClientRequest::BedrockSetup {
            request_id,
            params: BedrockSetupParams::Profile {
                profile: " engineering ".to_string(),
                region: " us-west-2 ".to_string(),
            },
        })
        .await?;
    assert_eq!(response, BedrockSetupResponse {});
    let config: toml::Value = toml::from_str(&std::fs::read_to_string(&config_path)?)?;
    assert_eq!(
        serde_json::to_value(&config["model_providers"]["amazon-bedrock"]["aws"])?,
        json!({"profile": "engineering", "region": "us-west-2"})
    );
    assert_eq!(std::fs::read_to_string(&dotenv_path)?, existing_dotenv);

    let response: BedrockSetupResponse = app_server
        .request(|request_id| ClientRequest::BedrockSetup {
            request_id,
            params: BedrockSetupParams::Environment {
                region: "us-east-1".to_string(),
            },
        })
        .await?;
    assert_eq!(response, BedrockSetupResponse {});
    assert_eq!(std::fs::read_to_string(&dotenv_path)?, existing_dotenv);
    let config: toml::Value = toml::from_str(&std::fs::read_to_string(&config_path)?)?;
    assert_eq!(
        serde_json::to_value(&config["model_providers"]["amazon-bedrock"]["aws"])?,
        json!({"region": "us-east-1"})
    );

    let config: toml::Value = toml::from_str(&std::fs::read_to_string(&config_path)?)?;
    assert_eq!(
        config,
        toml::toml! {
            model_provider = "amazon-bedrock"

            [model_providers.amazon-bedrock]
            http_headers = { X-Existing = "preserved" }

            [model_providers.amazon-bedrock.aws]
            region = "us-east-1"
        }
        .into()
    );

    let managed_home = TempDir::new()?;
    let managed_config_path = managed_home.path().join("config.toml");
    std::fs::write(&managed_config_path, "")?;
    login_with_bedrock_api_key(
        managed_home.path(),
        "managed-bedrock-api-key",
        "us-east-1",
        AuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    )?;
    let auth_path = managed_home.path().join("auth.json");
    let expected_auth = std::fs::read_to_string(&auth_path)?;
    let mut app_server = bedrock_app_server(
        managed_home.path(),
        &[(
            "AWS_BEARER_TOKEN_BEDROCK",
            Some("environment-bedrock-api-key"),
        )],
    )
    .await?;
    let mut expected_config: toml::Value =
        toml::from_str(&std::fs::read_to_string(&managed_config_path)?)?;
    expected_config
        .as_table_mut()
        .expect("config should be a table")
        .extend(toml::toml! {
            model_provider = "amazon-bedrock"
            [model_providers.amazon-bedrock.aws]
            profile = "engineering"
            region = "us-west-2"
        });
    let response: BedrockSetupResponse = app_server
        .request(|request_id| ClientRequest::BedrockSetup {
            request_id,
            params: BedrockSetupParams::Profile {
                profile: "engineering".to_string(),
                region: "us-west-2".to_string(),
            },
        })
        .await?;
    assert_eq!(response, BedrockSetupResponse {});
    assert_eq!(
        toml::from_str::<toml::Value>(&std::fs::read_to_string(&managed_config_path)?)?,
        expected_config
    );

    let request_id = app_server
        .send_raw_request(
            "account/bedrock/setup",
            Some(json!({"type": "environment", "region": "us-east-1"})),
        )
        .await?;
    let error = timeout(
        READ_TIMEOUT,
        app_server.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(
        error.error.message,
        "Codex-managed Bedrock credentials are already configured and take priority over AWS environment credentials. Run `codex logout` and try again."
    );
    assert_eq!(std::fs::read_to_string(&auth_path)?, expected_auth);
    assert_eq!(
        toml::from_str::<toml::Value>(&std::fs::read_to_string(&managed_config_path)?)?,
        expected_config
    );
    assert!(!managed_home.path().join(".env").exists());

    Ok(())
}

#[tokio::test]
async fn setup_bedrock_rejects_invalid_or_conflicting_credentials() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut app_server = bedrock_app_server(codex_home.path(), &[]).await?;

    for (params, expected_error) in [
        (
            json!({"type": "profile", "profile": "engineering", "region": "us-west-1"}),
            "Amazon Bedrock does not support region `us-west-1`",
        ),
        (
            json!({"type": "profile", "profile": " ", "region": "us-west-2"}),
            "AWS profile name must not be empty.",
        ),
        (
            json!({"type": "profile", "profile": "missing", "region": "us-west-2"}),
            "failed to load credentials for AWS profile `missing`:",
        ),
        (
            json!({"type": "environment", "region": "us-west-2"}),
            "No AWS credentials found. Please Configure AWS credentials or complete AWS sign-in, then try again.",
        ),
    ] {
        let request_id = app_server
            .send_raw_request("account/bedrock/setup", Some(params))
            .await?;
        let error = timeout(
            READ_TIMEOUT,
            app_server.read_stream_until_error_message(RequestId::Integer(request_id)),
        )
        .await??;
        assert!(
            error.error.message.contains(expected_error),
            "expected {expected_error:?}, got {:?}",
            error.error.message
        );
    }
    assert!(!codex_home.path().join(".env").exists());
    assert!(!codex_home.path().join("config.toml").exists());

    let home = TempDir::new()?;
    std::fs::write(
        home.path().join("config.toml"),
        "forced_login_method = \"chatgpt\"\n",
    )?;
    let mut app_server = bedrock_app_server(home.path(), &[]).await?;
    let request_id = app_server
        .send_raw_request("account/bedrock/discover", Some(json!({})))
        .await?;
    let error = timeout(
        READ_TIMEOUT,
        app_server.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(
        error.error.message,
        "Amazon Bedrock login is disabled. Use ChatGPT login instead."
    );
    assert!(!home.path().join(".env").exists());

    Ok(())
}
