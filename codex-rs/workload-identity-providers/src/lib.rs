use codex_workload_identity::CredentialSourceConfig;
use codex_workload_identity::EnvironmentSubjectTokenSource;
use codex_workload_identity::FileSubjectTokenSource;
use codex_workload_identity::SubjectToken;
use codex_workload_identity::SubjectTokenError;
use codex_workload_identity::SubjectTokenProvider;
use codex_workload_identity_aws::AwsSubjectTokenProvider;
use codex_workload_identity_azure::AzureSubjectTokenProvider;
use codex_workload_identity_github_actions::GithubActionsSubjectTokenProvider;
use codex_workload_identity_spiffe::SpiffeSubjectTokenProvider;

pub use codex_workload_identity::WorkloadIdentityConfig;

pub type ConfiguredWorkloadIdentityClient =
    codex_workload_identity::WorkloadIdentityClient<ConfiguredSubjectTokenProvider>;

pub enum ConfiguredSubjectTokenProvider {
    Environment(EnvironmentSubjectTokenSource),
    File(FileSubjectTokenSource),
    Aws(AwsSubjectTokenProvider),
    Azure(AzureSubjectTokenProvider),
    GithubActions(GithubActionsSubjectTokenProvider),
    Spiffe(SpiffeSubjectTokenProvider),
}

impl ConfiguredSubjectTokenProvider {
    fn from_config(
        config: &CredentialSourceConfig,
        identity_provider_id: &str,
        audience: &str,
        http: reqwest::Client,
    ) -> Self {
        match config {
            CredentialSourceConfig::Environment { variable } => {
                Self::Environment(EnvironmentSubjectTokenSource::capture(variable))
            }
            CredentialSourceConfig::File { path } => {
                Self::File(FileSubjectTokenSource::new(path.clone()))
            }
            CredentialSourceConfig::Azure { token_file } => {
                Self::Azure(AzureSubjectTokenProvider::new(token_file.clone()))
            }
            CredentialSourceConfig::GithubActions {} => Self::GithubActions(
                GithubActionsSubjectTokenProvider::capture(audience.to_string(), http),
            ),
            CredentialSourceConfig::Spiffe {
                endpoint_socket,
                spiffe_id,
            } => {
                let _ = http;
                Self::Spiffe(SpiffeSubjectTokenProvider::new(
                    endpoint_socket.clone(),
                    spiffe_id.clone(),
                    audience.to_string(),
                ))
            }
            CredentialSourceConfig::Aws { region } => {
                let _ = http;
                Self::Aws(AwsSubjectTokenProvider::new(
                    identity_provider_id.to_string(),
                    audience.to_string(),
                    region.clone(),
                ))
            }
        }
    }
}

impl SubjectTokenProvider for ConfiguredSubjectTokenProvider {
    async fn subject_token(&self) -> Result<SubjectToken, SubjectTokenError> {
        match self {
            Self::Environment(source) => source.subject_token().await,
            Self::File(source) => source.subject_token().await,
            Self::Aws(source) => source.subject_token().await,
            Self::Azure(source) => source.subject_token().await,
            Self::GithubActions(source) => source.subject_token().await,
            Self::Spiffe(source) => source.subject_token().await,
        }
    }
}

pub fn build_client(
    config: WorkloadIdentityConfig,
    client_id: impl Into<String>,
    no_redirect_http: reqwest::Client,
) -> ConfiguredWorkloadIdentityClient {
    let source = ConfiguredSubjectTokenProvider::from_config(
        &config.credential_source,
        &config.identity_provider_id,
        &config.audience,
        no_redirect_http.clone(),
    );
    ConfiguredWorkloadIdentityClient::new(config, client_id, no_redirect_http, source)
}
