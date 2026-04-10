use std::collections::HashMap;
use std::path::Path;

use codex_app_server_protocol::JSONRPCErrorError;
use codex_protocol::models::FileSystemPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::ReadOnlyAccess;
use codex_protocol::protocol::SandboxPolicy;
use codex_sandboxing::SandboxCommand;
use codex_sandboxing::SandboxExecRequest;
use codex_sandboxing::SandboxManager;
use codex_sandboxing::SandboxTransformRequest;
use codex_sandboxing::SandboxablePreference;
use codex_utils_absolute_path::AbsolutePathBuf;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::ExecServerRuntimePaths;
use crate::FileSystemSandboxContext;
use crate::fs_helper::CODEX_FS_HELPER_ARG1;
use crate::fs_helper::FsHelperPayload;
use crate::fs_helper::FsHelperRequest;
use crate::fs_helper::FsHelperResponse;
use crate::local_file_system::current_sandbox_cwd;
use crate::rpc::internal_error;
use crate::rpc::invalid_request;

#[derive(Clone, Debug)]
pub(crate) struct FileSystemSandboxRunner {
    runtime_paths: ExecServerRuntimePaths,
}

impl FileSystemSandboxRunner {
    pub(crate) fn new(runtime_paths: ExecServerRuntimePaths) -> Self {
        Self { runtime_paths }
    }

    pub(crate) async fn run(
        &self,
        sandbox: &FileSystemSandboxContext,
        request: FsHelperRequest,
    ) -> Result<FsHelperPayload, JSONRPCErrorError> {
        let helper_sandbox_policy =
            sandbox_policy_with_helper_runtime_defaults(&sandbox.sandbox_policy);
        let cwd = current_sandbox_cwd().map_err(io_error)?;
        let cwd = AbsolutePathBuf::from_absolute_path(cwd.as_path())
            .map_err(|err| invalid_request(format!("current directory is not absolute: {err}")))?;
        let file_system_policy = FileSystemSandboxPolicy::from_legacy_sandbox_policy(
            &helper_sandbox_policy,
            cwd.as_path(),
        );
        let network_policy = NetworkSandboxPolicy::from(&helper_sandbox_policy);
        let command = self.sandbox_exec_request(
            &helper_sandbox_policy,
            &file_system_policy,
            network_policy,
            &cwd,
            sandbox,
        )?;
        let request_json = serde_json::to_vec(&request).map_err(json_error)?;
        run_command(command, request_json).await
    }

    fn sandbox_exec_request(
        &self,
        sandbox_policy: &SandboxPolicy,
        file_system_policy: &FileSystemSandboxPolicy,
        network_policy: NetworkSandboxPolicy,
        cwd: &AbsolutePathBuf,
        sandbox_context: &FileSystemSandboxContext,
    ) -> Result<SandboxExecRequest, JSONRPCErrorError> {
        let helper = self.helper_program();
        let sandbox_manager = SandboxManager::new();
        let sandbox = sandbox_manager.select_initial(
            file_system_policy,
            network_policy,
            SandboxablePreference::Auto,
            sandbox_context.windows_sandbox_level,
            /*has_managed_network_requirements*/ false,
        );
        let command = SandboxCommand {
            program: helper.as_path().as_os_str().to_owned(),
            args: vec![CODEX_FS_HELPER_ARG1.to_string()],
            cwd: cwd.clone(),
            env: HashMap::new(),
            additional_permissions: Some(self.helper_permissions(
                helper.as_path(),
                sandbox_context.additional_permissions.as_ref(),
            )),
        };
        sandbox_manager
            .transform(SandboxTransformRequest {
                command,
                policy: sandbox_policy,
                file_system_policy,
                network_policy,
                sandbox,
                enforce_managed_network: false,
                network: None,
                sandbox_policy_cwd: cwd.as_path(),
                codex_linux_sandbox_exe: self.runtime_paths.codex_linux_sandbox_exe.as_deref(),
                use_legacy_landlock: sandbox_context.use_legacy_landlock,
                windows_sandbox_level: sandbox_context.windows_sandbox_level,
                windows_sandbox_private_desktop: sandbox_context.windows_sandbox_private_desktop,
            })
            .map_err(|err| invalid_request(format!("failed to prepare fs sandbox: {err}")))
    }

    fn helper_program(&self) -> &AbsolutePathBuf {
        &self.runtime_paths.codex_self_exe
    }

    fn helper_permissions(
        &self,
        helper_path: &Path,
        additional_permissions: Option<&PermissionProfile>,
    ) -> PermissionProfile {
        let mut profile = additional_permissions.cloned().unwrap_or_default();
        let mut read = profile
            .file_system
            .as_ref()
            .and_then(|permissions| permissions.read.clone())
            .unwrap_or_default();

        for path in [
            Some(helper_path),
            self.runtime_paths.codex_linux_sandbox_exe.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            for readable_path in readable_helper_paths(path) {
                if !read.contains(&readable_path) {
                    read.push(readable_path);
                }
            }
        }

        let file_system = profile
            .file_system
            .get_or_insert_with(FileSystemPermissions::default);
        file_system.read = Some(read);
        profile
    }
}

