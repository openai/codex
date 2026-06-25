use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::Weak;
use std::sync::atomic::AtomicBool;

use crate::SkillsService;
use crate::agent::AgentControl;
use crate::agents_md_manager::AgentsMdManager;
use crate::attestation::AttestationProvider;
use crate::client::ModelClient;
use crate::config::NetworkProxyAuditMetadata;
use crate::config::StartedNetworkProxy;
use crate::current_time::TimeProvider;
use crate::environment_selection::ThreadEnvironments;
use crate::exec_policy::ExecPolicyManager;
use crate::guardian::GuardianRejection;
use crate::guardian::GuardianRejectionCircuitBreaker;
use crate::mcp::McpManager;
use crate::session::McpRuntimeSnapshot;
use crate::session::SelectedMcpRuntimeCache;
use crate::tools::code_mode::CodeModeService;
use crate::tools::handlers::ToolSearchHandlerCache;
use crate::tools::network_approval::NetworkApprovalService;
use crate::tools::sandboxing::ApprovalStore;
use crate::unified_exec::UnifiedExecProcessManager;
use anyhow::Result;
use arc_swap::ArcSwap;
use arc_swap::ArcSwapOption;
use codex_analytics::AnalyticsEventsClient;
use codex_core_plugins::PluginsManager;
use codex_core_skills::ExecutorSkillCatalogCache;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionRegistry;
use codex_hooks::Hooks;
use codex_login::AuthManager;
use codex_mcp::McpConfig;
use codex_mcp::McpConnectionManager;
use codex_mcp::McpRuntimeContext;
use codex_models_manager::manager::SharedModelsManager;
use codex_otel::SessionTelemetry;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_rollout::state_db::StateDbHandle;
use codex_rollout_trace::ThreadTraceContext;
use codex_thread_store::LiveThread;
use codex_thread_store::ThreadStore;
use std::path::PathBuf;
use tokio::runtime::Handle;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

pub(crate) struct SessionServices {
    /// The latest atomically published MCP config and manager pair.
    pub(crate) mcp_runtime: Arc<ArcSwapOption<McpRuntimeSnapshot>>,
    /// Managers that may still own an outstanding elicitation request.
    pub(crate) mcp_elicitation_managers: StdMutex<Vec<Weak<McpConnectionManager>>>,
    /// Successful executor projections and the augmented runtime built from them.
    pub(crate) selected_mcp_runtime: Mutex<SelectedMcpRuntimeCache>,
    pub(crate) mcp_startup_cancellation_token: Mutex<CancellationToken>,
    pub(crate) unified_exec_manager: UnifiedExecProcessManager,
    #[cfg_attr(not(unix), allow(dead_code))]
    pub(crate) shell_zsh_path: Option<PathBuf>,
    #[cfg_attr(not(unix), allow(dead_code))]
    pub(crate) main_execve_wrapper_exe: Option<PathBuf>,
    pub(crate) analytics_events_client: AnalyticsEventsClient,
    pub(crate) hooks: ArcSwap<Hooks>,
    pub(crate) rollout_thread_trace: ThreadTraceContext,
    pub(crate) user_shell: Arc<crate::shell::Shell>,
    pub(crate) show_raw_agent_reasoning: bool,
    pub(crate) exec_policy: Arc<ExecPolicyManager>,
    pub(crate) auth_manager: Arc<AuthManager>,
    pub(crate) models_manager: SharedModelsManager,
    pub(crate) session_telemetry: SessionTelemetry,
    pub(crate) tool_approvals: Mutex<ApprovalStore>,
    pub(crate) guardian_rejections: Mutex<HashMap<String, GuardianRejection>>,
    pub(crate) guardian_rejection_circuit_breaker: Mutex<GuardianRejectionCircuitBreaker>,
    pub(crate) runtime_handle: Handle,
    pub(crate) skills_service: Arc<SkillsService>,
    /// Catalogs discovered from executor capability roots. The cache is session-scoped because
    /// selected environment contents are stable for the session lifetime.
    pub(crate) executor_skill_catalog_cache: ExecutorSkillCatalogCache,
    pub(crate) agents_md_manager: Arc<AgentsMdManager>,
    pub(crate) plugins_manager: Arc<PluginsManager>,
    pub(crate) mcp_manager: Arc<McpManager>,
    pub(crate) extensions: Arc<ExtensionRegistry<crate::config::Config>>,
    pub(crate) session_extension_data: ExtensionData,
    pub(crate) thread_extension_data: ExtensionData,
    pub(crate) supports_openai_form_elicitation: AtomicBool,
    /// Raw capability selections for this thread. Each model step resolves them against its
    /// current executor environments before using them.
    pub(crate) selected_capability_roots: Vec<SelectedCapabilityRoot>,
    pub(crate) agent_control: AgentControl,
    pub(crate) network_proxy: ArcSwapOption<StartedNetworkProxy>,
    pub(crate) network_proxy_audit_metadata: NetworkProxyAuditMetadata,
    pub(crate) managed_network_requirements_configured: bool,
    pub(crate) network_approval: Arc<NetworkApprovalService>,
    pub(crate) state_db: Option<StateDbHandle>,
    pub(crate) live_thread: Option<LiveThread>,
    pub(crate) thread_store: Arc<dyn ThreadStore>,
    pub(crate) attestation_provider: Option<Arc<dyn AttestationProvider>>,
    pub(crate) time_provider: Arc<dyn TimeProvider>,
    /// Session-scoped model client shared across turns.
    pub(crate) model_client: ModelClient,
    pub(crate) code_mode_service: CodeModeService,
    pub(crate) tool_search_handler_cache: ToolSearchHandlerCache,
    pub(crate) turn_environments: Arc<ThreadEnvironments>,
}

