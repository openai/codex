use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::RwLock;
use std::time::Duration;

use super::backends::ExecutionMode;
use super::backends::backend_for_mode;
use super::cache::ApprovalCache;
use crate::codex::Session;
use crate::error::CodexErr;
use crate::error::SandboxErr;
use crate::error::get_error_message_ui;
use crate::exec::ExecParams;
use crate::exec::ExecToolCallOutput;
use crate::exec::SandboxType;
use crate::exec::StdoutStream;
use crate::exec::StreamOutput;
use crate::exec::execute_sandbox_launch;
use crate::executor::errors::ExecError;
use crate::executor::sandbox::RetrySandboxContext;
use crate::executor::sandbox::SandboxDecision;
use crate::executor::sandbox::SandboxLaunch;
use crate::executor::sandbox::SandboxLaunchError;
use crate::executor::sandbox::build_launch_for_sandbox;
use crate::executor::sandbox::select_sandbox;
use crate::function_tool::FunctionCallError;
use crate::protocol::AskForApproval;
use crate::protocol::SandboxPolicy;
use crate::shell;
use crate::tools::context::ExecCommandContext;

#[derive(Clone, Debug)]
pub(crate) struct ExecutorConfig {
    pub(crate) sandbox_policy: SandboxPolicy,
    pub(crate) sandbox_cwd: PathBuf,
    // Path to codex-linux-sandbox executable (Linux-only). Used by initial_launch when selecting Linux sandbox.
    pub(crate) codex_linux_sandbox_exe: Option<PathBuf>,
    // Path to the codex binary itself (used by apply_patch backend to self-invoke when needed).
    pub(crate) codex_exe: Option<PathBuf>,
}

impl ExecutorConfig {
    pub(crate) fn new(
        sandbox_policy: SandboxPolicy,
        sandbox_cwd: PathBuf,
        codex_linux_sandbox_exe: Option<PathBuf>,
        codex_exe: Option<PathBuf>,
    ) -> Self {
        let codex_exe = codex_exe.or_else(|| derive_codex_exe(&codex_linux_sandbox_exe));
        Self {
            sandbox_policy,
            sandbox_cwd,
            codex_linux_sandbox_exe,
            codex_exe,
        }
    }
}

fn derive_codex_exe(sandbox_exe: &Option<PathBuf>) -> Option<PathBuf> {
    sandbox_exe.as_ref().and_then(|path| {
        let stem_matches_sandbox = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .is_some_and(|stem| stem == "codex-linux-sandbox");
        if stem_matches_sandbox {
            None
        } else {
            Some(path.clone())
        }
    })
}

pub(crate) struct ExecutionPlan {
    request: ExecutionRequest,
    config: ExecutorConfig,
    sandbox_decision: SandboxDecision,
    stdout_stream: Option<StdoutStream>,
    context: ExecCommandContext,
}

impl ExecutionPlan {
    pub(crate) fn request(&self) -> &ExecutionRequest {
        &self.request
    }

    pub(crate) fn config(&self) -> &ExecutorConfig {
        &self.config
    }

    pub(crate) fn stdout_stream(&self) -> Option<StdoutStream> {
        self.stdout_stream.clone()
    }

    pub(crate) fn initial_launch(&self) -> Result<SandboxLaunch, SandboxLaunchError> {
        build_launch_for_sandbox(
            self.sandbox_decision.initial_sandbox,
            &self.request.params.command,
            &self.config.sandbox_policy,
            &self.config.sandbox_cwd,
            self.config.codex_linux_sandbox_exe.as_ref(),
        )
    }

    pub(crate) fn retry_launch(&self) -> Result<SandboxLaunch, SandboxLaunchError> {
        build_launch_for_sandbox(
            SandboxType::None,
            &self.request.params.command,
            &self.config.sandbox_policy,
            &self.config.sandbox_cwd,
            None,
        )
    }

    pub(crate) fn approval_command(&self) -> &[String] {
        &self.request.approval_command
    }

