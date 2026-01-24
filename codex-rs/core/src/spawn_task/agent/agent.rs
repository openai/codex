use std::path::PathBuf;
use std::sync::Arc;

use chrono::Utc;
use tokio_util::sync::CancellationToken;
use tokio_util::task::AbortOnDropHandle;
use tracing::error;
use tracing::info;
use tracing::warn;

use crate::agent::AgentControl;
use crate::auth::AuthManager;
use crate::codex::Codex;
use crate::codex::CodexSpawnOk;
use crate::config::Config;
use crate::loop_driver::LoopCondition;
use crate::loop_driver::LoopDriver;
use crate::loop_driver::LoopStopReason;
use crate::loop_driver::SummarizerContext;
use crate::loop_driver::git_ops;
use crate::models_manager::manager::ModelsManager;
use crate::skills::SkillsManager;
use crate::spawn_task::SpawnTask;
use crate::spawn_task::SpawnTaskMetadata;
use crate::spawn_task::SpawnTaskResult;
use crate::spawn_task::SpawnTaskStatus;
use crate::spawn_task::SpawnTaskType;
use crate::spawn_task::log_sink::LogFileSink;
use crate::spawn_task::metadata::load_metadata;
use crate::spawn_task::metadata::log_file_path;
use crate::spawn_task::metadata::save_metadata;
use crate::subagent::ApprovalMode;
use crate::subagent::expect_session_state;
use codex_protocol::config_types::PlanModeApprovalPolicy;
use codex_protocol::protocol::InitialHistory;
use codex_protocol::protocol::SessionSource;

/// Parameters for creating a SpawnAgent.
#[derive(Debug, Clone)]
pub struct SpawnAgentParams {
    /// Unique task ID.
    pub task_id: String,
    /// Loop condition.
    pub loop_condition: LoopCondition,
    /// User prompt (task description).
    pub prompt: String,
    /// Working directory.
    pub cwd: PathBuf,
    /// Custom loop prompt (optional).
    pub custom_loop_prompt: Option<String>,
    /// Approval mode for the spawned agent session.
    #[allow(dead_code)]
    pub approval_mode: ApprovalMode,
    /// Model override in "provider" or "provider/model" format.
    pub model_override: Option<String>,
    /// Forked plan content from parent agent (snapshot).
    pub forked_plan_content: Option<String>,
    /// Plan mode approval policy (default: AutoApprove for SpawnAgent).
    pub plan_mode_approval_policy: PlanModeApprovalPolicy,
}

/// Context needed to spawn a Codex session.
pub struct SpawnAgentContext {
    /// Auth manager.
    pub auth_manager: Arc<AuthManager>,
    /// Models manager.
    pub models_manager: Arc<ModelsManager>,
    /// Skills manager.
    pub skills_manager: Arc<SkillsManager>,
    /// Base config to use.
    pub config: Config,
    /// Codex home directory.
    pub codex_home: PathBuf,
    /// LSP server manager (shared from ThreadManager, None if LSP feature disabled).
    pub lsp_manager: Option<Arc<codex_lsp::LspServerManager>>,
}

/// Apply model override from "provider_id" or "provider_id/model" format.
///
/// - Looks up provider by provider_id (HashMap key in model_providers)
/// - If only provider_id specified: uses provider's ext.model_name
/// - If provider_id/model specified: uses explicit model
/// - Provider's ultrathink_config and model_parameters are inherited automatically
fn apply_model_override(config: &mut Config, model_str: &str) {
    let parts: Vec<&str> = model_str.splitn(2, '/').collect();
    let provider_id = parts[0];

    // Find provider by provider_id (HashMap key)
    let found = config
        .model_providers
        .get(provider_id)
        .map(|info| (provider_id.to_string(), info.clone()));

    if let Some((provider_id, provider_info)) = found {
        // Switch to new provider
        config.model_provider_id = provider_id.clone();
        config.model_provider = provider_info.clone();

        // Determine model name
        let model_name = if parts.len() > 1 {
            // Explicit model: "provider_id/model"
            Some(parts[1].to_string())
        } else {
            // Use provider's model_name from ext
            provider_info.ext.model_name.clone()
        };

        if let Some(name) = model_name {
            config.model = Some(name);
        }

        info!(
            provider_id = %provider_id,
            model = ?config.model,
            "Applied model override for spawn task"
        );
    } else {
        warn!(provider_id = %provider_id, "Model provider not found by provider_id in config");
    }
}

