mod auth;
mod catalog;
mod mantle;
mod provider_auth;

use std::path::PathBuf;
use std::sync::Arc;

use codex_api::Provider;
use codex_api::SharedAuthProvider;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_model_provider_info::AMAZON_BEDROCK_GPT_5_4_MODEL_ID;
use codex_model_provider_info::ModelProviderAwsAuthInfo;
use codex_model_provider_info::ModelProviderInfo;
use codex_models_manager::manager::SharedModelsManager;
use codex_models_manager::manager::StaticModelsManager;
use codex_protocol::account::ProviderAccount;
use codex_protocol::error::Result;
use codex_protocol::openai_models::ModelsResponse;

use crate::provider::ModelProvider;
use crate::provider::ProviderAccountResult;
use crate::provider::ProviderAccountState;
use crate::provider::ProviderCapabilities;
use auth::resolve_provider_auth;
pub(crate) use catalog::static_model_catalog;
use catalog::with_default_only_service_tier;
pub use mantle::is_supported_region;
use mantle::runtime_base_url;
use provider_auth::StoredAmazonBedrockAuth;
pub use provider_auth::delete_amazon_bedrock_auth;
pub use provider_auth::load_amazon_bedrock_auth;
pub use provider_auth::save_amazon_bedrock_auth;

/// Runtime provider for Amazon Bedrock's OpenAI-compatible Mantle endpoint.
#[derive(Clone, Debug)]
pub(crate) struct AmazonBedrockModelProvider {
    pub(crate) info: ModelProviderInfo,
    pub(crate) aws: ModelProviderAwsAuthInfo,
    stored_auth: StoredAmazonBedrockAuth,
}

impl AmazonBedrockModelProvider {
    pub(crate) fn new(provider_info: ModelProviderInfo, codex_home: Option<PathBuf>) -> Self {
        let aws = provider_info
            .aws
            .clone()
            .unwrap_or(ModelProviderAwsAuthInfo {
                profile: None,
                region: None,
            });
        let stored_auth = match codex_home.as_deref() {
            Some(codex_home) => load_amazon_bedrock_auth(codex_home)
                .map_err(|err| format!("failed to load Amazon Bedrock auth: {err}")),
            None => Ok(None),
        };
        Self {
            info: provider_info,
            aws,
            stored_auth,
        }
    }
}

#[async_trait::async_trait]
impl ModelProvider for AmazonBedrockModelProvider {
    fn info(&self) -> &ModelProviderInfo {
        &self.info
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            namespace_tools: true,
            image_generation: false,
            web_search: false,
        }
    }

    fn approval_review_preferred_model(&self) -> &'static str {
        AMAZON_BEDROCK_GPT_5_4_MODEL_ID
    }

    fn auth_manager(&self) -> Option<Arc<AuthManager>> {
        None
    }

    async fn auth(&self) -> Option<CodexAuth> {
        None
    }

    fn account_state(&self) -> ProviderAccountResult {
        let codex_managed_auth = matches!(&self.stored_auth, Ok(Some(_)));
        Ok(ProviderAccountState {
            account: Some(ProviderAccount::AmazonBedrock),
            requires_openai_auth: false,
            codex_managed_auth,
        })
    }

    async fn api_provider(&self) -> Result<Provider> {
        let mut api_provider_info = self.info.clone();
        api_provider_info.base_url = Some(runtime_base_url(&self.stored_auth, &self.aws).await?);
        api_provider_info.to_api_provider(/*auth_mode*/ None)
    }

    async fn runtime_base_url(&self) -> Result<Option<String>> {
        Ok(Some(runtime_base_url(&self.stored_auth, &self.aws).await?))
    }

    async fn api_auth(&self) -> Result<SharedAuthProvider> {
        resolve_provider_auth(&self.stored_auth, &self.aws).await
    }

    fn models_manager(
        &self,
        _codex_home: PathBuf,
        config_model_catalog: Option<ModelsResponse>,
    ) -> SharedModelsManager {
        Arc::new(StaticModelsManager::new(
            /*auth_manager*/ None,
            config_model_catalog.map_or_else(static_model_catalog, with_default_only_service_tier),
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;
    use std::time::UNIX_EPOCH;

    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn api_provider_for_bedrock_bearer_token_uses_configured_region_endpoint() {
        let region = "eu-central-1";
        let mut api_provider_info =
            ModelProviderInfo::create_amazon_bedrock_provider(/*aws*/ None);
        api_provider_info.base_url = Some(mantle::base_url(region).expect("supported region"));
        let api_provider = api_provider_info
            .to_api_provider(/*auth_mode*/ None)
            .expect("api provider should build");

        assert_eq!(
            api_provider.base_url,
            "https://bedrock-mantle.eu-central-1.api.aws/openai/v1"
        );
    }

    #[test]
    fn capabilities_disable_unsupported_hosted_tools() {
        let provider = AmazonBedrockModelProvider::new(
            ModelProviderInfo::create_amazon_bedrock_provider(/*aws*/ None),
            /*codex_home*/ None,
        );

        assert_eq!(
            provider.capabilities(),
            ProviderCapabilities {
                namespace_tools: true,
                image_generation: false,
                web_search: false,
            }
        );
    }

    #[test]
    fn approval_review_preferred_model_uses_bedrock_gpt_5_4() {
        let provider = AmazonBedrockModelProvider::new(
            ModelProviderInfo::create_amazon_bedrock_provider(/*aws*/ None),
            /*codex_home*/ None,
        );

        assert_eq!(
            provider.approval_review_preferred_model(),
            AMAZON_BEDROCK_GPT_5_4_MODEL_ID
        );
    }

    #[tokio::test]
    async fn stored_auth_is_loaded_when_provider_is_created() {
        let codex_home = std::env::temp_dir().join(format!(
            "codex-bedrock-provider-auth-init-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&codex_home);
        save_amazon_bedrock_auth(&codex_home, "bedrock-key", "eu-west-2").expect("save auth");

        let provider = AmazonBedrockModelProvider::new(
            ModelProviderInfo::create_amazon_bedrock_provider(/*aws*/ None),
            Some(codex_home.clone()),
        );

        delete_amazon_bedrock_auth(&codex_home).expect("delete auth");

        assert_eq!(
            provider
                .runtime_base_url()
                .await
                .expect("runtime base URL should use initialized auth"),
            Some("https://bedrock-mantle.eu-west-2.api.aws/openai/v1".to_string())
        );

        let _ = std::fs::remove_dir_all(&codex_home);
    }
}
