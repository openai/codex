use std::collections::BTreeMap;
use std::collections::HashMap;

use codex_analytics::CompactionImplementation;
use codex_analytics::CompactionPhase;
use codex_analytics::CompactionReason;
use codex_analytics::CompactionStrategy;
use codex_analytics::CompactionTrigger;
use codex_protocol::ThreadId;
use codex_protocol::protocol::InternalSessionSource;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::ThreadSource;
use codex_utils_string::to_ascii_json_string;
use http::HeaderMap as ApiHeaderMap;
use http::HeaderValue;
use serde::Serialize;
use serde_json::Value;

use crate::client::X_CODEX_INSTALLATION_ID_HEADER;
use crate::client::X_CODEX_PARENT_THREAD_ID_HEADER;
use crate::client::X_CODEX_TURN_METADATA_HEADER;
use crate::client::X_CODEX_WINDOW_ID_HEADER;
use crate::client::X_OPENAI_SUBAGENT_HEADER;

pub(crate) const INSTALLATION_ID_KEY: &str = "installation_id";
pub(crate) const SESSION_ID_KEY: &str = "session_id";
pub(crate) const THREAD_ID_KEY: &str = "thread_id";
pub(crate) const TURN_ID_KEY: &str = "turn_id";
pub(crate) const WINDOW_ID_KEY: &str = "window_id";
pub(crate) const REQUEST_KIND_KEY: &str = "request_kind";
pub(crate) const COMPACTION_KEY: &str = "compaction";
pub(crate) const TURN_STARTED_AT_UNIX_MS_KEY: &str = "turn_started_at_unix_ms";

pub(crate) const FORKED_FROM_THREAD_ID_KEY: &str = "forked_from_thread_id";
pub(crate) const PARENT_THREAD_ID_KEY: &str = "parent_thread_id";
pub(crate) const SUBAGENT_KIND_KEY: &str = "subagent_kind";
pub(crate) const THREAD_SOURCE_KEY: &str = "thread_source";
pub(crate) const SANDBOX_KEY: &str = "sandbox";
pub(crate) const WORKSPACES_KEY: &str = "workspaces";

// App-server clients can specify additional metadata in the `responsesapi_client_metadata` param
// when submitting a turn, but they must not override fields owned by core.
const RESERVED_METADATA_KEYS: &[&str] = &[
    INSTALLATION_ID_KEY,
    X_CODEX_INSTALLATION_ID_HEADER,
    SESSION_ID_KEY,
    THREAD_ID_KEY,
    TURN_ID_KEY,
    WINDOW_ID_KEY,
    X_CODEX_WINDOW_ID_HEADER,
    X_CODEX_TURN_METADATA_HEADER,
    X_CODEX_PARENT_THREAD_ID_HEADER,
    X_OPENAI_SUBAGENT_HEADER,
    REQUEST_KIND_KEY,
    COMPACTION_KEY,
    TURN_STARTED_AT_UNIX_MS_KEY,
    FORKED_FROM_THREAD_ID_KEY,
    PARENT_THREAD_ID_KEY,
    SUBAGENT_KIND_KEY,
    THREAD_SOURCE_KEY,
    SANDBOX_KEY,
    WORKSPACES_KEY,
];

/// Metadata attached to model requests whose purpose is conversation compaction.
///
/// This covers both local compaction requests sent through the normal `/responses` path and remote
/// compaction requests sent through `/responses/compact`. These fields describe the operation at
/// dispatch time. Post-response outcomes such as status, error, duration, and token deltas remain
/// in compaction analytics events.
#[derive(Clone, Copy, Debug, Serialize)]
pub(crate) struct CompactionTurnMetadata {
    trigger: CompactionTrigger,
    reason: CompactionReason,
    implementation: CompactionImplementation,
    phase: CompactionPhase,
    strategy: CompactionStrategy,
}

impl CompactionTurnMetadata {
    pub(crate) fn new(
        trigger: CompactionTrigger,
        reason: CompactionReason,
        implementation: CompactionImplementation,
        phase: CompactionPhase,
    ) -> Self {
        Self {
            trigger,
            reason,
            implementation,
            phase,
            strategy: CompactionStrategy::Memento,
        }
    }

    pub(crate) fn trigger(self) -> CompactionTrigger {
        self.trigger
    }

    pub(crate) fn reason(self) -> CompactionReason {
        self.reason
    }

    pub(crate) fn implementation(self) -> CompactionImplementation {
        self.implementation
    }

