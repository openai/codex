// Codex Hook System (Claudecode-inspired)
use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::warn;

/// Hook event types
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookEvent {
    /// Fired when a new task starts
    OnTaskStart,
    /// Fired when a task completes successfully
    OnTaskComplete,
    /// Fired when a task fails or encounters an error
    OnError,
    /// Fired when a task is aborted/interrupted
    OnTaskAbort,
    /// Fired when a subagent task starts
    OnSubAgentStart,
    /// Fired when a subagent task completes
    OnSubAgentComplete,
    /// Fired when a session is configured
    OnSessionStart,
    /// Fired when a session is shut down
    OnSessionEnd,
    /// Fired when a patch is applied
    OnPatchApply,
    /// Fired when a command is executed
    OnCommandExec,
}

impl std::fmt::Display for HookEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OnTaskStart => write!(f, "on_task_start"),
            Self::OnTaskComplete => write!(f, "on_task_complete"),
            Self::OnError => write!(f, "on_error"),
            Self::OnTaskAbort => write!(f, "on_task_abort"),
            Self::OnSubAgentStart => write!(f, "on_subagent_start"),
            Self::OnSubAgentComplete => write!(f, "on_subagent_complete"),
            Self::OnSessionStart => write!(f, "on_session_start"),
            Self::OnSessionEnd => write!(f, "on_session_end"),
            Self::OnPatchApply => write!(f, "on_patch_apply"),
            Self::OnCommandExec => write!(f, "on_command_exec"),
        }
    }
}

/// Hook configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookConfig {
    /// Map of hook events to shell commands
    pub hooks: HashMap<HookEvent, Vec<String>>,

    /// Whether to run hooks asynchronously (non-blocking)
    #[serde(default = "default_async_hooks")]
    pub async_execution: bool,

    /// Timeout for hook execution (in seconds)
    #[serde(default = "default_hook_timeout")]
    pub timeout_seconds: u64,

    /// Environment variables to pass to hooks
    #[serde(default)]
    pub environment: HashMap<String, String>,
}

fn default_async_hooks() -> bool {
    true
}

fn default_hook_timeout() -> u64 {
    30
}

impl Default for HookConfig {
    fn default() -> Self {
        Self {
            hooks: HashMap::new(),
            async_execution: true,
            timeout_seconds: 30,
            environment: HashMap::new(),
        }
    }
}

impl HookConfig {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a hook for an event
    pub fn add_hook(&mut self, event: HookEvent, command: String) {
        self.hooks
            .entry(event)
            .or_insert_with(Vec::new)
            .push(command);
    }

    /// Remove hooks for an event
    pub fn remove_hooks(&mut self, event: &HookEvent) {
        self.hooks.remove(event);
    }

    /// Get hooks for an event
    pub fn get_hooks(&self, event: &HookEvent) -> Option<&Vec<String>> {
        self.hooks.get(event)
    }

    /// Check if any hooks are registered for an event
    pub fn has_hooks(&self, event: &HookEvent) -> bool {
        self.hooks.get(event).map_or(false, |cmds| !cmds.is_empty())
    }
}

/// Hook execution context
#[derive(Debug, Clone)]
pub struct HookContext {
    pub event: HookEvent,
    pub task_id: Option<String>,
    pub agent_type: Option<String>,
    pub error_message: Option<String>,
    pub metadata: HashMap<String, String>,
}

impl HookContext {
    pub fn new(event: HookEvent) -> Self {
        Self {
            event,
            task_id: None,
            agent_type: None,
            error_message: None,
            metadata: HashMap::new(),
        }
    }

    pub fn with_task_id(mut self, task_id: String) -> Self {
        self.task_id = Some(task_id);
        self
    }

    pub fn with_agent_type(mut self, agent_type: String) -> Self {
        self.agent_type = Some(agent_type);
        self
    }

