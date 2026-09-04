use std::collections::HashMap;
#[cfg(any(windows, test))]
use std::time::Duration;

use codex_exec_server_protocol::JSONRPCErrorError;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_sandboxing::SandboxCommand;
use codex_sandboxing::SandboxDirectSpawnTransformRequest;
use codex_sandboxing::SandboxExecRequest;
use codex_sandboxing::SandboxManager;
use codex_sandboxing::SandboxTransformRequest;
use codex_sandboxing::SandboxType;
use codex_sandboxing::SandboxablePreference;
use codex_utils_absolute_path::AbsolutePathBuf;
#[cfg(not(target_os = "linux"))]
use codex_utils_absolute_path::canonicalize_preserving_symlinks;
use codex_utils_path_uri::PathUri;
#[cfg(any(windows, test))]
use tokio::io::AsyncBufReadExt;
#[cfg(any(windows, test))]
use tokio::io::AsyncReadExt;
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
#[cfg(any(windows, test))]
const FS_HELPER_EXIT_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 2);
#[cfg(any(windows, test))]
const MAX_FS_HELPER_STDERR_BYTES: u64 = 4096;
#[cfg(debug_assertions)]
const FS_HELPER_BAZEL_BWRAP_ENV_ALLOWLIST: &[&str] = &[
    "CARGO_BIN_EXE_bwrap",
    "RUNFILES_DIR",
    "RUNFILES_MANIFEST_FILE",
    "RUNFILES_MANIFEST_ONLY",
    "TEST_SRCDIR",
    "TEST_WORKSPACE",
];

#[derive(Debug, PartialEq, Eq)]
struct SandboxCwd {
    uri: PathUri,
    native: AbsolutePathBuf,
}

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
        let command = self.sandbox_command(sandbox)?;
        let request_json = serde_json::to_vec(&request).map_err(json_error)?;
        run_command(command, request_json).await
    }

    pub(crate) fn sandbox_command(
        &self,
        sandbox: &FileSystemSandboxContext,
    ) -> Result<SandboxExecRequest, JSONRPCErrorError> {
        let cwd = sandbox_cwd(sandbox)?;
        let native_workspace_roots = sandbox
            .workspace_roots
            .iter()
            .map(native_workspace_root)
            .collect::<Result<Vec<_>, _>>()?;
        let workspace_roots = native_workspace_roots.as_slice();
        let native_permissions: PermissionProfile =
            sandbox.permissions.clone().try_into().map_err(|err| {
                invalid_request(format!("invalid sandbox permission path URI: {err}"))
            })?;
        let native_permissions =
            native_permissions.materialize_project_roots_with_workspace_roots(workspace_roots);
        let mut file_system_policy = native_permissions.file_system_sandbox_policy();
        let helper_read_roots = if sandbox.use_legacy_landlock {
            Vec::new()
        } else {
            helper_read_roots(&self.runtime_paths)
        };
        add_helper_runtime_permissions(
            &mut file_system_policy,
            &helper_read_roots,
            cwd.native.as_path(),
        );
        // Linux resolves aliases in the sandbox helper. Doing it here also probes
        // unrelated permission roots synchronously on the executor's runtime thread.
        #[cfg(not(target_os = "linux"))]
        normalize_file_system_policy_root_aliases(&mut file_system_policy);
        let network_policy = NetworkSandboxPolicy::Restricted;
        let permission_profile = PermissionProfile::from_runtime_permissions_with_enforcement(
            native_permissions.enforcement(),
            &file_system_policy,
            network_policy,
        );
        self.sandbox_exec_request(&permission_profile, &cwd, workspace_roots, sandbox)
    }

    fn sandbox_exec_request(
        &self,
        permission_profile: &PermissionProfile,
        cwd: &SandboxCwd,
        workspace_roots: &[AbsolutePathBuf],
        sandbox_context: &FileSystemSandboxContext,
    ) -> Result<SandboxExecRequest, JSONRPCErrorError> {
        let helper = &self.runtime_paths.codex_self_exe;
        let sandbox_manager = SandboxManager::for_file_system_helpers();
        #[cfg(target_os = "macos")]
        let sandbox_manager = sandbox_manager.with_allowed_symlinked_codex_home(
            self.runtime_paths.allowed_symlinked_codex_home.clone(),
        );
        let sandbox = sandbox_manager.select_initial(
            permission_profile,
            SandboxablePreference::Require,
            sandbox_context.windows_sandbox_level,
            /*has_managed_network_requirements*/ false,
        );
        if sandbox == SandboxType::None {
            return Err(invalid_request(
                "filesystem sandbox cannot be enforced on this executor".to_string(),
            ));
        }
        let command = SandboxCommand {
            program: helper.as_path().as_os_str().to_owned(),
            args: vec![CODEX_FS_HELPER_ARG1.to_string()],
            cwd: cwd.uri.clone(),
            env: self.helper_env.clone(),
            managed_network: None,
            additional_permissions: None,
        };
        sandbox_manager
            .transform_for_direct_spawn(SandboxDirectSpawnTransformRequest {
                workspace_roots,
                windows_sandbox_proxy_settings_mode:
                    codex_sandboxing::WindowsSandboxProxySettingsMode::Preserve,
                transform: SandboxTransformRequest {
                    command,
                    permissions: permission_profile,
                    sandbox,
                    enforce_managed_network: false,
                    environment_id: None,
                    network: None,
                    sandbox_policy_cwd: &cwd.uri,
                    codex_linux_sandbox_exe: self.runtime_paths.codex_linux_sandbox_exe.as_deref(),
                    use_legacy_landlock: sandbox_context.use_legacy_landlock,
                    windows_sandbox_level: sandbox_context.windows_sandbox_level,
                    windows_sandbox_private_desktop: sandbox_context
                        .windows_sandbox_private_desktop,
                },
            })
            .map_err(|err| invalid_request(format!("failed to prepare fs sandbox: {err}")))
    }
}

