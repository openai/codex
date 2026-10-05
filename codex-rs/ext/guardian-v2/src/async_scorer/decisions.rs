//! Bounded Decisions transport for the optional Guardian comparison classifier.
//! Admission retains the newest requests and cancels the oldest unfinished request at capacity.
//! Unsupported evidence is rejected intact; errors never contain credentials or wire bodies.
//! Only host-annotated trusted-tool evidence is appended to the developer-level rubric.
//! The caller records measurements after baseline publication; dropping its task aborts Decisions work.

use super::sampler::LunaSampler;
use super::sampler::LunaSamplingRequest;
use codex_context_fragments::RenderedFragment;
use codex_guardian_context::TrustedTool;
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
    #[error("Decisions cannot represent the complete classifier evidence")]
    UnsupportedEvidence,
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

    /// Retain the newest requests; the caller joins after baseline publication.
    pub(super) fn spawn(
        self: &Arc<Self>,
        request: &LunaSamplingRequest,
        max_input_tokens: usize,
    ) -> DecisionsTask {
        let started = Instant::now();
        let body = request_body(
            &request.instructions,
            &request.input,
            request.parent_compaction.as_ref(),
        )
        .and_then(|body| {
            // Reuse Guardian's complete budget without changing its sampling path.
            // The temporary copy is dropped before admission; Responses prepares again.
            let (_, tokens) = LunaSampler::prepare_input(
                request,
                /*history*/ None,
                request
                    .input
                    .iter()
                    .cloned()
                    .map(ResponseItemEnvelope::new)
                    .collect(),
            );
            if tokens > max_input_tokens.saturating_sub(/*rhs*/ 256) {
                return Err(DecisionsError::InputTooLarge);
            }
            Ok(body)
        });
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

fn request_body(
    instructions: &RenderedFragment,
    evidence: &[ResponseItem],
    parent_compaction: Option<&ResponseItem>,
) -> Result<Value, DecisionsError> {
    if parent_compaction.is_some() {
        return Err(DecisionsError::UnsupportedEvidence);
    }
    let ContentItem::InputText { text: rubric } = instructions.annotated_content().content() else {
        return Err(DecisionsError::UnsupportedEvidence);
    };
    let mut rubric = rubric.to_owned();
    let mut image_bytes = 0usize;
    let mut messages = Vec::new();
    for item in evidence {
        let ResponseItem::Message {
            role,
            content,
            internal_chat_message_metadata_passthrough: metadata,
            ..
        } = item
        else {
            return Err(DecisionsError::UnsupportedEvidence);
        };
        let kinds = metadata
            .as_ref()
            .and_then(|metadata| metadata.content_item_kinds.as_deref());
        // Decisions only accepts user input, so keep Codex's trusted-tool note in the rubric.
        // This preserves its developer role; other developer messages remain unsupported.
        match (role.as_str(), content.as_slice(), kinds) {
            ("developer", [ContentItem::InputText { text }], Some([kind]))
                if kind.0 == TrustedTool::KIND =>
            {
                rubric.push_str("\n\n");
                rubric.push_str(text);
                continue;
            }
            ("user", _, _) => {}
            _ => return Err(DecisionsError::UnsupportedEvidence),
        }
        let mut parts = Vec::new();
        for part in content {
            match part {
                ContentItem::InputText { text } => {
                    parts.push(json!({"type": "input_text", "text": text}));
                }
                ContentItem::InputImage {
                    image: ImageReference::Inline { image_url },
                    ..
                } if image_url.starts_with("data:") => {
                    image_bytes = image_bytes.saturating_add(image_url.len());
                    if image_bytes > MAX_IMAGE_BYTES {
                        return Err(DecisionsError::InputTooLarge);
                    }
                    parts.push(
                        // Luna also clears image detail before sending evidence.
                        json!({"type": "input_image", "image_url": image_url, "detail": null}),
                    );
                }
                ContentItem::InputImage { .. }
                | ContentItem::InputAudio { .. }
                | ContentItem::OutputText { .. } => {
                    return Err(DecisionsError::UnsupportedEvidence);
                }
            }
        }
        // Harness annotations/IDs are not part of the Decisions contract. Preserve every
        // user content part and message boundary.
        let mut message = json!({"role": "user"});
        message["content"] = Value::Array(parts);
        messages.push(message);
    }
    let mut body = json!({
        "model": MODEL,
        "questions": [{"type": "choice", "name": "guardian_risk",
                       "choices": [{"value": "low"}, {"value": "high"}]}]
    });
    body["questions"][0]["instructions"] = Value::String(rubric);
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
