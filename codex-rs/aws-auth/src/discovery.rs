use aws_config::profile::ProfileFileCredentialsProvider;
use aws_config::provider_config::ProviderConfig;
use aws_credential_types::provider::ProvideCredentials;
use aws_types::os_shim_internal::Env;
use aws_types::os_shim_internal::Fs;
use aws_types::region::Region;

use crate::AwsAuthError;

/// A named AWS profile and the region configured directly on that profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AwsProfile {
    pub name: String,
    pub region: Option<String>,
}

/// Discovers AWS profiles using the SDK's shared config and credentials files.
pub async fn discover_aws_profiles() -> Result<Vec<AwsProfile>, AwsAuthError> {
    let profiles = aws_config::profile::load(
        &Fs::real(),
        &Env::real(),
        &Default::default(),
        /*selected_profile_override*/ None,
    )
    .await?;
    let selected_profile = profiles.selected_profile();
    let mut discovered = profiles
        .profiles()
        .filter_map(|name| {
            profiles.get_profile(name).map(|profile| AwsProfile {
                name: name.to_string(),
                region: profile.get("region").map(str::to_string),
            })
        })
        .collect::<Vec<_>>();

    discovered.sort_by(|left, right| {
        (left.name != selected_profile, &left.name)
            .cmp(&(right.name != selected_profile, &right.name))
    });

    Ok(discovered)
}

/// Resolves credentials only from the selected AWS profile.
pub async fn validate_aws_profile(
    profile: &str,
    region: &str,
    factory: codex_http_client::HttpClientFactory,
) -> Result<(), AwsAuthError> {
    profile_credentials_provider(profile, region, factory)
        .provide_credentials()
        .await?;
    Ok(())
}

pub(crate) fn profile_credentials_provider(
    profile: &str,
    region: &str,
    factory: codex_http_client::HttpClientFactory,
) -> ProfileFileCredentialsProvider {
    let config = ProviderConfig::without_region()
        .with_region(Some(Region::new(region.to_string())))
        .with_http_client(crate::transport::http_client(factory));
    ProfileFileCredentialsProvider::builder()
        .configure(&config)
        .profile_name(profile)
        .build()
}
