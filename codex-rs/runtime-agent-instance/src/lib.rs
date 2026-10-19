//! OpenAI runtime registration types, independent of credential storage and model providers.

mod bootstrap;

pub use bootstrap::ActorInfo;
pub use bootstrap::AgentHostAuthorization;
pub use bootstrap::AuthenticatedActorAuthorization;
pub use bootstrap::RuntimeRegistrationMetadata;
pub use bootstrap::RuntimeRegistrationRequest;
pub use bootstrap::RuntimeRegistrationResponse;
pub use bootstrap::RuntimeRole;
