//! Bounded Decisions transport for the optional Guardian comparison classifier.
//! Admission retains the newest requests and cancels the oldest unfinished request at capacity.
//! Comparisons omit reasoning and compaction.
//! Other unsupported evidence is rejected intact.
//! Errors never contain credentials or wire bodies.
//! Evidence preserves message roles, content order, and boundaries.
//! The caller records measurements after baseline publication; dropping its task aborts Decisions work.

use codex_history::ResponseItemEnvelope;
use codex_http_client::HttpClient;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ImageReference;
use codex_protocol::models::ResponseItem;
use serde_json::Value;
use serde_json::json;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::Weak;
use std::time::Duration;
use std::time::Instant;
use thiserror::Error;
use tokio::sync::Semaphore;
use tokio::task::AbortHandle;
use tokio::task::JoinHandle;

const MAX_CONCURRENT_REQUESTS: usize = 16;
pub(super) const URL: &str = "https://api.openai.com/v1/decisions";
const MODEL: &str = "gpt-6-luna";
// Provisional local rollout budgets, not Decisions API limits or model context limits.
// Guardian owns the text/context budget; bound HTTP lifetime, image bytes and response buffering here.
// Oversized images skip the complete request; transport failures never affect approvals.
const DEADLINE: Duration = Duration::from_secs(6);
const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;
const MAX_RESPONSE_BYTES: usize = 16 * 1024;

#[derive(Clone, Copy, Debug, Error, PartialEq)]
pub(super) enum DecisionsError {
    #[error("Decisions credentials are missing or empty")]
    Credentials,
    #[error("Decisions HTTP client setup failed")]
    ClientSetup,
    #[error("Decisions received unexpected classifier request setup")]
    UnexpectedRequestSetup,
    #[error("Decisions cannot represent this input item")]
    UnsupportedInputItem,
    #[error("Decisions cannot represent this image evidence")]
    UnsupportedImage,
    #[error("Decisions cannot represent audio evidence")]
    UnsupportedAudio,
    #[error("Decisions input exceeds the request limit")]
    InputTooLarge,
    #[error("Decisions request timed out")]
    Timeout,
    #[error("Decisions transport failed")]
    Transport,
    #[error("Decisions returned HTTP {0}")]
    Http(u16),
    #[error("Decisions response exceeds the response limit")]
    ResponseTooLarge,
    #[error("Decisions returned an invalid classification")]
    InvalidResponse,
}

pub(super) struct DecisionsSampler {
    client: HttpClient,
    // No Debug implementation: credentials must not reach diagnostics.
    api_key: String,
    url: String,
    slots: Semaphore,
    active_requests: Mutex<VecDeque<AbortHandle>>,
}

/// Owns the measurement until it is recorded after baseline publication.
/// Dropping the classification, including during the join, aborts unfinished work.
pub(super) struct DecisionsTask {
    handle: JoinHandle<(Result<&'static str, DecisionsError>, Duration)>,
    sampler: Weak<DecisionsSampler>,
}

impl Drop for DecisionsTask {
    fn drop(&mut self) {
        self.handle.abort();
        if let Some(sampler) = self.sampler.upgrade() {
            sampler
                .active_requests
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .retain(|task| task.id() != self.handle.id());
        }
    }
}

impl DecisionsTask {
    pub(super) async fn finish(
        mut self,
    ) -> Result<(Result<&'static str, DecisionsError>, Duration), tokio::task::JoinError> {
        (&mut self.handle).await
    }
}

impl DecisionsSampler {
    pub(super) fn new(
        client: HttpClient,
        api_key: String,
        url: String,
    ) -> Result<Self, DecisionsError> {
        if api_key.trim().is_empty() {
            return Err(DecisionsError::Credentials);
        }
        Ok(Self {
            client,
            api_key,
            url,
            // Keep HTTP concurrency bounded while evicted tasks release their permits.
            slots: Semaphore::new(MAX_CONCURRENT_REQUESTS),
            active_requests: Mutex::new(VecDeque::new()),
        })
    }

    pub(super) fn start_conversation_comparison(
        self: &Arc<Self>,
        input: &[ResponseItemEnvelope],
    ) -> DecisionsTask {
        let started = Instant::now();
        let body = (|| {
            let [tools, instructions, evidence @ ..] = input else {
                return Err(DecisionsError::UnexpectedRequestSetup);
            };
            if !matches!(
                &tools.item,
                ResponseItem::AdditionalTools { role, tools, .. }
                    if role == "developer" && tools.is_empty()
            ) {
                return Err(DecisionsError::UnexpectedRequestSetup);
            }
            let ResponseItem::Message { role, content, .. } = &instructions.item else {
                return Err(DecisionsError::UnexpectedRequestSetup);
            };
            let [rubric] = content.as_slice() else {
                return Err(DecisionsError::UnexpectedRequestSetup);
            };
            if role != "developer" {
                return Err(DecisionsError::UnexpectedRequestSetup);
            }
            request_body(rubric, evidence.iter().map(|item| &item.item))
        })();
        self.spawn_request(body, started)
    }

