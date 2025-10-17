//! Executor: centralized sandbox policy, approvals, and execution planning.
//!
//! Purpose and responsibilities
//! - Normalizes per‑mode parameters via backends (`backends.rs`).
//! - Selects sandbox placement and handles approvals (`sandbox.rs`).
//! - Produces an `ExecutionPlan` (single source of truth for policy) that
//!   callers can either execute directly via `Executor::run` (non‑PTY, piped),
//!   or consume piecemeal (e.g., Unified Exec) to launch with a PTY while
//!   retaining consistent policy decisions.
//!
//! Key types
//! - `ExecutionMode`: `Shell`, `InteractiveShell`, `ApplyPatch`.
//! - `ExecutionRequest`: inputs + mode + stdout streaming preference.
//! - `ExecutionPlan`: immutable snapshot of the policy decision and helpers to
//!   build a `SandboxLaunch` and retry without a sandbox when approved.
//! - `SandboxLaunch`: concrete program/args/env to execute under the chosen
//!   sandbox.
//!
//! Typical flows
//! - Non‑PTY (piped): `Executor::run(request, …)` handles plan → launch →
//!   execution and post‑processing, including converting sandbox failures into
//!   user‑facing messages.
//! - PTY (Unified Exec): build the plan with `prepare_execution_plan` and then
//!   use `ExecutionPlan::attempt_with_retry_if` to drive the spawn with
//!   `SandboxLaunch`; PTY I/O and buffering remain the caller’s responsibility.
//!
//! This separation keeps sandbox logic and user interaction consistent while
//! allowing different transports (piped vs PTY) to manage their own lifecycles.

mod backends;
mod cache;
mod runner;
mod sandbox;

pub(crate) use backends::ExecutionMode;
pub(crate) use runner::ExecutionPlan;
pub(crate) use runner::ExecutionRequest;
pub(crate) use runner::Executor;
pub(crate) use runner::ExecutorConfig;
pub(crate) use runner::normalize_exec_result;
pub(crate) use sandbox::SandboxLaunch;
pub(crate) use sandbox::SandboxLaunchError;
pub(crate) use sandbox::build_launch_for_sandbox;

pub(crate) mod linkers {
    use crate::exec::ExecParams;
    use crate::exec::StdoutStream;
    use crate::executor::backends::ExecutionMode;
    use crate::executor::runner::ExecutionRequest;
    use crate::tools::context::ExecCommandContext;

    pub struct PreparedExec {
        pub(crate) context: ExecCommandContext,
        pub(crate) request: ExecutionRequest,
    }

    impl PreparedExec {
        pub fn new(
            context: ExecCommandContext,
            params: ExecParams,
            approval_command: Vec<String>,
            mode: ExecutionMode,
            stdout_stream: Option<StdoutStream>,
            use_shell_profile: bool,
        ) -> Self {
            let request = ExecutionRequest {
                params,
                approval_command,
                mode,
                stdout_stream,
                use_shell_profile,
            };

            Self { context, request }
        }
    }
}

pub mod errors {
    use crate::error::CodexErr;
    use crate::executor::SandboxLaunchError;
    use crate::function_tool::FunctionCallError;
    use thiserror::Error;

    #[derive(Debug, Error)]
    pub enum ExecError {
        #[error(transparent)]
        Function(#[from] FunctionCallError),
        #[error(transparent)]
        Codex(#[from] CodexErr),
    }

    impl ExecError {
        pub(crate) fn rejection(msg: impl Into<String>) -> Self {
            FunctionCallError::RespondToModel(msg.into()).into()
        }
    }

    impl From<SandboxLaunchError> for ExecError {
        fn from(err: SandboxLaunchError) -> Self {
            CodexErr::from(err).into()
        }
    }
}