impl SessionServices {
    /// Installs the manager before validating required servers so startup-time elicitation can
    /// resolve through the session's manager while validation waits.
    pub(crate) async fn install_mcp_connection_manager(
        &self,
        runtime_config: Arc<McpConfig>,
        runtime_context: McpRuntimeContext,
        manager: McpConnectionManager,
    ) -> Result<()> {
        let runtime = self
            .replace_base_mcp_runtime(runtime_config, runtime_context, manager)
            .await;
        runtime.manager().validate_required_servers().await
    }

    pub(crate) async fn replace_base_mcp_runtime(
        &self,
        runtime_config: Arc<McpConfig>,
        runtime_context: McpRuntimeContext,
        manager: McpConnectionManager,
    ) -> Arc<McpRuntimeSnapshot> {
        let runtime = Arc::new(McpRuntimeSnapshot::new(
            runtime_config,
            Arc::new(manager),
            runtime_context,
        ));
        self.replace_base_with_runtime(Arc::clone(&runtime)).await;
        runtime
    }

    pub(crate) async fn replace_base_with_runtime(&self, runtime: Arc<McpRuntimeSnapshot>) {
        self.selected_mcp_runtime
            .lock()
            .await
            .replace_base_and_invalidate_selected(Arc::clone(&runtime));
        self.publish_existing_mcp_runtime(runtime);
    }

    pub(crate) fn publish_existing_mcp_runtime(&self, runtime: Arc<McpRuntimeSnapshot>) {
        let manager = runtime.manager_arc();
        self.track_mcp_elicitation_manager(&manager);
        self.mcp_runtime.store(Some(runtime));
    }

    pub(crate) fn track_mcp_elicitation_manager(&self, manager: &Arc<McpConnectionManager>) {
        let weak_manager = Arc::downgrade(&manager);
        let mut managers = self
            .mcp_elicitation_managers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        managers.retain(|manager| manager.strong_count() > 0);
        if !managers.iter().any(|manager| manager.ptr_eq(&weak_manager)) {
            managers.push(weak_manager);
        }
    }

    pub(crate) fn latest_mcp_runtime(&self) -> Arc<McpRuntimeSnapshot> {
        let Some(runtime) = self.mcp_runtime.load_full() else {
            unreachable!("MCP runtime must be installed before handling requests");
        };
        runtime
    }
}
