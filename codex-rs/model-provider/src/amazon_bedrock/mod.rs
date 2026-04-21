mod auth;
mod mantle;

use std::sync::Arc;

use codex_api::Provider;
use codex_api::SharedAuthProvider;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_model_provider_info::ModelProviderAwsAuthInfo;
use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::error::Result;

use crate::provider::ModelProvider;
use auth::resolve_provider_auth;
use auth::resolve_region;
use mantle::base_url;

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

    async fn api_provider(&self) -> Result<Provider> {
        let region = resolve_region(&self.aws).await?;
        let mut api_provider_info = self.info.clone();
        api_provider_info.base_url = Some(base_url(&region)?);
        api_provider_info.to_api_provider(/*auth_mode*/ None)
    }

    async fn api_auth(&self) -> Result<SharedAuthProvider> {
        resolve_provider_auth(&self.aws).await
    }
}

#[cfg(test)]
mod tests {
    use codex_aws_auth::region_from_bedrock_bearer_token;
    use pretty_assertions::assert_eq;

    use super::*;

    fn bedrock_token_for_region(region: &str) -> String {
        let encoded = match region {
            "eu-central-1" => {
                "YmVkcm9jay5hbWF6b25hd3MuY29tLz9BY3Rpb249Q2FsbFdpdGhCZWFyZXJUb2tlbiZYLUFtei1DcmVkZW50aWFsPUFLSURFWEFNUExFJTJGMjAyNjA0MjAlMkZldS1jZW50cmFsLTElMkZiZWRyb2NrJTJGYXdzNF9yZXF1ZXN0JlZlcnNpb249MQ=="
            }
            _ => panic!("test token fixture missing for {region}"),
        };
        format!("bedrock-api-key-{encoded}")
    }

    #[test]
    fn api_provider_for_bedrock_bearer_token_uses_token_region_endpoint() {
        let token = bedrock_token_for_region("eu-central-1");
        let region = region_from_bedrock_bearer_token(&token).expect("bearer token should resolve");
        let mut api_provider_info =
            ModelProviderInfo::create_amazon_bedrock_provider(/*aws*/ None);
        api_provider_info.base_url = Some(base_url(&region).expect("supported region"));
        let api_provider = api_provider_info
            .to_api_provider(/*auth_mode*/ None)
            .expect("api provider should build");

        assert_eq!(
            api_provider.base_url,
            "https://bedrock-mantle.eu-central-1.api.aws/v1"
        );
    }
}
