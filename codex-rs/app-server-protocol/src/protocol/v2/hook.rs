use super::shared::v2_enum_from_core;
use codex_protocol::protocol::HookEventName as CoreHookEventName;
use codex_protocol::protocol::HookExecutionMode as CoreHookExecutionMode;
use codex_protocol::protocol::HookHandlerType as CoreHookHandlerType;
use codex_protocol::protocol::HookOutputEntry as CoreHookOutputEntry;
use codex_protocol::protocol::HookOutputEntryKind as CoreHookOutputEntryKind;
use codex_protocol::protocol::HookRunStatus as CoreHookRunStatus;
use codex_protocol::protocol::HookRunSummary as CoreHookRunSummary;
use codex_protocol::protocol::HookScope as CoreHookScope;
use codex_protocol::protocol::HookSource as CoreHookSource;
use codex_protocol::protocol::HookTrustStatus as CoreHookTrustStatus;
use codex_utils_absolute_path::AbsolutePathBuf;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

v2_enum_from_core!(
    pub enum HookEventName from CoreHookEventName {
        PreToolUse, PermissionRequest, PostToolUse, SessionStart, UserPromptSubmit, Stop
    }
);

v2_enum_from_core!(
    pub enum HookHandlerType from CoreHookHandlerType {
        Command, Prompt, Agent
    }
);

v2_enum_from_core!(
    pub enum HookExecutionMode from CoreHookExecutionMode {
        Sync, Async
    }
);

v2_enum_from_core!(
    pub enum HookScope from CoreHookScope {
        Thread, Turn
    }
);

v2_enum_from_core!(
    pub enum HookSource from CoreHookSource {
        System,
        User,
        Project,
        Mdm,
        SessionFlags,
        Plugin,
        CloudRequirements,
        LegacyManagedConfigFile,
        LegacyManagedConfigMdm,
        Unknown,
    }
);

v2_enum_from_core!(
    pub enum HookTrustStatus from CoreHookTrustStatus {
        Managed, Untrusted, Trusted, Modified
    }
);

fn default_hook_source() -> HookSource {
    HookSource::Unknown
}

v2_enum_from_core!(
    pub enum HookRunStatus from CoreHookRunStatus {
        Running, Completed, Failed, Blocked, Stopped
    }
);

v2_enum_from_core!(
    pub enum HookOutputEntryKind from CoreHookOutputEntryKind {
        Warning, Stop, Feedback, Context, Error
    }
);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct HookOutputEntry {
    pub kind: HookOutputEntryKind,
    pub text: String,
}

impl From<CoreHookOutputEntry> for HookOutputEntry {
    fn from(value: CoreHookOutputEntry) -> Self {
        Self {
            kind: value.kind.into(),
            text: value.text,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct HookRunSummary {
    pub id: String,
    pub event_name: HookEventName,
    pub handler_type: HookHandlerType,
    pub execution_mode: HookExecutionMode,
    pub scope: HookScope,
    pub source_path: AbsolutePathBuf,
    #[serde(default = "default_hook_source")]
    pub source: HookSource,
    pub display_order: i64,
    pub status: HookRunStatus,
    pub status_message: Option<String>,
    pub started_at: i64,
    pub completed_at: Option<i64>,
    pub duration_ms: Option<i64>,
    pub entries: Vec<HookOutputEntry>,
}

impl From<CoreHookRunSummary> for HookRunSummary {
    fn from(value: CoreHookRunSummary) -> Self {
        Self {
            id: value.id,
            event_name: value.event_name.into(),
            handler_type: value.handler_type.into(),
            execution_mode: value.execution_mode.into(),
            scope: value.scope.into(),
            source_path: value.source_path,
            source: value.source.into(),
            display_order: value.display_order,
            status: value.status.into(),
            status_message: value.status_message,
            started_at: value.started_at,
            completed_at: value.completed_at,
            duration_ms: value.duration_ms,
            entries: value.entries.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct HookStartedNotification {
    pub thread_id: String,
    pub turn_id: Option<String>,
    pub run: HookRunSummary,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct HookCompletedNotification {
    pub thread_id: String,
    pub turn_id: Option<String>,
    pub run: HookRunSummary,
}
