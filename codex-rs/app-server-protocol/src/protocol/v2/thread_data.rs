use super::CodexErrorInfo;
use super::ThreadEnvironment;
use super::ThreadItem;
use super::ThreadStatus;
use super::TurnStatus;
use crate::JsonSchema;
use crate::TS;
use codex_experimental_api_macros::ExperimentalApi;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::MisalignmentErrorDetails as CoreMisalignmentErrorDetails;
use codex_protocol::protocol::MisalignmentSteer as CoreMisalignmentSteer;
use codex_protocol::protocol::SessionSource as CoreSessionSource;
use codex_protocol::protocol::SubAgentSource as CoreSubAgentSource;
use codex_protocol::protocol::ThreadHistoryMode as CoreThreadHistoryMode;
use codex_protocol::protocol::ThreadSource as CoreThreadSource;
use codex_utils_absolute_path::AbsolutePathBuf;
#[cfg(test)]
use schemars::r#gen::SchemaGenerator;
#[cfg(test)]
use schemars::schema::Schema;
use serde::Deserialize;
use serde::Serialize;
use std::fmt;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase", export_to = "v2/")]
#[derive(Default)]
pub enum SessionSource {
    Cli,
    #[serde(rename = "vscode")]
    #[ts(rename = "vscode")]
    #[default]
    VsCode,
    Exec,
    AppServer,
    Custom(String),
    SubAgent(CoreSubAgentSource),
    #[serde(other)]
    Unknown,
}

impl From<CoreSessionSource> for SessionSource {
    fn from(value: CoreSessionSource) -> Self {
        match value {
            CoreSessionSource::Cli => SessionSource::Cli,
            CoreSessionSource::VSCode => SessionSource::VsCode,
            CoreSessionSource::Exec => SessionSource::Exec,
            CoreSessionSource::Mcp => SessionSource::AppServer,
            CoreSessionSource::Custom(source) => SessionSource::Custom(source),
            // We do not want to render those at the app-server level.
            CoreSessionSource::Internal(_) => SessionSource::Unknown,
            CoreSessionSource::SubAgent(sub) => SessionSource::SubAgent(sub),
            CoreSessionSource::Unknown => SessionSource::Unknown,
        }
    }
}