    pub(crate) async fn prompt_retry_without_sandbox(
        &self,
        session: &Session,
        failure_message: impl Into<String>,
    ) -> bool {
        if !self.sandbox_decision.escalate_on_failure {
            return false;
        }

        let approval = crate::executor::sandbox::request_retry_without_sandbox(
            session,
            failure_message.into(),
            self.approval_command(),
            self.request.params.cwd.clone(),
            RetrySandboxContext {
                sub_id: &self.context.sub_id,
                call_id: &self.context.call_id,
                tool_name: &self.context.tool_name,
                otel_event_manager: &self.context.otel_event_manager,
            },
        )
        .await;

        approval.is_some()
    }

    /// Like `attempt_with_retry`, but only retries if `should_retry(err)` returns true.
    pub(crate) async fn attempt_with_retry_if<R, E, Fut, Attempt, Should>(
        &self,
        session: &Session,
        attempt: Attempt,
        should_retry: Should,
    ) -> Result<R, E>
    where
        Attempt: Fn(SandboxLaunch) -> Fut,
        Fut: Future<Output = Result<R, E>>,
        Should: Fn(&E) -> bool,
        E: From<SandboxLaunchError> + std::fmt::Display,
    {
        let initial_launch = self.initial_launch().map_err(E::from)?;
        match attempt(initial_launch).await {
            Ok(result) => Ok(result),
            Err(err) if self.sandbox_decision.escalate_on_failure && should_retry(&err) => {
                let failure = format!("Execution failed: {err}");
                if self.prompt_retry_without_sandbox(session, failure).await {
                    let retry_launch = self.retry_launch().map_err(E::from)?;
                    attempt(retry_launch).await
                } else {
                    Err(err)
                }
            }
            Err(err) => Err(err),
        }
    }
}

/// Coordinates sandbox selection, backend-specific preparation, and command
/// execution for tool calls requested by the model.
pub(crate) struct Executor {
    approval_cache: ApprovalCache,
    config: Arc<RwLock<ExecutorConfig>>,
}

impl Executor {
    pub(crate) fn new(config: ExecutorConfig) -> Self {
        Self {
            approval_cache: ApprovalCache::default(),
            config: Arc::new(RwLock::new(config)),
        }
    }

    pub(crate) fn record_session_approval(&self, command: Vec<String>) {
        self.approval_cache.insert(command);
    }

    pub(crate) async fn prepare_execution_plan(
        &self,
        mut request: ExecutionRequest,
        session: &Session,
        approval_policy: AskForApproval,
        context: &ExecCommandContext,
    ) -> Result<ExecutionPlan, ExecError> {
        if matches!(
            request.mode,
            ExecutionMode::Shell | ExecutionMode::InteractiveShell
        ) {
            request.params =
                maybe_translate_shell_command(request.params, session, request.use_shell_profile);
        }

        // Step 1: Snapshot sandbox configuration so it stays stable for this run.
        let config = self
            .config
            .read()
            .map_err(|_| ExecError::rejection("executor config poisoned"))?
            .clone();

        // Step 2: Normalise parameters via the selected backend.
        let backend = backend_for_mode(&request.mode);
        let stdout_stream = if backend.stream_stdout(&request.mode) {
            request.stdout_stream.clone()
        } else {
            None
        };
        request.params = backend
            .prepare(request.params, &request.mode, &config)
            .map_err(ExecError::from)?;

        let config = self
            .config
            .read()
            .map_err(|_| ExecError::rejection("executor config poisoned"))?
            .clone();

        // Step 3: Decide sandbox placement, prompting for approval when needed.
        let sandbox_decision = select_sandbox(
            &request,
            approval_policy,
            self.approval_cache.snapshot(),
            &config,
            session,
            &context.sub_id,
            &context.call_id,
            &context.otel_event_manager,
        )
        .await?;
        if sandbox_decision.record_session_approval {
            self.approval_cache.insert(request.approval_command.clone());
        }

        Ok(ExecutionPlan {
            request,
            config,
            sandbox_decision,
            stdout_stream,
            context: context.clone(),
        })
    }

