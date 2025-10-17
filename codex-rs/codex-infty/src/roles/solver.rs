use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use codex_core::cross_session::AssistantMessage;
use codex_core::cross_session::CrossSessionHub;
use codex_core::cross_session::SessionEventStream;
use codex_protocol::ConversationId;
use serde::de::Error as _;
use serde_json::Value;

use crate::progress::ProgressReporter;
use crate::roles::Role;
use crate::session;
use crate::signals::AggregatedVerifierVerdict;
use crate::signals::DirectiveResponse;

pub struct SolverRole {
    hub: Arc<CrossSessionHub>,
    run_id: String,
    role: String,
    conversation_id: ConversationId,
    progress: Option<Arc<dyn ProgressReporter>>,
}

impl SolverRole {
    pub fn new(
        hub: Arc<CrossSessionHub>,
        run_id: impl Into<String>,
        role: impl Into<String>,
        conversation_id: ConversationId,
        progress: Option<Arc<dyn ProgressReporter>>,
    ) -> Self {
        Self {
            hub,
            run_id: run_id.into(),
            role: role.into(),
            conversation_id,
            progress,
        }
    }

    pub fn solver_signal_schema() -> Value {
        // Only allow asking the director or sending the final result.
        serde_json::json!({
            "type": "object",
            "properties": {
                "type": { "type": "string", "enum": ["direction_request", "final_delivery"] },
                "prompt": { "type": ["string", "null"] },
                "deliverable_path": { "type": ["string", "null"] },
                "summary": { "type": ["string", "null"] }
            },
            "required": ["type", "prompt", "deliverable_path", "summary"],
            "additionalProperties": false
        })
    }

    pub fn final_delivery_schema() -> Value {
        serde_json::json!({
            "type": "object",
            "required": ["type", "deliverable_path", "summary"],
            "properties": {
                "type": { "const": "final_delivery" },
                "deliverable_path": { "type": "string" },
                "summary": { "type": ["string", "null"] }
            },
            "additionalProperties": false
        })
    }

    pub async fn post(
        &self,
        text: impl Into<String>,
        final_output_json_schema: Option<Value>,
    ) -> Result<()> {
        let _ = session::post_turn(
            self.hub.as_ref(),
            &self.run_id,
            &self.role,
            text,
            final_output_json_schema,
        )
        .await?;
        Ok(())
    }

    pub fn stream_events(
        &self,
    ) -> Result<SessionEventStream, codex_core::cross_session::CrossSessionError> {
        self.hub.stream_events(self.conversation_id)
    }

    pub async fn request_finalization_signal(&self) -> Result<()> {
        let handle = session::post_turn(
            self.hub.as_ref(),
            &self.run_id,
            &self.role,
            crate::types::FINALIZATION_PROMPT,
            Some(Self::final_delivery_schema()),
        )
        .await?;
        // Allow more time for the solver to start emitting the
        // finalization signal before timing out as "idle".
        let _ =
            session::await_first_idle(self.hub.as_ref(), &handle, Duration::from_secs(120), None)
                .await?;
        Ok(())
    }
}

pub struct SolverPost {
    pub text: String,
    pub final_output_json_schema: Option<Value>,
    pub timeout: Duration,
}

pub enum SolverRequest {
    Directive(DirectiveResponse),
    VerificationSummary(AggregatedVerifierVerdict),
}

impl From<DirectiveResponse> for SolverRequest {
    fn from(d: DirectiveResponse) -> Self {
        SolverRequest::Directive(d)
    }
}

impl From<&AggregatedVerifierVerdict> for SolverRequest {
    fn from(v: &AggregatedVerifierVerdict) -> Self {
        SolverRequest::VerificationSummary(v.clone())
    }
}

impl SolverRequest {
    fn to_text(&self) -> Result<String> {
        match self {
            SolverRequest::Directive(d) => Ok(serde_json::to_string_pretty(d)?),
            SolverRequest::VerificationSummary(s) => Ok(serde_json::to_string_pretty(s)?),
        }
    }
}

impl Role<SolverPost, AssistantMessage> for SolverRole {
    fn call<'a>(
        &'a self,
        req: &'a SolverPost,
    ) -> futures::future::BoxFuture<'a, Result<AssistantMessage>> {
        Box::pin(async move {
            let handle = session::post_turn(
                self.hub.as_ref(),
                &self.run_id,
                &self.role,
                req.text.clone(),
                req.final_output_json_schema.clone(),
            )
            .await?;
            let progress = self
                .progress
                .as_deref()
                .map(|reporter| (reporter, self.role.as_str()));
            session::await_first_idle(self.hub.as_ref(), &handle, req.timeout, progress).await
        })
    }
}

impl Role<SolverRequest, ()> for SolverRole {
    fn call<'a>(&'a self, req: &'a SolverRequest) -> futures::future::BoxFuture<'a, Result<()>> {
        Box::pin(async move {
            let text = req.to_text()?;
            self.post(text, Some(Self::solver_signal_schema())).await
        })
    }
}

#[derive(Debug, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SolverSignal {
    DirectionRequest {
        #[serde(default)]
        prompt: Option<String>,
    },
    FinalDelivery {
        #[serde(default)]
        deliverable_path: Option<String>,
        #[serde(default)]
        summary: Option<String>,
    },
}

pub fn parse_solver_signal(message: &str) -> Option<SolverSignal> {
    let trimmed = message.trim();
    if trimmed.is_empty() {
        return None;
    }
    serde_json::from_str(trimmed)
        .or_else(|_| {
            crate::roles::strip_json_code_fence(trimmed)
                .map(|inner| serde_json::from_str(inner.trim()))
                .unwrap_or_else(|| Err(serde_json::Error::custom("invalid payload")))
        })
        .ok()
}
