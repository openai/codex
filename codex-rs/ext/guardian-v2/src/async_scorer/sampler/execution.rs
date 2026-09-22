//! Executes one prepared classifier request with the existing retry and streaming rules.
//! The first output returns immediately; a detached drain preserves reuse and token accounting.
//! Requests and retries stop when the account owner that started the classification changes.

use super::CLASSIFICATION_TOKEN_USAGE_METRIC;
use super::ConnectionPool;
use super::LunaSamplerConfig;
use super::LunaSamplerError;
use super::MAX_OUTPUT_BYTES;
use super::connection_pool::RequestMode;
use codex_api::ApiError;
use codex_api::ResponseEvent;
use codex_api::ResponsesApiRequest;
use codex_api::TransportError;
use codex_extension_api::ExtensionMetrics;
use codex_login::UnauthorizedRecovery;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::TokenUsage;
use http::StatusCode;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use tokio::sync::oneshot;

const MAX_SAMPLING_RETRIES: usize = 2;
const RESPONSES_LITE_METADATA_KEY: &str =
    "ws_request_header_x_openai_internal_codex_responses_lite";
const TURN_METADATA_KEY: &str = "x-codex-turn-metadata";

pub(super) struct SamplingExecution {
    pub(super) auth_owner_generation: Option<u64>,
    pub(super) config: Arc<LunaSamplerConfig>,
    pub(super) connections: Arc<ConnectionPool>,
    pub(super) request: ResponsesApiRequest,
    pub(super) turn_id: String,
    pub(super) parent_response_id: Option<String>,
    pub(super) parent_turn_id: String,
    pub(super) root_turn_id: Option<String>,
}

impl SamplingExecution {
    async fn retry_after_failure(
        &self,
        error: &LunaSamplerError,
        auth_recovery: &mut Option<UnauthorizedRecovery>,
        retries: &mut usize,
    ) -> bool {
        let retryable = match error {
            LunaSamplerError::ConnectionTimeout
            | LunaSamplerError::Api(
                ApiError::Retryable { .. }
                | ApiError::RateLimitExceeded { .. }
                | ApiError::Stream(_)
                | ApiError::ServerOverloaded,
            )
            | LunaSamplerError::Api(ApiError::Transport(
                TransportError::RetryLimit
                | TransportError::Timeout
                | TransportError::Connection(_)
                | TransportError::Network(_),
            )) => true,
            LunaSamplerError::Api(ApiError::Transport(TransportError::Http { status, .. }))
            | LunaSamplerError::Api(ApiError::Api { status, .. }) => {
                if *status == StatusCode::UNAUTHORIZED {
                    let Some(recovery) = auth_recovery.as_mut() else {
                        return false;
                    };
                    if !recovery.has_next() || recovery.next().await.is_err() {
                        return false;
                    }
                    self.connections.clear();
                    return true;
                } else {
                    status.is_server_error() || *status == StatusCode::TOO_MANY_REQUESTS
                }
            }
            LunaSamplerError::Provider(_)
            | LunaSamplerError::MissingOutput
            | LunaSamplerError::OutputTooLarge
            | LunaSamplerError::Superseded
            | LunaSamplerError::IncompatibleCompaction
            | LunaSamplerError::InputTooLarge
            | LunaSamplerError::Api(
                ApiError::Transport(
                    TransportError::Build(_) | TransportError::ResponseTooLarge { .. },
                )
                | ApiError::ContextWindowExceeded
                | ApiError::QuotaExceeded
                | ApiError::UsageNotIncluded
                | ApiError::RateLimit(_)
                | ApiError::InvalidRequest { .. }
                | ApiError::InvalidPrompt { .. }
                | ApiError::MisalignmentPolicyViolation { .. }
                | ApiError::CyberPolicy { .. }
                | ApiError::BioPolicy { .. },
            ) => false,
        };
        if retryable && *retries < MAX_SAMPLING_RETRIES {
            *retries += 1;
            return true;
        }
        false
    }

