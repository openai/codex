use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use codex_core::CodexConversation;
use codex_core::NewConversation;
use codex_core::config::Config;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::SandboxPolicy;
use codex_protocol::ConversationId;

pub(crate) const DEFAULT_DIRECTOR_TIMEOUT: Duration = Duration::from_secs(1200);
pub(crate) const DEFAULT_VERIFIER_TIMEOUT: Duration = Duration::from_secs(1800);
pub(crate) const FINALIZATION_PROMPT: &str = "Create deliverable/: include compiled artifacts or scripts, usage docs, and tests. Write deliverable/summary.txt capturing the final answer, evidence, and follow-up steps. Also provide deliverable/README.md with overview, manifest (paths and sizes), verification steps, and limitations. Remove scratch files. Reply with JSON: {\"type\":\"final_delivery\",\"deliverable_path\":\"deliverable/summary.txt\",\"summary\":\"<answer plus supporting context>\"}.";

#[derive(Clone)]
pub struct RoleConfig {
    pub role: String,
    pub config: Config,
    pub config_path: Option<PathBuf>,
}

impl RoleConfig {
    pub fn new(role: impl Into<String>, mut config: Config) -> Self {
        config.sandbox_policy = SandboxPolicy::DangerFullAccess;
        config.approval_policy = AskForApproval::Never;
        Self {
            role: role.into(),
            config,
            config_path: None,
        }
    }

    pub fn with_path(role: impl Into<String>, config: Config, config_path: PathBuf) -> Self {
        Self {
            role: role.into(),
            config,
            config_path: Some(config_path),
        }
    }
}

pub struct RunParams {
    pub run_id: String,
    pub run_root: Option<PathBuf>,
    pub solver: RoleConfig,
    pub director: RoleConfig,
    pub verifiers: Vec<RoleConfig>,
}

#[derive(Clone)]
pub struct RunExecutionOptions {
    pub objective: Option<String>,
    pub director_timeout: Duration,
    pub verifier_timeout: Duration,
}

impl Default for RunExecutionOptions {
    fn default() -> Self {
        Self {
            objective: None,
            director_timeout: DEFAULT_DIRECTOR_TIMEOUT,
            verifier_timeout: DEFAULT_VERIFIER_TIMEOUT,
        }
    }
}

pub struct RunOutcome {
    pub run_id: String,
    pub deliverable_path: PathBuf,
    pub summary: Option<String>,
    pub raw_message: String,
}

pub struct RoleSession {
    pub role: String,
    pub conversation_id: ConversationId,
    pub conversation: Arc<CodexConversation>,
    pub session_configured: codex_core::protocol::SessionConfiguredEvent,
    pub rollout_path: PathBuf,
    pub config: Config,
}

impl RoleSession {
    pub(crate) fn from_new(role: String, session: NewConversation, config: Config) -> Self {
        Self {
            role,
            conversation_id: session.conversation_id,
            conversation: session.conversation,
            session_configured: session.session_configured.clone(),
            rollout_path: session.session_configured.rollout_path.clone(),
            config,
        }
    }
}

pub struct RunSessions {
    pub run_id: String,
    pub solver: RoleSession,
    pub director: RoleSession,
    pub verifiers: Vec<RoleSession>,
    pub store: crate::RunStore,
}
