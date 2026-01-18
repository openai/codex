//! AWS credential provider for Bedrock authentication.
//!
//! Supports loading credentials from:
//! - AWS profiles (`AWS_PROFILE` environment variable)
//! - Environment variables (`AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`)
//! - IAM instance roles (automatic via aws-config)

use aws_config::BehaviorVersion;
use aws_credential_types::Credentials;
use aws_credential_types::provider::ProvideCredentials;
use aws_credential_types::provider::SharedCredentialsProvider;
use thiserror::Error;

/// Error type for AWS authentication operations.
#[derive(Debug, Error)]
pub enum AwsAuthError {
    #[error("Failed to load AWS credentials: {0}")]
    CredentialsError(String),
    #[error("No AWS credentials available")]
    NoCredentials,
}

/// AWS authentication provider that loads credentials from the environment.
#[derive(Clone)]
pub struct AwsAuthProvider {
    region: String,
    credentials_provider: SharedCredentialsProvider,
}

impl std::fmt::Debug for AwsAuthProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AwsAuthProvider")
            .field("region", &self.region)
            .finish_non_exhaustive()
    }
}

impl AwsAuthProvider {
    /// Create a new AWS auth provider for the given region.
    ///
    /// If `profile` is Some, it will use that specific profile.
    /// Otherwise, it will use the default credential chain which checks:
    /// 1. Environment variables (AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY)
    /// 2. AWS_PROFILE environment variable
    /// 3. Default profile in ~/.aws/credentials
    /// 4. IAM instance roles (EC2, ECS, Lambda)
    pub async fn new(region: &str, profile: Option<String>) -> Result<Self, AwsAuthError> {
        let config = if let Some(profile_name) = profile {
            aws_config::defaults(BehaviorVersion::latest())
                .profile_name(&profile_name)
                .region(aws_config::Region::new(region.to_string()))
                .load()
                .await
        } else {
            aws_config::defaults(BehaviorVersion::latest())
                .region(aws_config::Region::new(region.to_string()))
                .load()
                .await
        };

        let credentials_provider = config
            .credentials_provider()
            .ok_or_else(|| AwsAuthError::NoCredentials)?;

        Ok(Self {
            region: region.to_string(),
            credentials_provider: SharedCredentialsProvider::new(credentials_provider),
        })
    }

    /// Get the AWS region.
    pub fn region(&self) -> &str {
        &self.region
    }

    /// Load credentials from the provider.
    ///
    /// This may involve network calls for IAM roles or token refresh.
    pub async fn credentials(&self) -> Result<Credentials, AwsAuthError> {
        self.credentials_provider
            .provide_credentials()
            .await
            .map_err(|e| AwsAuthError::CredentialsError(e.to_string()))
    }
}

/// Builder for creating AwsAuthProvider with custom configuration.
pub struct AwsAuthProviderBuilder {
    region: String,
    profile: Option<String>,
}

impl AwsAuthProviderBuilder {
    /// Create a new builder with the specified region.
    pub fn new(region: impl Into<String>) -> Self {
        Self {
            region: region.into(),
            profile: None,
        }
    }

    /// Set a specific AWS profile to use.
    pub fn profile(mut self, profile: impl Into<String>) -> Self {
        self.profile = Some(profile.into());
        self
    }

    /// Build the auth provider.
    pub async fn build(self) -> Result<AwsAuthProvider, AwsAuthError> {
        AwsAuthProvider::new(&self.region, self.profile).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_builder_pattern() {
        // This test just verifies the API compiles correctly.
        // Actual credential loading requires AWS configuration.
        let builder = AwsAuthProviderBuilder::new("us-east-1").profile("test-profile");
        assert_eq!(builder.region, "us-east-1");
        assert_eq!(builder.profile, Some("test-profile".to_string()));
    }
}