impl From<SessionSource> for CoreSessionSource {
    fn from(value: SessionSource) -> Self {
        match value {
            SessionSource::Cli => CoreSessionSource::Cli,
            SessionSource::VsCode => CoreSessionSource::VSCode,
            SessionSource::Exec => CoreSessionSource::Exec,
            SessionSource::AppServer => CoreSessionSource::Mcp,
            SessionSource::Custom(source) => CoreSessionSource::Custom(source),
            SessionSource::SubAgent(sub) => CoreSessionSource::SubAgent(sub),
            SessionSource::Unknown => CoreSessionSource::Unknown,
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(rename_all = "lowercase", export_to = "v2/")]
pub enum ThreadHistoryMode {
    #[default]
    Legacy,
    Paginated,
}

impl From<CoreThreadHistoryMode> for ThreadHistoryMode {
    fn from(value: CoreThreadHistoryMode) -> Self {
        match value {
            CoreThreadHistoryMode::Legacy => Self::Legacy,
            CoreThreadHistoryMode::Paginated => Self::Paginated,
        }
    }
}

impl From<ThreadHistoryMode> for CoreThreadHistoryMode {
    fn from(value: ThreadHistoryMode) -> Self {
        match value {
            ThreadHistoryMode::Legacy => Self::Legacy,
            ThreadHistoryMode::Paginated => Self::Paginated,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, TS)]
#[serde(try_from = "String", into = "String")]
#[ts(type = "string")]
#[ts(export_to = "v2/")]
pub enum ThreadSource {
    User,
    Subagent,
    GuardianReview,
    Feature(String),
    MemoryConsolidation,
}

#[cfg(test)]
impl JsonSchema for ThreadSource {
    fn schema_name() -> String {
        "ThreadSource".to_string()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        String::json_schema(generator)
    }
}

impl TryFrom<String> for ThreadSource {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse::<CoreThreadSource>().map(Into::into)
    }
}

impl From<ThreadSource> for String {
    fn from(value: ThreadSource) -> Self {
        CoreThreadSource::from(value).into()
    }
}

impl From<CoreThreadSource> for ThreadSource {
    fn from(value: CoreThreadSource) -> Self {
        match value {
            CoreThreadSource::User => ThreadSource::User,
            CoreThreadSource::Subagent => ThreadSource::Subagent,
            CoreThreadSource::GuardianReview => ThreadSource::GuardianReview,
            CoreThreadSource::Feature(feature) => ThreadSource::Feature(feature),
            CoreThreadSource::MemoryConsolidation => ThreadSource::MemoryConsolidation,
        }
    }
}

impl From<ThreadSource> for CoreThreadSource {
    fn from(value: ThreadSource) -> Self {
        match value {
            ThreadSource::User => CoreThreadSource::User,
            ThreadSource::Subagent => CoreThreadSource::Subagent,
            ThreadSource::GuardianReview => CoreThreadSource::GuardianReview,
            ThreadSource::Feature(feature) => CoreThreadSource::Feature(feature),
            ThreadSource::MemoryConsolidation => CoreThreadSource::MemoryConsolidation,
        }
    }
}

/// Extra app-server data for a thread.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase", export_to = "v2/")]
pub struct ThreadExtra {}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct GitInfo {
    pub sha: Option<String>,
    pub branch: Option<String>,
    pub origin_url: Option<String>,
}

/// An independently persisted, user-visible thread section.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadSection {
    /// Opaque UUIDv7 identity that remains stable when the section is renamed.
    pub id: String,
    /// The current user-visible section name.
    pub name: String,
    /// Optional appearance synchronized across clients.
    #[serde(default)]
    pub appearance: Option<ThreadSectionAppearance>,
}

/// Extensible visual presentation for a custom thread section.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadSectionAppearance {
    pub icon: Option<String>,
    pub color: Option<String>,
}

