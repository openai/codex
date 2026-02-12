use std::sync::Arc;

use crate::AuthManager;
use crate::RolloutRecorder;
use crate::agent::AgentControl;
use crate::analytics_client::AnalyticsEventsClient;
use crate::client::ModelClient;
use crate::config::StartedNetworkProxy;
use crate::exec_policy::ExecPolicyManager;
use crate::file_watcher::FileWatcher;
use crate::mcp_connection_manager::McpConnectionManager;
use crate::models_manager::manager::ModelsManager;
use crate::skills::SkillsManager;
use crate::state_db::StateDbHandle;
use crate::tools::sandboxing::ApprovalStore;
use crate::unified_exec::UnifiedExecProcessManager;
use codex_hooks::Hooks;
use codex_otel::OtelManager;
use tokio::sync::Mutex;
use tokio::sync::RwLock;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

pub(crate) struct SessionServices {
    pub(crate) mcp_connection_manager: Arc<RwLock<McpConnectionManager>>,
    pub(crate) mcp_startup_cancellation_token: Mutex<CancellationToken>,
    pub(crate) unified_exec_manager: UnifiedExecProcessManager,
    pub(crate) analytics_events_client: AnalyticsEventsClient,
    pub(crate) hooks: Hooks,
    pub(crate) rollout: Mutex<Option<RolloutRecorder>>,
    pub(crate) user_shell: Arc<crate::shell::Shell>,
    pub(crate) shell_snapshot_tx: watch::Sender<Option<Arc<crate::shell_snapshot::ShellSnapshot>>>,
    pub(crate) show_raw_agent_reasoning: bool,
    pub(crate) exec_policy: ExecPolicyManager,
    pub(crate) auth_manager: Arc<AuthManager>,
    pub(crate) models_manager: Arc<ModelsManager>,
    pub(crate) otel_manager: OtelManager,
    pub(crate) tool_approvals: Mutex<ApprovalStore>,
    pub(crate) skills_manager: Arc<SkillsManager>,
    pub(crate) file_watcher: Arc<FileWatcher>,
    pub(crate) agent_control: AgentControl,
    pub(crate) network_proxy: Option<StartedNetworkProxy>,
    pub(crate) state_db: Option<StateDbHandle>,
    /// Session-scoped model client shared across turns.
    pub(crate) model_client: ModelClient,
}
