use std::path::PathBuf;

use pretty_assertions::assert_eq;

use super::*;

fn valid_config() -> WorkloadIdentityConfig {
    WorkloadIdentityConfig {
        identity_provider_id: "idp_example".to_string(),
        identity_provider_mapping_id: "idpm_example".to_string(),
        audience: "https://auth.openai.com/workload-identity".to_string(),
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

#[test]
fn every_source_variant_has_a_stable_name() {
    let sources = [
        CredentialSourceConfig::Environment {
            variable: "OPENAI_WIF_TOKEN".to_string(),
        },
        CredentialSourceConfig::File {
            path: PathBuf::from("/var/run/openai/token"),
        },
        CredentialSourceConfig::Azure { token_file: None },
        CredentialSourceConfig::Gcp {
            service_account: None,
        },
        CredentialSourceConfig::GithubActions {},
        CredentialSourceConfig::Spiffe {
            endpoint_socket: None,
            spiffe_id: None,
        },
        CredentialSourceConfig::Aws {},
    ];

    assert_eq!(
        sources.map(|source| source.source_name()),
        [
            "environment",
            "file",
            "azure",
            "gcp",
            "github_actions",
            "spiffe",
            "aws",
        ]
    );
}

#[test]
fn reports_only_secret_bearing_environment_inputs() {
    assert_eq!(
        CredentialSourceConfig::Environment {
            variable: "OPENAI_WIF_TOKEN".to_string(),
        }
        .sensitive_environment_variables(),
        vec!["OPENAI_WIF_TOKEN"]
    );
    assert_eq!(
        CredentialSourceConfig::GithubActions {}.sensitive_environment_variables(),
        vec![
            "ACTIONS_ID_TOKEN_REQUEST_URL",
            "ACTIONS_ID_TOKEN_REQUEST_TOKEN"
        ]
    );
    assert!(
        CredentialSourceConfig::File {
            path: PathBuf::from("/var/run/openai/token")
        }
        .sensitive_environment_variables()
        .is_empty()
    );
}
