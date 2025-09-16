use chrono::SecondsFormat;
use chrono::Utc;
use codex_protocol::mcp_protocol::AuthMode;
use codex_protocol::mcp_protocol::ConversationId;
use codex_protocol::protocol::InputItem;
use codex_protocol::protocol::ReviewDecision;
use reqwest::Error;
use reqwest::Response;
use serde::Serialize;
use std::time::Duration;
use strum_macros::Display;

#[derive(Debug, Clone, Serialize, Display)]
#[serde(rename_all = "snake_case")]
pub enum ToolDecisionSource {
    Config,
    User,
}

#[derive(Debug, Clone)]
pub struct OtelEventMetadata {
    conversation_id: ConversationId,
    auth_mode: Option<String>,
    account_id: Option<String>,
    model: String,
    slug: String,
    log_user_prompts: bool,
    app_version: &'static str,
    terminal_type: String,
}

#[derive(Debug, Clone)]
pub struct OtelEventManager {
    metadata: OtelEventMetadata,
}

impl OtelEventManager {
    pub fn new(
        conversation_id: ConversationId,
        model: &str,
        slug: &str,
        account_id: Option<String>,
        auth_mode: Option<AuthMode>,
        log_user_prompts: bool,
        terminal_type: String,
    ) -> OtelEventManager {
        Self {
            metadata: OtelEventMetadata {
                conversation_id,
                auth_mode: auth_mode.map(|m| m.to_string()),
                account_id,
                model: model.to_owned(),
                slug: slug.to_owned(),
                log_user_prompts,
                app_version: env!("CARGO_PKG_VERSION"),
                terminal_type,
            },
        }
    }

    pub fn with_model(&self, model: &str, slug: &str) -> Self {
        let mut manager = self.clone();
        manager.metadata.model = model.to_owned();
        manager.metadata.slug = slug.to_owned();
        manager
    }

    pub fn request(
        &self,
        cf_ray: Option<String>,
        attempt: u64,
        duration: Duration,
        response: &Result<Response, Error>,
    ) {
        let (status, error) = match response {
            Ok(response) => (Some(response.status().as_u16()), None),
            Err(error) => (error.status().map(|s| s.as_u16()), Some(error.to_string())),
        };

        tracing::event!(
            tracing::Level::INFO,
            event.name = "codex.api_request",
            event.timestamp = %timestamp(),
            conversation.id = %self.metadata.conversation_id,
            app.version = %self.metadata.app_version,
            auth_mode = self.metadata.auth_mode,
            user.account_id = self.metadata.account_id,
            terminal.type = %self.metadata.terminal_type,
            model = %self.metadata.model,
            slug = %self.metadata.slug,
            cf_ray = cf_ray,
            duration_ms = %duration.as_millis(),
            http.response.status_code = status,
            error.message = error,
            attempt = attempt,
        );
    }

    pub fn sse_event(&self, kind: String, duration: Duration) {
        tracing::event!(
            tracing::Level::INFO,
            event.name = "codex.sse_event",
            event.timestamp = %timestamp(),
            event.kind = %kind,
            conversation.id = %self.metadata.conversation_id,
            app.version = %self.metadata.app_version,
            auth_mode = self.metadata.auth_mode,
            user.account_id = self.metadata.account_id,
            terminal.type = %self.metadata.terminal_type,
            model = %self.metadata.model,
            slug = %self.metadata.slug,
            duration_ms = %duration.as_millis(),
        );
    }

    pub fn sse_event_failed(&self, kind: Option<String>, duration: Duration, error: &str) {
        tracing::event!(
            tracing::Level::INFO,
            event.name = "codex.sse_event",
            event.timestamp = %timestamp(),
            event.kind = kind,
            conversation.id = %self.metadata.conversation_id,
            app.version = %self.metadata.app_version,
            auth_mode = self.metadata.auth_mode,
            user.account_id = self.metadata.account_id,
            terminal.type = %self.metadata.terminal_type,
            model = %self.metadata.model,
            slug = %self.metadata.slug,
            duration_ms = %duration.as_millis(),
            error.message = %error,
        );
    }

    pub fn sse_event_completed(
        &self,
        duration: Duration,
        input_token_count: u64,
        output_token_count: u64,
        cached_token_count: Option<u64>,
        reasoning_token_count: Option<u64>,
        tool_token_count: u64,
    ) {
        tracing::event!(
            tracing::Level::INFO,
            event.name = "codex.sse_event",
            event.timestamp = %timestamp(),
            event.kind = "response.completed",
            conversation.id = %self.metadata.conversation_id,
            app.version = %self.metadata.app_version,
            auth_mode = self.metadata.auth_mode,
            user.account_id = self.metadata.account_id,
            terminal.type = %self.metadata.terminal_type,
            model = %self.metadata.model,
            slug = %self.metadata.slug,
            duration_ms = %duration.as_millis(),
            input_token_count = %input_token_count,
            output_token_count = %output_token_count,
            cached_token_count = cached_token_count,
            reasoning_token_count = reasoning_token_count,
            tool_token_count = %tool_token_count,
        );
    }

    pub fn user_prompt(&self, items: &[InputItem]) {
        let prompt = items
            .iter()
            .flat_map(|item| match item {
                InputItem::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<String>();

        let prompt_to_log = if self.metadata.log_user_prompts {
            prompt.as_str()
        } else {
            "[REDACTED]"
        };

        tracing::event!(
            tracing::Level::INFO,
            event.name = "codex.user_prompt",
            event.timestamp = %timestamp(),
            conversation.id = %self.metadata.conversation_id,
            app.version = %self.metadata.app_version,
            auth_mode = self.metadata.auth_mode,
            user.account_id = self.metadata.account_id,
            terminal.type = %self.metadata.terminal_type,
            model = %self.metadata.model,
            slug = %self.metadata.slug,
            prompt_length = %prompt.chars().count(),
            prompt = %prompt_to_log,
        );
    }

    pub fn tool_decision(
        &self,
        tool_name: &str,
        call_id: &str,
        decision: ReviewDecision,
        source: ToolDecisionSource,
    ) {
        tracing::event!(
            tracing::Level::INFO,
            event.name = "codex.tool_decision",
            event.timestamp = %timestamp(),
            conversation.id = %self.metadata.conversation_id,
            app.version = %self.metadata.app_version,
            auth_mode = self.metadata.auth_mode,
            user.account_id = self.metadata.account_id,
            terminal.type = %self.metadata.terminal_type,
            model = %self.metadata.model,
            slug = %self.metadata.slug,
            tool_name = %tool_name,
            call_id = %call_id,
            decision = decision.to_string().to_lowercase(),
            source = source.to_string(),
        );
    }

    pub fn tool_result(
        &self,
        tool_name: &str,
        call_id: &str,
        arguments: &str,
        duration: Duration,
        success: bool,
        output: String,
    ) {
        let success_str = if success { "true" } else { "false" };

        tracing::event!(
            tracing::Level::INFO,
            event.name = "codex.tool_result",
            event.timestamp = %timestamp(),
            conversation.id = %self.metadata.conversation_id,
            app.version = %self.metadata.app_version,
            auth_mode = self.metadata.auth_mode,
            user.account_id = self.metadata.account_id,
            terminal.type = %self.metadata.terminal_type,
            model = %self.metadata.model,
            slug = %self.metadata.slug,
            tool_name = %tool_name,
            call_id = %call_id,
            arguments = %arguments,
            duration_ms = %duration.as_millis(),
            success = %success_str,
            output = %output,
        );
    }
}

fn timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}
