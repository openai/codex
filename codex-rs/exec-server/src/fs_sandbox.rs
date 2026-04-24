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
use crate::file_system::file_system_policy_has_cwd_dependent_entries;
use crate::fs_helper::CODEX_FS_HELPER_ARG1;
use crate::fs_helper::FsHelperPayload;
use crate::fs_helper::FsHelperRequest;
use crate::fs_helper::FsHelperResponse;
use crate::local_file_system::current_sandbox_cwd;
use crate::rpc::internal_error;
use crate::rpc::invalid_request;

const FS_HELPER_ENV_ALLOWLIST: &[&str] = &["PATH", "TMPDIR", "TMP", "TEMP"];

#[derive(Clone, Debug)]
pub(crate) struct FileSystemSandboxRunner {
    runtime_paths: ExecServerRuntimePaths,
    helper_env: HashMap<String, String>,
}

impl FileSystemSandboxRunner {
    pub(crate) fn new(runtime_paths: ExecServerRuntimePaths) -> Self {
        Self {
            runtime_paths,
            helper_env: helper_env(),
        }
    }

    pub(crate) async fn run(
        &self,
        sandbox: &FileSystemSandboxContext,
        request: FsHelperRequest,
    ) -> Result<FsHelperPayload, JSONRPCErrorError> {
        let cwd = sandbox_cwd(sandbox)?;
        let mut file_system_policy = sandbox.permissions.file_system_sandbox_policy();
        let helper_read_roots = if sandbox.use_legacy_landlock {
            Vec::new()
        } else {
            helper_read_roots(&self.runtime_paths)
        };
        add_helper_runtime_permissions(&mut file_system_policy, &helper_read_roots, cwd.as_path());
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
            env: self.helper_env.clone(),
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

fn sandbox_cwd(sandbox: &FileSystemSandboxContext) -> Result<AbsolutePathBuf, JSONRPCErrorError> {
    if let Some(cwd) = &sandbox.cwd {
        return Ok(cwd.clone());
    }

    let file_system_policy = sandbox.permissions.file_system_sandbox_policy();
    if file_system_policy_has_cwd_dependent_entries(&file_system_policy) {
        return Err(invalid_request(
            "file system sandbox context with cwd-relative permissions requires cwd".to_string(),
        ));
    }

    let cwd = current_sandbox_cwd().map_err(io_error)?;
    AbsolutePathBuf::from_absolute_path(cwd.as_path())
        .map_err(|err| invalid_request(format!("current directory is not absolute: {err}")))
}

fn helper_read_roots(runtime_paths: &ExecServerRuntimePaths) -> Vec<AbsolutePathBuf> {
    let mut roots = Vec::new();
    for path in std::iter::once(runtime_paths.codex_self_exe.as_path())
        .chain(runtime_paths.codex_linux_sandbox_exe.as_deref().into_iter())
    {
        if let Some(parent) = path.parent()
            && let Ok(root) = AbsolutePathBuf::from_absolute_path(parent)
            && !roots.contains(&root)
        {
            roots.push(root);
        }
    }
    roots
}

fn add_helper_runtime_permissions(
    file_system_policy: &mut FileSystemSandboxPolicy,
    helper_read_roots: &[AbsolutePathBuf],
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

    for helper_read_root in helper_read_roots {
        if file_system_policy.can_read_path_with_cwd(helper_read_root.as_path(), cwd) {
            continue;
        }

        file_system_policy.entries.push(FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: helper_read_root.clone(),
            },
            access: FileSystemAccessMode::Read,
        });
    }
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

fn helper_env() -> HashMap<String, String> {
    helper_env_from_vars(std::env::vars_os())
}

fn helper_env_from_vars(
    vars: impl IntoIterator<Item = (std::ffi::OsString, std::ffi::OsString)>,
) -> HashMap<String, String> {
    vars.into_iter()
        .filter_map(|(key, value)| {
            let key = key.to_string_lossy();
            helper_env_key_is_allowed(&key)
                .then(|| (key.into_owned(), value.to_string_lossy().into_owned()))
        })
        .collect()
}

fn helper_env_key_is_allowed(key: &str) -> bool {
    FS_HELPER_ENV_ALLOWLIST.contains(&key) || (cfg!(windows) && key.eq_ignore_ascii_case("PATH"))
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
    use std::collections::HashMap;
    use std::ffi::OsString;

    use codex_protocol::models::PermissionProfile;
    use codex_protocol::permissions::FileSystemAccessMode;
    use codex_protocol::permissions::FileSystemPath;
    use codex_protocol::permissions::FileSystemSandboxEntry;
    use codex_protocol::permissions::FileSystemSandboxPolicy;
    use codex_protocol::permissions::FileSystemSpecialPath;
    use codex_protocol::permissions::NetworkSandboxPolicy;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;

    use crate::ExecServerRuntimePaths;

    use super::FileSystemSandboxRunner;
    use super::add_helper_runtime_permissions;
    use super::compatibility_sandbox_policy;
    use super::helper_env;
    use super::helper_env_from_vars;
    use super::helper_env_key_is_allowed;
    use super::helper_read_roots;
    use super::sandbox_cwd;

    #[test]
    fn helper_permissions_enable_minimal_reads_for_restricted_profile() {
        let cwd = AbsolutePathBuf::from_absolute_path(std::env::temp_dir().as_path())
            .expect("absolute cwd");
        let mut policy = restricted_policy(Vec::new());

        add_helper_runtime_permissions(&mut policy, /*helper_read_roots*/ &[], cwd.as_path());

        assert!(policy.include_platform_defaults());
    }

    #[test]
    fn helper_permissions_enable_minimal_reads_for_restricted_profile_with_writes() {
        let cwd = AbsolutePathBuf::from_absolute_path(std::env::temp_dir().as_path())
            .expect("absolute cwd");
        let mut policy = restricted_policy(vec![path_entry(
            cwd.join("writable"),
            FileSystemAccessMode::Write,
        )]);

        add_helper_runtime_permissions(&mut policy, /*helper_read_roots*/ &[], cwd.as_path());

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
        let mut policy = restricted_policy(vec![path_entry(
            writable.clone(),
            FileSystemAccessMode::Write,
        )]);
        let readable = AbsolutePathBuf::from_absolute_path(
            runtime_paths
                .codex_self_exe
                .parent()
                .expect("current exe parent"),
        )
        .expect("absolute readable path");

        add_helper_runtime_permissions(
            &mut policy,
            &helper_read_roots(&runtime_paths),
            cwd.as_path(),
        );

        assert!(policy.can_read_path_with_cwd(readable.as_path(), cwd.as_path()));
        assert!(policy.can_write_path_with_cwd(writable.as_path(), cwd.as_path()));
    }

    #[test]
    fn helper_env_carries_only_allowlisted_runtime_vars() {
        let env = helper_env();

        let expected = std::env::vars_os()
            .filter_map(|(key, value)| {
                let key = key.to_string_lossy();
                helper_env_key_is_allowed(&key)
                    .then(|| (key.into_owned(), value.to_string_lossy().into_owned()))
            })
            .collect::<HashMap<_, _>>();

        assert_eq!(env, expected);
    }

    #[test]
    fn helper_env_preserves_path_for_system_bwrap_discovery_without_leaking_secrets() {
        let env = helper_env_from_vars(
            [
                ("PATH", "/usr/bin:/bin"),
                ("TMPDIR", "/tmp/codex"),
                ("TMP", "/tmp"),
                ("TEMP", "/tmp"),
                ("HOME", "/home/user"),
                ("OPENAI_API_KEY", "secret"),
                ("HTTPS_PROXY", "http://proxy.example"),
            ]
            .map(|(key, value)| (OsString::from(key), OsString::from(value))),
        );

        assert_eq!(
            env,
            HashMap::from([
                ("PATH".to_string(), "/usr/bin:/bin".to_string()),
                ("TMPDIR".to_string(), "/tmp/codex".to_string()),
                ("TMP".to_string(), "/tmp".to_string()),
                ("TEMP".to_string(), "/tmp".to_string()),
            ])
        );
    }

    #[cfg(windows)]
    #[test]
    fn helper_env_preserves_windows_path_key_for_system_bwrap_discovery() {
        let env = helper_env_from_vars(
            [
                ("Path", r"C:\Windows\System32"),
                ("PATH_INJECTION", "bad"),
                ("OPENAI_API_KEY", "secret"),
            ]
            .map(|(key, value)| (OsString::from(key), OsString::from(value))),
        );

        assert_eq!(
            env,
            HashMap::from([("Path".to_string(), r"C:\Windows\System32".to_string())])
        );
    }

    #[test]
    fn sandbox_exec_request_carries_helper_env() {
        let Some((path_key, path)) = std::env::vars_os().find(|(key, _)| {
            let key = key.to_string_lossy();
            key == "PATH" || (cfg!(windows) && key.eq_ignore_ascii_case("PATH"))
        }) else {
            return;
        };
        let path_key = path_key.to_string_lossy().into_owned();
        let path = path.to_string_lossy().into_owned();
        let codex_self_exe = std::env::current_exe().expect("current exe");
        let runtime_paths =
            ExecServerRuntimePaths::new(codex_self_exe.clone(), Some(codex_self_exe))
                .expect("runtime paths");
        let runner = FileSystemSandboxRunner::new(runtime_paths);
        let cwd = AbsolutePathBuf::current_dir().expect("cwd");
        let file_system_policy =
            restricted_policy(vec![path_entry(cwd.clone(), FileSystemAccessMode::Write)]);
        let network_policy = NetworkSandboxPolicy::Restricted;
        let sandbox_policy =
            compatibility_sandbox_policy(&file_system_policy, network_policy, cwd.as_path());
        let sandbox_context = sandbox_context_with_cwd(&file_system_policy, cwd.clone());

        let request = runner
            .sandbox_exec_request(
                &sandbox_policy,
                &file_system_policy,
                network_policy,
                &cwd,
                &sandbox_context,
            )
            .expect("sandbox exec request");

        assert_eq!(request.env.get(&path_key), Some(&path));
    }

    #[test]
    fn sandbox_cwd_uses_context_cwd() {
        let cwd = AbsolutePathBuf::from_absolute_path(std::env::temp_dir().as_path())
            .expect("absolute cwd");
        let policy = restricted_policy(vec![special_entry(
            FileSystemSpecialPath::CurrentWorkingDirectory,
            FileSystemAccessMode::Write,
        )]);
        let sandbox_context = sandbox_context_with_cwd(&policy, cwd.clone());

        assert_eq!(sandbox_cwd(&sandbox_context).expect("sandbox cwd"), cwd);
    }

    #[test]
    fn sandbox_cwd_rejects_cwd_dependent_profile_without_context_cwd() {
        let policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::CurrentWorkingDirectory,
            },
            access: FileSystemAccessMode::Write,
        }]);
        let sandbox_context = crate::FileSystemSandboxContext::from_permission_profile(
            PermissionProfile::from_runtime_permissions(&policy, NetworkSandboxPolicy::Restricted),
        );

        let err = sandbox_cwd(&sandbox_context).expect_err("missing cwd should be rejected");

        assert_eq!(
            err.message,
            "file system sandbox context with cwd-relative permissions requires cwd"
        );
    }

    #[test]
    fn helper_permissions_include_helper_read_root_without_additional_permissions() {
        let codex_self_exe = std::env::current_exe().expect("current exe");
        let runtime_paths =
            ExecServerRuntimePaths::new(codex_self_exe, /*codex_linux_sandbox_exe*/ None)
                .expect("runtime paths");
        let cwd = AbsolutePathBuf::from_absolute_path(std::env::temp_dir().as_path())
            .expect("absolute cwd");
        let mut policy = restricted_policy(Vec::new());
        let readable = AbsolutePathBuf::from_absolute_path(
            runtime_paths
                .codex_self_exe
                .parent()
                .expect("current exe parent"),
        )
        .expect("absolute readable path");

        add_helper_runtime_permissions(
            &mut policy,
            &helper_read_roots(&runtime_paths),
            cwd.as_path(),
        );

        assert!(policy.can_read_path_with_cwd(readable.as_path(), cwd.as_path()));
    }

    #[test]
    fn helper_permissions_include_linux_sandbox_alias_parent() {
        let root = tempfile::tempdir().expect("temp dir");
        let codex_self_exe = root.path().join("bin").join("codex");
        let codex_linux_sandbox_exe = root.path().join("aliases").join("codex-linux-sandbox");
        let runtime_paths =
            ExecServerRuntimePaths::new(codex_self_exe, Some(codex_linux_sandbox_exe))
                .expect("runtime paths");
        let cwd = AbsolutePathBuf::from_absolute_path(std::env::temp_dir().as_path())
            .expect("absolute cwd");
        let mut policy = restricted_policy(Vec::new());
        let codex_parent = AbsolutePathBuf::from_absolute_path(root.path().join("bin"))
            .expect("absolute codex parent");
        let alias_parent = AbsolutePathBuf::from_absolute_path(root.path().join("aliases"))
            .expect("absolute alias parent");

        add_helper_runtime_permissions(
            &mut policy,
            &helper_read_roots(&runtime_paths),
            cwd.as_path(),
        );

        assert!(policy.can_read_path_with_cwd(codex_parent.as_path(), cwd.as_path()));
        assert!(policy.can_read_path_with_cwd(alias_parent.as_path(), cwd.as_path()));
    }

    fn restricted_policy(entries: Vec<FileSystemSandboxEntry>) -> FileSystemSandboxPolicy {
        FileSystemSandboxPolicy::restricted(entries)
    }

    fn sandbox_context_with_cwd(
        policy: &FileSystemSandboxPolicy,
        cwd: AbsolutePathBuf,
    ) -> crate::FileSystemSandboxContext {
        crate::FileSystemSandboxContext::from_permission_profile_with_cwd(
            PermissionProfile::from_runtime_permissions(policy, NetworkSandboxPolicy::Restricted),
            cwd,
        )
    }

    fn path_entry(path: AbsolutePathBuf, access: FileSystemAccessMode) -> FileSystemSandboxEntry {
        FileSystemSandboxEntry {
            path: FileSystemPath::Path { path },
            access,
        }
    }

    fn special_entry(
        value: FileSystemSpecialPath,
        access: FileSystemAccessMode,
    ) -> FileSystemSandboxEntry {
        FileSystemSandboxEntry {
            path: FileSystemPath::Special { value },
            access,
        }
    }
}
