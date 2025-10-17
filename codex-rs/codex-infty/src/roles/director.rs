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
use crate::signals::DirectiveResponse;

#[derive(Serialize)]
pub struct DirectionRequestPayload<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    pub prompt: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub objective: Option<&'a str>,
}

impl<'a> DirectionRequestPayload<'a> {
    pub fn new(prompt: &'a str, objective: Option<&'a str>) -> Self {
        Self {
            kind: "direction_request",
            prompt,
            objective,
        }
    }
}

pub struct DirectorRole {
    hub: Arc<CrossSessionHub>,
    run_id: String,
    role: String,
    timeout: Duration,
    progress: Option<Arc<dyn ProgressReporter>>,
}

impl DirectorRole {
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

    pub fn response_schema() -> Value {
        serde_json::json!({
            "type": "object",
            "required": ["directive", "rationale"],
            "properties": {
                "directive": { "type": "string" },
                "rationale": { "type": ["string", "null"] }
            },
            "additionalProperties": false
        })
    }
}

impl Role<DirectionRequestPayload<'_>, DirectiveResponse> for DirectorRole {
    fn call<'a>(
        &'a self,
        req: &'a DirectionRequestPayload<'a>,
    ) -> futures::future::BoxFuture<'a, Result<DirectiveResponse>> {
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
