use std::collections::HashMap;

use codex_app_server_protocol::JSONRPCErrorError;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::ReadOnlyAccess;
use codex_protocol::protocol::SandboxPolicy;
use codex_sandboxing::SandboxCommand;
use codex_sandboxing::SandboxExecRequest;
use codex_sandboxing::SandboxManager;
use codex_sandboxing::SandboxTransformRequest;
use codex_sandboxing::SandboxablePreference;
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
        let cwd = current_sandbox_cwd().map_err(io_error)?;
        let cwd = AbsolutePathBuf::from_absolute_path(cwd.as_path())
            .map_err(|err| invalid_request(format!("current directory is not absolute: {err}")))?;
        let mut file_system_policy = sandbox.permissions.file_system_sandbox_policy();
        add_helper_runtime_permissions(
            &mut file_system_policy,
            helper_read_root(&self.runtime_paths),
            cwd.as_path(),
        );
        normalize_file_system_policy_root_aliases(&mut file_system_policy);
        let network_policy = NetworkSandboxPolicy::Restricted;
        let sandbox_policy =
            compatibility_sandbox_policy(&file_system_policy, network_policy, cwd.as_path());
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
            env: HashMap::new(),
            additional_permissions: None,
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
}

fn helper_read_root(runtime_paths: &ExecServerRuntimePaths) -> Option<AbsolutePathBuf> {
    runtime_paths
        .codex_self_exe
        .parent()
        .and_then(|path| AbsolutePathBuf::from_absolute_path(path).ok())
}

fn add_helper_runtime_permissions(
    file_system_policy: &mut FileSystemSandboxPolicy,
    helper_read_root: Option<AbsolutePathBuf>,
    cwd: &std::path::Path,
) {
    if !file_system_policy.has_full_disk_read_access() {
        let minimal_read_entry = FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Minimal,
            },
            access: FileSystemAccessMode::Read,
        };
        if !file_system_policy.entries.contains(&minimal_read_entry) {
            file_system_policy.entries.push(minimal_read_entry);
        }
    }

    let Some(helper_read_root) = helper_read_root else {
        return;
    };
    if file_system_policy.can_read_path_with_cwd(helper_read_root.as_path(), cwd) {
        return;
    }

    file_system_policy.entries.push(FileSystemSandboxEntry {
        path: FileSystemPath::Path {
            path: helper_read_root,
        },
        access: FileSystemAccessMode::Read,
    });
}

fn compatibility_sandbox_policy(
    file_system_policy: &FileSystemSandboxPolicy,
    network_policy: NetworkSandboxPolicy,
    cwd: &std::path::Path,
) -> SandboxPolicy {
    file_system_policy
        .to_legacy_sandbox_policy(network_policy, cwd)
        .unwrap_or_else(|_| compatibility_workspace_write_policy(file_system_policy, cwd))
}

fn compatibility_workspace_write_policy(
    file_system_policy: &FileSystemSandboxPolicy,
    cwd: &std::path::Path,
) -> SandboxPolicy {
    let read_only_access = if file_system_policy.has_full_disk_read_access() {
        ReadOnlyAccess::FullAccess
    } else {
        ReadOnlyAccess::Restricted {
            include_platform_defaults: file_system_policy.include_platform_defaults(),
            readable_roots: file_system_policy.get_readable_roots_with_cwd(cwd),
        }
    };
    let cwd_abs = AbsolutePathBuf::from_absolute_path(cwd).ok();
    let writable_roots = file_system_policy
        .get_writable_roots_with_cwd(cwd)
        .into_iter()
        .map(|root| root.root)
        .filter(|root| cwd_abs.as_ref() != Some(root))
        .collect();

    SandboxPolicy::WorkspaceWrite {
        writable_roots,
        read_only_access,
        network_access: false,
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
    }
}

