use std::collections::HashMap;
use std::sync::Arc;

use crate::RolloutRecorder;
use crate::SkillsManager;
use crate::agent::AgentControl;
use crate::agent_identity::AgentIdentityManager;
use crate::client::ModelClient;
use crate::config::StartedNetworkProxy;
use crate::exec_policy::ExecPolicyManager;
use crate::guardian::GuardianRejection;
use crate::mcp::McpManager;
use crate::plugins::PluginsManager;
use crate::skills_watcher::SkillsWatcher;
use crate::tools::code_mode::CodeModeService;
use crate::tools::network_approval::NetworkApprovalService;
use crate::tools::sandboxing::ApprovalStore;
use crate::unified_exec::UnifiedExecProcessManager;
use codex_analytics::AnalyticsEventsClient;
use codex_exec_server::Environment;
use codex_hooks::Hooks;
use codex_login::AuthManager;
use codex_mcp::McpConnectionManager;
use codex_models_manager::manager::ModelsManager;
use codex_otel::SessionTelemetry;
use codex_rollout::state_db::StateDbHandle;
use codex_thread_store::LocalThreadStore;
use std::path::PathBuf;
use tokio::sync::Mutex;
use tokio::sync::RwLock;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

pub(crate) struct SessionServices {
    pub(crate) mcp_connection_manager: Arc<RwLock<McpConnectionManager>>,
    pub(crate) mcp_connection_manager_mode: McpConnectionManagerMode,
    pub(crate) mcp_startup_cancellation_token: Mutex<CancellationToken>,
    pub(crate) unified_exec_manager: UnifiedExecProcessManager,
    #[cfg_attr(not(unix), allow(dead_code))]
    pub(crate) shell_zsh_path: Option<PathBuf>,
    #[cfg_attr(not(unix), allow(dead_code))]
    pub(crate) main_execve_wrapper_exe: Option<PathBuf>,
    pub(crate) analytics_events_client: AnalyticsEventsClient,
    pub(crate) hooks: Hooks,
    pub(crate) rollout: Mutex<Option<RolloutRecorder>>,
    pub(crate) user_shell: Arc<crate::shell::Shell>,
    pub(crate) agent_identity_manager: Arc<AgentIdentityManager>,
    pub(crate) shell_snapshot_tx: watch::Sender<Option<Arc<crate::shell_snapshot::ShellSnapshot>>>,
    pub(crate) show_raw_agent_reasoning: bool,
    pub(crate) exec_policy: Arc<ExecPolicyManager>,
    pub(crate) auth_manager: Arc<AuthManager>,
    pub(crate) models_manager: Arc<ModelsManager>,
    pub(crate) session_telemetry: SessionTelemetry,
    pub(crate) tool_approvals: Mutex<ApprovalStore>,
    pub(crate) guardian_rejections: Mutex<HashMap<String, GuardianRejection>>,
    pub(crate) skills_manager: Arc<SkillsManager>,
    pub(crate) plugins_manager: Arc<PluginsManager>,
    pub(crate) mcp_manager: Arc<McpManager>,
    pub(crate) skills_watcher: Arc<SkillsWatcher>,
    pub(crate) agent_control: AgentControl,
    pub(crate) network_proxy: Option<StartedNetworkProxy>,
    pub(crate) network_approval: Arc<NetworkApprovalService>,
    pub(crate) state_db: Option<StateDbHandle>,
    pub(crate) thread_store: LocalThreadStore,
    /// Session-scoped model client shared across turns.
    pub(crate) model_client: ModelClient,
    pub(crate) code_mode_service: CodeModeService,
    pub(crate) environment: Option<Arc<Environment>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum McpConnectionManagerMode {
    Owned,
    Inherited,
    Disabled,
}

impl McpConnectionManagerMode {
    pub(crate) fn allows_refresh(self) -> bool {
        matches!(self, Self::Owned)
    }

    pub(crate) fn owns_policy_state(self) -> bool {
        matches!(self, Self::Owned | Self::Disabled)
    }

    pub(crate) fn can_install_mcp_dependencies(self) -> bool {
        matches!(self, Self::Owned)
    }
}
