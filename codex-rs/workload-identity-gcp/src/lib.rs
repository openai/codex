use std::time::Duration;

use codex_workload_identity::BoundedResponseBodyError;
use codex_workload_identity::MAX_SUBJECT_TOKEN_BYTES;
use codex_workload_identity::SubjectToken;
use codex_workload_identity::SubjectTokenError;
use codex_workload_identity::SubjectTokenProvider;
use codex_workload_identity::read_bounded_response_body;
use reqwest::header::HeaderValue;
use url::Url;

const GCP_METADATA_ORIGIN: &str = "http://metadata.google.internal";
const GCP_METADATA_HOST_ENV: &str = "GCE_METADATA_HOST";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Debug)]
pub struct GcpSubjectTokenProvider {
    metadata_origin: String,
    service_account: String,
    audience: String,
    http: reqwest::Client,
}

impl GcpSubjectTokenProvider {
    pub fn new(service_account: Option<String>, audience: String, http: reqwest::Client) -> Self {
        let metadata_origin = std::env::var(GCP_METADATA_HOST_ENV)
            .ok()
            .filter(|host| !host.is_empty())
            .map(|host| format!("http://{host}"))
            .unwrap_or_else(|| GCP_METADATA_ORIGIN.to_string());
        Self {
            metadata_origin,
            service_account: service_account.unwrap_or_else(|| "default".to_string()),
            audience,
            http,
        }
    }

    fn request_url(&self) -> Result<Url, SubjectTokenError> {
        let mut url = Url::parse(&self.metadata_origin)
            .map_err(|_| SubjectTokenError::InvalidConfiguration { provider: "gcp" })?;
        let allowed_host = url
            .host_str()
            .is_some_and(|host| host.eq_ignore_ascii_case("metadata.google.internal"))
            || url.host().is_some_and(|host| match host {
                url::Host::Domain(domain) => domain.eq_ignore_ascii_case("localhost"),
                url::Host::Ipv4(address) => address.is_loopback(),
                url::Host::Ipv6(address) => address.is_loopback(),
            });
        if url.scheme() != "http"
            || !allowed_host
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err(SubjectTokenError::InvalidConfiguration { provider: "gcp" });
        }
        url.path_segments_mut()
            .map_err(|()| SubjectTokenError::InvalidConfiguration { provider: "gcp" })?
            .clear()
            .extend([
                "computeMetadata",
                "v1",
                "instance",
                "service-accounts",
                self.service_account.as_str(),
                "identity",
            ]);
        url.query_pairs_mut()
            .append_pair("audience", &self.audience)
            .append_pair("format", "full");
        Ok(url)
    }
}

impl SubjectTokenProvider for GcpSubjectTokenProvider {
    async fn subject_token(&self) -> Result<SubjectToken, SubjectTokenError> {
        let response = self
            .http
            .get(self.request_url()?)
            .header("Metadata-Flavor", HeaderValue::from_static("Google"))
            .timeout(REQUEST_TIMEOUT)
            .send()
            .await
            .map_err(|_| SubjectTokenError::Unavailable { provider: "gcp" })?;
        if !response.status().is_success() {
            return Err(SubjectTokenError::InvalidResponse { provider: "gcp" });
        }
        let body = read_bounded_response_body(response, MAX_SUBJECT_TOKEN_BYTES)
            .await
            .map_err(|error| match error {
                BoundedResponseBodyError::Request(_) => {
                    SubjectTokenError::InvalidResponse { provider: "gcp" }
                }
                BoundedResponseBodyError::TooLarge => {
                    SubjectTokenError::TooLarge { provider: "gcp" }
                }
            })?;
        let body = String::from_utf8(body)
            .map_err(|_| SubjectTokenError::InvalidResponse { provider: "gcp" })?;
        SubjectToken::jwt(body, "gcp")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::header;
    use wiremock::matchers::method;
    use wiremock::matchers::path;
    use wiremock::matchers::query_param;

    #[tokio::test]
    async fn requests_identity_token_with_exact_audience() -> anyhow::Result<()> {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(
                "/computeMetadata/v1/instance/service-accounts/default/identity",
            ))
            .and(header("Metadata-Flavor", "Google"))
            .and(query_param("audience", "openai-audience"))
            .and(query_param("format", "full"))
            .respond_with(ResponseTemplate::new(200).set_body_string("gcp.jwt.token\n"))
            .expect(1)
            .mount(&server)
            .await;
        let source = GcpSubjectTokenProvider {
            metadata_origin: server.uri(),
            service_account: "default".to_string(),
            audience: "openai-audience".to_string(),
            http: reqwest::Client::new(),
        };

        assert_eq!(
            source.subject_token().await?,
            SubjectToken::jwt("gcp.jwt.token", "gcp")?
        );
        Ok(())
    }

    #[tokio::test]
    async fn rejects_non_metadata_non_loopback_host() {
        let source = GcpSubjectTokenProvider {
            metadata_origin: "http://attacker.example".to_string(),
            service_account: "default".to_string(),
            audience: "openai-audience".to_string(),
            http: reqwest::Client::new(),
        };

        assert!(matches!(
            source.subject_token().await,
            Err(SubjectTokenError::InvalidConfiguration { provider: "gcp" })
        ));
    }
}