fn sandbox_cwd(sandbox: &FileSystemSandboxContext) -> Result<SandboxCwd, JSONRPCErrorError> {
    if let Some(uri) = &sandbox.cwd {
        return Ok(SandboxCwd {
            native: native_sandbox_cwd(uri)?,
            uri: uri.clone(),
        });
    }

    if sandbox.has_cwd_dependent_permissions() {
        return Err(invalid_request(
            "file system sandbox context with dynamic permissions requires cwd".to_string(),
        ));
    }

    let native = AbsolutePathBuf::from_absolute_path(current_sandbox_cwd().map_err(io_error)?)
        .map_err(|err| invalid_request(format!("current directory is not absolute: {err}")))?;
    let uri = PathUri::from_abs_path(&native);
    Ok(SandboxCwd { uri, native })
}

fn native_sandbox_cwd(cwd: &PathUri) -> Result<AbsolutePathBuf, JSONRPCErrorError> {
    cwd.to_abs_path()
        .map_err(|err| invalid_request(err.to_string()))
}

fn native_workspace_root(root: &PathUri) -> Result<AbsolutePathBuf, JSONRPCErrorError> {
    root.to_abs_path().map_err(|err| {
        invalid_request(format!(
            "file system sandbox workspace root is not native to this exec-server host: {err}"
        ))
    })
}

fn helper_read_roots(runtime_paths: &ExecServerRuntimePaths) -> Vec<AbsolutePathBuf> {
    let mut roots = vec![runtime_paths.codex_self_exe.clone()];
    if let Some(path) = &runtime_paths.codex_linux_sandbox_exe
        && !roots.contains(path)
    {
        roots.push(path.clone());
    }
    roots
}

fn add_helper_runtime_permissions(
    file_system_policy: &mut FileSystemSandboxPolicy,
    helper_read_roots: &[AbsolutePathBuf],
    cwd: &std::path::Path,
) {
    if !file_system_policy.has_full_disk_read_access() {
        let minimal_read_entry = FileSystemSandboxEntry::new(
            FileSystemPath::Special {
                value: FileSystemSpecialPath::Minimal,
            },
            FileSystemAccessMode::Read,
        );
        if !file_system_policy.entries.contains(&minimal_read_entry) {
            file_system_policy.entries.push(minimal_read_entry);
        }
    }

    for helper_read_root in helper_read_roots {
        if file_system_policy.can_read_local_path_with_cwd(helper_read_root.as_path(), cwd) {
            continue;
        }

        file_system_policy.entries.push(FileSystemSandboxEntry::new(
            helper_read_root.clone().into(),
            FileSystemAccessMode::Read,
        ));
    }
}

