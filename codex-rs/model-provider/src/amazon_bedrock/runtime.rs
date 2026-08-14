use codex_aws_auth::AwsAuthConfig;
use codex_login::auth::BedrockApiKeyAuth;
use codex_model_provider_info::ModelProviderAwsAuthInfo;
use codex_protocol::error::Result;

use super::BedrockEndpoint;
use super::auth::resolve_region;
use super::mantle::region_from_config;

const BEDROCK_RUNTIME_SERVICE_NAME: &str = "bedrock";

pub(super) fn aws_auth_config(aws: &ModelProviderAwsAuthInfo) -> AwsAuthConfig {
    AwsAuthConfig {
        profile: aws.profile.clone(),
        region: region_from_config(aws),
        service: BEDROCK_RUNTIME_SERVICE_NAME.to_string(),
    }
}

pub(super) fn base_url(region: &str) -> String {
    format!("https://bedrock-runtime.{region}.amazonaws.com/openai/v1")
}

pub(super) async fn bedrock_runtime_base_url(
    managed_auth: Option<&BedrockApiKeyAuth>,
    aws: &ModelProviderAwsAuthInfo,
) -> Result<String> {
    let region = resolve_region(managed_auth, aws, BedrockEndpoint::Runtime).await?;
    Ok(base_url(&region))
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
