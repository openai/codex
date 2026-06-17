use codex_workload_identity::CredentialSourceConfig;
use codex_workload_identity::EnvironmentSubjectTokenSource;
use codex_workload_identity::FileSubjectTokenSource;
use codex_workload_identity::SubjectToken;
use codex_workload_identity::SubjectTokenError;
use codex_workload_identity::SubjectTokenProvider;
use codex_workload_identity::UnavailableSubjectTokenSource;
#[cfg(feature = "aws")]
use codex_workload_identity_aws::AwsSubjectTokenProvider;
#[cfg(feature = "azure")]
use codex_workload_identity_azure::AzureSubjectTokenProvider;
#[cfg(feature = "github-actions")]
use codex_workload_identity_github_actions::GithubActionsSubjectTokenProvider;
#[cfg(feature = "spiffe")]
use codex_workload_identity_spiffe::SpiffeSubjectTokenProvider;

pub use codex_workload_identity::WorkloadIdentityConfig;

pub type ConfiguredWorkloadIdentityClient =
    codex_workload_identity::WorkloadIdentityClient<ConfiguredSubjectTokenProvider>;

pub enum ConfiguredSubjectTokenProvider {
    Environment(EnvironmentSubjectTokenSource),
    File(FileSubjectTokenSource),
    #[cfg(feature = "aws")]
    Aws(AwsSubjectTokenProvider),
    #[cfg(feature = "azure")]
    Azure(AzureSubjectTokenProvider),
    #[cfg(feature = "github-actions")]
    GithubActions(GithubActionsSubjectTokenProvider),
    #[cfg(feature = "spiffe")]
    Spiffe(SpiffeSubjectTokenProvider),
    Unavailable(UnavailableSubjectTokenSource),
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
                #[cfg(feature = "azure")]
                {
                    Self::Azure(AzureSubjectTokenProvider::new(token_file.clone()))
                }
                #[cfg(not(feature = "azure"))]
                {
                    let _ = token_file;
                    Self::Unavailable(UnavailableSubjectTokenSource::new("azure"))
                }
            }
            CredentialSourceConfig::GithubActions {} => {
                #[cfg(feature = "github-actions")]
                {
                    Self::GithubActions(GithubActionsSubjectTokenProvider::capture(
                        audience.to_string(),
                        http,
                    ))
                }
                #[cfg(not(feature = "github-actions"))]
                {
                    let _ = http;
                    Self::Unavailable(UnavailableSubjectTokenSource::new("github_actions"))
                }
            }
            CredentialSourceConfig::Spiffe {
                endpoint_socket,
                spiffe_id,
            } => {
                #[cfg(feature = "spiffe")]
                {
                    let _ = http;
                    Self::Spiffe(SpiffeSubjectTokenProvider::new(
                        endpoint_socket.clone(),
                        spiffe_id.clone(),
                        audience.to_string(),
                    ))
                }
                #[cfg(not(feature = "spiffe"))]
                {
                    let _ = (endpoint_socket, spiffe_id, http);
                    Self::Unavailable(UnavailableSubjectTokenSource::new("spiffe"))
                }
            }
            CredentialSourceConfig::Aws { region } => {
                #[cfg(feature = "aws")]
                {
                    let _ = http;
                    Self::Aws(AwsSubjectTokenProvider::new(
                        identity_provider_id.to_string(),
                        audience.to_string(),
                        region.clone(),
                    ))
                }
                #[cfg(not(feature = "aws"))]
                {
                    let _ = (identity_provider_id, audience, region, http);
                    Self::Unavailable(UnavailableSubjectTokenSource::new("aws"))
                }
            }
        }
    }
}

impl SubjectTokenProvider for ConfiguredSubjectTokenProvider {
    async fn subject_token(&self) -> Result<SubjectToken, SubjectTokenError> {
        match self {
            Self::Environment(source) => source.subject_token().await,
            Self::File(source) => source.subject_token().await,
            #[cfg(feature = "aws")]
            Self::Aws(source) => source.subject_token().await,
            #[cfg(feature = "azure")]
            Self::Azure(source) => source.subject_token().await,
            #[cfg(feature = "github-actions")]
            Self::GithubActions(source) => source.subject_token().await,
            #[cfg(feature = "spiffe")]
            Self::Spiffe(source) => source.subject_token().await,
            Self::Unavailable(source) => source.subject_token().await,
        }
    }
}

pub fn build_client(
    config: WorkloadIdentityConfig,
    client_id: impl Into<String>,
    http: reqwest::Client,
) -> ConfiguredWorkloadIdentityClient {
    let source = ConfiguredSubjectTokenProvider::from_config(
        &config.credential_source,
        &config.identity_provider_id,
        &config.audience,
        http.clone(),
    );
    ConfiguredWorkloadIdentityClient::new(config, client_id, http, source)
}