    pub fn with_error(mut self, error_message: String) -> Self {
        self.error_message = Some(error_message);
        self
    }

    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

/// Hook executor
#[derive(Debug)]
pub struct HookExecutor {
    config: HookConfig,
}

impl HookExecutor {
    pub fn new(config: HookConfig) -> Self {
        Self { config }
    }

    /// Execute hooks for an event
    pub async fn execute(&self, context: HookContext) -> Result<Vec<HookResult>> {
        let hooks = match self.config.get_hooks(&context.event) {
            Some(hooks) if !hooks.is_empty() => hooks,
            _ => {
                debug!("No hooks registered for event: {}", context.event);
                return Ok(Vec::new());
            }
        };

        info!(
            "Executing {} hook(s) for event: {}",
            hooks.len(),
            context.event
        );

        let mut results = Vec::new();

        for (idx, command) in hooks.iter().enumerate() {
            debug!("Executing hook {}/{}: {}", idx + 1, hooks.len(), command);

            let result = if self.config.async_execution {
                self.execute_async(command, &context).await
            } else {
                self.execute_sync(command, &context).await
            };

            match &result {
                Ok(hook_result) => {
                    if hook_result.success {
                        info!("Hook executed successfully: {}", command);
                    } else {
                        warn!(
                            "Hook failed (exit code {}): {}",
                            hook_result.exit_code, command
                        );
                    }
                }
                Err(e) => {
                    error!("Hook execution error: {}", e);
                }
            }

            results.push(result?);
        }

        Ok(results)
    }

    /// Execute hook synchronously (blocking)
    async fn execute_sync(&self, command: &str, context: &HookContext) -> Result<HookResult> {
        self.run_command(command, context).await
    }

    /// Execute hook asynchronously (non-blocking)
    async fn execute_async(&self, command: &str, context: &HookContext) -> Result<HookResult> {
        let command = command.to_string();
        let context = context.clone();
        let timeout = self.config.timeout_seconds;
        let env_vars = self.config.environment.clone();

        let handle = tokio::spawn(async move {
            execute_command_with_timeout(&command, &context, timeout, env_vars).await
        });

        handle
            .await
            .map_err(|e| anyhow::anyhow!("Hook task join error: {}", e))?
    }

    /// Run command with environment variables
    async fn run_command(&self, command: &str, context: &HookContext) -> Result<HookResult> {
        execute_command_with_timeout(
            command,
            context,
            self.config.timeout_seconds,
            self.config.environment.clone(),
        )
        .await
    }
}

/// Execute command with timeout
async fn execute_command_with_timeout(
    command: &str,
    context: &HookContext,
    timeout_seconds: u64,
    mut env_vars: HashMap<String, String>,
) -> Result<HookResult> {
    // Add context variables to environment
    env_vars.insert("CODEX_HOOK_EVENT".to_string(), context.event.to_string());

    if let Some(task_id) = &context.task_id {
        env_vars.insert("CODEX_TASK_ID".to_string(), task_id.clone());
    }

    if let Some(agent_type) = &context.agent_type {
        env_vars.insert("CODEX_AGENT_TYPE".to_string(), agent_type.clone());
    }

    if let Some(error) = &context.error_message {
        env_vars.insert("CODEX_ERROR_MESSAGE".to_string(), error.clone());
    }

    for (key, value) in &context.metadata {
        env_vars.insert(format!("CODEX_META_{}", key.to_uppercase()), value.clone());
    }

    // Determine shell
    let (shell, shell_arg) = if cfg!(target_os = "windows") {
        ("cmd", "/C")
    } else {
        ("sh", "-c")
    };

    let start_time = std::time::Instant::now();

    // Execute command with timeout
    let output = tokio::time::timeout(
        tokio::time::Duration::from_secs(timeout_seconds),
        tokio::process::Command::new(shell)
            .arg(shell_arg)
            .arg(command)
            .envs(env_vars)
            .output(),
    )
    .await
    .map_err(|_| anyhow::anyhow!("Hook execution timeout after {} seconds", timeout_seconds))?
    .map_err(|e| anyhow::anyhow!("Hook execution failed: {}", e))?;

    let duration = start_time.elapsed();

    Ok(HookResult {
        success: output.status.success(),
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        duration_ms: duration.as_millis() as u64,
    })
}

/// Hook execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookResult {
    pub success: bool,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_hook_config() {
        let mut config = HookConfig::new();

        config.add_hook(HookEvent::OnTaskStart, "echo 'Task started'".to_string());
        config.add_hook(
            HookEvent::OnTaskComplete,
            "echo 'Task completed'".to_string(),
        );

        assert_eq!(config.hooks.len(), 2);
        assert!(config.has_hooks(&HookEvent::OnTaskStart));
        assert!(!config.has_hooks(&HookEvent::OnError));
    }

