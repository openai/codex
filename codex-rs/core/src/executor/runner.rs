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
use crate::exec::process_exec_tool_call;
use crate::executor::errors::ExecError;
use crate::executor::sandbox::select_sandbox;
use crate::function_tool::FunctionCallError;
use crate::protocol::AskForApproval;
use crate::protocol::ReviewDecision;
use crate::protocol::SandboxPolicy;
use crate::shell;
use crate::tools::context::ExecCommandContext;
use codex_otel::otel_event_manager::ToolDecisionSource;

#[derive(Clone, Debug)]
pub(crate) struct ExecutorConfig {
    pub(crate) sandbox_policy: SandboxPolicy,
    pub(crate) sandbox_cwd: PathBuf,
    codex_linux_sandbox_exe: Option<PathBuf>,
}

impl ExecutorConfig {
    pub(crate) fn new(
        sandbox_policy: SandboxPolicy,
        sandbox_cwd: PathBuf,
        codex_linux_sandbox_exe: Option<PathBuf>,
    ) -> Self {
        Self {
            sandbox_policy,
            sandbox_cwd,
            codex_linux_sandbox_exe,
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
        mut request: ExecutionRequest,
        session: &Session,
        approval_policy: AskForApproval,
        context: &ExecCommandContext,
    ) -> Result<ExecToolCallOutput, ExecError> {
        if matches!(request.mode, ExecutionMode::Shell) {
            request.params =
                maybe_translate_shell_command(request.params, session, request.use_shell_profile);
        }

        // Step 1: Normalise parameters via the selected backend.
        let backend = backend_for_mode(&request.mode);
        let stdout_stream = if backend.stream_stdout(&request.mode) {
            request.stdout_stream.clone()
        } else {
            None
        };
        request.params = backend
            .prepare(request.params, &request.mode)
            .map_err(ExecError::from)?;

        // Step 2: Snapshot sandbox configuration so it stays stable for this run.
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

        // Step 4: Launch the command within the chosen sandbox.
        let first_attempt = self
            .spawn(
                request.params.clone(),
                sandbox_decision.initial_sandbox,
                &config,
                stdout_stream.clone(),
            )
            .await;

        // Step 5: Handle sandbox outcomes, optionally escalating to an unsandboxed retry.
        match first_attempt {
            Ok(output) => Ok(output),
            Err(CodexErr::Sandbox(SandboxErr::Timeout { output })) => {
                Err(CodexErr::Sandbox(SandboxErr::Timeout { output }).into())
            }
            Err(CodexErr::Sandbox(error)) => {
                if sandbox_decision.escalate_on_failure {
                    self.retry_without_sandbox(
                        &request,
                        &config,
                        session,
                        context,
                        stdout_stream,
                        error,
                    )
                    .await
                } else {
                    let message = sandbox_failure_message(error);
                    Err(ExecError::rejection(message))
                }
            }
            Err(err) => Err(err.into()),
        }
    }

    /// Fallback path invoked when a sandboxed run is denied so the user can
    /// approve rerunning without isolation.
    async fn retry_without_sandbox(
        &self,
        request: &ExecutionRequest,
        config: &ExecutorConfig,
        session: &Session,
        context: &ExecCommandContext,
        stdout_stream: Option<StdoutStream>,
        sandbox_error: SandboxErr,
    ) -> Result<ExecToolCallOutput, ExecError> {
        session
            .notify_background_event(
                &context.sub_id,
                format!("Execution failed: {sandbox_error}"),
            )
            .await;
        let decision = session
            .request_command_approval(
                context.sub_id.to_string(),
                context.call_id.to_string(),
                request.approval_command.clone(),
                request.params.cwd.clone(),
                Some("command failed; retry without sandbox?".to_string()),
            )
            .await;

        context.otel_event_manager.tool_decision(
            &context.tool_name,
            &context.call_id,
            decision,
            ToolDecisionSource::User,
        );
        match decision {
            ReviewDecision::Approved | ReviewDecision::ApprovedForSession => {
                if matches!(decision, ReviewDecision::ApprovedForSession) {
                    self.approval_cache.insert(request.approval_command.clone());
                }
                session
                    .notify_background_event(&context.sub_id, "retrying command without sandbox")
                    .await;

                let retry_output = self
                    .spawn(
                        request.params.clone(),
                        SandboxType::None,
                        config,
                        stdout_stream,
                    )
                    .await?;

                Ok(retry_output)
            }
            ReviewDecision::Denied | ReviewDecision::Abort => {
                Err(ExecError::rejection("exec command rejected by user"))
            }
        }
    }

    async fn spawn(
        &self,
        params: ExecParams,
        sandbox: SandboxType,
        config: &ExecutorConfig,
        stdout_stream: Option<StdoutStream>,
    ) -> Result<ExecToolCallOutput, CodexErr> {
        process_exec_tool_call(
            params,
            sandbox,
            &config.sandbox_policy,
            &config.sandbox_cwd,
            &config.codex_linux_sandbox_exe,
            stdout_stream,
        )
        .await
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
