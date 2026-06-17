use std::time::Duration;

use codex_workload_identity::BoundedResponseBodyError;
use codex_workload_identity::MAX_SUBJECT_TOKEN_BYTES;
use codex_workload_identity::SubjectToken;
use codex_workload_identity::SubjectTokenError;
use codex_workload_identity::SubjectTokenProvider;
use codex_workload_identity::read_bounded_response_body;
use serde::Deserialize;
use url::Host;
use url::Url;

const REQUEST_URL_ENV: &str = "ACTIONS_ID_TOKEN_REQUEST_URL";
const REQUEST_TOKEN_ENV: &str = "ACTIONS_ID_TOKEN_REQUEST_TOKEN";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

pub struct GithubActionsSubjectTokenProvider {
    request_url: Option<String>,
    request_token: Option<String>,
    audience: String,
    http: reqwest::Client,
}

impl GithubActionsSubjectTokenProvider {
    pub fn capture(audience: String, http: reqwest::Client) -> Self {
        Self {
            request_url: std::env::var(REQUEST_URL_ENV).ok(),
            request_token: std::env::var(REQUEST_TOKEN_ENV).ok(),
            audience,
            http,
        }
    }

    fn request_url(&self) -> Result<Url, SubjectTokenError> {
        let raw_url = self
            .request_url
            .as_deref()
            .filter(|value| !value.is_empty())
            .ok_or(SubjectTokenError::MissingPrerequisite {
                provider: "github_actions",
                prerequisite: REQUEST_URL_ENV.to_string(),
            })?;
        let mut url = Url::parse(raw_url).map_err(|_| SubjectTokenError::InvalidConfiguration {
            provider: "github_actions",
        })?;
        let github_https = url.scheme() == "https"
            && url.host_str().is_some_and(|host| {
                host.eq_ignore_ascii_case("actions.githubusercontent.com")
                    || host
                        .to_ascii_lowercase()
                        .ends_with(".actions.githubusercontent.com")
            });
        let loopback_http = url.scheme() == "http"
            && url.host().is_some_and(|host| match host {
                Host::Domain(domain) => domain.eq_ignore_ascii_case("localhost"),
                Host::Ipv4(address) => address.is_loopback(),
                Host::Ipv6(address) => address.is_loopback(),
            });
        if (!github_https && !loopback_http)
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
        {
            return Err(SubjectTokenError::InvalidConfiguration {
                provider: "github_actions",
            });
        }
        let existing_query = url
            .query_pairs()
            .filter(|(name, _)| name != "audience")
            .map(|(name, value)| (name.into_owned(), value.into_owned()))
            .collect::<Vec<_>>();
        url.set_query(None);
        url.query_pairs_mut()
            .extend_pairs(existing_query)
            .append_pair("audience", &self.audience);
        Ok(url)
    }
}

impl std::fmt::Debug for GithubActionsSubjectTokenProvider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GithubActionsSubjectTokenProvider")
            .field("request_url", &"[REDACTED]")
            .field("request_token", &"[REDACTED]")
            .field("audience", &self.audience)
            .finish_non_exhaustive()
    }
}

impl SubjectTokenProvider for GithubActionsSubjectTokenProvider {
    async fn subject_token(&self) -> Result<SubjectToken, SubjectTokenError> {
        let request_token = self
            .request_token
            .as_deref()
            .filter(|value| !value.is_empty())
            .ok_or(SubjectTokenError::MissingPrerequisite {
                provider: "github_actions",
                prerequisite: REQUEST_TOKEN_ENV.to_string(),
            })?;
        let response = self
            .http
            .get(self.request_url()?)
            .bearer_auth(request_token)
            .timeout(REQUEST_TIMEOUT)
            .send()
            .await
            .map_err(|_| SubjectTokenError::Unavailable {
                provider: "github_actions",
            })?;
        if !response.status().is_success() {
            return Err(SubjectTokenError::InvalidResponse {
                provider: "github_actions",
            });
        }
        let body = read_bounded_response_body(response, MAX_SUBJECT_TOKEN_BYTES)
            .await
            .map_err(|error| match error {
                BoundedResponseBodyError::Request(_) => SubjectTokenError::InvalidResponse {
                    provider: "github_actions",
                },
                BoundedResponseBodyError::TooLarge => SubjectTokenError::TooLarge {
                    provider: "github_actions",
                },
            })?;
        let response = serde_json::from_slice::<OidcTokenResponse>(&body).map_err(|_| {
            SubjectTokenError::InvalidResponse {
                provider: "github_actions",
            }
        })?;
        SubjectToken::jwt(response.value, "github_actions")
    }
}

#[derive(Deserialize)]
struct OidcTokenResponse {
    value: String,
}

#[cfg(test)]
#[path = "github_actions_tests.rs"]
mod tests;
