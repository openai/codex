/*
Module: sandboxing

Core keeps the prepared sandbox request with its execution options until the
local launcher takes ownership. Remote launches retain the portable paths and
permissions sent to the executor.
*/

use crate::exec::ExecCapturePolicy;
use crate::exec::ExecExpiration;
use crate::exec::StdoutStream;
use crate::exec::capture_sandbox_request;
#[cfg(target_os = "macos")]
use crate::spawn::CODEX_SANDBOX_ENV_VAR;
use crate::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR;
use codex_file_system::FileSystemSandboxContext;
use codex_network_proxy::ManagedNetworkSandboxContext;
use codex_network_proxy::NetworkProxy;
use codex_network_proxy::RemoteNetworkProxyLaunchConfig;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::error::CodexErr;
use codex_protocol::exec_output::ExecToolCallOutput;
use codex_protocol::models::PermissionProfile;
pub use codex_protocol::models::SandboxPermissions;
use codex_protocol::sandbox::SandboxOverride;
use codex_sandboxing::SandboxExecRequest;
use codex_sandboxing::SandboxManager;
use codex_sandboxing::SandboxTransformRequest;
use codex_sandboxing::SandboxType;
use codex_sandboxing::WindowsSandboxFilesystemOverrides;
use codex_sandboxing::WrappedSandboxCommand;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use codex_utils_string::truncate_middle_with_token_budget;
use std::collections::HashMap;
use std::ffi::OsStr;

#[derive(Debug)]
pub(crate) struct ExecOptions {
    pub(crate) expiration: ExecExpiration,
    pub(crate) capture_policy: ExecCapturePolicy,
}

#[derive(Clone, Debug)]
pub(crate) struct ExecServerEnvConfig {
    pub(crate) policy: codex_exec_server::ExecEnvPolicy,
    pub(crate) local_policy_env: HashMap<String, String>,
}

#[derive(Debug)]
pub struct ExecRequest {
    // Only the capture adapter takes ownership. Other launchers borrow the prepared workload.
    launch: SandboxExecRequest,
    pub(crate) exec_server_env_config: Option<ExecServerEnvConfig>,
    pub(crate) exec_server_shell_snapshot: Option<codex_exec_server::ShellSnapshotRequest>,
    pub expiration: ExecExpiration,
    pub capture_policy: ExecCapturePolicy,
    pub windows_sandbox_workspace_roots: Vec<AbsolutePathBuf>,
    pub(crate) windows_sandbox_filesystem_overrides: Option<WindowsSandboxFilesystemOverrides>,
    pub(crate) exec_server_sandbox: Option<FileSystemSandboxContext>,
    pub(crate) exec_server_enforce_managed_network: bool,
    pub(crate) exec_server_managed_network: Option<ManagedNetworkSandboxContext>,
    pub(crate) exec_server_network_proxy: Option<RemoteNetworkProxyLaunchConfig>,
}

/// A prepared workload whose sandbox can be entered by spawning its command.
pub struct DirectSandboxCommand<'a> {
    pub program: &'a OsStr,
    pub args: &'a [String],
    pub cwd: &'a PathUri,
    pub env: &'a HashMap<String, String>,
    pub arg0: &'a Option<String>,
}

/// A local launch whose command has already been prepared for its selected sandbox.
#[derive(Debug)]
pub struct PreparedLocalExecRequest(ExecRequest);

impl PreparedLocalExecRequest {
    /// Attach capture options and native Windows launcher roots to a prepared command.
    /// A command that already owns a Windows token session must use its session launcher.
    pub async fn from_wrapped(
        command: WrappedSandboxCommand,
        expiration: ExecExpiration,
        capture_policy: ExecCapturePolicy,
        workspace_roots: &[PathUri],
    ) -> Result<Self, CodexErr> {
        let request = ExecRequest::from_sandbox_exec_request(
            command.into_command_request()?,
            ExecOptions {
                expiration,
                capture_policy,
            },
            workspace_roots,
        )
        .await?;
        Ok(Self(request))
    }

    /// Borrow the prepared command for a caller that owns its terminal and output streams.
    pub fn direct_command(&self) -> Option<DirectSandboxCommand<'_>> {
        let launch = &self.0.launch;
        if launch.sandbox == SandboxType::WindowsRestrictedToken {
            return None;
        }
        let (program, args) = launch.program_and_args()?;
        Some(DirectSandboxCommand {
            program,
            args,
            arg0: &launch.arg0,
            cwd: &launch.cwd,
            env: &launch.env,
        })
    }

    pub fn into_exec_request(self) -> ExecRequest {
        self.0
    }
}

impl AsRef<ExecRequest> for PreparedLocalExecRequest {
    fn as_ref(&self) -> &ExecRequest {
        &self.0
    }
}

impl ExecRequest {
    /// Borrow the host executable and arguments for a local launch.
    pub(crate) fn program_and_args(&self) -> Option<(&OsStr, &[String])> {
        self.launch.program_and_args()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        command: Vec<String>,
        cwd: AbsolutePathBuf,
        env: HashMap<String, String>,
        network: Option<NetworkProxy>,
        network_environment_id: Option<String>,
        expiration: ExecExpiration,
        capture_policy: ExecCapturePolicy,
        sandbox: SandboxType,
        windows_sandbox_workspace_roots: Vec<AbsolutePathBuf>,
        windows_sandbox_level: WindowsSandboxLevel,
        permission_profile: PermissionProfile,
        arg0: Option<String>,
    ) -> Self {
        let cwd = PathUri::from_abs_path(&cwd);
        let windows_sandbox_policy_cwd = cwd.clone();
        Self {
            launch: SandboxExecRequest {
                native_program: None,
                command,
                cwd,
                env,
                network,
                network_environment_id,
                sandbox,
                sandbox_policy_cwd: windows_sandbox_policy_cwd,
                windows_sandbox_level,
                permission_profile,
                arg0,
                sandbox_override: SandboxOverride::NoOverride,
            },
            expiration,
            capture_policy,
            windows_sandbox_workspace_roots,
            windows_sandbox_filesystem_overrides: None,
            exec_server_env_config: None,
            exec_server_shell_snapshot: None,
            exec_server_sandbox: None,
            exec_server_enforce_managed_network: false,
            exec_server_managed_network: None,
            exec_server_network_proxy: None,
        }
    }

