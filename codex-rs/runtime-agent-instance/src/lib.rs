//! Ephemeral OpenAI runtimes: composite registration and per-request assertions.

mod bootstrap;
mod registration;
mod runtime;

pub use bootstrap::ActorInfo;
pub use bootstrap::AgentHostAuthorization;
pub use bootstrap::AuthenticatedActorAuthorization;
pub use bootstrap::RuntimeRegistrationMetadata;
pub use bootstrap::RuntimeRegistrationRequest;
pub use bootstrap::RuntimeRegistrationResponse;
pub use bootstrap::RuntimeRole;

pub use registration::DEFAULT_REGISTRATION_URL;
pub use registration::RegistrationError;
pub use registration::RegistrationErrorCode;
pub use registration::RuntimeRegistrationClient;
pub use runtime::RuntimeAgentInstance;
pub use runtime::RuntimeRegistration;
