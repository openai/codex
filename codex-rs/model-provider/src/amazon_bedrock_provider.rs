use std::sync::Arc;

use codex_api::Provider;
use codex_api::SharedAuthProvider;
use codex_aws_auth::AwsAuthConfig;
use codex_aws_auth::AwsAuthContext;
use codex_aws_auth::AwsAuthError;
use codex_aws_auth::region_from_bedrock_bearer_token;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_model_provider_info::ModelProviderAwsAuthInfo;
use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::error::CodexErr;

use crate::aws_auth_provider::AwsBedrockBearerAuthProvider;
use crate::aws_auth_provider::AwsSigV4AuthProvider;
use crate::provider::ModelProvider;

const AWS_BEARER_TOKEN_BEDROCK_ENV_VAR: &str = "AWS_BEARER_TOKEN_BEDROCK";
const BEDROCK_MANTLE_SERVICE_NAME: &str = "bedrock-mantle";
const BEDROCK_MANTLE_SUPPORTED_REGIONS: [&str; 12] = [
    "us-east-2",
    "us-east-1",
    "us-west-2",
    "ap-southeast-3",
    "ap-south-1",
    "ap-northeast-1",
    "eu-central-1",
    "eu-west-1",
    "eu-west-2",
    "eu-south-1",
    "eu-north-1",
    "sa-east-1",
];

/// Runtime provider for Amazon Bedrock's OpenAI-compatible Mantle endpoint.
#[derive(Clone, Debug)]
pub(crate) struct AmazonBedrockModelProvider {
    pub(crate) info: ModelProviderInfo,
    pub(crate) aws: ModelProviderAwsAuthInfo,
}

#[async_trait::async_trait]
impl ModelProvider for AmazonBedrockModelProvider {
    fn info(&self) -> &ModelProviderInfo {
        &self.info
    }

    fn auth_manager(&self) -> Option<Arc<AuthManager>> {
        None
    }

    async fn auth(&self) -> Option<CodexAuth> {
        None
    }

    async fn api_provider(&self) -> codex_protocol::error::Result<Provider> {
        let region = resolve_bedrock_region(&self.aws).await?;
        let mut api_provider_info = self.info.clone();
        api_provider_info.base_url = Some(bedrock_mantle_base_url(&region)?);
        api_provider_info.to_api_provider(/*auth_mode*/ None)
    }

    async fn api_auth(&self) -> codex_protocol::error::Result<SharedAuthProvider> {
        resolve_bedrock_auth(&self.aws).await
    }
}

async fn resolve_bedrock_auth(
    aws: &ModelProviderAwsAuthInfo,
) -> codex_protocol::error::Result<SharedAuthProvider> {
    if let Some(token) = bedrock_bearer_token_from_env() {
        return resolve_bedrock_bearer_auth(token);
    }

    let config = bedrock_aws_auth_config(aws);
    let context = AwsAuthContext::load(config.clone())
        .await
        .map_err(aws_auth_error_to_codex_error)?;
    Ok(Arc::new(AwsSigV4AuthProvider::with_context(
        config, context,
    )))
}

async fn resolve_bedrock_region(
    aws: &ModelProviderAwsAuthInfo,
) -> codex_protocol::error::Result<String> {
    if let Some(token) = bedrock_bearer_token_from_env() {
        return region_from_bedrock_bearer_token(&token).map_err(aws_auth_error_to_codex_error);
    }

    let context = AwsAuthContext::load(bedrock_aws_auth_config(aws))
        .await
        .map_err(aws_auth_error_to_codex_error)?;
    Ok(context.region().to_string())
}

fn resolve_bedrock_bearer_auth(token: String) -> codex_protocol::error::Result<SharedAuthProvider> {
    let _region =
        region_from_bedrock_bearer_token(&token).map_err(aws_auth_error_to_codex_error)?;
    Ok(Arc::new(AwsBedrockBearerAuthProvider::new(token)))
}

fn bedrock_bearer_token_from_env() -> Option<String> {
    std::env::var(AWS_BEARER_TOKEN_BEDROCK_ENV_VAR)
        .ok()
        .map(|token| token.trim().to_string())
        .filter(|token| !token.is_empty())
}

