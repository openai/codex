use codex_workload_identity::SubjectToken;
use codex_workload_identity::SubjectTokenError;
use codex_workload_identity::SubjectTokenProvider;
use spiffe::SpiffeId;
use spiffe::WorkloadApiClient;

const SPIFFE_ENDPOINT_SOCKET_ENV: &str = "SPIFFE_ENDPOINT_SOCKET";

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
        let spiffe_id = self
            .spiffe_id
            .as_deref()
            .map(str::parse::<SpiffeId>)
            .transpose()
            .map_err(|_| SubjectTokenError::InvalidConfiguration { provider: "spiffe" })?;
        let client = WorkloadApiClient::connect_to(endpoint_socket)
            .await
            .map_err(|_| SubjectTokenError::Unavailable { provider: "spiffe" })?;
        let jwt_svid = client
            .fetch_jwt_svid([self.audience.as_str()], spiffe_id.as_ref())
            .await
            .map_err(|_| SubjectTokenError::InvalidResponse { provider: "spiffe" })?;
        SubjectToken::jwt(jwt_svid.token(), "spiffe")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rejects_invalid_explicit_spiffe_id_before_connecting() {
        let source = SpiffeSubjectTokenProvider::new(
            Some("unix:/tmp/does-not-exist.sock".to_string()),
            Some("not-a-spiffe-id".to_string()),
            "openai-audience".to_string(),
        );

        assert!(matches!(
            source.subject_token().await,
            Err(SubjectTokenError::InvalidConfiguration { provider: "spiffe" })
        ));
    }
}
