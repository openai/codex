use aws_config::BehaviorVersion;
use aws_config::SdkConfig;
use aws_credential_types::provider::SharedCredentialsProvider;
use aws_types::region::Region;

use crate::AwsAuthConfig;
use crate::AwsAuthError;

pub(crate) async fn load_sdk_config(config: &AwsAuthConfig) -> Result<SdkConfig, AwsAuthError> {
    if config.service.trim().is_empty() {
        return Err(AwsAuthError::EmptyService);
    }

    let mut loader = aws_config::defaults(BehaviorVersion::latest());
    if let Some(region) = config.region.as_ref() {
        loader = loader.region(Region::new(region.clone()));
    }
    if let Some(profile) = config.profile.as_ref() {
        loader = loader.profile_name(profile);
    }

    Ok(loader.load().await)
}

pub(crate) fn credentials_provider(
    sdk_config: &SdkConfig,
) -> Result<SharedCredentialsProvider, AwsAuthError> {
    sdk_config
        .credentials_provider()
        .ok_or(AwsAuthError::MissingCredentialsProvider)
}

pub(crate) fn resolved_region(sdk_config: &SdkConfig) -> Result<String, AwsAuthError> {
    sdk_config
        .region()
        .map(ToString::to_string)
        .ok_or(AwsAuthError::MissingRegion)
}