    pub(crate) fn phase(self) -> CompactionPhase {
        self.phase
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum CodexResponsesRequestKind {
    Turn,
    Prewarm,
    Compaction(CompactionTurnMetadata),
    Memory,
}

impl CodexResponsesRequestKind {
    fn metadata(self) -> (&'static str, Option<CompactionTurnMetadata>) {
        match self {
            CodexResponsesRequestKind::Turn => ("turn", None),
            CodexResponsesRequestKind::Prewarm => ("prewarm", None),
            CodexResponsesRequestKind::Compaction(metadata) => ("compaction", Some(metadata)),
            CodexResponsesRequestKind::Memory => ("memory", None),
        }
    }

    fn has_turn_identity(self) -> bool {
        !matches!(self, CodexResponsesRequestKind::Memory)
    }
}

#[derive(Clone, Debug, Serialize, Default)]
pub(crate) struct TurnMetadataWorkspace {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) associated_remote_urls: Option<BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) latest_git_commit_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) has_changes: Option<bool>,
}

/// Caller-owned snapshot of Codex metadata sent to ResponsesAPI.
///
/// The full Codex turn metadata blob is transported canonically as
/// `client_metadata["x-codex-turn-metadata"]`. Flat `client_metadata` keys and direct HTTP/ws
/// headers are generated compatibility projections of this snapshot, not separate sources of
/// truth.
#[derive(Clone, Debug)]
pub struct CodexResponsesMetadata {
    pub(crate) installation_id: String,
    pub(crate) session_id: String,
    pub(crate) thread_id: String,
    pub(crate) turn_id: Option<String>,
    pub(crate) window_id: String,
    pub(crate) request_kind: Option<CodexResponsesRequestKind>,
    pub(crate) forked_from_thread_id: Option<ThreadId>,
    pub(crate) parent_thread_id: Option<ThreadId>,
    pub(crate) subagent_header: Option<String>,
    pub(crate) subagent_kind: Option<String>,
    pub(crate) thread_source: Option<ThreadSource>,
    pub(crate) sandbox: Option<String>,
    pub(crate) workspaces: BTreeMap<String, TurnMetadataWorkspace>,
    pub(crate) turn_started_at_unix_ms: Option<i64>,
    pub(crate) extra: BTreeMap<String, String>,
}

impl CodexResponsesMetadata {
    pub(crate) fn new(
        installation_id: String,
        session_id: String,
        thread_id: String,
        window_id: String,
    ) -> Self {
        Self {
            installation_id,
            session_id,
            thread_id,
            turn_id: None,
            window_id,
            request_kind: None,
            forked_from_thread_id: None,
            parent_thread_id: None,
            subagent_header: None,
            subagent_kind: None,
            thread_source: None,
            sandbox: None,
            workspaces: BTreeMap::new(),
            turn_started_at_unix_ms: None,
            extra: BTreeMap::new(),
        }
    }

    pub(crate) fn has_turn_metadata(&self) -> bool {
        self.request_kind.is_some()
    }

    pub(crate) fn turn_metadata_json(&self) -> Option<String> {
        to_ascii_json_string(&self.turn_metadata_payload()).ok()
    }

    pub(crate) fn turn_metadata_value(&self) -> Option<Value> {
        serde_json::to_value(self.turn_metadata_payload()).ok()
    }

    pub(crate) fn client_metadata(&self) -> HashMap<String, String> {
        let mut client_metadata = HashMap::from([
            (
                X_CODEX_INSTALLATION_ID_HEADER.to_string(),
                self.installation_id.clone(),
            ),
            (SESSION_ID_KEY.to_string(), self.session_id.clone()),
            (THREAD_ID_KEY.to_string(), self.thread_id.clone()),
            (X_CODEX_WINDOW_ID_HEADER.to_string(), self.window_id.clone()),
        ]);
        if let Some(turn_id) = &self.turn_id {
            client_metadata.insert(TURN_ID_KEY.to_string(), turn_id.clone());
        }
        if let Some(subagent_header) = &self.subagent_header {
            client_metadata.insert(
                X_OPENAI_SUBAGENT_HEADER.to_string(),
                subagent_header.clone(),
            );
        }
        if let Some(parent_thread_id) = self.parent_thread_id {
            client_metadata.insert(
                X_CODEX_PARENT_THREAD_ID_HEADER.to_string(),
                parent_thread_id.to_string(),
            );
        }
        if self.has_turn_metadata()
            && let Some(turn_metadata_json) = self.turn_metadata_json()
        {
            client_metadata.insert(X_CODEX_TURN_METADATA_HEADER.to_string(), turn_metadata_json);
        }
        client_metadata
    }

    pub(crate) fn compatibility_headers(&self) -> ApiHeaderMap {
        let mut headers = ApiHeaderMap::new();
        insert_header(&mut headers, X_CODEX_WINDOW_ID_HEADER, &self.window_id);
        // Direct x-codex-turn-metadata is compatibility output. New per-request consumers should
        // prefer client_metadata["x-codex-turn-metadata"], which is rendered from this same object.
        if self.has_turn_metadata()
            && let Some(turn_metadata_json) = self.turn_metadata_json()
        {
            insert_header(
                &mut headers,
                X_CODEX_TURN_METADATA_HEADER,
                &turn_metadata_json,
            );
        }
        if let Some(parent_thread_id) = self.parent_thread_id {
            insert_header(
                &mut headers,
                X_CODEX_PARENT_THREAD_ID_HEADER,
                &parent_thread_id.to_string(),
            );
        }
        if let Some(subagent_header) = &self.subagent_header {
            insert_header(&mut headers, X_OPENAI_SUBAGENT_HEADER, subagent_header);
        }
        headers
    }

