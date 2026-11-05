//! Account details shared by startup and fresh threads, resolved by the connected server.

use super::*;
use crate::config_update::EffectiveConfig;
use codex_app_server_protocol::ConfigReadParams;
use codex_model_provider_info::OPENAI_PROVIDER_ID;
use codex_model_provider_info::built_in_model_providers;

#[derive(Default)]
pub(crate) struct SessionAccount {
    pub(crate) display: Option<StatusAccountDisplay>,
    pub(crate) plan: Option<codex_protocol::account::PlanType>,
    pub(crate) requires_openai_auth: bool,
}

impl From<GetAccountResponse> for SessionAccount {
    fn from(response: GetAccountResponse) -> Self {
        let mut account = Self {
            requires_openai_auth: response.requires_openai_auth,
            ..Self::default()
        };
        account.display = match response.account {
            Some(Account::Chatgpt {
                email,
                plan_type,
                workspace_name,
            }) => {
                account.plan = Some(plan_type);
                Some(StatusAccountDisplay::ChatGpt {
                    email,
                    plan: Some(plan_type_display_name(plan_type)),
                    workspace: workspace_display(workspace_name, response.workspace_routing),
                })
            }
            Some(Account::ApiKey {}) => Some(StatusAccountDisplay::ApiKey),
            Some(Account::AmazonBedrock { .. } | Account::Gcp {}) | None => None,
        };
        account
    }
}

impl SessionAccount {
    pub(crate) fn has_chatgpt_account(&self) -> bool {
        matches!(self.display, Some(StatusAccountDisplay::ChatGpt { .. }))
    }

    pub(super) fn auth_mode(&self) -> Option<TelemetryAuthMode> {
        match self.display {
            Some(StatusAccountDisplay::ChatGpt { .. }) => Some(TelemetryAuthMode::Chatgpt),
            Some(StatusAccountDisplay::ApiKey) => Some(TelemetryAuthMode::ApiKey),
            None => None,
        }
    }

    pub(super) fn email(&self) -> Option<&str> {
        match &self.display {
            Some(StatusAccountDisplay::ChatGpt { email, .. }) => email.as_deref(),
            Some(StatusAccountDisplay::ApiKey) | None => None,
        }
    }

    pub(crate) fn feedback_audience(&self) -> FeedbackAudience {
        if self
            .email()
            .is_some_and(|email| email.ends_with("@openai.com"))
        {
            FeedbackAudience::OpenAiEmployee
        } else {
            FeedbackAudience::External
        }
    }
}

impl AppServerSession {
    pub(crate) async fn read_thread_account(
        &mut self,
        session: &ThreadSessionState,
    ) -> Result<SessionAccount> {
        let response = self.read_account().await?;
        // account/read resolves the server's non-project configuration. A thread can override it.
        let request_id = self.next_request_id();
        let defaults: EffectiveConfig = self
            .request_handle
            .request_typed(ClientRequest::ConfigRead {
                request_id,
                params: ConfigReadParams {
                    include_layers: false,
                    cwd: None,
                },
            })
            .await?;
        let config = defaults.config;
        let provider = config
            .model_provider
            .as_deref()
            .unwrap_or(OPENAI_PROVIDER_ID);
        // The default can change between the two reads. Only apply compatible capabilities.
        let auth_mismatch = built_in_model_providers(/*openai_base_url*/ None)
            .get(provider)
            .is_some_and(|info| info.requires_openai_auth != response.requires_openai_auth);
        if provider != session.model_provider_id || auth_mismatch {
            color_eyre::eyre::bail!(
                "the server's account details do not match this chat's provider"
            );
        }
        Ok(response.into())
    }
}
