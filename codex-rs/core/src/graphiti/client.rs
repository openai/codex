use chrono::DateTime;
use chrono::Utc;
use reqwest::Client;
use reqwest::Method;
use reqwest::StatusCode;
use reqwest::Url;
use serde::Deserialize;
use serde::Serialize;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GraphitiClientError {
    #[error("invalid graphiti base url: {0}")]
    InvalidBaseUrl(String),

    #[error("failed to join graphiti url: {0}")]
    UrlJoin(#[from] url::ParseError),

    #[error("http request failed: {0}")]
    Request(#[from] reqwest::Error),

    #[error("graphiti request returned unexpected status {status}: {body}")]
    UnexpectedStatus { status: StatusCode, body: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum GraphitiRoleType {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphitiMessage {
    pub content: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,

    pub role_type: GraphitiRoleType,

    pub role: Option<String>,

    pub timestamp: DateTime<Utc>,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source_description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AddMessagesRequest {
    pub group_id: String,
    pub messages: Vec<GraphitiMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphitiResultDto {
    pub message: String,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HealthcheckResponse {
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SearchQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_ids: Option<Vec<String>>,

    pub query: String,

    pub max_facts: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FactResult {
    pub uuid: String,
    pub name: String,
    pub fact: String,
    pub valid_at: Option<DateTime<Utc>>,
    pub invalid_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub expired_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SearchResults {
    pub facts: Vec<FactResult>,
}

#[derive(Clone)]
pub struct GraphitiClient {
    base_url: Url,
    http: Client,
    bearer_token: Option<String>,
}

impl GraphitiClient {
    pub fn from_base_url_str(
        base_url: &str,
        bearer_token: Option<String>,
    ) -> Result<Self, GraphitiClientError> {
        let base_url = Url::parse(base_url)
            .map_err(|_| GraphitiClientError::InvalidBaseUrl(base_url.to_string()))?;
        Ok(Self::new(base_url, bearer_token))
    }

    pub fn new(base_url: Url, bearer_token: Option<String>) -> Self {
        Self {
            base_url,
            http: Client::new(),
            bearer_token,
        }
    }

    fn request(
        &self,
        method: Method,
        path: &str,
        timeout: Duration,
    ) -> Result<reqwest::RequestBuilder, url::ParseError> {
        let url = self.base_url.join(path.trim_start_matches('/'))?;
        let mut request = self.http.request(method, url).timeout(timeout);
        if let Some(token) = &self.bearer_token {
            request = request.bearer_auth(token);
        }
        Ok(request)
    }

    pub async fn healthcheck(
        &self,
        timeout: Duration,
    ) -> Result<HealthcheckResponse, GraphitiClientError> {
        let response = self
            .request(Method::GET, "/healthcheck", timeout)?
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(GraphitiClientError::UnexpectedStatus { status, body });
        }

        Ok(response.json::<HealthcheckResponse>().await?)
    }

    pub async fn add_messages(
        &self,
        request: AddMessagesRequest,
        timeout: Duration,
        max_batch_size: usize,
    ) -> Result<GraphitiResultDto, GraphitiClientError> {
        if max_batch_size == 0 {
            return Ok(GraphitiResultDto {
                message: "no-op (max_batch_size=0)".to_string(),
                success: true,
            });
        }

        let mut last_result = None;
        for chunk in request.messages.chunks(max_batch_size) {
            let response = self
                .request(Method::POST, "/messages", timeout)?
                .json(&AddMessagesRequest {
                    group_id: request.group_id.clone(),
                    messages: chunk.to_vec(),
                })
                .send()
                .await?;

            if response.status() != StatusCode::ACCEPTED && !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                return Err(GraphitiClientError::UnexpectedStatus { status, body });
            }

            last_result = Some(response.json::<GraphitiResultDto>().await?);
        }

        Ok(last_result.unwrap_or(GraphitiResultDto {
            message: "no-op (empty messages)".to_string(),
            success: true,
        }))
    }

    pub async fn search(
        &self,
        request: SearchQuery,
        timeout: Duration,
    ) -> Result<SearchResults, GraphitiClientError> {
        let response = self
            .request(Method::POST, "/search", timeout)?
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(GraphitiClientError::UnexpectedStatus { status, body });
        }

        Ok(response.json::<SearchResults>().await?)
    }

    pub async fn get_episodes(
        &self,
        group_id: &str,
        last_n: usize,
        timeout: Duration,
    ) -> Result<serde_json::Value, GraphitiClientError> {
        let path = format!("/episodes/{group_id}?last_n={last_n}");
        let response = self.request(Method::GET, &path, timeout)?.send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(GraphitiClientError::UnexpectedStatus { status, body });
        }

        Ok(response.json::<serde_json::Value>().await?)
    }

    pub async fn delete_group(
        &self,
        group_id: &str,
        timeout: Duration,
    ) -> Result<GraphitiResultDto, GraphitiClientError> {
        let path = format!("/group/{group_id}");
        let response = self.request(Method::DELETE, &path, timeout)?.send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(GraphitiClientError::UnexpectedStatus { status, body });
        }

        Ok(response.json::<GraphitiResultDto>().await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::body_json;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    #[tokio::test]
    async fn healthcheck_parses_response() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/healthcheck"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(HealthcheckResponse {
                    status: "healthy".to_string(),
                }),
            )
            .mount(&server)
            .await;

        let client = GraphitiClient::from_base_url_str(&server.uri(), None).unwrap();
        let out = client
            .healthcheck(Duration::from_millis(250))
            .await
            .unwrap();
        assert_eq!(out.status, "healthy");
    }

    #[tokio::test]
    async fn add_messages_splits_batches() {
        let server = MockServer::start().await;

        let request = AddMessagesRequest {
            group_id: "g1".to_string(),
            messages: vec![
                GraphitiMessage {
                    content: "a".to_string(),
                    uuid: None,
                    name: String::new(),
                    role_type: GraphitiRoleType::User,
                    role: None,
                    timestamp: DateTime::<Utc>::MIN_UTC,
                    source_description: String::new(),
                },
                GraphitiMessage {
                    content: "b".to_string(),
                    uuid: None,
                    name: String::new(),
                    role_type: GraphitiRoleType::Assistant,
                    role: None,
                    timestamp: DateTime::<Utc>::MIN_UTC,
                    source_description: String::new(),
                },
            ],
        };

        Mock::given(method("POST"))
            .and(path("/messages"))
            .and(body_json(&AddMessagesRequest {
                group_id: "g1".to_string(),
                messages: vec![request.messages[0].clone()],
            }))
            .respond_with(ResponseTemplate::new(202).set_body_json(GraphitiResultDto {
                message: "ok1".to_string(),
                success: true,
            }))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/messages"))
            .and(body_json(&AddMessagesRequest {
                group_id: "g1".to_string(),
                messages: vec![request.messages[1].clone()],
            }))
            .respond_with(ResponseTemplate::new(202).set_body_json(GraphitiResultDto {
                message: "ok2".to_string(),
                success: true,
            }))
            .expect(1)
            .mount(&server)
            .await;

        let client = GraphitiClient::from_base_url_str(&server.uri(), None).unwrap();
        let out = client
            .add_messages(request, Duration::from_millis(250), 1)
            .await
            .unwrap();
        assert_eq!(out.message, "ok2");
        assert_eq!(out.success, true);
    }
}