/// SpawnAgent - Full Codex agent with loop driver.
///
/// Implements `SpawnTask` trait for unified lifecycle management.
pub struct SpawnAgent {
    params: SpawnAgentParams,
    context: SpawnAgentContext,
    cancellation_token: CancellationToken,
    cwd: PathBuf,
}

impl SpawnAgent {
    /// Create a new SpawnAgent.
    pub fn new(params: SpawnAgentParams, context: SpawnAgentContext) -> Self {
        let cwd = params.cwd.clone();
        Self {
            params,
            context,
            cancellation_token: CancellationToken::new(),
            cwd,
        }
    }
}

impl SpawnTask for SpawnAgent {
    fn task_id(&self) -> &str {
        &self.params.task_id
    }

    fn task_type(&self) -> SpawnTaskType {
        SpawnTaskType::Agent
    }

    fn set_cwd(&mut self, cwd: PathBuf) {
        self.cwd = cwd;
    }

    fn cancellation_token(&self) -> &CancellationToken {
        &self.cancellation_token
    }

    fn metadata(&self) -> SpawnTaskMetadata {
        SpawnTaskMetadata {
            task_id: self.params.task_id.clone(),
            task_type: SpawnTaskType::Agent,
            status: SpawnTaskStatus::Running,
            created_at: Utc::now(),
            completed_at: None,
            cwd: self.cwd.clone(),
            error_message: None,
            loop_condition: Some(self.params.loop_condition.clone()),
            user_query: Some(self.params.prompt.clone()),
            iterations_completed: 0,
            iterations_failed: 0,
            model_override: self.params.model_override.clone(),
            workflow_path: None,
            worktree_path: None,
            branch_name: None,
            base_branch: None,
            log_file: Some(log_file_path(
                &self.context.codex_home,
                &self.params.task_id,
            )),
            execution_result: None,
        }
    }

