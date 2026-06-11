use std::path::PathBuf;

use pretty_assertions::assert_eq;

use super::*;

fn valid_config() -> WorkloadIdentityConfig {
    WorkloadIdentityConfig {
        identity_provider_id: "idp_example".to_string(),
        identity_provider_mapping_id: "idpm_example".to_string(),
        token_url: "https://auth.openai.com/oauth/token".to_string(),
        credential_source: CredentialSourceConfig::Azure {
            token_file: Some(PathBuf::from(
                "/var/run/secrets/azure/tokens/azure-identity-token",
            )),
        },
    }
}

#[test]
fn validates_complete_configuration() {
    assert_eq!(valid_config().validate(), Ok(()));
}

#[test]
fn rejects_non_http_token_endpoint() {
    let mut config = valid_config();
    config.token_url = "file:///tmp/token".to_string();

    assert_eq!(
        config.validate(),
        Err(WorkloadIdentityConfigError::UnsupportedTokenUrlScheme)
    );
}

#[test]
fn rejects_non_loopback_http_token_endpoint() {
    let mut config = valid_config();
    config.token_url = "http://attacker.example/oauth/token".to_string();

    assert_eq!(
        config.validate(),
        Err(WorkloadIdentityConfigError::UnsupportedTokenUrlScheme)
    );
}

#[test]
fn allows_loopback_http_token_endpoint_for_local_development() {
    let mut config = valid_config();
    config.token_url = "http://127.0.0.1:3007/oauth/token".to_string();

    assert_eq!(config.validate(), Ok(()));
}

#[test]
fn rejects_relative_token_file() {
    let mut config = valid_config();
    config.credential_source = CredentialSourceConfig::Azure {
        token_file: Some(PathBuf::from("azure-token")),
    };

    assert_eq!(
        config.validate(),
        Err(WorkloadIdentityConfigError::RelativeTokenFile)
    );
}
