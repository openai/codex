use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use codex_core::cross_session::AssistantMessage;
use codex_core::cross_session::CrossSessionHub;
use serde::Serialize;
use serde_json::Value;

use crate::progress::ProgressReporter;
use crate::roles::Role;
use crate::roles::parse_json_struct;
use crate::session;
use crate::signals::VerifierVerdict;

#[derive(Serialize)]
pub struct VerificationRequestPayload<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    pub claim_path: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub objective: Option<&'a str>,
}

impl<'a> VerificationRequestPayload<'a> {
    pub fn new(claim_path: &'a str, notes: Option<&'a str>, objective: Option<&'a str>) -> Self {
        Self {
            kind: "verification_request",
            claim_path,
            notes,
            objective,
        }
    }
}

pub struct VerifierRole {
    hub: Arc<CrossSessionHub>,
    run_id: String,
    role: String,
    timeout: Duration,
    progress: Option<Arc<dyn ProgressReporter>>,
}

impl VerifierRole {
    pub fn new(
        hub: Arc<CrossSessionHub>,
        run_id: impl Into<String>,
        role: impl Into<String>,
        timeout: Duration,
        progress: Option<Arc<dyn ProgressReporter>>,
    ) -> Self {
        Self {
            hub,
            run_id: run_id.into(),
            role: role.into(),
            timeout,
            progress,
        }
    }

    pub fn role(&self) -> &str {
        &self.role
    }

    pub fn response_schema() -> Value {
        serde_json::json!({
            "type": "object",
            "required": ["verdict", "reasons", "suggestions"],
            "properties": {
                "verdict": { "type": "string", "enum": ["pass", "fail"] },
                "reasons": { "type": "array", "items": { "type": "string" } },
                "suggestions": { "type": "array", "items": { "type": "string" } }
            },
            "additionalProperties": false
        })
    }
}

impl Role<VerificationRequestPayload<'_>, VerifierVerdict> for VerifierRole {
    fn call<'a>(
        &'a self,
        req: &'a VerificationRequestPayload<'a>,
    ) -> futures::future::BoxFuture<'a, Result<VerifierVerdict>> {
        Box::pin(async move {
            let request_text = serde_json::to_string_pretty(req)?;
            let handle = session::post_turn(
                self.hub.as_ref(),
                &self.run_id,
                &self.role,
                request_text,
                Some(Self::response_schema()),
            )
            .await?;
            let progress = self
                .progress
                .as_deref()
                .map(|reporter| (reporter, self.role.as_str()));
            let response: AssistantMessage =
                session::await_first_idle(self.hub.as_ref(), &handle, self.timeout, progress)
                    .await?;
            parse_json_struct(&response.message.message)
        })
    }
}

pub fn aggregate_verdicts(items: Vec<(String, VerifierVerdict)>) -> AggregatedVerifierVerdict {
    let mut overall = VerifierDecision::Pass;
    let mut verdicts = Vec::with_capacity(items.len());

    for (role, verdict) in items {
        if !verdict.verdict.is_pass() {
            overall = VerifierDecision::Fail;
        }
        verdicts.push(VerifierReport {
            role,
            verdict: verdict.verdict,
            reasons: verdict.reasons,
            suggestions: verdict.suggestions,
        });
    }

    AggregatedVerifierVerdict {
        kind: "verification_feedback",
        overall,
        verdicts,
    }
}
use crate::signals::AggregatedVerifierVerdict;
use crate::signals::VerifierDecision;
use crate::signals::VerifierReport;