#[cfg(not(target_os = "linux"))]
fn normalize_file_system_policy_root_aliases(file_system_policy: &mut FileSystemSandboxPolicy) {
    for entry in &mut file_system_policy.entries {
        // Alias normalization uses this executor's filesystem; leave foreign
        // or opaque PathUris unchanged.
        if let FileSystemPath::Path { path } = &mut entry.path
            && let Ok(native_path) = path.to_abs_path()
        {
            *path = normalize_top_level_alias(native_path).into();
        }
    }
}

#[cfg(not(target_os = "linux"))]
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
    FS_HELPER_ENV_ALLOWLIST.contains(&key)
        // CoreFoundation consults this before falling back to user lookup during helper startup.
        || (cfg!(target_os = "macos") && key == "__CF_USER_TEXT_ENCODING")
        || bazel_bwrap_env_key_is_allowed(key)
        || (cfg!(windows) && key.eq_ignore_ascii_case("PATH"))
}

#[cfg(debug_assertions)]
fn bazel_bwrap_env_key_is_allowed(key: &str) -> bool {
    option_env!("BAZEL_PACKAGE").is_some() && FS_HELPER_BAZEL_BWRAP_ENV_ALLOWLIST.contains(&key)
}

#[cfg(not(debug_assertions))]
fn bazel_bwrap_env_key_is_allowed(_key: &str) -> bool {
    false
}

async fn run_command(
    command: SandboxExecRequest,
    request_json: Vec<u8>,
) -> Result<FsHelperPayload, JSONRPCErrorError> {
    let mut child = spawn_command(command, std::process::Stdio::piped())?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| internal_error("failed to open fs sandbox helper stdin".to_string()))?;

    #[cfg(windows)]
    let mut request_json = request_json;
    #[cfg(windows)]
    request_json.push(b'\n');
    stdin.write_all(&request_json).await.map_err(io_error)?;

    #[cfg(windows)]
    let response = {
        stdin.flush().await.map_err(io_error)?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| internal_error("failed to open fs sandbox helper stdout".to_string()))?;
        let stderr = drain_helper_stderr(&mut child);
        let response = read_helper_response(stdout).await;
        drop(stdin);
        reap_helper_after_response(child, stderr).await?;
        response?
    };

    #[cfg(not(windows))]
    let response = {
        stdin.shutdown().await.map_err(io_error)?;
        drop(stdin);
        wait_for_helper_output(child).await?.stdout
    };

    let response = serde_json::from_slice(&response).map_err(json_error)?;
    match response {
        FsHelperResponse::Ok(payload) => Ok(payload),
        FsHelperResponse::Error(error) => Err(error),
    }
}

#[cfg(any(windows, test))]
pub(crate) async fn read_helper_response(
    stdout: impl tokio::io::AsyncRead + Unpin,
) -> Result<Vec<u8>, JSONRPCErrorError> {
    let mut response = Vec::new();
    let bytes_read = tokio::io::BufReader::new(stdout)
        .read_until(b'\n', &mut response)
        .await
        .map_err(io_error)?;
    if bytes_read == 0 {
        return Err(internal_error(
            "fs sandbox helper closed stdout without responding".to_string(),
        ));
    }
    Ok(response)
}

#[cfg(any(windows, test))]
pub(crate) fn drain_helper_stderr(
    child: &mut tokio::process::Child,
) -> tokio::task::JoinHandle<Result<Vec<u8>, std::io::Error>> {
    let stderr_pipe = child.stderr.take();
    tokio::spawn(async move {
        let mut stderr = Vec::new();
        if let Some(mut stderr_pipe) = stderr_pipe {
            (&mut stderr_pipe)
                .take(MAX_FS_HELPER_STDERR_BYTES)
                .read_to_end(&mut stderr)
                .await?;
            tokio::io::copy(&mut stderr_pipe, &mut tokio::io::sink()).await?;
        }
        Ok::<_, std::io::Error>(stderr)
    })
}

