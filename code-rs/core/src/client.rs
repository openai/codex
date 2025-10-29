use std::collections::BTreeMap;
use std::io::BufRead;
use std::path::Path;
// use std::sync::OnceLock;
use std::time::Duration;

use crate::AuthManager;
use bytes::Bytes;
use code_app_server_protocol::AuthMode;
use code_protocol::models::ResponseItem;
use eventsource_stream::Eventsource;
use futures::prelude::*;
// use regex_lite::Regex;
use reqwest::StatusCode;
use reqwest::header::HeaderMap;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_util::io::ReaderStream;
use tracing::debug;
use tracing::trace;
use tracing::warn;
use uuid::Uuid;

use crate::agent_defaults::{default_agent_configs, enabled_agent_model_specs};
use crate::chat_completions::AggregateStreamExt;
use crate::chat_completions::stream_chat_completions;
use crate::client_common::Prompt;
use crate::client_common::ResponseEvent;
use crate::client_common::ResponseStream;
use crate::client_common::ResponsesApiRequest;
use crate::client_common::create_reasoning_param_for_request;
use crate::config::Config;
use crate::config_types::ReasoningEffort as ReasoningEffortConfig;
use crate::config_types::ReasoningSummary as ReasoningSummaryConfig;
use crate::config_types::TextVerbosity as TextVerbosityConfig;
use crate::debug_logger::DebugLogger;
use crate::default_client::create_client;
use crate::error::CodexErr;
use crate::error::Result;
use crate::error::RetryLimitReachedError;
use crate::error::UnexpectedResponseError;
use crate::error::UsageLimitReachedError;
use crate::flags::CODEX_RS_SSE_FIXTURE;
use crate::model_family::ModelFamily;
use crate::model_provider_info::ModelProviderInfo;
use crate::model_provider_info::WireApi;
use crate::openai_model_info::get_model_info;
use crate::openai_tools::create_tools_json_for_responses_api;
use crate::openai_tools::ConfigShellToolType;
use crate::openai_tools::ToolsConfig;
use crate::protocol::RateLimitSnapshotEvent;
use crate::protocol::SandboxPolicy;
use crate::protocol::TokenUsage;
use crate::slash_commands::get_enabled_agents;
use crate::util::backoff;
use code_otel::otel_event_manager::OtelEventManager;
use std::sync::Arc;
use std::sync::Mutex;

const RESPONSES_BETA_HEADER_V1: &str = "responses=v1";
const RESPONSES_BETA_HEADER_EXPERIMENTAL: &str = "responses=experimental";

#[derive(Debug, Deserialize)]
struct ErrorResponse {
    error: Error,
}

#[derive(Debug, Deserialize)]
struct Error {
    r#type: Option<String>,
    #[allow(dead_code)]
    code: Option<String>,
    message: Option<String>,

    // Optional fields available on "usage_limit_reached" and "usage_not_included" errors
    plan_type: Option<String>,
    resets_in_seconds: Option<u64>,
}