    /// Updates the sandbox policy and working directory used for future
    /// executions without recreating the executor.
    pub(crate) fn update_environment(&self, sandbox_policy: SandboxPolicy, sandbox_cwd: PathBuf) {
        if let Ok(mut cfg) = self.config.write() {
            cfg.sandbox_policy = sandbox_policy;
            cfg.sandbox_cwd = sandbox_cwd;
        }
    }

    /// Runs a prepared execution request end-to-end: prepares parameters, decides on
    /// sandbox placement (prompting the user when necessary), launches the command,
    /// and lets the backend post-process the final output.
    pub(crate) async fn run(
        &self,
        request: ExecutionRequest,
        session: &Session,
        approval_policy: AskForApproval,
        context: &ExecCommandContext,
    ) -> Result<ExecToolCallOutput, ExecError> {
        let plan = self
            .prepare_execution_plan(request, session, approval_policy, context)
            .await?;

        let stdout_stream = plan.stdout_stream();
        let sandbox_policy = plan.config().sandbox_policy.clone();

        // Drive attempts via the shared helper, but do not retry on timeouts.
        let result: Result<ExecToolCallOutput, CodexErr> = plan
            .attempt_with_retry_if(
                session,
                |launch| {
                    let params = plan.request().params.clone();
                    let sandbox = plan.sandbox_decision.initial_sandbox;
                    let policy = sandbox_policy.clone();
                    let stream = stdout_stream.clone();
                    async move { execute_sandbox_launch(params, launch, sandbox, &policy, stream).await }
                },
                |err: &CodexErr| { !matches!(err, CodexErr::Sandbox(SandboxErr::Timeout { .. })) },
            )
            .await;

        match result {
            Ok(output) => Ok(output),
            Err(CodexErr::Sandbox(SandboxErr::Timeout { output })) => {
                Err(ExecError::Codex(CodexErr::Sandbox(SandboxErr::Timeout {
                    output,
                })))
            }
            Err(CodexErr::Sandbox(error)) => {
                // Convert non-timeout sandbox errors into user-facing rejection messages.
                let message = sandbox_failure_message(error);
                Err(ExecError::rejection(message))
            }
            Err(err) => Err(err.into()),
        }
    }
}

fn maybe_translate_shell_command(
    params: ExecParams,
    session: &Session,
    use_shell_profile: bool,
) -> ExecParams {
    let should_translate =
        matches!(session.user_shell(), shell::Shell::PowerShell(_)) || use_shell_profile;

    if should_translate
        && let Some(command) = session
            .user_shell()
            .format_default_shell_invocation(params.command.clone())
    {
        return ExecParams { command, ..params };
    }

    params
}

fn sandbox_failure_message(error: SandboxErr) -> String {
    let codex_error = CodexErr::Sandbox(error);
    let friendly = get_error_message_ui(&codex_error);
    format!("failed in sandbox: {friendly}")
}

pub(crate) struct ExecutionRequest {
    pub params: ExecParams,
    pub approval_command: Vec<String>,
    pub mode: ExecutionMode,
    pub stdout_stream: Option<StdoutStream>,
    pub use_shell_profile: bool,
}

pub(crate) struct NormalizedExecOutput<'a> {
    borrowed: Option<&'a ExecToolCallOutput>,
    synthetic: Option<ExecToolCallOutput>,
}

