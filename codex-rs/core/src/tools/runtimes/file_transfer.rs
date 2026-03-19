use crate::exec::ExecExpiration;
use crate::exec::ExecToolCallOutput;
use crate::sandboxing::CommandSpec;
use crate::sandboxing::SandboxPermissions;
use crate::sandboxing::execute_env;
use crate::tools::network_approval::NetworkApprovalMode;
use crate::tools::network_approval::NetworkApprovalSpec;
use crate::tools::sandboxing::Approvable;
use crate::tools::sandboxing::ApprovalCtx;
use crate::tools::sandboxing::ExecApprovalRequirement;
use crate::tools::sandboxing::SandboxAttempt;
use crate::tools::sandboxing::Sandboxable;
use crate::tools::sandboxing::SandboxablePreference;
use crate::tools::sandboxing::ToolCtx;
use crate::tools::sandboxing::ToolError;
use crate::tools::sandboxing::ToolRuntime;
use codex_file_transfer::CODEX_CORE_FILE_TRANSFER_ARG1;
use codex_file_transfer::FileTransferRequest;
use codex_network_proxy::NetworkProxy;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ReviewDecision;
use futures::future::BoxFuture;
use std::collections::HashMap;
use std::path::PathBuf;

const DEFAULT_FILE_TRANSFER_TIMEOUT_MS: u64 = 120_000;

#[derive(Clone, Debug)]
pub struct InternalFileTransferRequest {
    pub request: FileTransferRequest,
    pub cwd: PathBuf,
    pub env: HashMap<String, String>,
    pub network: Option<NetworkProxy>,
    pub sandbox_permissions: SandboxPermissions,
    pub additional_permissions: Option<PermissionProfile>,
    pub codex_exe: Option<PathBuf>,
}

#[derive(Default)]
pub struct FileTransferRuntime;

impl FileTransferRuntime {
    pub fn new() -> Self {
        Self
    }

    fn build_command_spec(req: &InternalFileTransferRequest) -> Result<CommandSpec, ToolError> {
        let exe = if let Some(path) = &req.codex_exe {
            path.clone()
        } else {
            #[cfg(target_os = "windows")]
            {
                codex_windows_sandbox::resolve_current_exe_for_launch(&req.cwd, "codex.exe")
            }
            #[cfg(not(target_os = "windows"))]
            {
                std::env::current_exe().map_err(|err| {
                    ToolError::Rejected(format!("failed to determine codex exe: {err}"))
                })?
            }
        };
        let request_json = serde_json::to_string(&req.request).map_err(|err| {
            ToolError::Rejected(format!("failed to encode file transfer request: {err}"))
        })?;
        Ok(CommandSpec {
            program: exe.to_string_lossy().to_string(),
            args: vec![CODEX_CORE_FILE_TRANSFER_ARG1.to_string(), request_json],
            cwd: req.cwd.clone(),
            expiration: ExecExpiration::Timeout(std::time::Duration::from_millis(
                DEFAULT_FILE_TRANSFER_TIMEOUT_MS,
            )),
            env: req.env.clone(),
            sandbox_permissions: req.sandbox_permissions,
            additional_permissions: req.additional_permissions.clone(),
            justification: None,
        })
    }

    fn stdout_stream(ctx: &ToolCtx) -> Option<crate::exec::StdoutStream> {
        Some(crate::exec::StdoutStream {
            sub_id: ctx.turn.sub_id.clone(),
            call_id: ctx.call_id.clone(),
            tx_event: ctx.session.get_tx_event(),
        })
    }
}

impl Sandboxable for FileTransferRuntime {
    fn sandbox_preference(&self) -> SandboxablePreference {
        SandboxablePreference::Auto
    }

    fn escalate_on_failure(&self) -> bool {
        true
    }
}

impl Approvable<InternalFileTransferRequest> for FileTransferRuntime {
    type ApprovalKey = ();

    fn approval_keys(&self, _req: &InternalFileTransferRequest) -> Vec<Self::ApprovalKey> {
        vec![]
    }

    fn start_approval_async<'a>(
        &'a mut self,
        _req: &'a InternalFileTransferRequest,
        _ctx: ApprovalCtx<'a>,
    ) -> BoxFuture<'a, ReviewDecision> {
        Box::pin(async { ReviewDecision::Approved })
    }

    fn wants_no_sandbox_approval(&self, policy: AskForApproval) -> bool {
        match policy {
            AskForApproval::Never => false,
            AskForApproval::Granular(granular_config) => granular_config.allows_sandbox_approval(),
            AskForApproval::OnFailure => true,
            AskForApproval::OnRequest => true,
            AskForApproval::UnlessTrusted => true,
        }
    }

    fn exec_approval_requirement(
        &self,
        _req: &InternalFileTransferRequest,
    ) -> Option<ExecApprovalRequirement> {
        Some(ExecApprovalRequirement::Skip {
            bypass_sandbox: false,
            proposed_execpolicy_amendment: None,
        })
    }
}

impl ToolRuntime<InternalFileTransferRequest, ExecToolCallOutput> for FileTransferRuntime {
    fn network_approval_spec(
        &self,
        req: &InternalFileTransferRequest,
        _ctx: &ToolCtx,
    ) -> Option<NetworkApprovalSpec> {
        req.network.as_ref()?;
        Some(NetworkApprovalSpec {
            network: req.network.clone(),
            mode: NetworkApprovalMode::Deferred,
        })
    }

    async fn run(
        &mut self,
        req: &InternalFileTransferRequest,
        attempt: &SandboxAttempt<'_>,
        ctx: &ToolCtx,
    ) -> Result<ExecToolCallOutput, ToolError> {
        let spec = Self::build_command_spec(req)?;
        let env = attempt
            .env_for(spec, req.network.as_ref())
            .map_err(|err| ToolError::Codex(err.into()))?;
        execute_env(env, Self::stdout_stream(ctx))
            .await
            .map_err(ToolError::Codex)
    }
}