#[derive(Serialize, Debug, Clone, PartialEq, JsonSchema, TS, ExperimentalApi)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct Thread {
    /// Identifier for this thread. Codex-generated thread IDs are UUIDv7.
    pub id: String,
    /// Current environments for a loaded thread, in priority order, primary first.
    /// `null` means the thread is not loaded or the server does not expose its selection.
    /// An empty list means no environments are selected. This does not report connection status.
    #[experimental("thread.environments")]
    #[serde(default)]
    pub environments: Option<Vec<ThreadEnvironment>>,
    /// Optional implementation-specific thread data.
    #[experimental("thread.extra")]
    pub extra: Option<ThreadExtra>,
    /// Session id shared by threads that belong to the same session tree.
    pub session_id: String,
    /// Source thread id when this thread was created by forking another thread.
    pub forked_from_id: Option<String>,
    /// The ID of the parent thread. This will only be set if this thread is a subagent.
    pub parent_thread_id: Option<String>,
    /// Usually the first user message in the thread, if available.
    pub preview: String,
    /// Whether the thread is ephemeral and should not be materialized on disk.
    pub ephemeral: bool,
    /// The independently persisted section selected for this thread, if any.
    #[serde(default)]
    pub section: Option<ThreadSection>,
    /// Unix timestamp in seconds when the thread entered its current section.
    #[serde(default)]
    #[ts(type = "number | null")]
    pub section_entered_at: Option<i64>,
    /// Canonical project assignment owned by app-server, if any.
    #[schemars(
        required,
        schema_with = "crate::protocol::serde_helpers::nullable_string_schema"
    )]
    pub project_id: Option<String>,
    /// Persisted thread history contract selected when this thread was created.
    #[serde(default)]
    pub history_mode: ThreadHistoryMode,
    /// Model provider used for this thread (for example, 'openai').
    pub model_provider: String,
    /// Current configured model when loaded, otherwise the latest persisted model.
    /// Null when unavailable. This is not per-turn execution telemetry.
    pub model: Option<String>,
    /// Current configured reasoning effort when loaded, otherwise the latest persisted effort.
    /// Null when unset or unavailable. This is not per-turn execution telemetry.
    pub reasoning_effort: Option<ReasoningEffort>,
    /// Unix timestamp (in seconds) when the thread was created.
    #[ts(type = "number")]
    pub created_at: i64,
    /// Unix timestamp (in seconds) when the thread was last updated.
    #[ts(type = "number")]
    pub updated_at: i64,
    /// Unix timestamp (in seconds) used for thread recency ordering.
    #[ts(type = "number | null")]
    pub recency_at: Option<i64>,
    /// Current runtime status for the thread.
    pub status: ThreadStatus,
    /// [UNSTABLE] Path to the thread on disk.
    pub path: Option<PathBuf>,
    /// Working directory captured for the thread.
    pub cwd: AbsolutePathBuf,
    /// Version of the CLI that created the thread.
    pub cli_version: String,
    /// Originator recorded when the thread was created, independent of its current client or executor.
    /// Null when the recorded originator is unavailable.
    pub originator: Option<String>,
    /// Origin of the thread (CLI, VSCode, codex exec, codex app-server, etc.).
    pub source: SessionSource,
    /// Whether the app server accepts direct turn input for this loaded thread.
    /// `None` means the capability is unavailable, such as for an unloaded stored thread.
    #[experimental("thread.canAcceptDirectInput")]
    pub can_accept_direct_input: Option<bool>,
    /// Optional analytics source classification for this thread.
    pub thread_source: Option<ThreadSource>,
    /// Optional random unique nickname assigned to an AgentControl-spawned sub-agent.
    pub agent_nickname: Option<String>,
    /// Optional role (agent_role) assigned to an AgentControl-spawned sub-agent.
    pub agent_role: Option<String>,
    /// Optional Git metadata captured when the thread was created.
    pub git_info: Option<GitInfo>,
    /// Optional user-facing thread title.
    pub name: Option<String>,
    /// Saved Daybreak choice, independent of turn execution. Null if unset.
    #[experimental("thread.daybreakEnabled")]
    pub daybreak_enabled: Option<bool>,
    /// Only populated on `thread/resume`, `thread/fork`, and `thread/read`
    /// (when `includeTurns` is true) responses.
    /// For all other responses and notifications returning a Thread,
    /// the turns field will be an empty list.
    pub turns: Vec<Turn>,
}

// TODO: Remove this compatibility decoder after app-server versions that omitted
// `projectId` have aged out of the supported TUI -> remote app-server version-skew window.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ThreadCompatibility {
    id: String,
    #[serde(default)]
    environments: Option<Vec<ThreadEnvironment>>,
    extra: Option<ThreadExtra>,
    session_id: String,
    forked_from_id: Option<String>,
    parent_thread_id: Option<String>,
    preview: String,
    ephemeral: bool,
    #[serde(default)]
    section: Option<ThreadSection>,
    #[serde(default)]
    section_entered_at: Option<i64>,
    #[serde(default)]
    project_id: Option<String>,
    #[serde(default)]
    history_mode: ThreadHistoryMode,
    model_provider: String,
    model: Option<String>,
    reasoning_effort: Option<ReasoningEffort>,
    created_at: i64,
    updated_at: i64,
    recency_at: Option<i64>,
    status: ThreadStatus,
    path: Option<PathBuf>,
    cwd: AbsolutePathBuf,
    cli_version: String,
    originator: Option<String>,
    source: SessionSource,
    can_accept_direct_input: Option<bool>,
    thread_source: Option<ThreadSource>,
    agent_nickname: Option<String>,
    agent_role: Option<String>,
    git_info: Option<GitInfo>,
    name: Option<String>,
    daybreak_enabled: Option<bool>,
    turns: Vec<Turn>,
}

