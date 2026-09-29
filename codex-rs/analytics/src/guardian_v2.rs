//! Guardian V2 classifier and fast-decision facts, enriched by the existing reducer.
//! Payloads contain attribution and bounded outcomes, never prompts or tool arguments.

use crate::events::CodexAppServerClientMetadata;
use crate::events::CodexRuntimeMetadata;
use codex_protocol::protocol::ThreadSource;
use serde::Serialize;

#[derive(Serialize)]
pub struct GuardianV2Event {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: Option<String>,
    pub model: Option<String>,
    pub occurred_at_ms: u64,
    #[serde(flatten)]
    pub kind: GuardianV2EventKind,
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum GuardianV2EventKind {
    Classification {
        guardian_context_mode: &'static str,
        outcome: &'static str,
        risk_level: Option<&'static str>,
        duration_ms: u64,
    },
    FastDecision {
        decision: &'static str,
    },
}

#[derive(Serialize)]
pub(crate) struct GuardianV2EventRequest {
    pub(crate) event_type: &'static str,
    pub(crate) event_params: GuardianV2EventParams,
}

#[derive(Serialize)]
pub(crate) struct GuardianV2EventParams {
    pub(crate) session_id: String,
    pub(crate) app_server_client: CodexAppServerClientMetadata,
    pub(crate) runtime: CodexRuntimeMetadata,
    pub(crate) thread_source: Option<ThreadSource>,
    pub(crate) subagent_source: Option<String>,
    pub(crate) parent_thread_id: Option<String>,
    #[serde(flatten)]
    pub(crate) guardian_v2: GuardianV2Event,
}
