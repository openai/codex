use std::time::Duration;

use codex_workload_identity::SubjectToken;
use codex_workload_identity::SubjectTokenError;
use codex_workload_identity::SubjectTokenProvider;
use spiffe::SpiffeId;
use spiffe::WorkloadApiClient;
use spiffe::transport::Endpoint;
use tokio::time::timeout;

const SPIFFE_ENDPOINT_SOCKET_ENV: &str = "SPIFFE_ENDPOINT_SOCKET";
#[cfg(not(test))]
const WORKLOAD_API_TIMEOUT: Duration = Duration::from_secs(10);
#[cfg(test)]
const WORKLOAD_API_TIMEOUT: Duration = Duration::from_millis(100);

#[derive(Clone, Debug)]
pub struct SpiffeSubjectTokenProvider {
    endpoint_socket: Option<String>,
    spiffe_id: Option<String>,
    audience: String,
}

impl SpiffeSubjectTokenProvider {
    pub fn new(
        endpoint_socket: Option<String>,
        spiffe_id: Option<String>,
        audience: String,
    ) -> Self {
        Self {
            endpoint_socket: endpoint_socket
                .or_else(|| std::env::var(SPIFFE_ENDPOINT_SOCKET_ENV).ok()),
            spiffe_id,
            audience,
        }
    }
}

impl SubjectTokenProvider for SpiffeSubjectTokenProvider {
    async fn subject_token(&self) -> Result<SubjectToken, SubjectTokenError> {
        let endpoint_socket = self
            .endpoint_socket
            .as_deref()
            .filter(|value| !value.is_empty())
            .ok_or(SubjectTokenError::MissingPrerequisite {
                provider: "spiffe",
                prerequisite: SPIFFE_ENDPOINT_SOCKET_ENV.to_string(),
            })?;
        let endpoint = Endpoint::parse(endpoint_socket)
            .map_err(|_| SubjectTokenError::InvalidConfiguration { provider: "spiffe" })?;
        if !matches!(&endpoint, Endpoint::Unix(_)) {
            return Err(SubjectTokenError::InvalidConfiguration { provider: "spiffe" });
        }
        let spiffe_id = self
            .spiffe_id
            .as_deref()
            .map(str::parse::<SpiffeId>)
            .transpose()
            .map_err(|_| SubjectTokenError::InvalidConfiguration { provider: "spiffe" })?;
        let jwt_svid = timeout(WORKLOAD_API_TIMEOUT, async {
            let client = WorkloadApiClient::connect(endpoint)
                .await
                .map_err(|_| SubjectTokenError::Unavailable { provider: "spiffe" })?;
            client
                .fetch_jwt_svid([self.audience.as_str()], spiffe_id.as_ref())
                .await
                .map_err(|_| SubjectTokenError::InvalidResponse { provider: "spiffe" })
        })
        .await
        .map_err(|_| SubjectTokenError::Unavailable { provider: "spiffe" })??;
        SubjectToken::jwt(jwt_svid.token(), "spiffe")
    }
}

#[cfg(test)]
#[path = "spiffe_tests.rs"]
mod tests;
