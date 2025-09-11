use chrono::SecondsFormat;
use chrono::Utc;
use codex_protocol::mcp_protocol::AuthMode;
use codex_protocol::mcp_protocol::ConversationId;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::InputItem;
use opentelemetry::propagation::TextMapPropagator;
use opentelemetry_http::HeaderInjector;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use reqwest::StatusCode;
use reqwest::header::HeaderMap;
use serde::Serialize;
use tracing::Span;
use tracing::info_span;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use strum_macros::Display;

pub struct RequestSpan(pub(crate) Span);

impl RequestSpan {
    pub fn new(metadata: TraceMetadata) -> Self {
        let span = info_span!(
            "codex.api_request",
            conversation.id = %metadata.conversation_id,
            app.version = %metadata.app_version,
            auth_mode = %metadata.auth_mode,
            user.account_id = tracing::field::Empty,
            terminal.type = %metadata.terminal_type,
            event.timestamp = %timestamp(),
            model = %metadata.model,
            slug = %metadata.slug,
            prompt = tracing::field::Empty,
            http.response.status_code = tracing::field::Empty,
            error.message = tracing::field::Empty,
            attempt = tracing::field::Empty,
            input_tokens = tracing::field::Empty,
            output_tokens = tracing::field::Empty,
            cache_read_tokens = tracing::field::Empty,
            cache_creation_tokens = tracing::field::Empty,
            request_id = tracing::field::Empty,
        );

        if let Some(account_id) = &metadata.account_id {
            span.record("account_id", account_id);
        }
        // Todo include prompt conditionally

        Self(span)
    }

    pub fn request_id(&self, request_id: &str) -> &Self {
        self.0.record("request_id", request_id);
        self
    }

    pub fn status_code(&self, status: StatusCode) -> &Self {
        self.0.record("http.response.status_code", status.as_str());
        self
    }

    pub fn error(&self, attempt: u64, status: Option<StatusCode>, message: &str) -> &Self {
        self.0.record("attempt", attempt);
        self.0.record("error.message", message);
        if let Some(code) = status {
            self.0.record("http.response.status_code", code.as_u16());
        }
        self
    }

    pub fn span(&self) -> Span {
        self.0.clone()
    }
}

pub struct SSESpan(pub(crate) Span);

impl SSESpan {
    pub fn new(metadata: TraceMetadata) -> Self {
        let span = info_span!(
            "codex.sse_event",
            conversation.id = %metadata.conversation_id,
            app.version = %metadata.app_version,
            auth_mode = %metadata.auth_mode,
            user.account_id = tracing::field::Empty,
            terminal.type = %metadata.terminal_type,
            event.timestamp = %timestamp(),
            model = %metadata.model,
            slug = %metadata.slug,
            error.message = tracing::field::Empty,
            input_token_count = tracing::field::Empty,
            output_token_count = tracing::field::Empty,
            cached_content_token_count = tracing::field::Empty,
            thoughts_token_count = tracing::field::Empty,
            reasoning_token_count = tracing::field::Empty,
            tool_token_count = tracing::field::Empty,
            response_body = tracing::field::Empty,
        );

        if let Some(account_id) = &metadata.account_id {
            span.record("account_id", account_id);
        }

        SSESpan(span)
    }

    pub fn body(&self, body: &str) -> &Self {
        self.0.record("response_body", body);
        self
    }

    pub fn token_usage(
        &self,
        input_token_count: u64,
        output_token_count: u64,
        cached_token_count: Option<u64>,
        reasoning_token_count: Option<u64>,
        tool_token_count: u64,
    ) -> &Self {
        self.0.record("input_token_count", input_token_count);
        self.0.record("output_token_count", output_token_count);
        self.0.record("cached_token_count", cached_token_count);
        self.0
            .record("reasoning_token_count", reasoning_token_count);
        self.0.record("tool_token_count", tool_token_count);
        self
    }

    pub fn error(&self, error: &str) -> &Self {
        self.0.record("error.message", error);
        self
    }

