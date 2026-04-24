mod amazon_bedrock;
mod auth;
mod bearer_auth_provider;
mod provider;

pub use codex_agent_identity::AgentTaskExternalRef;

pub use auth::ProviderAuthScope;
pub use auth::auth_provider_from_agent_task;
pub use auth::auth_provider_from_auth;
pub use auth::background_auth_provider_from_agent_identity_auth;
pub use auth::background_auth_provider_from_auth;
pub use auth::provider_uses_first_party_auth_path;
pub use auth::unauthenticated_auth_provider;
pub use bearer_auth_provider::BearerAuthProvider;
pub use bearer_auth_provider::BearerAuthProvider as CoreAuthProvider;
pub use provider::ModelProvider;
pub use provider::SharedModelProvider;
pub use provider::create_model_provider;
