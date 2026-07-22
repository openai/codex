use codex_core::CodexThread;
use codex_core::NewThread;
use codex_core::StartThreadOptions;
use codex_core::ThreadManager;
use codex_core::config::Config;
use codex_protocol::ThreadId;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::protocol::W3cTraceContext;
use codex_protocol::user_input::UserInput;
use std::sync::Arc;
use std::sync::Weak;

/// A fully resolved agent invocation.
///
/// Agent discovery owns rendering `prompt`, including any selected skill
/// references. The runtime only starts that prompt in isolated forked context.
pub struct AgentInvocation {
    pub config: Config,
    pub prompt: String,
    pub parent_trace: Option<W3cTraceContext>,
}

/// A spawned agent whose initial turn has been submitted.
pub struct AgentRun {
    pub thread_id: ThreadId,
    pub turn_id: String,
    pub thread: Arc<CodexThread>,
}

/// Runs resolved agents in threads forked by the owning [`ThreadManager`].
#[derive(Clone)]
pub struct AgentRunner {
    thread_manager: Weak<ThreadManager>,
}

impl AgentRunner {
    pub fn new(thread_manager: Weak<ThreadManager>) -> Self {
        Self { thread_manager }
    }

    /// Starts a resolved agent in a fork of `parent_thread_id`.
    pub async fn start(
        &self,
        parent_thread_id: ThreadId,
        invocation: AgentInvocation,
    ) -> CodexResult<AgentRun> {
        let AgentInvocation {
            config,
            prompt,
            parent_trace,
        } = invocation;
        if prompt.trim().is_empty() {
            return Err(CodexErr::InvalidRequest(
                "agent prompt must not be empty".to_string(),
            ));
        }

        let thread_manager = self
            .thread_manager
            .upgrade()
            .ok_or_else(|| CodexErr::UnsupportedOperation("thread manager dropped".to_string()))?;
        let NewThread {
            thread_id, thread, ..
        } = thread_manager
            .spawn_subagent(
                parent_thread_id,
                StartThreadOptions {
                    parent_trace: parent_trace.clone(),
                    ..StartThreadOptions::new(config)
                },
            )
            .await?;
        let turn_id = thread
            .submit_with_trace(
                vec![UserInput::Text {
                    text: prompt,
                    text_elements: Vec::new(),
                }]
                .into(),
                parent_trace,
            )
            .await?;

        Ok(AgentRun {
            thread_id,
            turn_id,
            thread,
        })
    }
}