    #[test]
    fn test_hook_context() {
        let context = HookContext::new(HookEvent::OnTaskStart)
            .with_task_id("task-123".to_string())
            .with_agent_type("CodeExpert".to_string())
            .with_metadata("key1".to_string(), "value1".to_string());

        assert_eq!(context.event, HookEvent::OnTaskStart);
        assert_eq!(context.task_id, Some("task-123".to_string()));
        assert_eq!(context.agent_type, Some("CodeExpert".to_string()));
        assert_eq!(context.metadata.get("key1"), Some(&"value1".to_string()));
    }

    #[tokio::test]
    async fn test_hook_executor_simple() {
        let mut config = HookConfig::new();

        // Simple echo command
        let command = if cfg!(target_os = "windows") {
            "echo test"
        } else {
            "echo test"
        };

        config.add_hook(HookEvent::OnTaskComplete, command.to_string());

        let executor = HookExecutor::new(config);
        let context =
            HookContext::new(HookEvent::OnTaskComplete).with_task_id("test-task".to_string());

        let results = executor.execute(context).await.unwrap();

        assert_eq!(results.len(), 1);
        assert!(results[0].success);
        assert_eq!(results[0].exit_code, 0);
    }

    #[tokio::test]
    async fn test_hook_environment_variables() {
        let mut config = HookConfig::new();
        config
            .environment
            .insert("TEST_VAR".to_string(), "test_value".to_string());

        let command = if cfg!(target_os = "windows") {
            "echo %CODEX_HOOK_EVENT%"
        } else {
            "echo $CODEX_HOOK_EVENT"
        };

        config.add_hook(HookEvent::OnTaskStart, command.to_string());

        let executor = HookExecutor::new(config);
        let context = HookContext::new(HookEvent::OnTaskStart);

        let results = executor.execute(context).await.unwrap();

        assert_eq!(results.len(), 1);
        assert!(results[0].stdout.contains("on_task_start"));
    }

    #[tokio::test]
    async fn test_hook_timeout() {
        let mut config = HookConfig::new();
        config.timeout_seconds = 1; // 1秒タイムアウト

        // 長時間実行されるコマンド
        let command = if cfg!(target_os = "windows") {
            "timeout /t 5 /nobreak"
        } else {
            "sleep 5"
        };

        config.add_hook(HookEvent::OnTaskStart, command.to_string());

        let executor = HookExecutor::new(config);
        let context = HookContext::new(HookEvent::OnTaskStart);

        let result = executor.execute(context).await;

        // タイムアウトエラーが発生するはず
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_multiple_hooks() {
        let mut config = HookConfig::new();

        config.add_hook(HookEvent::OnTaskComplete, "echo 'Hook 1'".to_string());
        config.add_hook(HookEvent::OnTaskComplete, "echo 'Hook 2'".to_string());
        config.add_hook(HookEvent::OnTaskComplete, "echo 'Hook 3'".to_string());

        let executor = HookExecutor::new(config);
        let context = HookContext::new(HookEvent::OnTaskComplete);

        let results = executor.execute(context).await.unwrap();

        assert_eq!(results.len(), 3);
        for result in results {
            assert!(result.success);
        }
    }
}