fn normalize_file_system_policy_root_aliases(file_system_policy: &mut FileSystemSandboxPolicy) {
    for entry in &mut file_system_policy.entries {
        if let FileSystemPath::Path { path } = &mut entry.path {
            *path = normalize_top_level_alias(path.clone());
        }
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
    use codex_protocol::permissions::FileSystemAccessMode;
    use codex_protocol::permissions::FileSystemPath;
    use codex_protocol::permissions::FileSystemSandboxEntry;
    use codex_protocol::permissions::FileSystemSandboxPolicy;
    use codex_protocol::protocol::ReadOnlyAccess;
    use codex_protocol::protocol::SandboxPolicy;
    use codex_utils_absolute_path::AbsolutePathBuf;

    use crate::ExecServerRuntimePaths;

    use super::add_helper_runtime_permissions;
    use super::helper_read_root;

    #[test]
    fn helper_permissions_enable_minimal_reads_for_read_only_access() {
        let cwd = AbsolutePathBuf::from_absolute_path(std::env::temp_dir().as_path())
            .expect("absolute cwd");
        let sandbox_policy = SandboxPolicy::ReadOnly {
            access: ReadOnlyAccess::Restricted {
                include_platform_defaults: false,
                readable_roots: Vec::new(),
            },
            network_access: false,
        };
        let mut policy =
            FileSystemSandboxPolicy::from_legacy_sandbox_policy(&sandbox_policy, cwd.as_path());

        add_helper_runtime_permissions(&mut policy, /*helper_read_root*/ None, cwd.as_path());

        assert!(policy.include_platform_defaults());
    }

    #[test]
    fn helper_permissions_enable_minimal_reads_for_workspace_read_access() {
        let cwd = AbsolutePathBuf::from_absolute_path(std::env::temp_dir().as_path())
            .expect("absolute cwd");
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
        let mut policy =
            FileSystemSandboxPolicy::from_legacy_sandbox_policy(&sandbox_policy, cwd.as_path());

        add_helper_runtime_permissions(&mut policy, /*helper_read_root*/ None, cwd.as_path());

        assert!(policy.include_platform_defaults());
    }

    #[test]
    fn helper_permissions_preserve_existing_writes() {
        let codex_self_exe = std::env::current_exe().expect("current exe");
        let runtime_paths =
            ExecServerRuntimePaths::new(codex_self_exe, /*codex_linux_sandbox_exe*/ None)
                .expect("runtime paths");
        let cwd = AbsolutePathBuf::from_absolute_path(std::env::temp_dir().as_path())
            .expect("absolute cwd");
        let writable = cwd.join("writable");
        let sandbox_policy = SandboxPolicy::ReadOnly {
            access: ReadOnlyAccess::Restricted {
                include_platform_defaults: false,
                readable_roots: Vec::new(),
            },
            network_access: true,
        };
        let mut policy =
            FileSystemSandboxPolicy::from_legacy_sandbox_policy(&sandbox_policy, cwd.as_path());
        policy.entries.push(FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: writable.clone(),
            },
            access: FileSystemAccessMode::Write,
        });
        let readable = AbsolutePathBuf::from_absolute_path(
            runtime_paths
                .codex_self_exe
                .parent()
                .expect("current exe parent"),
        )
        .expect("absolute readable path");

        add_helper_runtime_permissions(
            &mut policy,
            helper_read_root(&runtime_paths),
            cwd.as_path(),
        );

        assert!(policy.can_read_path_with_cwd(readable.as_path(), cwd.as_path()));
        assert!(policy.can_write_path_with_cwd(writable.as_path(), cwd.as_path()));
    }

    #[test]
    fn helper_permissions_include_helper_read_root_without_additional_permissions() {
        let codex_self_exe = std::env::current_exe().expect("current exe");
        let runtime_paths =
            ExecServerRuntimePaths::new(codex_self_exe, /*codex_linux_sandbox_exe*/ None)
                .expect("runtime paths");
        let cwd = AbsolutePathBuf::from_absolute_path(std::env::temp_dir().as_path())
            .expect("absolute cwd");
        let sandbox_policy = SandboxPolicy::ReadOnly {
            access: ReadOnlyAccess::Restricted {
                include_platform_defaults: false,
                readable_roots: Vec::new(),
            },
            network_access: false,
        };
        let mut policy =
            FileSystemSandboxPolicy::from_legacy_sandbox_policy(&sandbox_policy, cwd.as_path());
        let readable = AbsolutePathBuf::from_absolute_path(
            runtime_paths
                .codex_self_exe
                .parent()
                .expect("current exe parent"),
        )
        .expect("absolute readable path");

        add_helper_runtime_permissions(
            &mut policy,
            helper_read_root(&runtime_paths),
            cwd.as_path(),
        );

        assert!(policy.can_read_path_with_cwd(readable.as_path(), cwd.as_path()));
    }
}