    pub fn sandbox(&self) -> SandboxType {
        self.launch.sandbox
    }

    pub(crate) fn argv(&self) -> &[String] {
        &self.launch.command
    }

    pub(crate) fn cwd(&self) -> &PathUri {
        &self.launch.cwd
    }

    pub(crate) fn env(&self) -> &HashMap<String, String> {
        &self.launch.env
    }

    pub(crate) fn arg0(&self) -> &Option<String> {
        &self.launch.arg0
    }

    pub(crate) fn sandbox_policy_cwd(&self) -> &PathUri {
        &self.launch.sandbox_policy_cwd
    }

    pub(crate) fn has_network_proxy(&self) -> bool {
        self.launch.network.is_some()
    }

    /// Core's legacy native Windows session still needs its token and route-specific policy.
    pub(crate) fn native_windows(&self) -> Option<&SandboxExecRequest> {
        (self.sandbox() == SandboxType::WindowsRestrictedToken).then_some(&self.launch)
    }

    /// Prepare an authorized local attempt for a launcher that owns its pipes or terminal.
    pub(crate) async fn prepare_local_sandbox(
        manager: &SandboxManager,
        request: SandboxTransformRequest<'_>,
        options: ExecOptions,
        sandbox_override: SandboxOverride,
    ) -> Result<Self, CodexErr> {
        let workspace_roots = request.workspace_roots.unwrap_or_default();
        let mut request = manager.transform(request)?.into_command_request()?;
        request.sandbox_override = sandbox_override;
        Self::from_sandbox_exec_request(request, options, workspace_roots).await
    }

    pub(crate) async fn from_sandbox_exec_request(
        mut request: SandboxExecRequest,
        options: ExecOptions,
        workspace_roots: &[PathUri],
    ) -> Result<Self, CodexErr> {
        let windows_sandbox_workspace_roots =
            if request.sandbox == SandboxType::WindowsRestrictedToken {
                workspace_roots
                    .iter()
                    .map(PathUri::to_abs_path)
                    .collect::<std::io::Result<Vec<_>>>()
                    .map_err(|err| {
                        CodexErr::InvalidRequest(format!(
                            "invalid Windows sandbox workspace roots: {err}"
                        ))
                    })?
            } else {
                Vec::new()
            };
        codex_exec_server::run_integrity_checks((&request).into()).await;
        let sandbox = request.sandbox;
        let windows_sandbox_policy_cwd = &request.sandbox_policy_cwd;
        let windows_sandbox_level = request.windows_sandbox_level;
        let permission_profile = &request.permission_profile;
        let env = &mut request.env;
        let ExecOptions {
            expiration,
            capture_policy,
        } = options;
        let windows_sandbox_filesystem_overrides = if sandbox == SandboxType::WindowsRestrictedToken
        {
            let sandbox_policy_cwd = windows_sandbox_policy_cwd
                .to_abs_path()
                .map_err(|err| CodexErr::InvalidRequest(format!("invalid sandbox cwd: {err}")))?;
            WindowsSandboxFilesystemOverrides::resolve(
                sandbox,
                permission_profile,
                &sandbox_policy_cwd,
                windows_sandbox_level,
                env,
            )
            .map_err(|error| {
                CodexErr::UnsupportedOperation(
                    truncate_middle_with_token_budget(&error, /*max_tokens*/ 900).0,
                )
            })?
        } else {
            None
        };
        let network_sandbox_policy = permission_profile.network_sandbox_policy();
        if !network_sandbox_policy.is_enabled() {
            env.insert(
                CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR.to_string(),
                "1".to_string(),
            );
        }
        #[cfg(target_os = "macos")]
        if sandbox == SandboxType::MacosSeatbelt {
            env.insert(CODEX_SANDBOX_ENV_VAR.to_string(), "seatbelt".to_string());
        }
        Ok(Self {
            launch: request,
            expiration,
            capture_policy,
            windows_sandbox_workspace_roots,
            windows_sandbox_filesystem_overrides,
            exec_server_env_config: None,
            exec_server_shell_snapshot: None,
            exec_server_sandbox: None,
            exec_server_enforce_managed_network: false,
            exec_server_managed_network: None,
            exec_server_network_proxy: None,
        })
    }
}

pub async fn execute_env(
    exec_request: ExecRequest,
    stdout_stream: Option<StdoutStream>,
) -> codex_protocol::error::Result<ExecToolCallOutput> {
    execute_exec_request_with_after_spawn(exec_request, stdout_stream, /*after_spawn*/ None).await
}

pub async fn execute_exec_request_with_after_spawn(
    exec_request: ExecRequest,
    stdout_stream: Option<StdoutStream>,
    after_spawn: Option<Box<dyn FnOnce() + Send>>,
) -> codex_protocol::error::Result<ExecToolCallOutput> {
    capture_sandbox_request(
        exec_request.launch,
        ExecOptions {
            expiration: exec_request.expiration,
            capture_policy: exec_request.capture_policy,
        },
        exec_request.windows_sandbox_workspace_roots,
        exec_request.windows_sandbox_filesystem_overrides,
        stdout_stream,
        after_spawn,
    )
    .await
}
