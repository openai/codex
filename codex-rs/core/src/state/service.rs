use std::sync::Arc;

use crate::AuthManager;
use crate::RolloutRecorder;
use crate::mcp_connection_manager::McpConnectionManager;
use crate::openai_models::models_manager::ModelsManager;
use crate::skills::SkillsManager;
use crate::tools::sandboxing::ApprovalStore;
use crate::unified_exec::UnifiedExecSessionManager;
use crate::user_notification::UserNotifier;
use codex_otel::otel_manager::OtelManager;
use tokio::sync::Mutex;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

pub(crate) struct SessionServices {
    pub(crate) mcp_connection_manager: Arc<RwLock<McpConnectionManager>>,
    pub(crate) mcp_startup_cancellation_token: CancellationToken,
    pub(crate) unified_exec_manager: UnifiedExecSessionManager,
    pub(crate) notifier: UserNotifier,
    pub(crate) rollout: Mutex<Option<RolloutRecorder>>,
    pub(crate) user_shell: Arc<crate::shell::Shell>,
    pub(crate) show_raw_agent_reasoning: bool,
    pub(crate) auth_manager: Arc<AuthManager>,
    pub(crate) models_manager: Arc<ModelsManager>,
    pub(crate) otel_manager: OtelManager,
    pub(crate) tool_approvals: Mutex<ApprovalStore>,
    pub(crate) skills_manager: Arc<SkillsManager>,
}
