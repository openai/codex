use chrono::SecondsFormat;
use chrono::Utc;
use codex_app_server_protocol::AuthMode;
use codex_protocol::ConversationId;
use codex_protocol::config_types::ReasoningEffort;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::InputItem;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::SandboxPolicy;
use eventsource_stream::Event as StreamEvent;
use eventsource_stream::EventStreamError as StreamError;
use reqwest::Error;
use reqwest::Response;
use serde::Serialize;
use std::borrow::Cow;
use std::fmt::Display;
use std::time::Duration;
use std::time::Instant;
use strum_macros::Display;
use tokio::time::error::Elapsed;

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

    #[allow(clippy::too_many_arguments)]
    pub fn conversation_starts(
        &self,
        provider_name: &str,
        reasoning_effort: Option<ReasoningEffort>,
        reasoning_summary: ReasoningSummary,
        context_window: Option<u64>,
        max_output_tokens: Option<u64>,
        auto_compact_token_limit: Option<i64>,
        approval_policy: AskForApproval,
        sandbox_policy: SandboxPolicy,
        mcp_servers: Vec<&str>,
        active_profile: Option<String>,
    ) {
        tracing::event!(
            tracing::Level::INFO,
            event.name = "codex.conversation_starts",
            event.timestamp = %timestamp(),
            conversation.id = %self.metadata.conversation_id,
            app.version = %self.metadata.app_version,
            auth_mode = self.metadata.auth_mode,
            user.account_id = self.metadata.account_id,
            terminal.type = %self.metadata.terminal_type,
            model = %self.metadata.model,
            slug = %self.metadata.slug,
            provider_name = %provider_name,
            reasoning_effort = reasoning_effort.map(|e| e.to_string()),
            reasoning_summary = %reasoning_summary,
            context_window = context_window,
            max_output_tokens = max_output_tokens,
            auto_compact_token_limit = auto_compact_token_limit,
            approval_policy = %approval_policy,
            sandbox_policy = %sandbox_policy,
            mcp_servers = mcp_servers.join(", "),
            active_profile = active_profile,
        )
    }

    pub async fn log_request<F, Fut>(&self, attempt: u64, f: F) -> Result<Response, Error>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<Response, Error>>,
    {
        let start = std::time::Instant::now();
        let response = f().await;
        let duration = start.elapsed();

        let (status, error) = match &response {
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
            duration_ms = %duration.as_millis(),
            http.response.status_code = status,
            error.message = error,
            attempt = attempt,
        );

        response
    }

    pub async fn log_sse_event<Next, Fut, E>(
        &self,
        next: Next,
    ) -> Result<Option<Result<StreamEvent, StreamError<E>>>, Elapsed>
    where
        Next: FnOnce() -> Fut,
        Fut: Future<Output = Result<Option<Result<StreamEvent, StreamError<E>>>, Elapsed>>,
        E: Display,
    {
        let start = std::time::Instant::now();
        let response = next().await;
        let duration = start.elapsed();

        match response {
            Ok(Some(Ok(ref sse))) => {
                if sse.data.trim() == "[DONE]" {
                    self.sse_event(&sse.event, duration);
                } else {
                    match serde_json::from_str::<serde_json::Value>(&sse.data) {
                        Ok(error) if sse.event == "response.failed" => {
                            self.sse_event_failed(Some(&sse.event), duration, &error);
                        }
                        Ok(content) if sse.event == "response.output_item.done" => {
                            match serde_json::from_value::<ResponseItem>(content) {
                                Ok(_) => self.sse_event(&sse.event, duration),
                                Err(_) => {
                                    self.sse_event_failed(
                                        Some(&sse.event),
                                        duration,
                                        &"failed to parse response.output_item.done",
                                    );
                                }
                            };
                        }
                        Ok(_) => {
                            self.sse_event(&sse.event, duration);
                        }
                        Err(error) => {
                            self.sse_event_failed(Some(&sse.event), duration, &error);
                        }
                    }
                }
            }
            Ok(Some(Err(ref error))) => {
                self.sse_event_failed(None, duration, error);
            }
            Ok(None) => {}
            Err(_) => {
                self.sse_event_failed(None, duration, &"idle timeout waiting for SSE");
            }
        }

        response
    }

    fn sse_event(&self, kind: &str, duration: Duration) {
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

    pub fn sse_event_failed<T>(&self, kind: Option<&String>, duration: Duration, error: &T)
    where
        T: Display,
    {
        match kind {
            Some(kind) => tracing::event!(
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
                error.message = %error,
            ),
            None => tracing::event!(
                tracing::Level::INFO,
                event.name = "codex.sse_event",
                event.timestamp = %timestamp(),
                conversation.id = %self.metadata.conversation_id,
                app.version = %self.metadata.app_version,
                auth_mode = self.metadata.auth_mode,
                user.account_id = self.metadata.account_id,
                terminal.type = %self.metadata.terminal_type,
                model = %self.metadata.model,
                slug = %self.metadata.slug,
                duration_ms = %duration.as_millis(),
                error.message = %error,
            ),
        }
    }

    pub fn see_event_completed_failed<T>(&self, error: &T)
    where
        T: Display,
    {
        tracing::event!(
            tracing::Level::INFO,
            event.name = "codex.sse_event",
            event.kind = %"response.completed",
            event.timestamp = %timestamp(),
            conversation.id = %self.metadata.conversation_id,
            app.version = %self.metadata.app_version,
            auth_mode = self.metadata.auth_mode,
            user.account_id = self.metadata.account_id,
            terminal.type = %self.metadata.terminal_type,
            model = %self.metadata.model,
            slug = %self.metadata.slug,
            error.message = %error,
        )
    }

    pub fn sse_event_completed(
        &self,
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
            event.kind = %"response.completed",
            conversation.id = %self.metadata.conversation_id,
            app.version = %self.metadata.app_version,
            auth_mode = self.metadata.auth_mode,
            user.account_id = self.metadata.account_id,
            terminal.type = %self.metadata.terminal_type,
            model = %self.metadata.model,
            slug = %self.metadata.slug,
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
            decision = %decision.to_string().to_lowercase(),
            source = %source.to_string(),
        );
    }

    pub async fn log_tool_result<F, Fut, E>(
        &self,
        tool_name: &str,
        call_id: &str,
        arguments: &str,
        f: F,
    ) -> Result<(String, bool), E>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<(String, bool), E>>,
        E: Display,
    {
        let start = Instant::now();
        let result = f().await;
        let duration = start.elapsed();

        let (output, success) = match &result {
            Ok((preview, success)) => (Cow::Borrowed(preview.as_str()), *success),
            Err(error) => (Cow::Owned(error.to_string()), false),
        };

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
            // `output` is truncated by the tool layer before reaching telemetry.
            output = %output,
        );

        result
    }

    pub fn log_tool_failed(&self, tool_name: &str, error: &str) {
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
            duration_ms = %Duration::ZERO.as_millis(),
            success = %false,
            output = %error,
        );
    }

    pub fn tool_result(
        &self,
        tool_name: &str,
        call_id: &str,
        arguments: &str,
        duration: Duration,
        success: bool,
        output: &str,
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