    fn spawn_request(
        self: &Arc<Self>,
        body: Result<Value, DecisionsError>,
        started: Instant,
    ) -> DecisionsTask {
        let body = match body {
            Ok(body) => body,
            Err(error) => {
                return DecisionsTask {
                    handle: tokio::spawn(async move { (Err(error), started.elapsed()) }),
                    sampler: Weak::new(),
                };
            }
        };
        let mut active = self
            .active_requests
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        active.retain(|task| !task.is_finished());
        if active.len() == MAX_CONCURRENT_REQUESTS
            && let Some(oldest) = active.pop_front()
        {
            oldest.abort();
        }
        let sampler = Arc::clone(self);
        let handle = tokio::spawn(async move {
            let Ok(_permit) = sampler.slots.acquire().await else {
                return (Err(DecisionsError::Transport), started.elapsed());
            };
            let result = tokio::time::timeout(DEADLINE, sampler.request(body))
                .await
                .unwrap_or(Err(DecisionsError::Timeout));
            (result, started.elapsed())
        });
        active.push_back(handle.abort_handle());
        DecisionsTask {
            handle,
            sampler: Arc::downgrade(self),
        }
    }

    async fn request(&self, body: Value) -> Result<&'static str, DecisionsError> {
        let request = self
            .client
            .post(&self.url)
            .bearer_auth(&self.api_key)
            .json(&body);
        drop(body);
        let mut response = request
            .send()
            .await
            .map_err(|_| DecisionsError::Transport)?;
        if !response.status().is_success() {
            return Err(DecisionsError::Http(response.status().as_u16()));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| DecisionsError::Transport)?
        {
            if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
                return Err(DecisionsError::ResponseTooLarge);
            }
            bytes.extend_from_slice(&chunk);
        }
        parse_answer(&serde_json::from_slice(&bytes).map_err(|_| DecisionsError::InvalidResponse)?)
    }
}

fn request_body<'a>(
    instructions: &ContentItem,
    evidence: impl IntoIterator<Item = &'a ResponseItem>,
) -> Result<Value, DecisionsError> {
    let ContentItem::InputText { text: rubric } = instructions else {
        return Err(DecisionsError::UnexpectedRequestSetup);
    };
    let mut image_bytes = 0usize;
    let mut messages = Vec::new();
    for item in evidence {
        if matches!(
            item,
            ResponseItem::Reasoning { .. }
                | ResponseItem::Compaction { .. }
                | ResponseItem::ContextCompaction { .. }
        ) {
            continue;
        }
        let ResponseItem::Message {
            role,
            content,
            phase,
            ..
        } = item
        else {
            return Err(DecisionsError::UnsupportedInputItem);
        };
        let mut parts = Vec::with_capacity(content.len());
        for part in content {
            match part {
                ContentItem::InputText { .. } | ContentItem::OutputText { .. } => {
                    parts.push(json!(part));
                }
                ContentItem::InputImage {
                    image: ImageReference::Inline { image_url },
                    ..
                } if role == "user" && image_url.starts_with("data:") => {
                    image_bytes = image_bytes.saturating_add(image_url.len());
                    if image_bytes > MAX_IMAGE_BYTES {
                        return Err(DecisionsError::InputTooLarge);
                    }
                    parts.push(
                        // Luna also clears image detail before sending evidence.
                        json!({"type": "input_image", "image_url": image_url, "detail": null}),
                    );
                }
                ContentItem::InputImage { .. } => {
                    return Err(DecisionsError::UnsupportedImage);
                }
                ContentItem::InputAudio { .. } => {
                    return Err(DecisionsError::UnsupportedAudio);
                }
            }
        }
        // Harness annotations/IDs are not part of the Decisions contract.
        let mut message = json!({"role": role});
        message["content"] = Value::Array(parts);
        if let Some(phase) = phase {
            message["phase"] = json!(phase);
        }
        messages.push(message);
    }
    let mut body = json!({
        "model": MODEL,
        "questions": [{"type": "choice", "name": "guardian_risk",
                       "instructions": rubric,
                       "choices": [{"value": "low"}, {"value": "high"}]}]
    });
    body["input"] = Value::Array(messages);
    Ok(body)
}

fn parse_answer(body: &Value) -> Result<&'static str, DecisionsError> {
    let invalid = DecisionsError::InvalidResponse;
    let answers = body["answers"].as_array().ok_or(invalid)?;
    if answers.len() != 1 {
        return Err(invalid);
    }
    let answer = &answers[0];
    if answer["type"] != "choice" || answer["name"] != "guardian_risk" {
        return Err(invalid);
    }
    let choice = match answer["choice"].as_str() {
        Some("low") => "low",
        Some("high") => "high",
        _ => return Err(invalid),
    };
    Ok(choice)
}

#[cfg(test)]
#[path = "decisions_tests.rs"]
mod tests;
