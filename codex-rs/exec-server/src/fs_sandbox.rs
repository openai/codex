use std::collections::HashMap;

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
use codex_sandboxing::policy_transforms::merge_permission_profiles;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_absolute_path::canonicalize_preserving_symlinks;
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

const PATH_ENV_VAR: &str = "PATH";

#[derive(Clone, Debug)]
pub(crate) struct FileSystemSandboxRunner {
    runtime_paths: ExecServerRuntimePaths,
}

struct HelperSandboxInputs {
    sandbox_policy: SandboxPolicy,
    file_system_policy: FileSystemSandboxPolicy,
    network_policy: NetworkSandboxPolicy,
    cwd: AbsolutePathBuf,
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
        let HelperSandboxInputs {
            sandbox_policy,
            file_system_policy,
            network_policy,
            cwd,
        } = helper_sandbox_inputs(sandbox)?;
        let command = self.sandbox_exec_request(
            &sandbox_policy,
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
        let helper = &self.runtime_paths.codex_self_exe;
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
            env: helper_env(),
            additional_permissions: self.helper_permissions(
                sandbox_context.additional_permissions.as_ref(),
                /*include_helper_read_root*/ !sandbox_context.use_legacy_landlock,
            ),
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

    fn helper_permissions(
        &self,
        additional_permissions: Option<&PermissionProfile>,
        include_helper_read_root: bool,
    ) -> Option<PermissionProfile> {
        let inherited_permissions = additional_permissions
            .map(|permissions| PermissionProfile {
                network: None,
                file_system: permissions.file_system.clone(),
            })
            .filter(|permissions| !permissions.is_empty());
        let helper_permissions = include_helper_read_root
            .then(|| {
                self.runtime_paths
                    .codex_self_exe
                    .parent()
                    .and_then(|path| AbsolutePathBuf::from_absolute_path(path).ok())
            })
            .flatten()
            .map(|helper_read_root| PermissionProfile {
                network: None,
                file_system: Some(FileSystemPermissions {
                    read: Some(vec![helper_read_root]),
                    write: None,
                }),
            });

        merge_permission_profiles(inherited_permissions.as_ref(), helper_permissions.as_ref())
    }
}

fn helper_sandbox_inputs(
    sandbox: &FileSystemSandboxContext,
) -> Result<HelperSandboxInputs, JSONRPCErrorError> {
    let sandbox_policy = normalize_sandbox_policy_root_aliases(
        sandbox_policy_with_helper_runtime_defaults(&sandbox.sandbox_policy),
    );
    let cwd = match &sandbox.sandbox_policy_cwd {
        Some(cwd) => cwd.clone(),
        None if sandbox.file_system_sandbox_policy.is_some() => {
            return Err(invalid_request(
                "fileSystemSandboxPolicy requires sandboxPolicyCwd".to_string(),
            ));
        }
        None => {
            let cwd = current_sandbox_cwd().map_err(io_error)?;
            AbsolutePathBuf::from_absolute_path(cwd.as_path()).map_err(|err| {
                invalid_request(format!("current directory is not absolute: {err}"))
            })?
        }
    };
    let file_system_policy = sandbox
        .file_system_sandbox_policy
        .clone()
        .unwrap_or_else(|| {
            FileSystemSandboxPolicy::from_legacy_sandbox_policy(&sandbox_policy, cwd.as_path())
        });
    Ok(HelperSandboxInputs {
        sandbox_policy,
        file_system_policy,
        network_policy: NetworkSandboxPolicy::Restricted,
        cwd,
    })
}

fn helper_env() -> HashMap<String, String> {
    std::env::var_os(PATH_ENV_VAR)
        .map(|path| {
            HashMap::from([(
                PATH_ENV_VAR.to_string(),
                path.to_string_lossy().into_owned(),
            )])
        })
        .unwrap_or_default()
}

fn normalize_sandbox_policy_root_aliases(sandbox_policy: SandboxPolicy) -> SandboxPolicy {
    let mut sandbox_policy = sandbox_policy;
    match &mut sandbox_policy {
        SandboxPolicy::ReadOnly {
            access: ReadOnlyAccess::Restricted { readable_roots, .. },
            ..
        } => {
            normalize_root_aliases(readable_roots);
        }
        SandboxPolicy::WorkspaceWrite {
            writable_roots,
            read_only_access,
            ..
        } => {
            normalize_root_aliases(writable_roots);
            if let ReadOnlyAccess::Restricted { readable_roots, .. } = read_only_access {
                normalize_root_aliases(readable_roots);
            }
        }
        _ => {}
    }
    sandbox_policy
}

fn normalize_root_aliases(paths: &mut Vec<AbsolutePathBuf>) {
    for path in paths {
        *path = normalize_top_level_alias(path.clone());
    }
}

fn normalize_top_level_alias(path: AbsolutePathBuf) -> AbsolutePathBuf {
    let raw_path = path.to_path_buf();
    for ancestor in raw_path.ancestors() {
        if std::fs::symlink_metadata(ancestor).is_err() {
            continue;
        }
        let Ok(normalized_ancestor) = canonicalize_preserving_symlinks(ancestor) else {
            continue;
        };
        if normalized_ancestor == ancestor {
            continue;
        }
        let Ok(suffix) = raw_path.strip_prefix(ancestor) else {
            continue;
        };
        if let Ok(normalized_path) =
            AbsolutePathBuf::from_absolute_path(normalized_ancestor.join(suffix))
        {
            return normalized_path;
        }
    }
    path
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

fn sandbox_policy_with_helper_runtime_defaults(sandbox_policy: &SandboxPolicy) -> SandboxPolicy {
    let mut sandbox_policy = sandbox_policy.clone();
    match &mut sandbox_policy {
        SandboxPolicy::ReadOnly {
            access,
            network_access,
        } => {
            enable_platform_defaults(access);
            *network_access = false;
        }
        SandboxPolicy::WorkspaceWrite {
            read_only_access,
            network_access,
            ..
        } => {
            enable_platform_defaults(read_only_access);
            *network_access = false;
        }
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
#[path = "fs_sandbox_tests.rs"]
mod tests;