    fn spawn(self: Box<Self>) -> AbortOnDropHandle<SpawnTaskResult> {
        let params = self.params;
        let context = self.context;
        let token = self.cancellation_token;
        let cwd = self.cwd;

        AbortOnDropHandle::new(tokio::spawn(async move {
            // Create log file sink for this task
            let log_path = log_file_path(&context.codex_home, &params.task_id);
            let sink = LogFileSink::new(&log_path).ok();

            if let Some(ref s) = sink {
                s.log_start(&params.task_id, &params.loop_condition.display());
            }

            info!(
                task_id = %params.task_id,
                condition = %params.loop_condition.display(),
                cwd = %cwd.display(),
                "Starting SpawnAgent"
            );

            // Build config for spawned agent session
            let mut spawn_config = context.config.clone();
            spawn_config.cwd = cwd.clone();

            // Disable features that require session-level services
            // Spawn agents run in different worktrees and don't have these services
            use crate::features::Feature;
            spawn_config.features.disable(Feature::Retrieval);
            spawn_config.features.disable(Feature::Lsp);

            // Apply model override if specified
            if let Some(model_str) = &params.model_override {
                apply_model_override(&mut spawn_config, model_str);
            }

            // Spawn Codex session
            let CodexSpawnOk {
                codex,
                conversation_id,
                ..
            } = match Codex::spawn(
                spawn_config.clone(),
                context.auth_manager.clone(),
                context.models_manager,
                context.skills_manager,
                InitialHistory::New,
                SessionSource::SpawnAgent,
                AgentControl::default(),
                context.lsp_manager.clone(),
                None, // Spawn agents don't need retrieval
            )
            .await
            {
                Ok(spawn_ok) => spawn_ok,
                Err(e) => {
                    let error_msg = format!("Failed to spawn Codex: {e}");
                    error!(task_id = %params.task_id, error = %e, "Failed to spawn Codex");
                    if let Some(ref s) = sink {
                        s.log(&format!("ERROR: {error_msg}"));
                    }
                    return SpawnTaskResult {
                        task_id: params.task_id,
                        status: SpawnTaskStatus::Failed,
                        iterations_completed: 0,
                        iterations_failed: 0,
                        error_message: Some(error_msg),
                    };
                }
            };

            // Set plan mode approval policy for this session
            let stores = expect_session_state(&conversation_id);
            stores.set_plan_mode_approval_policy(params.plan_mode_approval_policy);

            // Create loop driver with progress callback
            let mut driver = LoopDriver::new(params.loop_condition.clone(), token.clone());

            if let Some(prompt) = &params.custom_loop_prompt {
                driver = driver.with_custom_prompt(prompt.clone());
            }

            // Set up progress callback with log sink
            if let Some(ref s) = sink {
                let sink_clone = s.clone();
                driver = driver.with_progress_callback(move |progress| {
                    sink_clone.log_iteration(
                        progress.iteration,
                        progress.succeeded,
                        progress.failed,
                    );
                });
            }

            // Enable context passing for cross-iteration state
            // Get base commit ID for context
            let base_commit = git_ops::get_head_commit(&cwd).await.unwrap_or_else(|e| {
                warn!(error = %e, "Failed to get HEAD commit for context passing");
                "unknown".to_string()
            });

            // Determine plan content: use forked_plan_content or read from file
            let plan_content = params
                .forked_plan_content
                .clone()
                .or_else(|| git_ops::read_plan_file_if_exists(&cwd));

            // Create summarizer context for LLM-based summarization
            let summarizer_ctx = SummarizerContext {
                auth_manager: context.auth_manager.clone(),
                config: Arc::new(spawn_config.clone()),
                conversation_id,
            };

            // Enable full context passing with LLM summarization
            driver = driver.with_context_passing(
                base_commit,
                params.prompt.clone(),
                plan_content.clone(),
                cwd.clone(),
                summarizer_ctx,
            );

            // Prepare prompt with forked plan context if available
            let enhanced_prompt = if let Some(plan_content) = &params.forked_plan_content {
                info!(
                    task_id = %params.task_id,
                    plan_content_len = plan_content.len(),
                    "Injecting forked plan context into prompt"
                );
                format!(
                    "<forked_plan_context>\n\
                    The following is the plan from the parent agent. Use it as context:\n\n\
                    {}\n\
                    </forked_plan_context>\n\n\
                    {}",
                    plan_content, params.prompt
                )
            } else {
                params.prompt.clone()
            };

            // Run the loop with actual Codex integration
            let result = driver
                .run_with_loop(&codex, &enhanced_prompt, sink.as_ref())
                .await;

            // Determine final status
            let status = match result.stop_reason {
                LoopStopReason::Completed | LoopStopReason::DurationElapsed => {
                    SpawnTaskStatus::Completed
                }
                LoopStopReason::Cancelled => SpawnTaskStatus::Cancelled,
                LoopStopReason::TaskAborted => SpawnTaskStatus::Failed,
            };

            if let Some(ref s) = sink {
                s.log_complete(&status.to_string());
            }

            info!(
                task_id = %params.task_id,
                status = ?status,
                iterations_completed = result.iterations_succeeded,
                iterations_failed = result.iterations_failed,
                "SpawnAgent finished"
            );

            // Persist final result to metadata
            if let Ok(mut metadata) = load_metadata(&context.codex_home, &params.task_id).await {
                match status {
                    SpawnTaskStatus::Completed => {
                        metadata
                            .mark_completed(result.iterations_succeeded, result.iterations_failed);
                    }
                    SpawnTaskStatus::Failed => {
                        metadata.mark_failed(
                            result.iterations_succeeded,
                            result.iterations_failed,
                            String::new(),
                        );
                    }
                    SpawnTaskStatus::Cancelled => {
                        metadata
                            .mark_cancelled(result.iterations_succeeded, result.iterations_failed);
                    }
                    SpawnTaskStatus::Running => {
                        // Should not happen at this point
                    }
                }
                if let Err(e) = save_metadata(&context.codex_home, &metadata).await {
                    error!(task_id = %params.task_id, error = %e, "Failed to save final metadata");
                }
            }

            SpawnTaskResult {
                task_id: params.task_id,
                status,
                iterations_completed: result.iterations_succeeded,
                iterations_failed: result.iterations_failed,
                error_message: None,
            }
        }))
    }
}