impl<'a> NormalizedExecOutput<'a> {
    pub(crate) fn event_output(&'a self) -> &'a ExecToolCallOutput {
        match (self.borrowed, self.synthetic.as_ref()) {
            (Some(output), _) => output,
            (None, Some(output)) => output,
            (None, None) => unreachable!("normalized exec output missing data"),
        }
    }
}

/// Converts a raw execution result into a uniform view that always exposes an
/// [`ExecToolCallOutput`], synthesizing error output when the command fails
/// before producing a response.
pub(crate) fn normalize_exec_result(
    result: &Result<ExecToolCallOutput, ExecError>,
) -> NormalizedExecOutput<'_> {
    match result {
        Ok(output) => NormalizedExecOutput {
            borrowed: Some(output),
            synthetic: None,
        },
        Err(ExecError::Codex(CodexErr::Sandbox(SandboxErr::Timeout { output }))) => {
            NormalizedExecOutput {
                borrowed: Some(output.as_ref()),
                synthetic: None,
            }
        }
        Err(err) => {
            let message = match err {
                ExecError::Function(FunctionCallError::RespondToModel(msg)) => msg.clone(),
                ExecError::Codex(e) => get_error_message_ui(e),
                err => err.to_string(),
            };
            let synthetic = ExecToolCallOutput {
                exit_code: -1,
                stdout: StreamOutput::new(String::new()),
                stderr: StreamOutput::new(message.clone()),
                aggregated_output: StreamOutput::new(message),
                duration: Duration::default(),
                timed_out: false,
            };
            NormalizedExecOutput {
                borrowed: None,
                synthetic: Some(synthetic),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CodexErr;
    use crate::error::EnvVarError;
    use crate::error::SandboxErr;
    use crate::exec::StreamOutput;
    use pretty_assertions::assert_eq;

    fn make_output(text: &str) -> ExecToolCallOutput {
        ExecToolCallOutput {
            exit_code: 1,
            stdout: StreamOutput::new(String::new()),
            stderr: StreamOutput::new(String::new()),
            aggregated_output: StreamOutput::new(text.to_string()),
            duration: Duration::from_millis(123),
            timed_out: false,
        }
    }

    #[test]
    fn normalize_success_borrows() {
        let out = make_output("ok");
        let result: Result<ExecToolCallOutput, ExecError> = Ok(out);
        let normalized = normalize_exec_result(&result);
        assert_eq!(normalized.event_output().aggregated_output.text, "ok");
    }

    #[test]
    fn normalize_timeout_borrows_embedded_output() {
        let out = make_output("timed out payload");
        let err = CodexErr::Sandbox(SandboxErr::Timeout {
            output: Box::new(out),
        });
        let result: Result<ExecToolCallOutput, ExecError> = Err(ExecError::Codex(err));
        let normalized = normalize_exec_result(&result);
        assert_eq!(
            normalized.event_output().aggregated_output.text,
            "timed out payload"
        );
    }

    #[test]
    fn sandbox_failure_message_uses_denied_stderr() {
        let output = ExecToolCallOutput {
            exit_code: 101,
            stdout: StreamOutput::new(String::new()),
            stderr: StreamOutput::new("sandbox stderr".to_string()),
            aggregated_output: StreamOutput::new(String::new()),
            duration: Duration::from_millis(10),
            timed_out: false,
        };
        let err = SandboxErr::Denied {
            output: Box::new(output),
        };
        let message = sandbox_failure_message(err);
        assert_eq!(message, "failed in sandbox: sandbox stderr");
    }

    #[test]
    fn sandbox_failure_message_falls_back_to_aggregated_output() {
        let output = ExecToolCallOutput {
            exit_code: 101,
            stdout: StreamOutput::new(String::new()),
            stderr: StreamOutput::new(String::new()),
            aggregated_output: StreamOutput::new("aggregate text".to_string()),
            duration: Duration::from_millis(10),
            timed_out: false,
        };
        let err = SandboxErr::Denied {
            output: Box::new(output),
        };
        let message = sandbox_failure_message(err);
        assert_eq!(message, "failed in sandbox: aggregate text");
    }

    #[test]
    fn normalize_function_error_synthesizes_payload() {
        let err = FunctionCallError::RespondToModel("boom".to_string());
        let result: Result<ExecToolCallOutput, ExecError> = Err(ExecError::Function(err));
        let normalized = normalize_exec_result(&result);
        assert_eq!(normalized.event_output().aggregated_output.text, "boom");
    }

    #[test]
    fn normalize_codex_error_synthesizes_user_message() {
        // Use a simple EnvVar error which formats to a clear message
        let e = CodexErr::EnvVar(EnvVarError {
            var: "FOO".to_string(),
            instructions: Some("set it".to_string()),
        });
        let result: Result<ExecToolCallOutput, ExecError> = Err(ExecError::Codex(e));
        let normalized = normalize_exec_result(&result);
        assert!(
            normalized
                .event_output()
                .aggregated_output
                .text
                .contains("Missing environment variable: `FOO`"),
            "expected synthesized user-friendly message"
        );
    }
}
