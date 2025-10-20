use std::sync::Arc;

use crate::AuthManager;
use crate::RolloutRecorder;
use crate::exec_command::ExecSessionManager;
use crate::executor::Executor;
use crate::mcp_connection_manager::McpConnectionManager;
use crate::unified_exec::UnifiedExecSessionManager;
use crate::user_notification::UserNotifier;
use codex_otel::otel_event_manager::OtelEventManager;
use tokio::sync::Mutex;

pub(crate) struct SessionServices {
    pub(crate) mcp_connection_manager: McpConnectionManager,
    pub(crate) session_manager: ExecSessionManager,
    pub(crate) unified_exec_manager: UnifiedExecSessionManager,
    pub(crate) notifier: UserNotifier,
    pub(crate) rollout: Mutex<Option<RolloutRecorder>>,
    pub(crate) user_shell: crate::shell::Shell,
    pub(crate) show_raw_agent_reasoning: bool,
    pub(crate) executor: Executor,
    pub(crate) auth_manager: Arc<AuthManager>,
    pub(crate) otel_event_manager: OtelEventManager,
}
