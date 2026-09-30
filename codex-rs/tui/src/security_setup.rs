//! Optional account-security reminders for the local, ChatGPT-authenticated CLI.
//! The server owns eligibility, rollout and cutoff copy. Reads never block a turn;
//! no reminder analytics are sent. Unknown or unavailable state produces no notice.

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::app_server_session::AppServerSession;
use crate::legacy_core::config::Config;
use codex_app_server_protocol::AuthMode;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::GetAuthStatusParams;
use codex_app_server_protocol::GetAuthStatusResponse;
use codex_app_server_protocol::RequestId;
use codex_http_client::ClientRouteClass;
use codex_http_client::RouteAwareClientPool;
use codex_login::CodexAuth;
use serde::Deserialize;
use std::time::Duration;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Notice {
    pub title: String,
    pub description: String,
    pub action: Action,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Action {
    pub label: String,
    pub url: String,
}

#[derive(Deserialize)]
struct Response {
    notice: Option<Notice>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Identity {
    pub account: String,
    pub user: String,
}
impl Identity {
    fn from_auth(auth: &CodexAuth) -> Option<Self> {
        Some(Self {
            account: auth.get_account_id()?,
            user: auth.get_chatgpt_user_id()?,
        })
    }
}
impl Notice {
    pub(crate) fn valid(&self) -> bool {
        !self.title.trim().is_empty()
            && self.title.len() <= 256
            && self.description.len() <= 2048
            && !self.title.chars().any(char::is_control)
            && !self.description.chars().any(char::is_control)
            && !self.action.label.trim().is_empty()
            && self.action.label.len() <= 128
            && !self.action.label.chars().any(char::is_control)
            && url::Url::parse(&self.action.url).is_ok_and(|url| {
                url.scheme() == "https"
                    && url.host_str() == Some("chatgpt.com")
                    && url.username().is_empty()
                    && url.password().is_none()
                    && url.port().is_none()
            })
    }
}

pub(crate) fn prefetch(
    config: &Config,
    server: &AppServerSession,
    tx: AppEventSender,
    request_id: Uuid,
) {
    if config.model_provider_id != "openai" || server.uses_remote_workspace() {
        return;
    }
    let config = config.clone();
    let request_handle = server.request_handle();
    tokio::spawn(async move {
        let result = tokio::time::timeout(Duration::from_secs(3), async {
            // Saved credentials must match the connected server, which may use external auth.
            let status: GetAuthStatusResponse = request_handle
                .request_typed(ClientRequest::GetAuthStatus {
                    request_id: RequestId::String(Uuid::new_v4().to_string()),
                    params: GetAuthStatusParams {
                        include_token: Some(true),
                        refresh_token: Some(false),
                    },
                })
                .await
                .ok()?;
            let auth = config
                .auth_config()
                .load_auth(/*enable_codex_api_key_env*/ false)
                .await
                .ok()
                .flatten()?;
            if !matches!(auth, CodexAuth::Chatgpt(_)) || auth.is_fedramp_account() {
                return None;
            }
            let saved_token = auth.get_token().ok()?;
            if status.auth_method != Some(AuthMode::Chatgpt)
                || status.auth_token.as_deref() != Some(saved_token.as_str())
            {
                return None;
            }
            let identity = Identity::from_auth(&auth)?;
            let client = RouteAwareClientPool::new_without_redirects(
                config.http_client_factory(),
                ClientRouteClass::Api,
            );
            let response = client
                .get(format!(
                    "{}/wham/security-setup",
                    config.chatgpt_base_url.trim_end_matches('/')
                ))
                .headers(codex_model_provider::auth_provider_from_auth(&auth).to_auth_headers())
                .header(
                    "User-Agent",
                    codex_login::default_client::get_codex_user_agent(),
                )
                .send()
                .await
                .ok()?;
            if !response.status().is_success() {
                return None;
            }
            let notice = response.json::<Response>().await.ok()?.notice?;
            tracing::debug!(valid = notice.valid(), "received security setup notice");
            if !notice.valid() {
                return None;
            }
            let current = config
                .auth_config()
                .load_auth(/*enable_codex_api_key_env*/ false)
                .await
                .ok()
                .flatten()?;
            if Identity::from_auth(&current).as_ref() != Some(&identity) {
                tracing::debug!("security setup identity changed during fetch");
                return None;
            }
            Some((identity, notice))
        })
        .await
        .ok()
        .flatten();
        if let Some((identity, notice)) = result {
            tracing::debug!("security setup notice ready");
            tx.send(AppEvent::SecuritySetupLoaded {
                request_id,
                identity,
                notice,
            });
        }
    });
}

#[cfg(test)]
#[path = "security_setup_tests.rs"]
mod tests;
