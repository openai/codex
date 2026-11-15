//! Generates Cloud titles with the Cloud account's routing and no agent tools.

use codex_api::Compression;
use codex_api::ReqwestTransport;
use codex_api::ResponseEvent;
use codex_api::ResponsesClient;
use codex_api::SharedAuthProvider;
use codex_api::create_text_param_for_request;
use codex_http_client::ClientRouteClass;
use codex_http_client::HttpClientFactory;
use codex_http_client::RouteAwareClientPool;
use codex_http_client::is_allowed_chatgpt_host;
use codex_login::CodexAuth;
use codex_login::default_client::add_originator_header;
use codex_login::default_client::get_codex_user_agent;
use codex_login::default_client::originator;
use codex_model_provider::ACCOUNT_ROUTING_HEADER;
use codex_model_provider::ChatgptBaseUrl;
use codex_model_provider::auth_provider_from_auth;
use codex_protocol::models::ContentItem;
use codex_protocol::models::MessagePhase;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ReasoningEffort;
use color_eyre::eyre::Result;
use color_eyre::eyre::bail;
use color_eyre::eyre::ensure;
use color_eyre::eyre::eyre;
use futures::StreamExt;
use serde_json::Value;
use serde_json::json;
use std::time::Duration;
use url::Url;

pub(crate) struct CloudTitleClient {
    factory: HttpClientFactory,
    auth: SharedAuthProvider,
    account_id: String,
    bootstrap: Url,
    require_origin_match: bool,
}

impl CloudTitleClient {
    pub(super) fn new(
        factory: HttpClientFactory,
        auth: &CodexAuth,
        account_id: String,
        chatgpt_base_url: ChatgptBaseUrl,
    ) -> Result<Self> {
        let require_origin_match = matches!(chatgpt_base_url, ChatgptBaseUrl::Configured(_));
        let bootstrap = Url::parse(chatgpt_base_url.resolve(auth))?;
        ensure!(
            bootstrap.scheme() == "https"
                && bootstrap.host_str().is_some_and(is_allowed_chatgpt_host)
                && bootstrap.username().is_empty()
                && bootstrap.password().is_none()
                && bootstrap.query().is_none()
                && bootstrap.fragment().is_none(),
            "invalid Cloud title routing bootstrap"
        );
        Ok(Self {
            factory,
            auth: auth_provider_from_auth(auth),
            account_id,
            bootstrap,
            require_origin_match,
        })
    }

    pub(crate) async fn generate(
        &self,
        model: &str,
        effort: Option<ReasoningEffort>,
        prompt: String,
        schema: Value,
    ) -> Result<String> {
        tokio::time::timeout(Duration::from_secs(/*secs*/ 30), async {
            let accounts = codex_backend_client::Client::new_without_redirects(
                self.bootstrap.as_str(),
                self.factory.clone(),
            )
            .with_auth_provider(self.auth.clone())
            .map_err(|_| eyre!("could not configure Cloud title authentication"))?
            .with_user_agent(get_codex_user_agent())
            .get_accounts_check()
            .await
            .map_err(|_| eyre!("Cloud title routing is unavailable"))?;
            let mut selected_accounts = accounts
                .accounts
                .into_iter()
                .filter(|account| account.id == self.account_id);
            let account = selected_accounts
                .next()
                .filter(|account| {
                    !account.is_deactivated
                        && !account.is_fedramp_compliant_workspace
                        && selected_accounts.next().is_none()
                })
                .ok_or_else(|| eyre!("Cloud title account is unavailable"))?;
            let origin = match account.workspace_backend_origin.as_deref() {
                Some("NO_CONSTRAINT") => self.bootstrap.origin(),
                Some(value) => {
                    let origin = Url::parse(value)?;
                    ensure!(
                        origin.scheme() == "https"
                            && origin.host_str().is_some_and(is_allowed_chatgpt_host)
                            && origin.username().is_empty()
                            && origin.password().is_none()
                            && origin.path() == "/"
                            && origin.query().is_none()
                            && origin.fragment().is_none()
                            && (!self.require_origin_match
                                || self.bootstrap.origin() == origin.origin()),
                        "invalid Cloud title backend origin"
                    );
                    origin.origin()
                }
                None => return Err(eyre!("Cloud title backend origin is unavailable")),
            };
            let path = Url::parse(codex_model_provider::CHATGPT_CODEX_BASE_URL)?;
            let base_url = format!("{}{}", origin.ascii_serialization(), path.path());
            let routing_header = match account.account_routing_override.as_deref() {
                Some("NO_CONSTRAINT") => None,
                Some("us") => Some("us"),
                Some("us_cr") => Some("us_cr"),
                _ => return Err(eyre!("invalid Cloud title routing override")),
            };
            let mut provider = codex_api::Provider {
                name: "OpenAI".to_string(),
                base_url,
                query_params: None,
                headers: Default::default(),
                retry: codex_api::RetryConfig {
                    max_attempts: 0,
                    base_delay: Duration::ZERO,
                    retry_429: false,
                    retry_5xx: false,
                    retry_transport: false,
                },
                stream_idle_timeout: Duration::from_secs(/*secs*/ 30),
            };
            if let Some(value) = routing_header {
                provider.headers.insert(ACCOUNT_ROUTING_HEADER, value.parse()?);
            }
            provider.headers.insert("user-agent", get_codex_user_agent().parse()?);
            add_originator_header(&mut provider.headers, &originator().value);
            let client = RouteAwareClientPool::with_chatgpt_cloudflare_cookies_without_redirects_or_request_logging(
                self.factory.clone(),
                ClientRouteClass::Api,
            ).into_client();
            let client =
                ResponsesClient::new(ReqwestTransport::from_http_client(client), provider, self.auth.clone());
            let mut body = json!({
                "model": model,
                "instructions": "Generate a thread title from the user prompt.",
                "input": [{"type": "message", "role": "user", "content": [{"type": "input_text", "text": prompt}]}],
                "store": false,
                "stream": true,
                "tools": [],
                "tool_choice": "none",
                "parallel_tool_calls": false,
                "include": [],
                "client_metadata": {"x-codex-turn-metadata": r#"{"thread_source":"thread_title"}"#},
                "text": create_text_param_for_request(/*verbosity*/ None, &Some(schema), /*output_schema_strict*/ true),
            });
            if let Some(effort) = effort {
                body["reasoning"] = json!({"effort": effort});
            }
            let mut stream = client
                .stream(body, Default::default(), Compression::None, /*turn_state*/ None)
                .await
                .map_err(|_| eyre!("Cloud title request failed"))?;
            let mut title = None;
            while let Some(event) = stream.next().await {
                match event.map_err(|_| eyre!("Cloud title stream failed"))? {
                    ResponseEvent::OutputItemDone(ResponseItem::Message {
                        role,
                        content,
                        phase,
                        ..
                    }) if role == "assistant" && phase != Some(MessagePhase::Commentary) => {
                        let mut text = String::new();
                        for item in content {
                            if let ContentItem::OutputText { text: part, .. } = item {
                                ensure!(text.len() + part.len() <= 8 * 1024, "Cloud title is too large");
                                text.push_str(&part);
                            }
                        }
                        title = Some(text);
                    }
                    ResponseEvent::Completed { .. } => {
                        return title.ok_or_else(|| eyre!("Cloud title response is empty"));
                    }
                    _ => {}
                }
            }
            bail!("Cloud title stream ended before completion");
        })
        .await?
    }
}

#[cfg(test)]
#[path = "thread_title_tests.rs"]
mod tests;
