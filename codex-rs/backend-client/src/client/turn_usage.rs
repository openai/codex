use super::Client;
use super::RequestError;
use http::HeaderMap;
use http::Method;
use http::header::CONTENT_TYPE;
use http::header::HeaderValue;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApiKeyTurnCostStatus {
    Pending,
    Priced,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ApiKeyResponseCost {
    pub response_id: String,
    pub total_usd: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ApiKeyTurnCost {
    pub turn_id: String,
    pub status: ApiKeyTurnCostStatus,
    pub total_usd: Option<String>,
    pub event_count: Option<u64>,
    pub responses: Option<Vec<ApiKeyResponseCost>>,
    pub model: Option<String>,
    pub speed: Option<String>,
    pub reasoning_effort: Option<String>,
}

#[derive(Serialize)]
struct ApiKeyTurnCostsRequest<'a> {
    turn_ids: &'a [String],
}

#[derive(Deserialize)]
struct ApiKeyTurnCostsResponse {
    turns: Vec<ApiKeyTurnCost>,
}

impl Client {
    pub async fn query_api_key_turn_costs(
        &self,
        turn_ids: &[String],
        provider_headers: &HeaderMap,
    ) -> Result<Vec<ApiKeyTurnCost>, RequestError> {
        let mut url =
            url::Url::parse(&self.base_url).map_err(|error| RequestError::Other(error.into()))?;
        let analytics_host = match url.host_str() {
            Some("chatgpt.com" | "chat.openai.com") => Some("api.chatgpt.com"),
            Some("chatgpt-staging.com") => Some("api.chatgpt-staging.com"),
            _ => None,
        };
        if let Some(host) = analytics_host {
            url.set_host(Some(host))
                .map_err(|error| RequestError::Other(error.into()))?;
        }
        url.set_path("/v1/analytics/codex/turn-costs");
        url.set_query(None);
        url.set_fragment(None);
        let provider_scope_headers = provider_headers
            .iter()
            .filter(|(name, _)| {
                ["openai-organization", "openai-project"]
                    .iter()
                    .any(|allowed| name.as_str().eq_ignore_ascii_case(allowed))
            })
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect();
        self.query_api_key_turn_costs_at(url.as_ref(), turn_ids, &provider_scope_headers)
            .await
    }

    /// Queries an API-key turn-cost endpoint chosen by the caller, attaching
    /// the supplied provider headers in addition to this client's auth.
    pub async fn query_api_key_turn_costs_at(
        &self,
        url: &str,
        turn_ids: &[String],
        provider_headers: &HeaderMap,
    ) -> Result<Vec<ApiKeyTurnCost>, RequestError> {
        let mut headers = provider_headers.clone();
        headers.extend(self.headers());
        let request = self
            .request(Method::POST, url)
            .headers(headers)
            .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
            .json(&ApiKeyTurnCostsRequest { turn_ids });
        let (body, content_type) = self.exec_request_detailed(request, "POST", url).await?;
        let response: ApiKeyTurnCostsResponse = self
            .decode_json(url, &content_type, &body)
            .map_err(RequestError::Other)?;
        Ok(response.turns)
    }
}

#[cfg(test)]
mod tests {
    use super::ApiKeyResponseCost;
    use super::ApiKeyTurnCost;
    use super::ApiKeyTurnCostStatus;
    use super::Client;
    use codex_http_client::HttpClientFactory;
    use codex_http_client::OutboundProxyPolicy;
    use codex_login::CodexAuth;
    use http::HeaderMap;
    use http::HeaderValue;
    use pretty_assertions::assert_eq;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::body_json;
    use wiremock::matchers::header;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    #[tokio::test]
    async fn api_key_turn_cost_queries_use_api_key_auth_and_provider_scope() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/analytics/codex/turn-costs"))
            .and(header("authorization", "Bearer sk-test"))
            .and(header("openai-organization", "org-test"))
            .and(header("openai-project", "project-test"))
            .and(body_json(serde_json::json!({
                "turn_ids": ["turn-priced", "turn-response"]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "turns": [
                    {
                        "turn_id": "turn-priced",
                        "status": "priced",
                        "total_usd": "1.2500000001",
                        "event_count": 2,
                        "model": "gpt-5.6",
                        "speed": "fast",
                        "reasoning_effort": "high"
                    },
                    {
                        "turn_id": "turn-response",
                        "status": "priced",
                        "total_usd": "0.5000000000",
                        "responses": [{
                            "response_id": "resp-one",
                            "total_usd": "0.5000000000"
                        }]
                    }
                ]
            })))
            .expect(1)
            .mount(&server)
            .await;

        let auth = CodexAuth::from_api_key("sk-test");
        let client = Client::from_auth(
            format!("{}/backend-api", server.uri()),
            &auth,
            HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        );
        let mut provider_headers = HeaderMap::new();
        provider_headers.insert("openai-organization", HeaderValue::from_static("org-test"));
        provider_headers.insert("openai-project", HeaderValue::from_static("project-test"));
        let costs = client
            .query_api_key_turn_costs(
                &["turn-priced".to_string(), "turn-response".to_string()],
                &provider_headers,
            )
            .await
            .expect("query API key turn costs");

        assert_eq!(
            costs,
            vec![
                ApiKeyTurnCost {
                    turn_id: "turn-priced".to_string(),
                    status: ApiKeyTurnCostStatus::Priced,
                    total_usd: Some("1.2500000001".to_string()),
                    event_count: Some(2),
                    responses: None,
                    model: Some("gpt-5.6".to_string()),
                    speed: Some("fast".to_string()),
                    reasoning_effort: Some("high".to_string()),
                },
                ApiKeyTurnCost {
                    turn_id: "turn-response".to_string(),
                    status: ApiKeyTurnCostStatus::Priced,
                    total_usd: Some("0.5000000000".to_string()),
                    event_count: None,
                    responses: Some(vec![ApiKeyResponseCost {
                        response_id: "resp-one".to_string(),
                        total_usd: "0.5000000000".to_string(),
                    }]),
                    model: None,
                    speed: None,
                    reasoning_effort: None,
                },
            ]
        );
    }

    #[tokio::test]
    async fn custom_turn_cost_queries_apply_client_auth_after_provider_headers() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/analytics/codex/turn-costs"))
            .and(header("authorization", "Bearer sk-test"))
            .and(header("chatgpt-account-id", "account-test"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "turns": []
            })))
            .expect(1)
            .mount(&server)
            .await;

        let auth = CodexAuth::from_api_key("sk-test");
        let client = Client::from_auth(
            server.uri(),
            &auth,
            HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        )
        .with_chatgpt_account_id("account-test");
        let mut provider_headers = HeaderMap::new();
        provider_headers.insert(
            "authorization",
            HeaderValue::from_static("Bearer provider-override"),
        );
        provider_headers.insert(
            "chatgpt-account-id",
            HeaderValue::from_static("provider-account-override"),
        );
        let costs = client
            .query_api_key_turn_costs_at(
                &format!("{}/analytics/codex/turn-costs", server.uri()),
                &["turn-one".to_string()],
                &provider_headers,
            )
            .await
            .expect("query custom-provider turn costs");

        assert_eq!(costs, Vec::new());
    }
}