async fn run_command(
    command: SandboxExecRequest,
    request_json: Vec<u8>,
) -> Result<FsHelperPayload, JSONRPCErrorError> {
    let mut child = spawn_command(command)?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| internal_error("failed to open fs sandbox helper stdin".to_string()))?;
    stdin.write_all(&request_json).await.map_err(io_error)?;
    stdin.shutdown().await.map_err(io_error)?;
    drop(stdin);

    let output = child.wait_with_output().await.map_err(io_error)?;
    if !output.status.success() {
        return Err(internal_error(format!(
            "fs sandbox helper failed with status {status}: {stderr}",
            status = output.status,
            stderr = String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let response: FsHelperResponse = serde_json::from_slice(&output.stdout).map_err(json_error)?;
    match response {
        FsHelperResponse::Ok(payload) => Ok(payload),
        FsHelperResponse::Error(error) => Err(error),
    }
}

fn spawn_command(
    SandboxExecRequest {
        command: argv,
        cwd,
        env,
        arg0,
        ..
    }: SandboxExecRequest,
) -> Result<tokio::process::Child, JSONRPCErrorError> {
    let Some((program, args)) = argv.split_first() else {
        return Err(invalid_request("fs sandbox command was empty".to_string()));
    };
    let mut command = Command::new(program);
    #[cfg(unix)]
    if let Some(arg0) = arg0 {
        command.arg0(arg0);
    }
    #[cfg(not(unix))]
    let _ = arg0;
    command.args(args);
    command.current_dir(cwd.as_path());
    command.env_clear();
    command.envs(env);
    command.stdin(std::process::Stdio::piped());
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());
    command.spawn().map_err(io_error)
}

fn readable_helper_paths(path: &Path) -> Vec<AbsolutePathBuf> {
    let mut paths = Vec::new();
    push_readable_path(&mut paths, path);
    if let Ok(canonical) = std::fs::canonicalize(path) {
        push_readable_path(&mut paths, canonical.as_path());
    }
    paths
}

fn push_readable_path(paths: &mut Vec<AbsolutePathBuf>, path: &Path) {
    if let Ok(path) = AbsolutePathBuf::from_absolute_path(path)
        && !paths.contains(&path)
    {
        paths.push(path);
    }
}

fn sandbox_policy_with_helper_runtime_defaults(sandbox_policy: &SandboxPolicy) -> SandboxPolicy {
    let mut sandbox_policy = sandbox_policy.clone();
    match &mut sandbox_policy {
        SandboxPolicy::ReadOnly { access, .. } => enable_platform_defaults(access),
        SandboxPolicy::WorkspaceWrite {
            read_only_access, ..
        } => enable_platform_defaults(read_only_access),
        SandboxPolicy::DangerFullAccess | SandboxPolicy::ExternalSandbox { .. } => {}
    }
    sandbox_policy
}

fn enable_platform_defaults(access: &mut ReadOnlyAccess) {
    if let ReadOnlyAccess::Restricted {
        include_platform_defaults,
        ..
    } = access
    {
        *include_platform_defaults = true;
    }
}

fn io_error(err: std::io::Error) -> JSONRPCErrorError {
    internal_error(err.to_string())
}

fn json_error(err: serde_json::Error) -> JSONRPCErrorError {
    internal_error(format!(
        "failed to encode or decode fs sandbox helper message: {err}"
    ))
}

#[cfg(test)]
mod tests {
    use codex_protocol::protocol::ReadOnlyAccess;
    use codex_protocol::protocol::SandboxPolicy;
    use pretty_assertions::assert_eq;

    use super::sandbox_policy_with_helper_runtime_defaults;

    #[test]
    fn helper_sandbox_policy_enables_platform_defaults_for_read_only_access() {
        let sandbox_policy = SandboxPolicy::ReadOnly {
            access: ReadOnlyAccess::Restricted {
                include_platform_defaults: false,
                readable_roots: Vec::new(),
            },
            network_access: false,
        };

        let updated = sandbox_policy_with_helper_runtime_defaults(&sandbox_policy);

        assert_eq!(
            updated,
            SandboxPolicy::ReadOnly {
                access: ReadOnlyAccess::Restricted {
                    include_platform_defaults: true,
                    readable_roots: Vec::new(),
                },
                network_access: false,
            }
        );
    }

    #[test]
    fn helper_sandbox_policy_enables_platform_defaults_for_workspace_read_access() {
        let sandbox_policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: Vec::new(),
            read_only_access: ReadOnlyAccess::Restricted {
                include_platform_defaults: false,
                readable_roots: Vec::new(),
            },
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        };

        let updated = sandbox_policy_with_helper_runtime_defaults(&sandbox_policy);

        assert_eq!(
            updated,
            SandboxPolicy::WorkspaceWrite {
                writable_roots: Vec::new(),
                read_only_access: ReadOnlyAccess::Restricted {
                    include_platform_defaults: true,
                    readable_roots: Vec::new(),
                },
                network_access: false,
                exclude_tmpdir_env_var: true,
                exclude_slash_tmp: true,
            }
        );
    }
}
