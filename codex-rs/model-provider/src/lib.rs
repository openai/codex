mod amazon_bedrock;
mod auth;
mod bearer_auth_provider;
mod combined_auth;
mod models_endpoint;
mod models_identity;
mod provider;
mod shared_state;
pub mod test_support;
mod workspace_routing;
pub use workspace_routing::ACCOUNT_ROUTING_HEADER;
pub use workspace_routing::ResolvedResponsesProvider;
pub use workspace_routing::ResponsesConnectionKey;
pub use workspace_routing::WorkspaceRoutingContext;

pub use amazon_bedrock::is_supported_amazon_bedrock_region;
pub use auth::AgentIdentitySessionFallback;
pub use auth::ProviderAuthScope;
pub use auth::ResolvedProviderAuth;
pub use auth::auth_provider_from_auth;
pub use auth::auth_provider_from_auth_manager;
pub use auth::unauthenticated_auth_provider;
pub use bearer_auth_provider::BearerAuthProvider;
pub use bearer_auth_provider::BearerAuthProvider as CoreAuthProvider;
pub use codex_model_provider_info::AMAZON_BEDROCK_PROVIDER_ID;
pub use codex_model_provider_info::AMAZON_BEDROCK_RUNTIME_PROVIDER_ID;
pub use codex_model_provider_info::CHATGPT_CODEX_BASE_URL;
pub use codex_protocol::account::ProviderAccount;
pub use provider::ModelProvider;
pub use provider::ModelProviderFuture;
pub use provider::ProviderAccountError;
pub use provider::ProviderAccountResult;
pub use provider::ProviderAccountState;
pub use provider::ProviderAuthRecoveryMessages;
pub use provider::ProviderCapabilities;
pub use provider::ProviderUnauthorizedRecovery;
pub use provider::RemoteCompactionSupport;
pub use provider::SharedModelProvider;
pub use provider::create_model_provider;

#[cfg(test)]
#[path = "workspace_routing_tests.rs"]
mod workspace_routing_tests;
