mod backends;
mod cache;
mod runner;
mod sandbox;

pub(crate) use backends::ExecutionMode;
pub(crate) use runner::ExecutionRequest;
pub(crate) use runner::Executor;
pub(crate) use runner::ExecutorConfig;
pub(crate) use runner::normalize_exec_result;

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
}
