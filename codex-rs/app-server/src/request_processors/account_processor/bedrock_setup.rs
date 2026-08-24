use super::super::bedrock_auth::BedrockProviderConfig;
use super::super::bedrock_auth::configure_bedrock_provider;
use super::super::bedrock_auth::ensure_user_model_provider_can_be_bedrock;
use super::AccountRequestProcessor;
use crate::error_code::internal_error;
use crate::error_code::invalid_request;
use codex_app_server_protocol::AwsCredentialType;
use codex_app_server_protocol::BedrockAwsProfile;
use codex_app_server_protocol::BedrockDiscoverParams;
use codex_app_server_protocol::BedrockDiscoverResponse;
use codex_app_server_protocol::BedrockEnvironmentCredential;
use codex_app_server_protocol::BedrockSetupParams;
use codex_app_server_protocol::BedrockSetupResponse;
use codex_app_server_protocol::ClientResponsePayload;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_login::CodexAuth;
use codex_model_provider::is_supported_amazon_bedrock_region;

const AWS_ACCESS_KEY_ID: &str = "AWS_ACCESS_KEY_ID";
const AWS_SECRET_ACCESS_KEY: &str = "AWS_SECRET_ACCESS_KEY";
const AWS_BEARER_TOKEN_BEDROCK: &str = "AWS_BEARER_TOKEN_BEDROCK";

impl AccountRequestProcessor {
    pub(crate) async fn bedrock_discover(
        &self,
        _params: BedrockDiscoverParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.ensure_bedrock_login_allowed()?;
        let profiles = codex_aws_auth::discover_aws_profiles()
            .await
            .map_err(|err| internal_error(format!("failed to discover AWS profiles: {err}")))?
            .into_iter()
            .map(|profile| BedrockAwsProfile {
                name: profile.name,
                region: profile.region,
            })
            .collect();

        let region = non_empty_env_var("AWS_REGION");
        let environment_credentials = [
            (
                AwsCredentialType::AccessKeys,
                non_empty_env_var(AWS_ACCESS_KEY_ID).is_some()
                    && non_empty_env_var(AWS_SECRET_ACCESS_KEY).is_some(),
            ),
            (
                AwsCredentialType::BedrockApiKey,
                non_empty_env_var(AWS_BEARER_TOKEN_BEDROCK).is_some(),
            ),
        ]
        .into_iter()
        .filter(|(_, available)| *available)
        .map(|(credential_type, _)| BedrockEnvironmentCredential {
            credential_type,
            region: region.clone(),
        })
        .collect();

        Ok(Some(
            BedrockDiscoverResponse {
                profiles,
                environment_credentials,
            }
            .into(),
        ))
    }

    pub(crate) async fn bedrock_setup(
        &self,
        params: BedrockSetupParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.ensure_bedrock_login_allowed()?;
        ensure_user_model_provider_can_be_bedrock(&self.config_manager).await?;
        if matches!(&params, BedrockSetupParams::Environment { .. })
            && matches!(
                self.auth_manager.auth_cached(),
                Some(CodexAuth::BedrockApiKey(_) | CodexAuth::BedrockAccessKeys(_))
            )
        {
            return Err(invalid_request(
                "Codex-managed Bedrock credentials are already configured and take priority over AWS environment credentials. Run `codex logout` and try again.",
            ));
        }

        let region = match &params {
            BedrockSetupParams::Profile { region, .. }
            | BedrockSetupParams::Environment { region, .. } => region.trim(),
        };
        if !is_supported_amazon_bedrock_region(region) {
            return Err(invalid_request(format!(
                "Amazon Bedrock does not support region `{region}`"
            )));
        }

        let profile = match &params {
            BedrockSetupParams::Profile { profile, .. } => {
                let profile = profile.trim();
                if profile.is_empty() {
                    return Err(invalid_request("AWS profile name must not be empty."));
                }
                codex_aws_auth::validate_aws_profile(profile, region)
                    .await
                    .map_err(|err| {
                        invalid_request(format!(
                            "failed to load credentials for AWS profile `{profile}`: {err}"
                        ))
                    })?;
                Some(profile.to_string())
            }
            BedrockSetupParams::Environment { .. } => {
                let has_bedrock_api_key = non_empty_env_var(AWS_BEARER_TOKEN_BEDROCK).is_some();
                let has_access_keys = non_empty_env_var(AWS_ACCESS_KEY_ID).is_some()
                    && non_empty_env_var(AWS_SECRET_ACCESS_KEY).is_some();
                if !has_bedrock_api_key && !has_access_keys {
                    return Err(invalid_request(
                        "No AWS credentials found. Please Configure AWS credentials or complete AWS sign-in, then try again.",
                    ));
                }
                None
            }
        };

        // TODO: Validate the selected credentials against Bedrock ListModels once supported.
        self.cancel_active_login().await;
        configure_bedrock_provider(
            &self.config_manager,
            BedrockProviderConfig {
                region: Some(region),
                profile: profile.as_deref(),
            },
        )
        .await?;

        Ok(Some(BedrockSetupResponse {}.into()))
    }
}

fn non_empty_env_var(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
