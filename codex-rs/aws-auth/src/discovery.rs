use aws_config::default_provider::region::DefaultRegionChain;
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
pub async fn validate_aws_profile(profile: &str, region: &str) -> Result<(), AwsAuthError> {
    profile_credentials_provider(profile, Some(region))
        .await
        .provide_credentials()
        .await?;
    Ok(())
}

pub(crate) async fn profile_credentials_provider(
    profile: &str,
    region: Option<&str>,
) -> ProfileFileCredentialsProvider {
    let region = match region {
        Some(region) => Some(Region::new(region.to_string())),
        None => {
            DefaultRegionChain::builder()
                .profile_name(profile)
                .build()
                .region()
                .await
        }
    };
    let provider_config = ProviderConfig::without_region().with_region(region);

    ProfileFileCredentialsProvider::builder()
        .configure(&provider_config)
        .profile_name(profile)
        .build()
}
