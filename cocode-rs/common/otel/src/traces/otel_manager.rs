use crate::otel_provider::traceparent_context_from_env;
use chrono::SecondsFormat;
use chrono::Utc;
use eventsource_stream::Event as StreamEvent;
use eventsource_stream::EventStreamError as StreamError;
use reqwest::Error;
use reqwest::Response;
use std::borrow::Cow;
use std::fmt::Display;
use std::future::Future;
use std::time::Duration;
use std::time::Instant;
use tokio::time::error::Elapsed;
use tracing::Span;
use tracing::trace_span;
use tracing_opentelemetry::OpenTelemetrySpanExt;

pub use crate::OtelEventMetadata;
pub use crate::OtelManager;
pub use crate::ToolDecisionSource;

impl OtelManager {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        conversation_id: impl Into<String>,
        model: &str,
        slug: &str,
        account_id: Option<String>,
        account_email: Option<String>,
        auth_mode: Option<&str>,
        log_user_prompts: bool,
        terminal_type: String,
        session_source: &str,
    ) -> OtelManager {
        let conversation_id = conversation_id.into();
        let session_span = trace_span!("new_session", conversation_id = %conversation_id, session_source = %session_source);

        if let Some(context) = traceparent_context_from_env() {
            let _ = session_span.set_parent(context);
        }

        Self {
            metadata: OtelEventMetadata {
                conversation_id,
                auth_mode: auth_mode.map(|m| m.to_string()),
                account_id,
                account_email,
                model: model.to_owned(),
                slug: slug.to_owned(),
                log_user_prompts,
                app_version: env!("CARGO_PKG_VERSION"),
                terminal_type,
            },
            session_span,
            metrics: crate::metrics::global(),
            metrics_use_metadata_tags: true,
        }
    }

    pub fn current_span(&self) -> &Span {
        &self.session_span
    }

    /// Record a response event with type name and optional metadata.
    pub fn record_response_event(
        &self,
        handle_responses_span: &Span,
        event_type: &str,
        from: Option<&str>,
        tool_name: Option<&str>,
    ) {
        handle_responses_span.record("otel.name", event_type);
        if let Some(from) = from {
            handle_responses_span.record("from", from);
        }
        if let Some(tool_name) = tool_name {
            handle_responses_span.record("tool_name", tool_name);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn conversation_starts(
        &self,
        provider_name: &str,
        reasoning_effort: Option<&str>,
        reasoning_summary: &str,
        context_window: Option<i64>,
        approval_policy: &str,
        sandbox_policy: &str,
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
            user.email = self.metadata.account_email,
            terminal.type = %self.metadata.terminal_type,
            model = %self.metadata.model,
            slug = %self.metadata.slug,
            provider_name = %provider_name,
            reasoning_effort = reasoning_effort,
            reasoning_summary = %reasoning_summary,
            context_window = context_window,
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
        let start = Instant::now();
        let response = f().await;
        let duration = start.elapsed();

        let (status, error) = match &response {
            Ok(response) => (Some(response.status().as_u16()), None),
            Err(error) => (error.status().map(|s| s.as_u16()), Some(error.to_string())),
        };
        self.record_api_request(attempt, status, error.as_deref(), duration);

        response
    }

    pub fn record_api_request(
        &self,
        attempt: u64,
        status: Option<u16>,
        error: Option<&str>,
        duration: Duration,
    ) {
        tracing::event!(
            tracing::Level::INFO,
            event.name = "codex.api_request",
            event.timestamp = %timestamp(),
            conversation.id = %self.metadata.conversation_id,
            app.version = %self.metadata.app_version,
            auth_mode = self.metadata.auth_mode,
            user.account_id = self.metadata.account_id,
            user.email = self.metadata.account_email,
            terminal.type = %self.metadata.terminal_type,
            model = %self.metadata.model,
            slug = %self.metadata.slug,
            duration_ms = %duration.as_millis(),
            http.response.status_code = status,
            error.message = error,
            attempt = attempt,
        );
    }

    /// Log SSE event with optional validation callback for parsing response data.
    pub fn log_sse_event_simple<E>(
        &self,
        response: &Result<Option<Result<StreamEvent, StreamError<E>>>, Elapsed>,
        duration: Duration,
    ) where
        E: Display,
    {
        match response {
            Ok(Some(Ok(sse))) => {
                if sse.data.trim() == "[DONE]" {
                    self.sse_event(&sse.event, duration);
                } else {
                    match serde_json::from_str::<serde_json::Value>(&sse.data) {
                        Ok(error) if sse.event == "response.failed" => {
                            self.sse_event_failed(Some(&sse.event), duration, &error);
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
            Ok(Some(Err(error))) => {
                self.sse_event_failed(None, duration, error);
            }
            Ok(None) => {}
            Err(_) => {
                self.sse_event_failed(None, duration, &"idle timeout waiting for SSE");
            }
        }
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
            user.email = self.metadata.account_email,
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
                user.email = self.metadata.account_email,
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
                user.email = self.metadata.account_email,
                terminal.type = %self.metadata.terminal_type,
                model = %self.metadata.model,
                slug = %self.metadata.slug,
                duration_ms = %duration.as_millis(),
                error.message = %error,
            ),
        }
    }

    pub fn sse_event_completed_failed<T>(&self, error: &T)
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
            user.email = self.metadata.account_email,
            terminal.type = %self.metadata.terminal_type,
            model = %self.metadata.model,
            slug = %self.metadata.slug,
            error.message = %error,
        )
    }

    pub fn sse_event_completed(
        &self,
        input_token_count: i64,
        output_token_count: i64,
        cached_token_count: Option<i64>,
        reasoning_token_count: Option<i64>,
        tool_token_count: i64,
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
            user.email = self.metadata.account_email,
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

    /// Log user prompt with content and length.
    pub fn user_prompt(&self, prompt_text: &str, prompt_length: usize) {
        let prompt_to_log = if self.metadata.log_user_prompts {
            prompt_text
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
            user.email = self.metadata.account_email,
            terminal.type = %self.metadata.terminal_type,
            model = %self.metadata.model,
            slug = %self.metadata.slug,
            prompt_length = %prompt_length,
            prompt = %prompt_to_log,
        );
    }

    /// Log tool decision with decision as string.
    pub fn tool_decision(
        &self,
        tool_name: &str,
        call_id: &str,
        decision: &str,
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
            user.email = self.metadata.account_email,
            terminal.type = %self.metadata.terminal_type,
            model = %self.metadata.model,
            slug = %self.metadata.slug,
            tool_name = %tool_name,
            call_id = %call_id,
            decision = %decision,
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

        self.tool_result(
            tool_name,
            call_id,
            arguments,
            duration,
            success,
            output.as_ref(),
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
            user.email = self.metadata.account_email,
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
        self.counter(
            "codex.tool.call",
            1,
            &[("tool", tool_name), ("success", success_str)],
        );
        tracing::event!(
            tracing::Level::INFO,
            event.name = "codex.tool_result",
            event.timestamp = %timestamp(),
            conversation.id = %self.metadata.conversation_id,
            app.version = %self.metadata.app_version,
            auth_mode = self.metadata.auth_mode,
            user.account_id = self.metadata.account_id,
            user.email = self.metadata.account_email,
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
