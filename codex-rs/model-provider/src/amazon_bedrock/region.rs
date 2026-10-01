//! Resolves the active Bedrock region using the same authentication precedence as requests.
//! This does not validate model availability or issue an inference request.

use std::sync::Arc;

use codex_login::AuthManager;
use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::error::Result;

use super::AmazonBedrockModelProvider;
use super::auth::resolve_region;

/// Resolves a Bedrock provider's region independently of endpoint availability.
pub async fn resolve_amazon_bedrock_region(
    provider_info: ModelProviderInfo,
    auth_manager: Option<Arc<AuthManager>>,
) -> Result<String> {
    let provider = AmazonBedrockModelProvider::new(provider_info, auth_manager);
    let managed_auth = provider.managed_auth();
    let factory = provider.http_client_factory.clone().with_network_policy(
        provider
            .http_client_factory
            .network_policy()
            .clone()
            .for_current_account(),
    );
    resolve_region(
        provider.auth_source(),
        managed_auth.as_ref(),
        &provider.aws,
        provider.endpoint,
        &factory,
    )
    .await
}
