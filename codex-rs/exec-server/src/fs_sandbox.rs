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

const FS_HELPER_ENV_ALLOWLIST: &[&str] = &["PATH", "TMPDIR", "TMP", "TEMP"];

#[derive(Clone, Debug)]
pub(crate) struct FileSystemSandboxRunner {
    runtime_paths: ExecServerRuntimePaths,
    helper_env: HashMap<String, String>,
}

struct HelperSandboxInputs {
    sandbox_policy: SandboxPolicy,
    file_system_policy: FileSystemSandboxPolicy,
    network_policy: NetworkSandboxPolicy,
    cwd: AbsolutePathBuf,
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
            env: self.helper_env.clone(),
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
mod tests {
    use std::collections::HashMap;
    use std::ffi::OsString;

    use codex_protocol::models::FileSystemPermissions;
    use codex_protocol::models::NetworkPermissions;
    use codex_protocol::models::PermissionProfile;
    use codex_protocol::permissions::FileSystemSandboxPolicy;
    use codex_protocol::permissions::NetworkSandboxPolicy;
    use codex_protocol::protocol::ReadOnlyAccess;
    use codex_protocol::protocol::SandboxPolicy;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;

    use crate::ExecServerRuntimePaths;
    use crate::FileSystemSandboxContext;

    use super::FileSystemSandboxRunner;
    use super::helper_env;
    use super::helper_env_from_vars;
    use super::helper_env_key_is_allowed;
    use super::helper_sandbox_inputs;
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
            network_access: true,
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

    #[test]
    fn helper_sandbox_inputs_use_context_cwd_and_file_system_policy() {
        let cwd = AbsolutePathBuf::from_absolute_path(std::env::temp_dir().as_path())
            .expect("absolute temp dir");
        let sandbox_policy = SandboxPolicy::new_workspace_write_policy();
        let file_system_policy =
            codex_protocol::permissions::FileSystemSandboxPolicy::from_legacy_sandbox_policy(
                &sandbox_policy,
                cwd.as_path(),
            );
        let mut sandbox_context = FileSystemSandboxContext::new(sandbox_policy.clone());
        sandbox_context.sandbox_policy_cwd = Some(cwd.clone());
        sandbox_context.file_system_sandbox_policy = Some(file_system_policy.clone());

        let inputs = helper_sandbox_inputs(&sandbox_context).expect("helper sandbox inputs");

        assert_eq!(inputs.cwd, cwd);
        assert_eq!(inputs.sandbox_policy, sandbox_policy);
        assert_eq!(inputs.file_system_policy, file_system_policy);
        assert_eq!(inputs.network_policy, NetworkSandboxPolicy::Restricted);
    }

    #[test]
    fn helper_sandbox_inputs_rejects_file_system_policy_without_cwd() {
        let cwd = AbsolutePathBuf::from_absolute_path(std::env::temp_dir().as_path())
            .expect("absolute temp dir");
        let sandbox_policy = SandboxPolicy::new_workspace_write_policy();
        let file_system_policy =
            codex_protocol::permissions::FileSystemSandboxPolicy::from_legacy_sandbox_policy(
                &sandbox_policy,
                cwd.as_path(),
            );
        let mut sandbox_context = FileSystemSandboxContext::new(sandbox_policy);
        sandbox_context.file_system_sandbox_policy = Some(file_system_policy);

        let err = match helper_sandbox_inputs(&sandbox_context) {
            Ok(_) => panic!("expected invalid sandbox inputs"),
            Err(err) => err,
        };

        assert_eq!(
            err.message,
            "fileSystemSandboxPolicy requires sandboxPolicyCwd"
        );
    }

    #[test]
    fn helper_permissions_strip_network_grants() {
        let codex_self_exe = std::env::current_exe().expect("current exe");
        let runtime_paths = ExecServerRuntimePaths::new(
            codex_self_exe.clone(),
            /*codex_linux_sandbox_exe*/ None,
        )
        .expect("runtime paths");
        let runner = FileSystemSandboxRunner::new(runtime_paths);
        let readable = AbsolutePathBuf::from_absolute_path(
            codex_self_exe.parent().expect("current exe parent"),
        )
        .expect("absolute readable path");
        let writable = AbsolutePathBuf::from_absolute_path(std::env::temp_dir().as_path())
            .expect("absolute writable path");

        let permissions = runner
            .helper_permissions(
                Some(&PermissionProfile {
                    network: Some(NetworkPermissions {
                        enabled: Some(true),
                    }),
                    file_system: Some(FileSystemPermissions {
                        read: Some(vec![]),
                        write: Some(vec![writable.clone()]),
                    }),
                }),
                /*include_helper_read_root*/ true,
            )
            .expect("helper permissions");

        assert_eq!(permissions.network, None);
        assert_eq!(
            permissions
                .file_system
                .as_ref()
                .and_then(|fs| fs.write.clone()),
            Some(vec![writable])
        );
        assert_eq!(
            permissions
                .file_system
                .as_ref()
                .and_then(|fs| fs.read.clone()),
            Some(vec![readable])
        );
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
        let sandbox_policy = SandboxPolicy::new_workspace_write_policy();
        let file_system_policy =
            FileSystemSandboxPolicy::from_legacy_sandbox_policy(&sandbox_policy, cwd.as_path());
        let sandbox_context = crate::FileSystemSandboxContext::new(sandbox_policy.clone());

        let request = runner
            .sandbox_exec_request(
                &sandbox_policy,
                &file_system_policy,
                NetworkSandboxPolicy::Restricted,
                &cwd,
                &sandbox_context,
            )
            .expect("sandbox exec request");

        assert_eq!(request.env.get(&path_key), Some(&path));
    }

    #[test]
    fn helper_permissions_include_helper_read_root_without_additional_permissions() {
        let codex_self_exe = std::env::current_exe().expect("current exe");
        let runtime_paths = ExecServerRuntimePaths::new(
            codex_self_exe.clone(),
            /*codex_linux_sandbox_exe*/ None,
        )
        .expect("runtime paths");
        let runner = FileSystemSandboxRunner::new(runtime_paths);
        let readable = AbsolutePathBuf::from_absolute_path(
            codex_self_exe.parent().expect("current exe parent"),
        )
        .expect("absolute readable path");

        let permissions = runner
            .helper_permissions(
                /*additional_permissions*/ None, /*include_helper_read_root*/ true,
            )
            .expect("helper permissions");

        assert_eq!(permissions.network, None);
        assert_eq!(
            permissions.file_system,
            Some(FileSystemPermissions {
                read: Some(vec![readable]),
                write: None,
            })
        );
    }

    #[test]
    fn legacy_landlock_helper_permissions_do_not_add_helper_read_root() {
        let codex_self_exe = std::env::current_exe().expect("current exe");
        let runtime_paths =
            ExecServerRuntimePaths::new(codex_self_exe, /*codex_linux_sandbox_exe*/ None)
                .expect("runtime paths");
        let runner = FileSystemSandboxRunner::new(runtime_paths);

        let permissions = runner.helper_permissions(
            /*additional_permissions*/ None, /*include_helper_read_root*/ false,
        );

        assert_eq!(permissions, None);
    }
}