fn bedrock_aws_auth_config(aws: &ModelProviderAwsAuthInfo) -> AwsAuthConfig {
    AwsAuthConfig {
        profile: aws.profile.clone(),
        service: BEDROCK_MANTLE_SERVICE_NAME.to_string(),
    }
}

fn bedrock_mantle_base_url(region: &str) -> codex_protocol::error::Result<String> {
    if BEDROCK_MANTLE_SUPPORTED_REGIONS.contains(&region) {
        Ok(format!("https://bedrock-mantle.{region}.api.aws/v1"))
    } else {
        Err(CodexErr::Fatal(format!(
            "Amazon Bedrock Mantle does not support region `{region}`"
        )))
    }
}

fn aws_auth_error_to_codex_error(error: AwsAuthError) -> CodexErr {
    CodexErr::Fatal(format!("failed to resolve Amazon Bedrock auth: {error}"))
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn bedrock_token_for_region(region: &str) -> String {
        let encoded = match region {
            "us-west-2" => {
                "YmVkcm9jay5hbWF6b25hd3MuY29tLz9BY3Rpb249Q2FsbFdpdGhCZWFyZXJUb2tlbiZYLUFtei1DcmVkZW50aWFsPUFLSURFWEFNUExFJTJGMjAyNjA0MjAlMkZ1cy13ZXN0LTIlMkZiZWRyb2NrJTJGYXdzNF9yZXF1ZXN0JlZlcnNpb249MQ=="
            }
            "eu-central-1" => {
                "YmVkcm9jay5hbWF6b25hd3MuY29tLz9BY3Rpb249Q2FsbFdpdGhCZWFyZXJUb2tlbiZYLUFtei1DcmVkZW50aWFsPUFLSURFWEFNUExFJTJGMjAyNjA0MjAlMkZldS1jZW50cmFsLTElMkZiZWRyb2NrJTJGYXdzNF9yZXF1ZXN0JlZlcnNpb249MQ=="
            }
            _ => panic!("test token fixture missing for {region}"),
        };
        format!("bedrock-api-key-{encoded}")
    }

    #[test]
    fn bedrock_mantle_base_url_uses_region_endpoint() {
        assert_eq!(
            bedrock_mantle_base_url("ap-northeast-1").expect("supported region"),
            "https://bedrock-mantle.ap-northeast-1.api.aws/v1"
        );
    }

    #[test]
    fn bedrock_mantle_base_url_rejects_unsupported_region() {
        let err = bedrock_mantle_base_url("us-west-1").expect_err("unsupported region");

        assert_eq!(
            err.to_string(),
            "Fatal error: Amazon Bedrock Mantle does not support region `us-west-1`"
        );
    }

    #[test]
    fn resolve_bedrock_bearer_auth_uses_token_region_and_header() {
        let token = bedrock_token_for_region("us-west-2");
        let region = region_from_bedrock_bearer_token(&token).expect("bearer token should resolve");
        let resolved = resolve_bedrock_bearer_auth(token).expect("bearer auth should resolve");
        let mut headers = http::HeaderMap::new();

        resolved.add_auth_headers(&mut headers);

        assert_eq!(region, "us-west-2");
        assert!(
            headers
                .get(http::header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                .is_some_and(|value| value.starts_with("Bearer bedrock-api-key-"))
        );
    }

    #[test]
    fn api_provider_for_bedrock_bearer_token_uses_token_region_endpoint() {
        let token = bedrock_token_for_region("eu-central-1");
        let region = region_from_bedrock_bearer_token(&token).expect("bearer token should resolve");
        let mut api_provider_info =
            ModelProviderInfo::create_amazon_bedrock_provider(/*aws*/ None);
        api_provider_info.base_url =
            Some(bedrock_mantle_base_url(&region).expect("supported region"));
        let api_provider = api_provider_info
            .to_api_provider(/*auth_mode*/ None)
            .expect("api provider should build");

        assert_eq!(
            api_provider.base_url,
            "https://bedrock-mantle.eu-central-1.api.aws/v1"
        );
    }

    #[test]
    fn bedrock_aws_auth_config_uses_profile_and_mantle_service() {
        assert_eq!(
            bedrock_aws_auth_config(&ModelProviderAwsAuthInfo {
                profile: Some("codex-bedrock".to_string()),
            }),
            AwsAuthConfig {
                profile: Some("codex-bedrock".to_string()),
                service: "bedrock-mantle".to_string(),
            }
        );
    }
}