    pub(super) async fn run(
        mut self,
        mut superseded: oneshot::Receiver<()>,
        scored: Arc<AtomicBool>,
    ) -> Result<String, LunaSamplerError> {
        let auth_changes = self
            .config
            .provider
            .auth_manager()
            .filter(|_| self.config.provider.info().auth.is_none())
            .map(|manager| manager.auth_change_state_receiver());
        let owner_generation = self.auth_owner_generation;
        let account_changed_error = || {
            LunaSamplerError::Provider(
                std::io::Error::other("account changed during Luna classification").into(),
            )
        };
        let ensure_account_owner = || {
            if auth_changes
                .as_ref()
                .map(|changes| changes.borrow().owner_generation)
                != owner_generation
            {
                return Err(account_changed_error());
            }
            Ok(())
        };
        let mut owner_changes = auth_changes.clone();
        let owner_changed = async move {
            let Some(changes) = owner_changes.as_mut() else {
                return std::future::pending::<()>().await;
            };
            while Some(changes.borrow_and_update().owner_generation) == owner_generation {
                if changes.changed().await.is_err() {
                    break;
                }
            }
        };
        tokio::pin!(owner_changed);
        let mut retries = 0;
        let mut auth_recovery = self
            .config
            .provider
            .auth_manager()
            .map(|manager| manager.unauthorized_recovery());
        'retry: loop {
            ensure_account_owner()?;
            let lease = match tokio::select! {
                biased;
                _ = &mut owner_changed => return Err(account_changed_error()),
                _ = &mut superseded => return Err(LunaSamplerError::Superseded),
                lease = self.connections.lease() => lease,
            } {
                Ok(lease) => lease,
                Err(error) => {
                    if self
                        .retry_after_failure(&error, &mut auth_recovery, &mut retries)
                        .await
                    {
                        continue;
                    }
                    return Err(error);
                }
            };
            ensure_account_owner()?;
            self.request.service_tier = if lease.request_kind == RequestMode::GuardianClassifier {
                None
            } else {
                self.config.service_tier.clone()
            };
            let thread_id = &lease.thread_id;
            let mut turn_metadata = json!({
                "session_id": self.config.session_id,
                "thread_id": thread_id,
                "guardian_classifier_source_thread_id": self.config.thread_id,
                "turn_id": self.turn_id,
                "parent_turn_id": self.parent_turn_id,
                "thread_source": "guardian_classifier",
                "turn_trigger": "guardian_classifier",
            });
            let mut client_metadata = HashMap::from([
                ("session_id".to_owned(), self.config.session_id.clone()),
                ("thread_id".to_owned(), thread_id.clone()),
                ("turn_id".to_owned(), self.turn_id.clone()),
                ("parent_turn_id".to_owned(), self.parent_turn_id.clone()),
                ("x-openai-subagent".to_owned(), "guardian".to_owned()),
                // Classifier requests do not advance their own context window.
                ("x-codex-window-id".to_owned(), format!("{thread_id}:0")),
                (RESPONSES_LITE_METADATA_KEY.to_owned(), "true".to_owned()),
            ]);
            if let Some(root_turn_id) = &self.root_turn_id {
                client_metadata.insert("root_turn_id".to_owned(), root_turn_id.clone());
                turn_metadata["root_turn_id"] = json!(root_turn_id);
            }
            client_metadata.insert(TURN_METADATA_KEY.to_owned(), turn_metadata.to_string());
            if lease.request_kind == RequestMode::GuardianClassifier
                && let Some(parent_response_id) = &self.parent_response_id
            {
                client_metadata.insert("parent_response_id".to_owned(), parent_response_id.clone());
            }
            self.request.client_metadata = Some(client_metadata);
            let mut stream = match tokio::select! {
                biased;
                _ = &mut owner_changed => return Err(account_changed_error()),
                _ = &mut superseded => return Err(LunaSamplerError::Superseded),
                stream = lease.stream_request(&self.request) => stream,
            } {
                Ok(stream) => stream,
                Err(error) => {
                    let error = LunaSamplerError::Api(error);
                    if self
                        .retry_after_failure(&error, &mut auth_recovery, &mut retries)
                        .await
                    {
                        continue;
                    }
                    return Err(error);
                }
            };

            let mut output = String::new();
            while let Some(event) = tokio::select! {
                biased;
                _ = &mut owner_changed => return Err(account_changed_error()),
                _ = &mut superseded => {
                    return if scored.load(Ordering::Relaxed) && !output.is_empty() {
                        Ok(output)
                    } else {
                        Err(LunaSamplerError::Superseded)
                    };
                }
                event = stream.rx_event.recv() => event,
            } {
                ensure_account_owner()?;
                let event = match event {
                    Ok(event) => event,
                    Err(error) => {
                        let error = LunaSamplerError::Api(error);
                        if self
                            .retry_after_failure(&error, &mut auth_recovery, &mut retries)
                            .await
                        {
                            continue 'retry;
                        }
                        return Err(error);
                    }
                };
                match event {
                    ResponseEvent::OutputTextDelta(delta) => {
                        if delta.is_empty() {
                            continue;
                        }
                        if delta.len() > MAX_OUTPUT_BYTES {
                            return Err(LunaSamplerError::OutputTooLarge);
                        }
                        // The first output token is the complete classification.
                        // Later output cannot revise that decision; drain it only
                        // to preserve connection reuse and token accounting.
                        scored.store(true, Ordering::Relaxed);
                        let mut remaining_events = stream.rx_event;
                        let metrics = self.config.metrics.clone();
                        tokio::spawn(async move {
                            while let Some(event) = tokio::select! {
                                biased;
                                _ = &mut superseded => None,
                                event = remaining_events.recv() => event,
                            } {
                                match event {
                                    Ok(ResponseEvent::Completed { token_usage, .. }) => {
                                        record_token_usage(
                                            metrics.as_deref(),
                                            token_usage.as_ref(),
                                        );
                                        lease.reuse();
                                        break;
                                    }
                                    Err(_) => break,
                                    _ => {}
                                }
                            }
                        });
                        return Ok(delta);
                    }
                    ResponseEvent::OutputItemDone(ResponseItem::Message {
                        role, content, ..
                    }) if role == "assistant" => {
                        for item in content {
                            if let ContentItem::OutputText { text } = item {
                                output.push_str(&text);
                            }
                        }
                    }
                    ResponseEvent::Completed { token_usage, .. } => {
                        record_token_usage(self.config.metrics.as_deref(), token_usage.as_ref());
                        lease.reuse();
                        if !output.is_empty() {
                            return Ok(output);
                        }
                        return Err(LunaSamplerError::MissingOutput);
                    }
                    _ => {}
                }
                if output.len() > MAX_OUTPUT_BYTES {
                    return Err(LunaSamplerError::OutputTooLarge);
                }
                if !output.is_empty() {
                    scored.store(true, Ordering::Relaxed);
                }
            }
            return Err(LunaSamplerError::MissingOutput);
        }
    }
}

fn record_token_usage(metrics: Option<&dyn ExtensionMetrics>, token_usage: Option<&TokenUsage>) {
    let (Some(metrics), Some(token_usage)) = (metrics, token_usage) else {
        return;
    };

    for (token_type, value) in [
        ("total", token_usage.total_tokens.max(0)),
        ("input", token_usage.input_tokens.max(0)),
        ("cached_input", token_usage.cached_input()),
        (
            "cache_write_input",
            token_usage.cache_write_input_tokens.max(0),
        ),
        ("non_cached_input", token_usage.non_cached_input()),
        ("output", token_usage.output_tokens.max(0)),
        (
            "reasoning_output",
            token_usage.reasoning_output_tokens.max(0),
        ),
    ] {
        metrics.histogram(
            CLASSIFICATION_TOKEN_USAGE_METRIC,
            value,
            &[("token_type", token_type)],
        );
    }
}