fn try_parse_retry_after(err: &Error) -> Option<std::time::Duration> {
    if let Some(seconds) = err.resets_in_seconds {
        return Some(std::time::Duration::from_secs(seconds));
    }

    let message = err.message.as_deref()?;
    let needle = "Please try again in ";
    let start = message.find(needle)? + needle.len();
    let rest = &message[start..];

    let mut value = String::new();
    let mut unit = String::new();
    for ch in rest.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            if unit.is_empty() {
                value.push(ch);
                continue;
            }
            break;
        } else if ch.is_ascii_alphabetic() {
            unit.push(ch);
        } else if !value.is_empty() && !unit.is_empty() {
            break;
        } else if !value.is_empty() {
            break;
        }
    }

    if value.is_empty() {
        return None;
    }

    match unit.as_str() {
        "ms" => value
            .parse::<f64>()
            .ok()
            .map(|ms| std::time::Duration::from_millis(ms as u64)),
        "s" | "sec" | "secs" | "seconds" => value.parse::<f64>().ok().map(std::time::Duration::from_secs_f64),
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub struct ModelClient {
    config: Arc<Config>,
    auth_manager: Option<Arc<AuthManager>>,
    otel_event_manager: Option<OtelEventManager>,
    client: reqwest::Client,
    provider: ModelProviderInfo,
    session_id: Uuid,
    effort: ReasoningEffortConfig,
    summary: ReasoningSummaryConfig,
    verbosity: TextVerbosityConfig,
    debug_logger: Arc<Mutex<DebugLogger>>,
}

impl ModelClient {
    pub fn new(
        config: Arc<Config>,
        auth_manager: Option<Arc<AuthManager>>,
        otel_event_manager: Option<OtelEventManager>,
        provider: ModelProviderInfo,
        effort: ReasoningEffortConfig,
        summary: ReasoningSummaryConfig,
        verbosity: TextVerbosityConfig,
        session_id: Uuid,
        debug_logger: Arc<Mutex<DebugLogger>>,
    ) -> Self {
        let client = create_client(&config.responses_originator_header);

        Self {
            config,
            auth_manager,
            otel_event_manager,
            client,
            provider,
            session_id,
            effort,
            summary,
            verbosity,
            debug_logger,
        }
    }

    /// Get the reasoning effort configuration
    pub fn get_reasoning_effort(&self) -> ReasoningEffortConfig {
        self.effort
    }

    /// Get the reasoning summary configuration
    pub fn get_reasoning_summary(&self) -> ReasoningSummaryConfig {
        self.summary
    }

    /// Get the text verbosity configuration
    #[allow(dead_code)]
    pub fn get_text_verbosity(&self) -> TextVerbosityConfig {
        self.verbosity
    }

    pub fn get_otel_event_manager(&self) -> Option<OtelEventManager> {
        self.otel_event_manager.clone()
    }

    pub fn code_home(&self) -> &Path {
        &self.config.code_home
    }

    pub fn build_tools_config_with_sandbox(
        &self,
        sandbox_policy: SandboxPolicy,
    ) -> ToolsConfig {
        let mut tools_config = ToolsConfig::new(
            &self.config.model_family,
            self.config.approval_policy,
            sandbox_policy.clone(),
            self.config.include_plan_tool,
            self.config.include_apply_patch_tool,
            self.config.tools_web_search_request,
            self.config.use_experimental_streamable_shell_tool,
            self.config.include_view_image_tool,
        );
        tools_config.web_search_allowed_domains = self.config.tools_web_search_allowed_domains.clone();

        let mut agent_models: Vec<String> = if self.config.agents.is_empty() {
            default_agent_configs()
                .into_iter()
                .filter(|cfg| cfg.enabled)
                .map(|cfg| cfg.name)
                .collect()
        } else {
            get_enabled_agents(&self.config.agents)
        };
        if agent_models.is_empty() {
            agent_models = enabled_agent_model_specs()
                .into_iter()
                .map(|spec| spec.slug.to_string())
                .collect();
        }
        agent_models.sort_by(|a, b| a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase()));
        agent_models.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        tools_config.set_agent_models(agent_models);

        let base_shell_type = tools_config.shell_type.clone();
        let base_uses_native_shell = matches!(
            &base_shell_type,
            ConfigShellToolType::LocalShell | ConfigShellToolType::StreamableShell
        );

        tools_config.shell_type = match sandbox_policy.clone() {
            SandboxPolicy::ReadOnly => {
                if base_uses_native_shell {
                    base_shell_type.clone()
                } else {
                    ConfigShellToolType::ShellWithRequest {
                        sandbox_policy: SandboxPolicy::ReadOnly,
                    }
                }
            }
            sp @ SandboxPolicy::WorkspaceWrite { .. } => {
                if base_uses_native_shell {
                    base_shell_type.clone()
                } else {
                    ConfigShellToolType::ShellWithRequest { sandbox_policy: sp }
                }
            }
            SandboxPolicy::DangerFullAccess => base_shell_type,
        };

        tools_config
    }

    pub fn build_tools_config(&self) -> ToolsConfig {
        self.build_tools_config_with_sandbox(self.config.sandbox_policy.clone())
    }

    pub fn get_auto_compact_token_limit(&self) -> Option<i64> {
        self.config.model_auto_compact_token_limit.or_else(|| {
            get_model_info(&self.config.model_family).and_then(|info| info.auto_compact_token_limit)
        })
    }

    pub fn default_model_slug(&self) -> &str {
        self.config.model.as_str()
    }

    pub fn default_model_family(&self) -> &ModelFamily {
        &self.config.model_family
    }

    /// Dispatches to either the Responses or Chat implementation depending on
    /// the provider config.  Public callers always invoke `stream()` – the
    /// specialised helpers are private to avoid accidental misuse.
    pub async fn stream(&self, prompt: &Prompt) -> Result<ResponseStream> {
        let log_tag = prompt.log_tag.as_deref();
        match self.provider.wire_api {
            WireApi::Responses => self.stream_responses(prompt, log_tag).await,
            WireApi::Chat => {
                let effective_family = prompt
                    .model_family_override
                    .as_ref()
                    .unwrap_or(&self.config.model_family);
                let model_slug = prompt
                    .model_override
                    .as_deref()
                    .unwrap_or(self.config.model.as_str());
                // Create the raw streaming connection first.
                let response_stream = stream_chat_completions(
                    prompt,
                    effective_family,
                    model_slug,
                    &self.client,
                    &self.provider,
                    &self.debug_logger,
                    self.auth_manager.clone(),
                    self.otel_event_manager.clone(),
                    log_tag,
                )
                .await?;

                // Wrap it with the aggregation adapter so callers see *only*
                // the final assistant message per turn (matching the
                // behaviour of the Responses API).
                let mut aggregated = if self.config.show_raw_agent_reasoning {
                    crate::chat_completions::AggregatedChatStream::streaming_mode(response_stream)
                } else {
                    response_stream.aggregate()
                };

                // Bridge the aggregated stream back into a standard
                // `ResponseStream` by forwarding events through a channel.
                let (tx, rx) = mpsc::channel::<Result<ResponseEvent>>(16);

                tokio::spawn(async move {
                    use futures::StreamExt;
                    while let Some(ev) = aggregated.next().await {
                        // Exit early if receiver hung up.
                        if tx.send(ev).await.is_err() {
                            break;
                        }
                    }
                });

                Ok(ResponseStream { rx_event: rx })
            }
        }
    }

    /// Implementation for the OpenAI *Responses* experimental API.
    async fn stream_responses(&self, prompt: &Prompt, log_tag: Option<&str>) -> Result<ResponseStream> {
        if let Some(path) = &*CODEX_RS_SSE_FIXTURE {
            // short circuit for tests
            warn!(path, "Streaming from fixture");
            return stream_from_fixture(path, self.provider.clone(), self.otel_event_manager.clone())
                .await;
        }

        let auth_manager = self.auth_manager.clone();

        let auth_mode = auth_manager
            .as_ref()
            .and_then(|m| m.auth())
            .as_ref()
            .map(|a| a.mode);

        // Use non-stored turns on all paths for stability.
        let store = false;

        let full_instructions = prompt.get_full_instructions(&self.config.model_family);
        let mut tools_json = create_tools_json_for_responses_api(&prompt.tools)?;
        if matches!(self.effort, ReasoningEffortConfig::Minimal) {
            tools_json.retain(|tool| {
                tool.get("type")
                    .and_then(|value| value.as_str())
                    .map(|tool_type| tool_type != "web_search")
                    .unwrap_or(true)
            });
        }

        let reasoning = create_reasoning_param_for_request(
            &self.config.model_family,
            Some(self.effort),
            self.summary,
        );

        // Request encrypted COT if we are not storing responses,
        // otherwise reasoning items will be referenced by ID
        let include: Vec<String> = if !store && reasoning.is_some() {
            vec!["reasoning.encrypted_content".to_string()]
        } else {
            vec![]
        };

        let input_with_instructions = prompt.get_formatted_input();

        // Build `text` parameter with conditional verbosity and optional format.
        // - Omit entirely for ChatGPT auth unless a `text.format` or output schema is present.
        // - Only include `text.verbosity` for GPT-5 family models; warn and ignore otherwise.
        // - When a structured `format` is present, omit `verbosity` in serialization.
        let want_format = prompt.text_format.clone().or_else(|| {
            prompt.output_schema.as_ref().map(|schema| crate::client_common::TextFormat {
                r#type: "json_schema".to_string(),
                name: Some("code_output_schema".to_string()),
                strict: Some(true),
                schema: Some(schema.clone()),
            })
        });

        let verbosity = match &self.config.model_family.family {
            family if family == "gpt-5" => Some(self.config.model_text_verbosity),
            _ => None,
        };

        let text = match (auth_mode, want_format, verbosity) {
            (Some(AuthMode::ChatGPT), None, _) => None,
            (_, Some(fmt), _) => Some(crate::client_common::Text {
                verbosity: self.verbosity.into(),
                format: Some(fmt),
            }),
            (_, None, Some(_)) => Some(crate::client_common::Text {
                verbosity: self.verbosity.into(),
                format: None,
            }),
            (_, None, None) => None,
        };

        // In general, we want to explicitly send `store: false` when using the Responses API,
        // but in practice, the Azure Responses API rejects `store: false`:
        //
        // - If store = false and id is sent an error is thrown that ID is not found
        // - If store = false and id is not sent an error is thrown that ID is required
        //
        // For Azure, we send `store: true` and preserve reasoning item IDs.
        let azure_workaround = self.provider.is_azure_responses_endpoint();

        let model_slug = prompt
            .model_override
            .as_deref()
            .unwrap_or(self.config.model.as_str());

        let session_id = prompt
            .session_id_override
            .unwrap_or(self.session_id);
        let session_id_str = session_id.to_string();

        let payload = ResponsesApiRequest {
            model: &self.config.model,
            instructions: &full_instructions,
            input: &input_with_instructions,
            tools: &tools_json,
            tool_choice: "auto",
            parallel_tool_calls: true,
            reasoning,
            text,
            store: azure_workaround,
            stream: true,
            include,
            // Use a stable per-process cache key (session id). With store=false this is inert.
            prompt_cache_key: Some(session_id_str.clone()),
        };

        let mut payload_json = serde_json::to_value(&payload)?;
        if let Some(model_value) = payload_json.get_mut("model") {
            *model_value = serde_json::Value::String(model_slug.to_string());
        }
        if azure_workaround {
            attach_item_ids(&mut payload_json, &input_with_instructions);
        }
        if let Some(openrouter_cfg) = self.provider.openrouter_config() {
            if let Some(obj) = payload_json.as_object_mut() {
                if let Some(provider) = &openrouter_cfg.provider {
                    obj.insert(
                        "provider".to_string(),
                        serde_json::to_value(provider)?
                    );
                }
                if let Some(route) = &openrouter_cfg.route {
                    obj.insert("route".to_string(), route.clone());
                }
                for (key, value) in &openrouter_cfg.extra {
                    obj.entry(key.clone()).or_insert(value.clone());
                }
            }
        }
        let payload_body = serde_json::to_string(&payload_json)?;

        let mut attempt = 0;
        let max_retries = self.provider.request_max_retries();
        let mut request_id = String::new();

        // Compute endpoint with the latest available auth (may be None at this point).
        let endpoint = self
            .provider
            .get_full_url(&auth_manager.as_ref().and_then(|m| m.auth()));
        trace!(
            "POST to {}: {}",
            endpoint,
            serde_json::to_string(&payload_json)?
        );

        loop {
            attempt += 1;

            // Always fetch the latest auth in case a prior attempt refreshed the token.
            let auth = auth_manager.as_ref().and_then(|m| m.auth());

            trace!(
                "POST to {}: {}",
                self.provider.get_full_url(&auth),
                payload_body.as_str()
            );

            let mut req_builder = self
                .provider
                .create_request_builder(&self.client, &auth)
                .await?;

            let has_beta_header = req_builder
                .try_clone()
                .and_then(|builder| builder.build().ok())
                .map_or(false, |req| req.headers().contains_key("OpenAI-Beta"));

            if !has_beta_header {
                let beta_value = if self.provider.is_public_openai_responses_endpoint() {
                    RESPONSES_BETA_HEADER_V1
                } else {
                    RESPONSES_BETA_HEADER_EXPERIMENTAL
                };
                req_builder = req_builder.header("OpenAI-Beta", beta_value);
            }

            // `Codex-Task-Type` differentiates traffic for caching; default to "standard" until
            // task-specific dispatch is re-introduced.
            let codex_task_type = "standard";

            req_builder = req_builder
                // Send `conversation_id`/`session_id` so the server can hit the prompt-cache.
                .header("conversation_id", session_id_str.clone())
                .header("session_id", session_id_str.clone())
                .header(reqwest::header::ACCEPT, "text/event-stream")
                .header("Codex-Task-Type", codex_task_type)
                .json(&payload_json);

            if let Some(auth) = auth.as_ref()
                && auth.mode == AuthMode::ChatGPT
                && let Some(account_id) = auth.get_account_id()
            {
                req_builder = req_builder.header("chatgpt-account-id", account_id);
            }

            if request_id.is_empty() {
                let endpoint_for_log = self.provider.get_full_url(&auth);
                let header_snapshot = req_builder
                    .try_clone()
                    .and_then(|builder| builder.build().ok())
                    .map(|req| header_map_to_json(req.headers()));

                if let Ok(logger) = self.debug_logger.lock() {
                    request_id = logger
                        .start_request_log(
                            &endpoint_for_log,
                            &payload_json,
                            header_snapshot.as_ref(),
                            log_tag,
                        )
                        .unwrap_or_default();
                }
            }

            let res = if let Some(otel) = self.otel_event_manager.as_ref() {
                otel.log_request(attempt, || req_builder.send()).await
            } else {
                req_builder.send().await
            };
            if let Ok(resp) = &res {
                trace!(
                    "Response status: {}, request-id: {}",
                    resp.status(),
                    resp.headers()
                        .get("x-request-id")
                        .map(|v| v.to_str().unwrap_or_default())
                        .unwrap_or_default()
                );
            }

            match res {
                Ok(resp) if resp.status().is_success() => {
                    // Log successful response initiation
                    if let Ok(logger) = self.debug_logger.lock() {
                        let _ = logger.append_response_event(
                            &request_id,
                            "stream_initiated",
                            &serde_json::json!({
                                "status": "success",
                                "status_code": resp.status().as_u16(),
                                "x_request_id": resp.headers()
                                    .get("x-request-id")
                                    .and_then(|v| v.to_str().ok())
                                    .unwrap_or_default()
                            }),
                        );
                    }
                    let (tx_event, rx_event) = mpsc::channel::<Result<ResponseEvent>>(1600);

                    if let Some(snapshot) = parse_rate_limit_snapshot(resp.headers()) {
                        debug!(
                            "rate limit headers:\n{}",
                            format_rate_limit_headers(resp.headers())
                        );

                        if tx_event
                            .send(Ok(ResponseEvent::RateLimits(snapshot)))
                            .await
                            .is_err()
                        {
                            debug!("receiver dropped rate limit snapshot event");
                        }
                    }

                    // spawn task to process SSE
                    let stream = resp.bytes_stream().map_err(CodexErr::Reqwest);
                    let debug_logger = Arc::clone(&self.debug_logger);
                    let request_id_clone = request_id.clone();
                    let otel_event_manager = self.otel_event_manager.clone();
                    tokio::spawn(process_sse(
                        stream,
                        tx_event,
                        self.provider.stream_idle_timeout(),
                        debug_logger,
                        request_id_clone,
                        otel_event_manager,
                    ));

                    return Ok(ResponseStream { rx_event });
                }
                Ok(res) => {
                    let status = res.status();
                    // Capture x-request-id up-front in case we consume the response body later.
                    let x_request_id = res
                        .headers()
                        .get("x-request-id")
                        .and_then(|v| v.to_str().ok())
                        .map(|s| s.to_string());

                    // Pull out Retry‑After header if present.
                    let retry_after_secs = res
                        .headers()
                        .get(reqwest::header::RETRY_AFTER)
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.parse::<u64>().ok());

                    if status == StatusCode::UNAUTHORIZED {
                        if let Some(a) = auth.as_ref() {
                            let _ = a.refresh_token().await;
                        }
                    }

                    // Read the response body once for diagnostics across error branches.
                    let body_text = res.text().await.unwrap_or_default();

                    // The OpenAI Responses endpoint returns structured JSON bodies even for 4xx/5xx
                    // errors. When we bubble early with only the HTTP status the caller sees an opaque
                    // "unexpected status 400 Bad Request" which makes debugging nearly impossible.
                    // Instead, read (and include) the response text so higher layers and users see the
                    // exact error message (e.g. "Unknown parameter: 'input[0].metadata'"). The body is
                    // small and this branch only runs on error paths so the extra allocation is
                    // negligible.
                    if !(status == StatusCode::TOO_MANY_REQUESTS
                        || status == StatusCode::UNAUTHORIZED
                        || status.is_server_error())
                    {
                        // Log error response
                        if let Ok(logger) = self.debug_logger.lock() {
                            let _ = logger.append_response_event(
                                &request_id,
                                "error",
                                &serde_json::json!({
                                    "status": status.as_u16(),
                                    "body": body_text
                                }),
                            );
                            let _ = logger.end_request_log(&request_id);
                        }
                        return Err(CodexErr::UnexpectedStatus(UnexpectedResponseError {
                            status,
                            body: body_text,
                            request_id: None,
                        }));
                    }

                    if status == StatusCode::TOO_MANY_REQUESTS {
                        let body = serde_json::from_str::<ErrorResponse>(&body_text).ok();
                        if let Some(ErrorResponse { error }) = body {
                            if error.r#type.as_deref() == Some("usage_limit_reached") {
                                // Prefer the plan_type provided in the error message if present
                                // because it's more up to date than the one encoded in the auth
                                // token.
                                let plan_type = error
                                    .plan_type
                                    .or_else(|| auth.and_then(|a| a.get_plan_type()));
                                let resets_in_seconds = error.resets_in_seconds;
                                return Err(CodexErr::UsageLimitReached(UsageLimitReachedError {
                                    plan_type,
                                    resets_in_seconds,
                                }));
                            } else if error.r#type.as_deref() == Some("usage_not_included") {
                                return Err(CodexErr::UsageNotIncluded);
                            }
                        }
                    }

                    if attempt > max_retries {
                        // On final attempt, surface rich diagnostics for server errors.
                        // On final attempt, surface rich diagnostics for server errors.
                        if status.is_server_error() {
                            let (message, body_excerpt) =
                                match serde_json::from_str::<ErrorResponse>(&body_text) {
                                    Ok(ErrorResponse { error }) => {
                                        let msg = error
                                            .message
                                            .unwrap_or_else(|| "server error".to_string());
                                        (msg, None)
                                    }
                                    Err(_) => {
                                        let mut excerpt = body_text;
                                        const MAX: usize = 600;
                                        if excerpt.len() > MAX {
                                            excerpt.truncate(MAX);
                                        }
                                        (
                                            "server error".to_string(),
                                            if excerpt.is_empty() {
                                                None
                                            } else {
                                                Some(excerpt)
                                            },
                                        )
                                    }
                                };

                            // Build a single-line, actionable message for the UI and logs.
                            let mut msg = format!("server error {status}: {message}");
                            if let Some(id) = &x_request_id {
                                msg.push_str(&format!(" (request-id: {id})"));
                            }
                            if let Some(excerpt) = &body_excerpt {
                                msg.push_str(&format!(" | body: {excerpt}"));
                            }

                            // Log detailed context to the debug logger and close the request log.
                            if let Ok(logger) = self.debug_logger.lock() {
                                let _ = logger.append_response_event(
                                    &request_id,
                                    "server_error_on_retry_limit",
                                    &serde_json::json!({
                                        "status": status.as_u16(),
                                        "x_request_id": x_request_id,
                                        "message": message,
                                        "body_excerpt": body_excerpt,
                                    }),
                                );
                                let _ = logger.end_request_log(&request_id);
                            }

                            return Err(CodexErr::ServerError(msg));
                        }

                        return Err(CodexErr::RetryLimit(RetryLimitReachedError {
                            status,
                            request_id: None,
                        }));
                    }

                    let delay = retry_after_secs
                        .map(|s| Duration::from_millis(s * 1_000))
                        .unwrap_or_else(|| backoff(attempt));
                    tokio::time::sleep(delay).await;
                }
                Err(e) => {
                    if attempt > max_retries {
                        // Log network error
                        if let Ok(logger) = self.debug_logger.lock() {
                            let _ = logger.log_error(&endpoint, &format!("Network error: {}", e), log_tag);
                        }
                        return Err(e.into());
                    }
                    let delay = backoff(attempt);
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    pub fn get_provider(&self) -> ModelProviderInfo {
        self.provider.clone()
    }

    /// Returns the currently configured model slug.
    #[allow(dead_code)]
    pub fn get_model(&self) -> String {
        self.config.model.clone()
    }

    /// Returns the currently configured model family.
    #[allow(dead_code)]
    pub fn get_model_family(&self) -> ModelFamily {
        self.config.model_family.clone()
    }

    #[allow(dead_code)]
    pub fn get_model_context_window(&self) -> Option<u64> {
        self.config.model_context_window
    }

    #[allow(dead_code)]
    pub fn get_auth_manager(&self) -> Option<Arc<AuthManager>> {
        self.auth_manager.clone()
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct SseEvent {
    #[serde(rename = "type")]
    kind: String,
    response: Option<Value>,
    item: Option<Value>,
    delta: Option<String>,
    // Present on delta events from the Responses API; used to correlate
    // streaming chunks with the final OutputItemDone.
    item_id: Option<String>,
    // Optional ordering metadata from the Responses API; used to filter
    // duplicates and out‑of‑order reasoning deltas.
    sequence_number: Option<u64>,
    output_index: Option<u32>,
    content_index: Option<u32>,
    summary_index: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct ResponseCompleted {
    id: String,
    usage: Option<ResponseCompletedUsage>,
}

#[derive(Debug, Deserialize)]
struct ResponseCompletedUsage {
    input_tokens: u64,
    input_tokens_details: Option<ResponseCompletedInputTokensDetails>,
    output_tokens: u64,
    output_tokens_details: Option<ResponseCompletedOutputTokensDetails>,
    total_tokens: u64,
}

impl From<ResponseCompletedUsage> for TokenUsage {
    fn from(val: ResponseCompletedUsage) -> Self {
        TokenUsage {
            input_tokens: val.input_tokens,
            cached_input_tokens: val
                .input_tokens_details
                .map(|d| d.cached_tokens)
                .unwrap_or(0),
            output_tokens: val.output_tokens,
            reasoning_output_tokens: val
                .output_tokens_details
                .map(|d| d.reasoning_tokens)
                .unwrap_or(0),
            total_tokens: val.total_tokens,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ResponseCompletedInputTokensDetails {
    cached_tokens: u64,
}

#[derive(Debug, Deserialize)]
struct ResponseCompletedOutputTokensDetails {
    reasoning_tokens: u64,
}

fn attach_item_ids(payload_json: &mut Value, original_items: &[ResponseItem]) {
    let Some(input_value) = payload_json.get_mut("input") else {
        return;
    };
    let serde_json::Value::Array(items) = input_value else {
        return;
    };

    for (value, item) in items.iter_mut().zip(original_items.iter()) {
        if let ResponseItem::Reasoning { id, .. }
        | ResponseItem::Message { id: Some(id), .. }
        | ResponseItem::WebSearchCall { id: Some(id), .. }
        | ResponseItem::FunctionCall { id: Some(id), .. }
        | ResponseItem::LocalShellCall { id: Some(id), .. }
        | ResponseItem::CustomToolCall { id: Some(id), .. } = item
        {
            if id.is_empty() {
                continue;
            }

            if let Some(obj) = value.as_object_mut() {
                obj.insert("id".to_string(), Value::String(id.clone()));
            }
        }
    }
}

fn parse_rate_limit_snapshot(headers: &HeaderMap) -> Option<RateLimitSnapshotEvent> {
    let primary_used_percent = parse_header_f64(headers, "x-codex-primary-used-percent")?;
    let secondary_used_percent = parse_header_f64(headers, "x-codex-secondary-used-percent")?;
    let primary_to_secondary_ratio_percent =
        parse_header_f64(headers, "x-codex-primary-over-secondary-limit-percent")?;
    let primary_window_minutes = parse_header_u64(headers, "x-codex-primary-window-minutes")?;
    let secondary_window_minutes = parse_header_u64(headers, "x-codex-secondary-window-minutes")?;
    let primary_reset_after_seconds =
        parse_header_u64(headers, "x-codex-primary-reset-after-seconds");
    let secondary_reset_after_seconds =
        parse_header_u64(headers, "x-codex-secondary-reset-after-seconds");

    Some(RateLimitSnapshotEvent {
        primary_used_percent,
        secondary_used_percent,
        primary_to_secondary_ratio_percent,
        primary_window_minutes,
        secondary_window_minutes,
        primary_reset_after_seconds,
        secondary_reset_after_seconds,
    })
}

fn format_rate_limit_headers(headers: &HeaderMap) -> String {
    let mut pairs: Vec<String> = headers
        .iter()
        .map(|(name, value)| {
            let value_str = value.to_str().unwrap_or("<invalid>");
            format!("{}: {}", name, value_str)
        })
        .collect();
    pairs.sort();
    pairs.join("\n")
}

fn parse_header_f64(headers: &HeaderMap, name: &str) -> Option<f64> {
    parse_header_str(headers, name)?
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
}

fn parse_header_u64(headers: &HeaderMap, name: &str) -> Option<u64> {
    parse_header_str(headers, name)?.parse::<u64>().ok()
}

fn parse_header_str<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name)?.to_str().ok()
}

fn header_map_to_json(headers: &HeaderMap) -> Value {
    let mut ordered: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (name, value) in headers.iter() {
        let entry = ordered.entry(name.as_str().to_string()).or_default();
        entry.push(value.to_str().unwrap_or_default().to_string());
    }

    serde_json::to_value(ordered).unwrap_or(Value::Null)
}

async fn process_sse<S>(
    stream: S,
    tx_event: mpsc::Sender<Result<ResponseEvent>>,
    idle_timeout: Duration,
    debug_logger: Arc<Mutex<DebugLogger>>,
    request_id: String,
    otel_event_manager: Option<OtelEventManager>,
) where
    S: Stream<Item = Result<Bytes>> + Unpin,
{
    let mut stream = stream.eventsource();

    // If the stream stays completely silent for an extended period treat it as disconnected.
    // The response id returned from the "complete" message.
    let mut response_completed: Option<ResponseCompleted> = None;
    let mut response_error: Option<CodexErr> = None;
    // Track the current item_id to include with delta events
    let mut current_item_id: Option<String> = None;

    // Monotonic sequence guards to drop duplicate/out‑of‑order deltas.
    // Keys are item_id strings.
    use std::collections::HashMap;
    // Track last sequence_number per (item_id, output_index[, content_index])
    // Default indices to 0 when absent for robustness across providers.
    let mut last_seq_reasoning_summary: HashMap<(String, u32, u32), u64> = HashMap::new();
    let mut last_seq_reasoning_content: HashMap<(String, u32, u32), u64> = HashMap::new();
    // Best-effort duplicate text guard when sequence_number is unavailable.
    let mut last_text_reasoning_summary: HashMap<(String, u32, u32), String> = HashMap::new();
    let mut last_text_reasoning_content: HashMap<(String, u32, u32), String> = HashMap::new();

    loop {
        let next_event = if let Some(manager) = otel_event_manager.as_ref() {
            manager
                .log_sse_event(|| timeout(idle_timeout, stream.next()))
                .await
        } else {
            timeout(idle_timeout, stream.next()).await
        };

        let sse = match next_event {
            Ok(Some(Ok(sse))) => sse,
            Ok(Some(Err(e))) => {
                debug!("SSE Error: {e:#}");
                let event = CodexErr::Stream(e.to_string(), None);
                let _ = tx_event.send(Err(event)).await;
                return;
            }
            Ok(None) => {
                match response_completed {
                    Some(ResponseCompleted {
                        id: response_id,
                        usage,
                    }) => {
                        if let (Some(usage), Some(manager)) = (&usage, otel_event_manager.as_ref()) {
                            manager.sse_event_completed(
                                usage.input_tokens,
                                usage.output_tokens,
                                usage
                                    .input_tokens_details
                                    .as_ref()
                                    .map(|d| d.cached_tokens),
                                usage
                                    .output_tokens_details
                                    .as_ref()
                                    .map(|d| d.reasoning_tokens),
                                usage.total_tokens,
                            );
                        }
                        let event = ResponseEvent::Completed {
                            response_id,
                            token_usage: usage.map(Into::into),
                        };
                        let _ = tx_event.send(Ok(event)).await;
                    }
                    None => {
                        let error = response_error.unwrap_or(CodexErr::Stream(
                            "stream closed before response.completed".into(),
                            None,
                        ));
                        if let Some(manager) = otel_event_manager.as_ref() {
                            manager.see_event_completed_failed(&error);
                        }
                        let _ = tx_event.send(Err(error)).await;
                    }
                }
                // Mark the request log as complete
                if let Ok(logger) = debug_logger.lock() {
                    let _ = logger.end_request_log(&request_id);
                }
                return;
            }
            Err(_) => {
                let _ = tx_event
                    .send(Err(CodexErr::Stream(
                        "idle timeout waiting for SSE".into(),
                        None,
                    )))
                    .await;
                return;
            }
        };

        let raw = sse.data.clone();
        trace!("SSE event: {}", raw);

        // Log the raw SSE event data
        if let Ok(logger) = debug_logger.lock() {
            if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&sse.data) {
                let _ = logger.append_response_event(&request_id, "sse_event", &json_value);
            }
        }

        let event: SseEvent = match serde_json::from_str(&sse.data) {
            Ok(event) => event,
            Err(e) => {
                // Log parse error with data excerpt, and record it in the debug logger as well.
                let mut excerpt = sse.data.clone();
                const MAX: usize = 600;
                if excerpt.len() > MAX {
                    excerpt.truncate(MAX);
                }
                debug!("Failed to parse SSE event: {e}, data: {excerpt}");
                if let Ok(logger) = debug_logger.lock() {
                    let _ = logger.append_response_event(
                        &request_id,
                        "sse_parse_error",
                        &serde_json::json!({
                            "error": e.to_string(),
                            "data_excerpt": excerpt,
                        }),
                    );
                }
                continue;
            }
        };

        match event.kind.as_str() {
            // Individual output item finalised. Forward immediately so the
            // rest of the agent can stream assistant text/functions *live*
            // instead of waiting for the final `response.completed` envelope.
            //
            // IMPORTANT: We used to ignore these events and forward the
            // duplicated `output` array embedded in the `response.completed`
            // payload.  That produced two concrete issues:
            //   1. No real‑time streaming – the user only saw output after the
            //      entire turn had finished, which broke the "typing" UX and
            //      made long‑running turns look stalled.
            //   2. Duplicate `function_call_output` items – both the
            //      individual *and* the completed array were forwarded, which
            //      confused the backend and triggered 400
            //      "previous_response_not_found" errors because the duplicated
            //      IDs did not match the incremental turn chain.
            //
            // The fix is to forward the incremental events *as they come* and
            // drop the duplicated list inside `response.completed`.
            "response.output_item.done" => {
                let Some(item_val) = event.item else { continue };
                // Special-case: web_search_call completion -> synthesize a completion event
                if item_val
                    .get("type")
                    .and_then(|v| v.as_str())
                    .is_some_and(|s| s == "web_search_call")
                {
                    let call_id = item_val
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let query = item_val
                        .get("action")
                        .and_then(|a| a.get("query"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    let ev = ResponseEvent::WebSearchCallCompleted { call_id, query };
                    if tx_event.send(Ok(ev)).await.is_err() {
                        return;
                    }
                }
                let Ok(item) = serde_json::from_value::<ResponseItem>(item_val.clone()) else {
                    debug!("failed to parse ResponseItem from output_item.done");
                    continue;
                };

                // Extract item_id if present
                if let Some(id) = item_val.get("id").and_then(|v| v.as_str()) {
                    current_item_id = Some(id.to_string());
                } else {
                    // Check within the parsed item structure
                    match &item {
                        ResponseItem::Message { id, .. }
                        | ResponseItem::FunctionCall { id, .. }
                        | ResponseItem::LocalShellCall { id, .. } => {
                            if let Some(item_id) = id {
                                current_item_id = Some(item_id.clone());
                            }
                        }
                        ResponseItem::Reasoning { id, .. } => {
                            current_item_id = Some(id.clone());
                        }
                        _ => {}
                    }
                }

                let event = ResponseEvent::OutputItemDone { item, sequence_number: event.sequence_number, output_index: event.output_index };
                if tx_event.send(Ok(event)).await.is_err() {
                    return;
                }
            }
            "response.output_text.delta" => {
                if let Some(delta) = event.delta {
                    // Prefer the explicit item_id from the SSE event; fall back to last seen.
                    if let Some(ref id) = event.item_id {
                        current_item_id = Some(id.clone());
                    }
                    tracing::debug!("sse.delta output_text id={:?} len={}", current_item_id, delta.len());
                    let ev = ResponseEvent::OutputTextDelta {
                        delta,
                        item_id: event.item_id.or_else(|| current_item_id.clone()),
                        sequence_number: event.sequence_number,
                        output_index: event.output_index,
                    };
                    if tx_event.send(Ok(ev)).await.is_err() {
                        return;
                    }
                }
            }
            "response.reasoning_summary_text.delta" => {
                if let Some(delta) = event.delta {
                    if let Some(ref id) = event.item_id {
                        current_item_id = Some(id.clone());
                    }
                    // Compose key using item_id + output_index
                    let out_idx: u32 = event.output_index.unwrap_or(0);
                    let sum_idx: u32 = event.summary_index.unwrap_or(0);
                    if let Some(ref id) = current_item_id {
                        // Drop duplicates/out‑of‑order by sequence_number when available
                        if let Some(sn) = event.sequence_number {
                            let last = last_seq_reasoning_summary.entry((id.clone(), out_idx, sum_idx)).or_insert(0);
                            if *last >= sn { continue; }
                            *last = sn;
                        } else {
                            // Best-effort: drop exact duplicate text for same key when seq is missing
                            let key = (id.clone(), out_idx, sum_idx);
                            if last_text_reasoning_summary.get(&key).map_or(false, |prev| prev == &delta) {
                                continue;
                            }
                            last_text_reasoning_summary.insert(key, delta.clone());
                        }
                    }
                    tracing::debug!(
                        "sse.delta reasoning_summary id={:?} out_idx={} sum_idx={} len={} seq={:?}",
                        current_item_id, out_idx, sum_idx,
                        delta.len(),
                        event.sequence_number
                    );
                    let ev = ResponseEvent::ReasoningSummaryDelta {
                        delta,
                        item_id: event.item_id.or_else(|| current_item_id.clone()),
                        sequence_number: event.sequence_number,
                        output_index: event.output_index,
                        summary_index: event.summary_index,
                    };
                    if tx_event.send(Ok(ev)).await.is_err() {
                        return;
                    }
                }
            }
            "response.reasoning_text.delta" => {
                if let Some(delta) = event.delta {
                    if let Some(ref id) = event.item_id {
                        current_item_id = Some(id.clone());
                    }
                    // Compose key using item_id + output_index + content_index
                    let out_idx: u32 = event.output_index.unwrap_or(0);
                    let content_idx: u32 = event.content_index.unwrap_or(0);
                    if let Some(ref id) = current_item_id {
                        // Drop duplicates/out‑of‑order by sequence_number when available
                        if let Some(sn) = event.sequence_number {
                            let last = last_seq_reasoning_content.entry((id.clone(), out_idx, content_idx)).or_insert(0);
                            if *last >= sn { continue; }
                            *last = sn;
                        } else {
                            // Best-effort: drop exact duplicate text for same key when seq is missing
                            let key = (id.clone(), out_idx, content_idx);
                            if last_text_reasoning_content.get(&key).map_or(false, |prev| prev == &delta) {
                                continue;
                            }
                            last_text_reasoning_content.insert(key, delta.clone());
                        }
                    }
                    tracing::debug!(
                        "sse.delta reasoning_content id={:?} out_idx={} content_idx={} len={} seq={:?}",
                        current_item_id, out_idx, content_idx,
                        delta.len(),
                        event.sequence_number
                    );
                    let ev = ResponseEvent::ReasoningContentDelta {
                        delta,
                        item_id: event.item_id.or_else(|| current_item_id.clone()),
                        sequence_number: event.sequence_number,
                        output_index: event.output_index,
                        content_index: event.content_index,
                    };
                    if tx_event.send(Ok(ev)).await.is_err() {
                        return;
                    }
                }
            }
            "response.created" => {
                if event.response.is_some() {
                    let _ = tx_event.send(Ok(ResponseEvent::Created {})).await;
                }
            }
            "response.failed" => {
                if let Some(resp_val) = event.response {
                    response_error = Some(CodexErr::Stream(
                        "response.failed event received".to_string(),
                        None,
                    ));

                    let error = resp_val.get("error");

                    if let Some(error) = error {
                        match serde_json::from_value::<Error>(error.clone()) {
                            Ok(error) => {
                                let delay = try_parse_retry_after(&error);
                                let message = error.message.unwrap_or_default();
                                response_error = Some(CodexErr::Stream(message, delay));
                            }
                            Err(e) => {
                                debug!("failed to parse ErrorResponse: {e}");
                            }
                        }
                    }
                }
            }
            // Final response completed – includes array of output items & id
            "response.completed" => {
                if let Some(resp_val) = event.response {
                    match serde_json::from_value::<ResponseCompleted>(resp_val) {
                        Ok(r) => {
                            response_completed = Some(r);
                        }
                        Err(e) => {
                            debug!("failed to parse ResponseCompleted: {e}");
                            continue;
                        }
                    };
                };
            }
            "response.content_part.done"
            | "response.function_call_arguments.delta"
            | "response.custom_tool_call_input.delta"
            | "response.custom_tool_call_input.done" // also emitted as response.output_item.done
            | "response.in_progress"
            | "response.output_item.added"
            | "response.output_text.done" => {
                if event.kind == "response.output_item.added" {
                    if let Some(item) = event.item.as_ref() {
                        // Detect web_search_call begin and forward a synthetic event upstream.
                        if let Some(ty) = item.get("type").and_then(|v| v.as_str()) {
                            if ty == "web_search_call" {
                                let call_id = item
                                    .get("id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let ev = ResponseEvent::WebSearchCallBegin { call_id };
                                if tx_event.send(Ok(ev)).await.is_err() {
                                    return;
                                }
                            }
                        }
                    }
                }
            }
            "response.reasoning_summary_part.added" => {
                // Boundary between reasoning summary sections (e.g., titles).
                let event = ResponseEvent::ReasoningSummaryPartAdded;
                if tx_event.send(Ok(event)).await.is_err() {
                    return;
                }
            }
            "response.reasoning_summary_text.done" => {}
            _ => {}
        }
    }
}

/// used in tests to stream from a text SSE file
async fn stream_from_fixture(
    path: impl AsRef<Path>,
    provider: ModelProviderInfo,
    otel_event_manager: Option<OtelEventManager>,
) -> Result<ResponseStream> {
    let (tx_event, rx_event) = mpsc::channel::<Result<ResponseEvent>>(1600);
    let f = std::fs::File::open(path.as_ref())?;
    let lines = std::io::BufReader::new(f).lines();

    // insert \n\n after each line for proper SSE parsing
    let mut content = String::new();
    for line in lines {
        content.push_str(&line?);
        content.push_str("\n\n");
    }

    let rdr = std::io::Cursor::new(content);
    let stream = ReaderStream::new(rdr).map_err(CodexErr::Io);
    // Create a dummy debug logger for testing
    let debug_logger = Arc::new(Mutex::new(DebugLogger::new(false).unwrap()));
    tokio::spawn(process_sse(
        stream,
        tx_event,
        provider.stream_idle_timeout(),
        debug_logger,
        String::new(), // Empty request_id for test fixture
        otel_event_manager,
    ));
    Ok(ResponseStream { rx_event })
}

// Note: legacy helpers for parsing Retry-After headers and rate-limit messages
// were removed during merge cleanup. If needed in the future, pick them from
// upstream and integrate with our error handling path.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_provider_info::{ModelProviderInfo, WireApi};
    use std::collections::HashMap;
    use serde_json::json;
    use tokio::sync::mpsc;
    use tokio_test::io::Builder as IoBuilder;
    use tokio_util::io::ReaderStream;

    // ────────────────────────────
    // Helpers
    // ────────────────────────────

    #[tokio::test]
    async fn responses_request_uses_beta_header_for_public_openai() {
        let provider = ModelProviderInfo {
            name: "openai".to_string(),
            base_url: Some("https://api.openai.com/v1".to_string()),
            env_key: None,
            env_key_instructions: None,
            wire_api: WireApi::Responses,
            query_params: None,
            http_headers: None,
            env_http_headers: None,
            request_max_retries: Some(0),
            stream_max_retries: None,
            stream_idle_timeout_ms: None,
            requires_openai_auth: false,
            openrouter: None,
        };

        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("client");

        let mut builder = provider
            .create_request_builder(&client, &None)
            .await
            .expect("builder");
        let has_beta = builder
            .try_clone()
            .and_then(|b| b.build().ok())
            .map_or(false, |req| req.headers().contains_key("OpenAI-Beta"));
        if !has_beta {
            builder = builder.header("OpenAI-Beta", RESPONSES_BETA_HEADER_V1);
        }
        let request = builder
            .try_clone()
            .expect("clone request builder")
            .build()
            .expect("build request");

        let header_value = request
            .headers()
            .get("OpenAI-Beta")
            .expect("OpenAI-Beta header present");
        assert_eq!(header_value, RESPONSES_BETA_HEADER_V1);
    }

    #[tokio::test]
    async fn responses_request_uses_experimental_for_backend() {
        let provider = ModelProviderInfo {
            name: "backend".to_string(),
            base_url: Some("https://chatgpt.com/backend-api/codex".to_string()),
            env_key: None,
            env_key_instructions: None,
            wire_api: WireApi::Responses,
            query_params: None,
            http_headers: None,
            env_http_headers: None,
            request_max_retries: Some(0),
            stream_max_retries: None,
            stream_idle_timeout_ms: None,
            requires_openai_auth: false,
            openrouter: None,
        };

        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("client");

        let mut builder = provider
            .create_request_builder(&client, &None)
            .await
            .expect("builder");
        let has_beta = builder
            .try_clone()
            .and_then(|b| b.build().ok())
            .map_or(false, |req| req.headers().contains_key("OpenAI-Beta"));
        if !has_beta {
            builder = builder.header("OpenAI-Beta", RESPONSES_BETA_HEADER_EXPERIMENTAL);
        }
        let request = builder
            .try_clone()
            .expect("clone request builder")
            .build()
            .expect("build request");

        let header_value = request
            .headers()
            .get("OpenAI-Beta")
            .expect("OpenAI-Beta header present");
        assert_eq!(header_value, RESPONSES_BETA_HEADER_EXPERIMENTAL);
    }

    #[tokio::test]
    async fn responses_request_respects_preexisting_beta_header() {
        let mut headers = HashMap::new();
        headers.insert("OpenAI-Beta".to_string(), "custom".to_string());
        let provider = ModelProviderInfo {
            name: "custom".to_string(),
            base_url: Some("https://api.openai.com/v1".to_string()),
            env_key: None,
            env_key_instructions: None,
            wire_api: WireApi::Responses,
            query_params: None,
            http_headers: Some(headers),
            env_http_headers: None,
            request_max_retries: Some(0),
            stream_max_retries: None,
            stream_idle_timeout_ms: None,
            requires_openai_auth: false,
            openrouter: None,
        };

        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("client");

        let request = provider
            .create_request_builder(&client, &None)
            .await
            .expect("builder")
            .try_clone()
            .expect("clone request builder")
            .build()
            .expect("build request");

        let header_value = request
            .headers()
            .get("OpenAI-Beta")
            .expect("OpenAI-Beta header present");
        assert_eq!(header_value, "custom");
    }

    /// Runs the SSE parser on pre-chunked byte slices and returns every event
    /// (including any final `Err` from a stream-closure check).
    async fn collect_events(
        chunks: &[&[u8]],
        provider: ModelProviderInfo,
    ) -> Vec<Result<ResponseEvent>> {
        let mut builder = IoBuilder::new();
        for chunk in chunks {
            builder.read(chunk);
        }

        let reader = builder.build();
        let stream = ReaderStream::new(reader).map_err(CodexErr::Io);
        let (tx, mut rx) = mpsc::channel::<Result<ResponseEvent>>(16);
        let debug_logger = Arc::new(Mutex::new(DebugLogger::new(false).unwrap()));
        tokio::spawn(process_sse(
            stream,
            tx,
            provider.stream_idle_timeout(),
            debug_logger,
            String::new(),
            None,
        ));

        let mut events = Vec::new();
        while let Some(ev) = rx.recv().await {
            events.push(ev);
        }
        events
    }

    /// Builds an in-memory SSE stream from JSON fixtures and returns only the
    /// successfully parsed events (panics on internal channel errors).
    async fn run_sse(
        events: Vec<serde_json::Value>,
        provider: ModelProviderInfo,
    ) -> Vec<ResponseEvent> {
        let mut body = String::new();
        for e in events {
            let kind = e
                .get("type")
                .and_then(|v| v.as_str())
                .expect("fixture event missing type");
            if e.as_object().map(|o| o.len() == 1).unwrap_or(false) {
                body.push_str(&format!("event: {kind}\n\n"));
            } else {
                body.push_str(&format!("event: {kind}\ndata: {e}\n\n"));
            }
        }

        let (tx, mut rx) = mpsc::channel::<Result<ResponseEvent>>(8);
        let stream = ReaderStream::new(std::io::Cursor::new(body)).map_err(CodexErr::Io);
        let debug_logger = Arc::new(Mutex::new(DebugLogger::new(false).unwrap()));
        tokio::spawn(process_sse(
            stream,
            tx,
            provider.stream_idle_timeout(),
            debug_logger,
            String::new(),
            None,
        ));

        let mut out = Vec::new();
        while let Some(ev) = rx.recv().await {
            out.push(ev.expect("channel closed"));
        }
        out
    }

    // ────────────────────────────
    // Tests from `implement-test-for-responses-api-sse-parser`
    // ────────────────────────────

    #[tokio::test]
    async fn parses_items_and_completed() {
        let item1 = json!({
            "type": "response.output_item.done",
            "item": {
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "Hello"}]
            }
        })
        .to_string();

        let item2 = json!({
            "type": "response.output_item.done",
            "item": {
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "World"}]
            }
        })
        .to_string();

        let completed = json!({
            "type": "response.completed",
            "response": { "id": "resp1" }
        })
        .to_string();

        let sse1 = format!("event: response.output_item.done\ndata: {item1}\n\n");
        let sse2 = format!("event: response.output_item.done\ndata: {item2}\n\n");
        let sse3 = format!("event: response.completed\ndata: {completed}\n\n");

        let provider = ModelProviderInfo {
            name: "test".to_string(),
            base_url: Some("https://test.com".to_string()),
            env_key: Some("TEST_API_KEY".to_string()),
            env_key_instructions: None,
            wire_api: WireApi::Responses,
            query_params: None,
            http_headers: None,
            env_http_headers: None,
            request_max_retries: Some(0),
            stream_max_retries: Some(0),
            stream_idle_timeout_ms: Some(1000),
            requires_openai_auth: false,
            openrouter: None,
        };

        let events = collect_events(
            &[sse1.as_bytes(), sse2.as_bytes(), sse3.as_bytes()],
            provider,
        )
        .await;

        assert_eq!(events.len(), 3);

        matches!(
            &events[0],
            Ok(ResponseEvent::OutputItemDone {
                item: ResponseItem::Message { role, .. },
                ..
            }) if role == "assistant"
        );

        matches!(
            &events[1],
            Ok(ResponseEvent::OutputItemDone {
                item: ResponseItem::Message { role, .. },
                ..
            }) if role == "assistant"
        );

        match &events[2] {
            Ok(ResponseEvent::Completed {
                response_id,
                token_usage,
            }) => {
                assert_eq!(response_id, "resp1");
                assert!(token_usage.is_none());
            }
            other => panic!("unexpected third event: {other:?}"),
        }
    }

    #[tokio::test]
    async fn error_when_missing_completed() {
        let item1 = json!({
            "type": "response.output_item.done",
            "item": {
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "Hello"}]
            }
        })
        .to_string();

        let sse1 = format!("event: response.output_item.done\ndata: {item1}\n\n");
        let provider = ModelProviderInfo {
            name: "test".to_string(),
            base_url: Some("https://test.com".to_string()),
            env_key: Some("TEST_API_KEY".to_string()),
            env_key_instructions: None,
            wire_api: WireApi::Responses,
            query_params: None,
            http_headers: None,
            env_http_headers: None,
            request_max_retries: Some(0),
            stream_max_retries: Some(0),
            stream_idle_timeout_ms: Some(1000),
            requires_openai_auth: false,
            openrouter: None,
        };

        let events = collect_events(&[sse1.as_bytes()], provider).await;

        assert_eq!(events.len(), 2);

        matches!(
            events[0],
            Ok(ResponseEvent::OutputItemDone { .. })
        );

        match &events[1] {
            Err(CodexErr::Stream(msg, _)) => {
                assert_eq!(msg, "stream closed before response.completed")
            }
            other => panic!("unexpected second event: {other:?}"),
        }
    }

    #[tokio::test]
    async fn error_when_error_event() {
        let raw_error = r#"{"type":"response.failed","sequence_number":3,"response":{"id":"resp_689bcf18d7f08194bf3440ba62fe05d803fee0cdac429894","object":"response","created_at":1755041560,"status":"failed","background":false,"error":{"code":"rate_limit_exceeded","message":"Rate limit reached for gpt-5 in organization org-AAA on tokens per min (TPM): Limit 30000, Used 22999, Requested 12528. Please try again in 11.054s. Visit https://platform.openai.com/account/rate-limits to learn more."}, "usage":null,"user":null,"metadata":{}}}"#;

        let sse1 = format!("event: response.failed\ndata: {raw_error}\n\n");
        let provider = ModelProviderInfo {
            name: "test".to_string(),
            base_url: Some("https://test.com".to_string()),
            env_key: Some("TEST_API_KEY".to_string()),
            env_key_instructions: None,
            wire_api: WireApi::Responses,
            query_params: None,
            http_headers: None,
            env_http_headers: None,
            request_max_retries: Some(0),
            stream_max_retries: Some(0),
            stream_idle_timeout_ms: Some(1000),
            requires_openai_auth: false,
            openrouter: None,
        };

        let events = collect_events(&[sse1.as_bytes()], provider).await;

        assert_eq!(events.len(), 1);

        match &events[0] {
            Err(CodexErr::Stream(msg, delay)) => {
                assert_eq!(
                    msg,
                    "Rate limit reached for gpt-5 in organization org-AAA on tokens per min (TPM): Limit 30000, Used 22999, Requested 12528. Please try again in 11.054s. Visit https://platform.openai.com/account/rate-limits to learn more."
                );
                assert_eq!(*delay, Some(Duration::from_secs_f64(11.054)));
            }
            other => panic!("unexpected second event: {other:?}"),
        }
    }

    // ────────────────────────────
    // Table-driven test from `main`
    // ────────────────────────────

    /// Verifies that the adapter produces the right `ResponseEvent` for a
    /// variety of incoming `type` values.
    #[tokio::test]
    async fn table_driven_event_kinds() {
        struct TestCase {
            name: &'static str,
            event: serde_json::Value,
            expect_first: fn(&ResponseEvent) -> bool,
            expected_len: usize,
        }

        fn is_created(ev: &ResponseEvent) -> bool {
            matches!(ev, ResponseEvent::Created)
        }
        fn is_output(ev: &ResponseEvent) -> bool {
            matches!(ev, ResponseEvent::OutputItemDone { .. })
        }
        fn is_completed(ev: &ResponseEvent) -> bool {
            matches!(ev, ResponseEvent::Completed { .. })
        }

        let completed = json!({
            "type": "response.completed",
            "response": {
                "id": "c",
                "usage": {
                    "input_tokens": 0,
                    "input_tokens_details": null,
                    "output_tokens": 0,
                    "output_tokens_details": null,
                    "total_tokens": 0
                },
                "output": []
            }
        });

        let cases = vec![
            TestCase {
                name: "created",
                event: json!({"type": "response.created", "response": {}}),
                expect_first: is_created,
                expected_len: 2,
            },
            TestCase {
                name: "output_item.done",
                event: json!({
                    "type": "response.output_item.done",
                    "item": {
                        "type": "message",
                        "role": "assistant",
                        "content": [
                            {"type": "output_text", "text": "hi"}
                        ]
                    }
                }),
                expect_first: is_output,
                expected_len: 2,
            },
            TestCase {
                name: "unknown",
                event: json!({"type": "response.new_tool_event"}),
                expect_first: is_completed,
                expected_len: 1,
            },
        ];

        for case in cases {
            let mut evs = vec![case.event];
            evs.push(completed.clone());

            let provider = ModelProviderInfo {
                name: "test".to_string(),
                base_url: Some("https://test.com".to_string()),
                env_key: Some("TEST_API_KEY".to_string()),
                env_key_instructions: None,
                wire_api: WireApi::Responses,
                query_params: None,
                http_headers: None,
                env_http_headers: None,
                request_max_retries: Some(0),
                stream_max_retries: Some(0),
                stream_idle_timeout_ms: Some(1000),
                requires_openai_auth: false,
                openrouter: None,
            };

            let out = run_sse(evs, provider).await;
            assert_eq!(out.len(), case.expected_len, "case {}", case.name);
            assert!(
                (case.expect_first)(&out[0]),
                "first event mismatch in case {}",
                case.name
            );
        }
    }

    #[test]
    fn test_try_parse_retry_after() {
        let err = Error {
            r#type: None,
            message: Some("Rate limit reached for gpt-5 in organization org- on tokens per min (TPM): Limit 1, Used 1, Requested 19304. Please try again in 28ms. Visit https://platform.openai.com/account/rate-limits to learn more.".to_string()),
            code: Some("rate_limit_exceeded".to_string()),
            plan_type: None,
            resets_in_seconds: None
        };

        let delay = try_parse_retry_after(&err);
        assert_eq!(delay, Some(Duration::from_millis(28)));
    }

    #[test]
    fn test_try_parse_retry_after_no_delay() {
        let err = Error {
            r#type: None,
            message: Some("Rate limit reached for gpt-5 in organization <ORG> on tokens per min (TPM): Limit 30000, Used 6899, Requested 24050. Please try again in 1.898s. Visit https://platform.openai.com/account/rate-limits to learn more.".to_string()),
            code: Some("rate_limit_exceeded".to_string()),
            plan_type: None,
            resets_in_seconds: None
        };
        let delay = try_parse_retry_after(&err);
        assert_eq!(delay, Some(Duration::from_secs_f64(1.898)));
    }
}
