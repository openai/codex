mod client;
mod config;

#[cfg(feature = "azure")]
mod azure;

pub use client::WorkloadIdentityAccessToken;
pub use client::WorkloadIdentityClient;
pub use client::WorkloadIdentityError;
pub use config::CredentialSourceConfig;
pub use config::WorkloadIdentityConfig;
pub use config::WorkloadIdentityConfigError;
pub use config::default_token_url;