    fn turn_metadata_payload(&self) -> CodexTurnMetadataPayload<'_> {
        let request_kind = self.request_kind;
        let (request_kind_value, compaction) = request_kind.map_or((None, None), |request_kind| {
            let (request_kind, compaction) = request_kind.metadata();
            (Some(request_kind), compaction)
        });
        let has_turn_identity =
            request_kind.is_none_or(CodexResponsesRequestKind::has_turn_identity);
        let has_request_identity =
            request_kind.is_some_and(CodexResponsesRequestKind::has_turn_identity);
        CodexTurnMetadataPayload {
            installation_id: has_request_identity.then_some(self.installation_id.as_str()),
            session_id: has_turn_identity.then_some(self.session_id.as_str()),
            thread_id: has_turn_identity.then_some(self.thread_id.as_str()),
            turn_id: has_turn_identity
                .then_some(self.turn_id.as_deref())
                .flatten(),
            window_id: has_request_identity.then_some(self.window_id.as_str()),
            request_kind: request_kind_value,
            forked_from_thread_id: self.forked_from_thread_id,
            parent_thread_id: self.parent_thread_id,
            subagent_kind: self.subagent_kind.as_deref(),
            thread_source: self.thread_source.as_ref(),
            sandbox: self.sandbox.as_deref(),
            workspaces: non_empty_workspaces(&self.workspaces),
            turn_started_at_unix_ms: self.turn_started_at_unix_ms,
            compaction,
            // responsesapi_client_metadata enriches the Codex turn metadata blob, not literal
            // top-level Responses client_metadata. Reserved Codex-owned keys are filtered when
            // these extras enter turn state.
            extra: &self.extra,
        }
    }
}

pub(crate) fn subagent_header_value(session_source: &SessionSource) -> Option<String> {
    match session_source {
        SessionSource::SubAgent(subagent_source) => match subagent_source {
            SubAgentSource::Review => Some("review".to_string()),
            SubAgentSource::Compact => Some("compact".to_string()),
            SubAgentSource::MemoryConsolidation => Some("memory_consolidation".to_string()),
            SubAgentSource::ThreadSpawn { .. } => Some("collab_spawn".to_string()),
            SubAgentSource::Other(label) => Some(label.clone()),
        },
        SessionSource::Internal(InternalSessionSource::MemoryConsolidation) => {
            Some("memory_consolidation".to_string())
        }
        SessionSource::Cli
        | SessionSource::VSCode
        | SessionSource::Exec
        | SessionSource::Mcp
        | SessionSource::Custom(_)
        | SessionSource::Unknown => None,
    }
}

pub(crate) fn subagent_metadata_kind(session_source: &SessionSource) -> Option<String> {
    match session_source {
        SessionSource::SubAgent(subagent_source) => Some(subagent_source.kind().to_string()),
        SessionSource::Cli
        | SessionSource::VSCode
        | SessionSource::Exec
        | SessionSource::Mcp
        | SessionSource::Custom(_)
        | SessionSource::Internal(_)
        | SessionSource::Unknown => None,
    }
}

fn insert_header(headers: &mut ApiHeaderMap, name: &'static str, value: &str) {
    if let Ok(header_value) = HeaderValue::from_str(value) {
        headers.insert(name, header_value);
    }
}

pub(crate) fn filter_extra_metadata(extra: HashMap<String, String>) -> BTreeMap<String, String> {
    extra
        .into_iter()
        .filter(|(key, _)| !RESERVED_METADATA_KEYS.contains(&key.as_str()))
        .collect()
}

fn non_empty_workspaces(
    workspaces: &BTreeMap<String, TurnMetadataWorkspace>,
) -> Option<&BTreeMap<String, TurnMetadataWorkspace>> {
    (!workspaces.is_empty()).then_some(workspaces)
}

#[derive(Serialize)]
struct CodexTurnMetadataPayload<'a> {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    installation_id: Option<&'a str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    session_id: Option<&'a str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    thread_id: Option<&'a str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    turn_id: Option<&'a str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    window_id: Option<&'a str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    request_kind: Option<&'static str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    forked_from_thread_id: Option<ThreadId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    parent_thread_id: Option<ThreadId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    subagent_kind: Option<&'a str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    thread_source: Option<&'a ThreadSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sandbox: Option<&'a str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    workspaces: Option<&'a BTreeMap<String, TurnMetadataWorkspace>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    turn_started_at_unix_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    compaction: Option<CompactionTurnMetadata>,
    #[serde(flatten)]
    extra: &'a BTreeMap<String, String>,
}