#[cfg(any(windows, test))]
pub(crate) async fn reap_helper_after_response(
    mut child: tokio::process::Child,
    stderr: tokio::task::JoinHandle<Result<Vec<u8>, std::io::Error>>,
) -> Result<(), JSONRPCErrorError> {
    let (status, stderr) = match tokio::time::timeout(FS_HELPER_EXIT_TIMEOUT, async {
        tokio::try_join!(child.wait(), async {
            stderr.await.map_err(std::io::Error::other)?
        })
    })
    .await
    {
        Ok(result) => result.map_err(io_error)?,
        Err(_) => {
            tokio::time::timeout(FS_HELPER_EXIT_TIMEOUT, child.kill())
                .await
                .map_err(|_| {
                    internal_error("fs sandbox helper did not stop after its response".to_string())
                })?
                .map_err(io_error)?;
            return Ok(());
        }
    };
    if status.success() {
        return Ok(());
    }

    Err(internal_error(format!(
        "fs sandbox helper failed with status {status}: {stderr}",
        stderr = String::from_utf8_lossy(&stderr).trim()
    )))
}

#[cfg(not(windows))]
pub(crate) async fn wait_for_helper_output(
    child: tokio::process::Child,
) -> Result<std::process::Output, JSONRPCErrorError> {
    let output = child.wait_with_output().await.map_err(io_error)?;
    if !output.status.success() {
        return Err(internal_error(format!(
            "fs sandbox helper failed with status {status}: {stderr}",
            status = output.status,
            stderr = String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(output)
}

pub(crate) fn spawn_command(
    SandboxExecRequest {
        command: argv,
        cwd,
        mut env,
        arg0,
        ..
    }: SandboxExecRequest,
    stdin: std::process::Stdio,
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
    // TODO(anp): Keep PathUri through the filesystem helper launch boundary.
    let cwd = cwd.to_abs_path().map_err(io_error)?;
    command.current_dir(cwd.as_path());
    env.retain(|name, _| !codex_protocol::shell_environment::is_non_inheritable_env_var(name));
    command.env_clear();
    command.envs(env);
    command.stdin(stdin);
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());
    command.kill_on_drop(true);
    // macOS cannot receive passed fds with close-on-exec set atomically.
    #[cfg(target_os = "macos")]
    // SAFETY: Descriptor cleanup only uses fork-safe system calls.
    unsafe {
        command.pre_exec(|| {
            codex_utils_pty::pty::close_inherited_fds_except(&[]);
            Ok(())
        });
    }
    command.spawn().map_err(io_error)
}

pub(crate) fn io_error(err: std::io::Error) -> JSONRPCErrorError {
    internal_error(err.to_string())
}

fn json_error(err: serde_json::Error) -> JSONRPCErrorError {
    internal_error(format!(
        "failed to encode or decode fs sandbox helper message: {err}"
    ))
}

#[cfg(test)]
#[path = "fs_sandbox_windows_tests.rs"]
mod windows_tests;

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
    use codex_utils_path_uri::PathUri;
    use pretty_assertions::assert_eq;

    use crate::ExecServerRuntimePaths;

    use super::FileSystemSandboxRunner;
    use super::SandboxCwd;
    use super::add_helper_runtime_permissions;
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
        let readable = runtime_paths.codex_self_exe.clone();

        add_helper_runtime_permissions(
            &mut policy,
            &helper_read_roots(&runtime_paths),
            cwd.as_path(),
        );

        assert!(policy.can_read_local_path_with_cwd(readable.as_path(), cwd.as_path()));
        assert!(policy.can_write_local_path_with_cwd(writable.as_path(), cwd.as_path()));
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

    #[cfg(target_os = "macos")]
    #[test]
    fn helper_env_preserves_corefoundation_text_encoding() {
        let env = helper_env_from_vars(
            [
                ("__CF_USER_TEXT_ENCODING", "0x1F6:0x0:0x0"),
                ("HOME", "/Users/test"),
            ]
            .map(|(key, value)| (OsString::from(key), OsString::from(value))),
        );

        assert_eq!(
            env,
            HashMap::from([(
                "__CF_USER_TEXT_ENCODING".to_string(),
                "0x1F6:0x0:0x0".to_string(),
            )])
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
        let native_cwd = AbsolutePathBuf::current_dir().expect("cwd");
        let cwd = PathUri::from_abs_path(&native_cwd);
        let file_system_policy = restricted_policy(vec![
            #[cfg(windows)]
            special_entry(FileSystemSpecialPath::Root, FileSystemAccessMode::Read),
            path_entry(native_cwd.clone(), FileSystemAccessMode::Write),
        ]);
        let network_policy = NetworkSandboxPolicy::Restricted;
        let permission_profile =
            PermissionProfile::from_runtime_permissions(&file_system_policy, network_policy);
        let sandbox_context = sandbox_context_with_cwd(&file_system_policy, cwd.clone());
        let sandbox_cwd = SandboxCwd {
            uri: cwd,
            native: native_cwd,
        };
        #[cfg(windows)]
        let sandbox_context = {
            let error = runner
                .sandbox_exec_request(
                    &permission_profile,
                    &sandbox_cwd,
                    std::slice::from_ref(&sandbox_cwd.native),
                    &sandbox_context,
                )
                .expect_err("disabled Windows sandbox must not run the helper unsandboxed");
            assert_eq!(
                error.message,
                "filesystem sandbox cannot be enforced on this executor"
            );
            crate::FileSystemSandboxContext {
                windows_sandbox_level:
                    codex_protocol::config_types::WindowsSandboxLevel::RestrictedToken,
                ..sandbox_context
            }
        };

        let request = runner
            .sandbox_exec_request(
                &permission_profile,
                &sandbox_cwd,
                std::slice::from_ref(&sandbox_cwd.native),
                &sandbox_context,
            )
            .expect("sandbox exec request");

        assert_eq!(request.env.get(&path_key), Some(&path));
    }

    #[test]
    fn sandbox_cwd_uses_context_cwd() {
        let native_cwd = AbsolutePathBuf::from_absolute_path(std::env::temp_dir().as_path())
            .expect("absolute cwd");
        let cwd = PathUri::from_abs_path(&native_cwd);
        let policy = restricted_policy(vec![special_entry(
            FileSystemSpecialPath::project_roots(/*subpath*/ None),
            FileSystemAccessMode::Write,
        )]);
        let sandbox_context = sandbox_context_with_cwd(&policy, cwd.clone());

        assert_eq!(
            sandbox_cwd(&sandbox_context).expect("sandbox cwd"),
            SandboxCwd {
                uri: cwd,
                native: native_cwd
            }
        );
    }

    #[test]
    fn sandbox_cwd_rejects_non_native_context_cwd_without_fallback() {
        let cwd = non_native_cwd();
        let policy = restricted_policy(vec![special_entry(
            FileSystemSpecialPath::project_roots(/*subpath*/ None),
            FileSystemAccessMode::Write,
        )]);
        let sandbox_context = sandbox_context_with_cwd(&policy, cwd.clone());

        let err = sandbox_cwd(&sandbox_context).expect_err("non-native cwd should be rejected");

        assert_eq!(
            err,
            crate::rpc::invalid_request(format!(
                "'{cwd}' is invalid on '{}'",
                std::env::consts::OS
            ))
        );
    }

    #[test]
    fn sandbox_cwd_rejects_cwd_dependent_profile_without_context_cwd() {
        let policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
            },
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        }]);
        let sandbox_context = codex_file_system::FileSystemSandboxContext::from_permission_profile(
            PermissionProfile::from_runtime_permissions(&policy, NetworkSandboxPolicy::Restricted),
        );

        let err = sandbox_cwd(&sandbox_context).expect_err("missing cwd should be rejected");

        assert_eq!(
            err.message,
            "file system sandbox context with dynamic permissions requires cwd"
        );
    }

    #[test]
    fn helper_permissions_include_only_the_helper_executable() {
        let codex_self_exe = std::env::current_exe().expect("current exe");
        let runtime_paths =
            ExecServerRuntimePaths::new(codex_self_exe, /*codex_linux_sandbox_exe*/ None)
                .expect("runtime paths");
        let cwd = AbsolutePathBuf::from_absolute_path(std::env::temp_dir().as_path())
            .expect("absolute cwd");
        let mut policy = restricted_policy(Vec::new());
        let parent = runtime_paths
            .codex_self_exe
            .parent()
            .expect("current exe parent");
        let sibling = parent.join("credentials.json");

        add_helper_runtime_permissions(
            &mut policy,
            &helper_read_roots(&runtime_paths),
            cwd.as_path(),
        );

        assert!(
            policy.can_read_local_path_with_cwd(
                runtime_paths.codex_self_exe.as_path(),
                cwd.as_path(),
            )
        );
        assert!(!policy.can_read_local_path_with_cwd(parent.as_path(), cwd.as_path()));
        assert!(!policy.can_read_local_path_with_cwd(sibling.as_path(), cwd.as_path()));
    }

    #[test]
    fn helper_permissions_include_only_linux_sandbox_alias_executable() {
        let root = tempfile::tempdir().expect("temp dir");
        let codex_self_exe = root.path().join("bin").join("codex");
        let codex_linux_sandbox_exe = root.path().join("aliases").join("codex-linux-sandbox");
        let runtime_paths =
            ExecServerRuntimePaths::new(codex_self_exe, Some(codex_linux_sandbox_exe))
                .expect("runtime paths");
        let cwd = AbsolutePathBuf::from_absolute_path(std::env::temp_dir().as_path())
            .expect("absolute cwd");
        let mut policy = restricted_policy(Vec::new());
        let codex_parent = runtime_paths.codex_self_exe.parent().expect("codex parent");
        let alias = runtime_paths
            .codex_linux_sandbox_exe
            .as_ref()
            .expect("linux sandbox alias");
        let alias_parent = alias.parent().expect("alias parent");

        add_helper_runtime_permissions(
            &mut policy,
            &helper_read_roots(&runtime_paths),
            cwd.as_path(),
        );

        assert!(
            policy.can_read_local_path_with_cwd(
                runtime_paths.codex_self_exe.as_path(),
                cwd.as_path(),
            )
        );
        assert!(policy.can_read_local_path_with_cwd(alias.as_path(), cwd.as_path()));
        assert!(!policy.can_read_local_path_with_cwd(codex_parent.as_path(), cwd.as_path()));
        assert!(!policy.can_read_local_path_with_cwd(alias_parent.as_path(), cwd.as_path()));
    }

    fn restricted_policy(entries: Vec<FileSystemSandboxEntry>) -> FileSystemSandboxPolicy {
        FileSystemSandboxPolicy::restricted(entries)
    }

    fn sandbox_context_with_cwd(
        policy: &FileSystemSandboxPolicy,
        cwd: PathUri,
    ) -> crate::FileSystemSandboxContext {
        codex_file_system::FileSystemSandboxContext::from_permission_profile_with_cwd(
            PermissionProfile::from_runtime_permissions(policy, NetworkSandboxPolicy::Restricted),
            cwd,
        )
    }

    fn non_native_cwd() -> PathUri {
        #[cfg(unix)]
        let uri = "file://server/share/checkout";
        #[cfg(windows)]
        let uri = "file:///usr/local/checkout";

        PathUri::parse(uri).expect("non-native cwd URI")
    }

    fn path_entry(path: AbsolutePathBuf, access: FileSystemAccessMode) -> FileSystemSandboxEntry {
        FileSystemSandboxEntry {
            path: path.into(),
            access,
            missing_path_behavior: None,
        }
    }

    fn special_entry(
        value: FileSystemSpecialPath,
        access: FileSystemAccessMode,
    ) -> FileSystemSandboxEntry {
        FileSystemSandboxEntry {
            path: FileSystemPath::Special { value },
            access,
            missing_path_behavior: None,
        }
    }
}
