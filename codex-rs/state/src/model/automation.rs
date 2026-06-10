use anyhow::Result;
use chrono::DateTime;
use chrono::Utc;
use codex_protocol::ThreadId;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::models::PermissionProfile;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::AskForApproval;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutomationKind {
    Cron,
    Heartbeat,
}

impl AutomationKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cron => "cron",
            Self::Heartbeat => "heartbeat",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "cron" => Ok(Self::Cron),
            "heartbeat" => Ok(Self::Heartbeat),
            _ => Err(anyhow::anyhow!("invalid automation kind: {value}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutomationStatus {
    Active,
    Paused,
}

impl AutomationStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "ACTIVE",
            Self::Paused => "PAUSED",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "ACTIVE" => Ok(Self::Active),
            "PAUSED" => Ok(Self::Paused),
            _ => Err(anyhow::anyhow!("invalid automation status: {value}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutomationTarget {
    Cron { cwds: Vec<PathBuf> },
    Heartbeat { thread_id: ThreadId },
}

impl AutomationTarget {
    pub fn kind(&self) -> AutomationKind {
        match self {
            Self::Cron { .. } => AutomationKind::Cron,
            Self::Heartbeat { .. } => AutomationKind::Heartbeat,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AutomationDispatchSettings {
    pub workspace_roots: Vec<PathBuf>,
    pub approval_policy: AskForApproval,
    pub approvals_reviewer: ApprovalsReviewer,
    pub permission_profile: PermissionProfile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Automation {
    pub id: String,
    pub owner_thread_id: ThreadId,
    pub name: String,
    pub prompt: String,
    pub status: AutomationStatus,
    pub rrule: String,
    pub next_run_at: Option<DateTime<Utc>>,
    pub last_run_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub model: Option<String>,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub target: AutomationTarget,
    pub dispatch_settings: Option<AutomationDispatchSettings>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutomationCreateParams {
    pub owner_thread_id: ThreadId,
    pub name: String,
    pub prompt: String,
    pub status: AutomationStatus,
    pub rrule: Option<String>,
    pub model: Option<String>,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub target: AutomationTarget,
    pub dispatch_settings: Option<AutomationDispatchSettings>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutomationUpdateParams {
    pub id: String,
    pub owner_thread_id: ThreadId,
    pub name: String,
    pub prompt: String,
    pub status: AutomationStatus,
    pub rrule: Option<String>,
    pub model: Option<String>,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub target: AutomationTarget,
    pub dispatch_settings: Option<AutomationDispatchSettings>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutomationDispatchMode {
    Scheduled,
    Manual,
}

impl AutomationDispatchMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Scheduled => "scheduled",
            Self::Manual => "manual",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "scheduled" => Ok(Self::Scheduled),
            "manual" => Ok(Self::Manual),
            _ => Err(anyhow::anyhow!("invalid automation dispatch mode: {value}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutomationDispatchClaim {
    pub automation: Automation,
    pub ownership_token: String,
    pub dispatch_mode: AutomationDispatchMode,
    pub in_flight_run_at: DateTime<Utc>,
    pub dispatch_cwd_index: usize,
    pub attempt_count: i64,
    pub next_run_at_after_claim: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutomationDispatchOutcome {
    NotFound,
    AlreadyClaimed,
    Claimed(Box<AutomationDispatchClaim>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutomationDispatchRetryOutcome {
    LostClaim,
    ReleasedForRetry,
    MarkedTerminal,
    Cancelled,
}

#[derive(Debug, Clone, sqlx::FromRow)]
#[allow(dead_code)]
pub(crate) struct AutomationRow {
    pub(crate) id: String,
    pub(crate) owner_thread_id: String,
    pub(crate) kind: String,
    pub(crate) name: String,
    pub(crate) prompt: String,
    pub(crate) status: String,
    pub(crate) rrule: String,
    pub(crate) next_run_at: Option<i64>,
    pub(crate) last_run_at: Option<i64>,
    pub(crate) created_at: i64,
    pub(crate) updated_at: i64,
    pub(crate) model: Option<String>,
    pub(crate) reasoning_effort: Option<String>,
    pub(crate) cron_cwds_json: Option<String>,
    pub(crate) target_thread_id: Option<String>,
    pub(crate) dispatch_workspace_roots_json: Option<String>,
    pub(crate) dispatch_approval_policy_json: Option<String>,
    pub(crate) dispatch_approvals_reviewer_json: Option<String>,
    pub(crate) dispatch_permission_profile_json: Option<String>,
    pub(crate) claimed_by: Option<String>,
    pub(crate) ownership_token: Option<String>,
    pub(crate) lease_until: Option<i64>,
    pub(crate) in_flight_run_at: Option<i64>,
    pub(crate) in_flight_dispatch_mode: Option<String>,
    pub(crate) dispatch_cwd_index: i64,
    pub(crate) retry_at: Option<i64>,
    pub(crate) attempt_count: i64,
    pub(crate) last_error: Option<String>,
    pub(crate) last_dispatch_succeeded: Option<bool>,
    pub(crate) last_dispatch_started_at: Option<i64>,
    pub(crate) last_dispatch_completed_at: Option<i64>,
    pub(crate) last_dispatch_failed_at: Option<i64>,
}

impl TryFrom<AutomationRow> for Automation {
    type Error = anyhow::Error;

    fn try_from(row: AutomationRow) -> Result<Self, Self::Error> {
        let AutomationRow {
            id,
            owner_thread_id,
            kind,
            name,
            prompt,
            status,
            rrule,
            next_run_at,
            last_run_at,
            created_at,
            updated_at,
            model,
            reasoning_effort,
            cron_cwds_json,
            target_thread_id,
            dispatch_workspace_roots_json,
            dispatch_approval_policy_json,
            dispatch_approvals_reviewer_json,
            dispatch_permission_profile_json,
            claimed_by: _,
            ownership_token: _,
            lease_until: _,
            in_flight_run_at: _,
            in_flight_dispatch_mode: _,
            dispatch_cwd_index: _,
            retry_at: _,
            attempt_count: _,
            last_error: _,
            last_dispatch_succeeded: _,
            last_dispatch_started_at: _,
            last_dispatch_completed_at: _,
            last_dispatch_failed_at: _,
        } = row;

        let kind = AutomationKind::parse(kind.as_str())?;
        let target = match kind {
            AutomationKind::Cron => {
                let cwds_json = cron_cwds_json
                    .ok_or_else(|| anyhow::anyhow!("missing cron_cwds_json for cron automation"))?;
                let raw_cwds: Vec<String> = serde_json::from_str(cwds_json.as_str())?;
                AutomationTarget::Cron {
                    cwds: raw_cwds.into_iter().map(PathBuf::from).collect(),
                }
            }
            AutomationKind::Heartbeat => {
                let thread_id = target_thread_id.ok_or_else(|| {
                    anyhow::anyhow!("missing target_thread_id for heartbeat automation")
                })?;
                AutomationTarget::Heartbeat {
                    thread_id: ThreadId::from_string(&thread_id)?,
                }
            }
        };

        let dispatch_settings = match kind {
            AutomationKind::Cron
                if dispatch_workspace_roots_json.is_none()
                    && dispatch_approval_policy_json.is_none()
                    && dispatch_approvals_reviewer_json.is_none()
                    && dispatch_permission_profile_json.is_none() =>
            {
                None
            }
            AutomationKind::Cron => Some(AutomationDispatchSettings {
                workspace_roots: dispatch_workspace_roots_json
                    .as_deref()
                    .map(serde_json::from_str)
                    .transpose()?
                    .unwrap_or_default(),
                approval_policy: dispatch_approval_policy_json
                    .as_deref()
                    .map(serde_json::from_str)
                    .transpose()?
                    .unwrap_or(AskForApproval::OnRequest),
                approvals_reviewer: dispatch_approvals_reviewer_json
                    .as_deref()
                    .map(serde_json::from_str)
                    .transpose()?
                    .unwrap_or_default(),
                permission_profile: dispatch_permission_profile_json
                    .as_deref()
                    .map(serde_json::from_str)
                    .transpose()?
                    .unwrap_or_default(),
            }),
            AutomationKind::Heartbeat => None,
        };

        let reasoning_effort = match reasoning_effort {
            Some(value) => Some(
                value
                    .parse::<ReasoningEffort>()
                    .map_err(anyhow::Error::msg)?,
            ),
            None => None,
        };
        let owner_thread_id = ThreadId::from_string(&owner_thread_id)?;

        Ok(Self {
            id,
            owner_thread_id,
            name,
            prompt,
            status: AutomationStatus::parse(status.as_str())?,
            rrule,
            next_run_at: next_run_at.map(epoch_seconds_to_datetime).transpose()?,
            last_run_at: last_run_at.map(epoch_seconds_to_datetime).transpose()?,
            created_at: epoch_seconds_to_datetime(created_at)?,
            updated_at: epoch_seconds_to_datetime(updated_at)?,
            model,
            reasoning_effort,
            target,
            dispatch_settings,
        })
    }
}

fn epoch_seconds_to_datetime(secs: i64) -> Result<DateTime<Utc>> {
    DateTime::<Utc>::from_timestamp(secs, 0)
        .ok_or_else(|| anyhow::anyhow!("invalid unix timestamp: {secs}"))
}