    pub fn span(&self) -> Span {
        self.0.clone()
    }
}

#[derive(Debug, Clone, Serialize, Display)]
#[serde(rename_all = "snake_case")]
pub enum ToolDecisionOutcome {
    Accept,
    Reject,
}

#[derive(Debug, Clone, Serialize, Display)]
#[serde(rename_all = "snake_case")]
pub enum ToolDecisionSource {
    Config,
    UserForSession,
    UserTemporary,
    UserAbort,
    UserReject,
}

pub struct ToolDecisionSpan(pub(crate) Span);

impl ToolDecisionSpan {
    pub fn new(
        metadata: TraceMetadata,
        tool_name: &str,
        outcome: ToolDecisionOutcome,
        source: ToolDecisionSource,
    ) -> Self {
        let span = info_span!(
            "codex.tool_decision",
            session.id = %metadata.conversation_id,
            app.version = %metadata.app_version,
            user.account_id = tracing::field::Empty,
            terminal.type = %metadata.terminal_type,
            event.timestamp = %timestamp(),
            tool_name = %tool_name,
            decision = outcome.to_string(),
            source = source.to_string(),
        );

        if let Some(account_id) = &metadata.account_id {
            span.record("user.account_id", account_id);
        }

        ToolDecisionSpan(span)
    }

    pub fn span(&self) -> Span {
        self.0.clone()
    }
}

pub struct UserPromptSpan(pub(crate) Span);

impl UserPromptSpan {
    pub fn new(metadata: TraceMetadata, prompt: &str) -> Self {
        let prompt_to_log = if metadata.log_user_prompts {
            prompt
        } else {
            "[REDACTED]"
        };

        let span = info_span!(
            "codex.user_prompt",
            session.id = %metadata.conversation_id,
            app.version = %metadata.app_version,
            user.account_id = tracing::field::Empty,
            terminal.type = %metadata.terminal_type,
            event.timestamp = %timestamp(),
            prompt_length = %prompt.chars().count(),
            prompt = %prompt_to_log,
        );

        if let Some(account_id) = &metadata.account_id {
            span.record("user.account_id", account_id);
        }

        Self(span)
    }

    pub fn span(&self) -> Span {
        self.0.clone()
    }
}

#[derive(Debug, Clone)]
pub struct TraceMetadata {
    conversation_id: ConversationId,
    auth_mode: AuthMode,
    account_id: Option<String>,
    model: String,
    slug: String,
    log_user_prompts: bool,
    app_version: &'static str,
    terminal_type: String,
}

#[derive(Debug, Clone)]
pub struct TraceManager {
    metadata: TraceMetadata,
}

impl TraceManager {
    pub fn new(
        conversation_id: ConversationId,
        model: &str,
        slug: &str,
        account_id: Option<String>,
        auth_mode: AuthMode,
        log_user_prompts: bool,
        terminal_type: String,
    ) -> TraceManager {
        Self {
            metadata: TraceMetadata {
                conversation_id,
                auth_mode,
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

    pub fn headers(span: &RequestSpan) -> HeaderMap {
        let mut injector = HeaderMap::new();
        TraceContextPropagator::default()
            .inject_context(&span.0.context(), &mut HeaderInjector(&mut injector));
        injector
    }

    pub fn request(&self, _prompt: &[ResponseItem]) -> RequestSpan {
        RequestSpan::new(self.metadata.clone())
    }

    pub fn response(&self) -> SSESpan {
        SSESpan::new(self.metadata.clone())
    }

    pub fn user_prompt(&self, items: &[InputItem]) -> UserPromptSpan {
        let prompt = items
            .iter()
            .flat_map(|item| match item {
                InputItem::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<String>();

        UserPromptSpan::new(self.metadata.clone(), prompt.as_ref())
    }

    pub fn tool_decision(&self, tool_name: &str, outcome: ToolDecisionOutcome, source: ToolDecisionSource) -> ToolDecisionSpan {
        ToolDecisionSpan::new(self.metadata.clone(), tool_name, outcome, source)
    }
}

fn timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}