impl<'de> Deserialize<'de> for Thread {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let thread = ThreadCompatibility::deserialize(deserializer)?;
        Ok(Self {
            id: thread.id,
            environments: thread.environments,
            extra: thread.extra,
            session_id: thread.session_id,
            forked_from_id: thread.forked_from_id,
            parent_thread_id: thread.parent_thread_id,
            preview: thread.preview,
            ephemeral: thread.ephemeral,
            section: thread.section,
            section_entered_at: thread.section_entered_at,
            project_id: thread.project_id,
            history_mode: thread.history_mode,
            model_provider: thread.model_provider,
            model: thread.model,
            reasoning_effort: thread.reasoning_effort,
            created_at: thread.created_at,
            updated_at: thread.updated_at,
            recency_at: thread.recency_at,
            status: thread.status,
            path: thread.path,
            cwd: thread.cwd,
            cli_version: thread.cli_version,
            originator: thread.originator,
            source: thread.source,
            can_accept_direct_input: thread.can_accept_direct_input,
            thread_source: thread.thread_source,
            agent_nickname: thread.agent_nickname,
            agent_role: thread.agent_role,
            git_info: thread.git_info,
            name: thread.name,
            daybreak_enabled: thread.daybreak_enabled,
            turns: thread.turns,
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct Turn {
    /// Identifier for this turn. Codex-generated turn IDs are UUIDv7.
    pub id: String,
    /// Thread items currently included in this turn payload.
    pub items: Vec<ThreadItem>,
    /// Describes how much of `items` has been loaded for this turn.
    #[serde(default)]
    pub items_view: TurnItemsView,
    pub status: TurnStatus,
    /// Only populated when the Turn's status is failed.
    pub error: Option<TurnError>,
    /// Unix timestamp (in seconds) when the turn started.
    #[ts(type = "number | null")]
    pub started_at: Option<i64>,
    /// Unix timestamp (in seconds) when the turn completed.
    #[ts(type = "number | null")]
    pub completed_at: Option<i64>,
    /// Duration between turn start and completion in milliseconds, if known.
    #[ts(type = "number | null")]
    pub duration_ms: Option<i64>,
}

#[derive(Default, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum TurnItemsView {
    /// `items` was not loaded for this turn. The field is intentionally empty.
    NotLoaded,
    /// `items` contains only a display summary for this turn.
    Summary,
    /// `items` contains every ThreadItem available from persisted app-server history for this turn.
    #[default]
    Full,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS, Error)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
#[error("{message}")]
pub struct TurnError {
    pub message: String,
    pub codex_error_info: Option<CodexErrorInfo>,
    #[serde(default)]
    pub additional_details: Option<String>,
    /// Optional public explanation and continuation instruction for a misalignment block.
    #[serde(default)]
    pub misalignment: Option<MisalignmentErrorDetails>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct MisalignmentErrorDetails {
    /// Open-ended classification; clients must accept categories added by Responses.
    pub error_type: Option<String>,
    /// A substantive localized explanation is required before offering continuation.
    pub detailed_explanation: Option<String>,
    /// Instruction to submit as the next turn's user input if continuation is confirmed.
    pub steer: Option<MisalignmentSteer>,
}

impl fmt::Debug for MisalignmentErrorDetails {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MisalignmentErrorDetails")
            .field("error_type", &self.error_type)
            .field(
                "has_detailed_explanation",
                &self.detailed_explanation.is_some(),
            )
            .field("has_steer", &self.steer.is_some())
            .finish()
    }
}

impl From<CoreMisalignmentErrorDetails> for MisalignmentErrorDetails {
    fn from(value: CoreMisalignmentErrorDetails) -> Self {
        Self {
            error_type: value.error_type,
            detailed_explanation: value.detailed_explanation,
            steer: value.steer.map(Into::into),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct MisalignmentSteer {
    pub message: String,
}

impl fmt::Debug for MisalignmentSteer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MisalignmentSteer")
            .field("message", &"[REDACTED]")
            .finish()
    }
}

impl From<CoreMisalignmentSteer> for MisalignmentSteer {
    fn from(value: CoreMisalignmentSteer) -> Self {
        Self {
            message: value.message,
        }
    }
}
