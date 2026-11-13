use std::collections::HashMap;
#[cfg(any(windows, test))]
use std::time::Duration;

use codex_exec_server_protocol::JSONRPCErrorError;
use codex_sandboxing::SandboxCommand;
use codex_sandboxing::SandboxExecRequest;
use codex_sandboxing::SandboxManager;
use codex_sandboxing::SandboxTransformRequest;
use codex_utils_path_uri::PathUri;
use codex_utils_pty::Child;
use codex_utils_pty::ChildStdin;
use codex_utils_pty::Command;
#[cfg(target_os = "macos")]
use codex_utils_pty::DescriptorPolicy;
use codex_utils_pty::SpawnFallback;
#[cfg(any(windows, test))]
use tokio::io::AsyncBufReadExt;
#[cfg(any(windows, test))]
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

use crate::ExecServerRuntimeOptions;
use crate::FileSystemSandboxContext;
use crate::fs_helper::CODEX_FS_HELPER_ARG1;
use crate::fs_helper::FsHelperPayload;
use crate::fs_helper::FsHelperRequest;
use crate::fs_helper::FsHelperResponse;
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

#[derive(Clone, Debug)]
pub(crate) struct FileSystemSandboxRunner {
    runtime_paths: ExecServerRuntimeOptions,
    helper_env: HashMap<String, String>,
}

impl FileSystemSandboxRunner {
    pub(crate) fn new(runtime_paths: ExecServerRuntimeOptions) -> Self {
        Self {
            runtime_paths,
            helper_env: helper_env(),
        }
    }

    #[tracing::instrument(name = "fs.sandbox_request", skip_all)]
    pub(crate) async fn run(
        &self,
        sandbox: &FileSystemSandboxContext,
        request: FsHelperRequest,
    ) -> Result<FsHelperPayload, JSONRPCErrorError> {
        let command = self.prepare_command(sandbox).await?;
        let request_json = serde_json::to_vec(&request).map_err(json_error)?;
        run_command(command, request_json).await
    }

    pub(crate) async fn prepare_command(
        &self,
        sandbox: &FileSystemSandboxContext,
    ) -> Result<SandboxExecRequest, JSONRPCErrorError> {
        let command = self.sandbox_command(sandbox)?;
        crate::run_integrity_checks(&command).await;
        Ok(command)
    }

    #[tracing::instrument(
        name = "fs.sandbox_prepare",
        skip_all,
        fields(permission_entries = tracing::field::Empty)
    )]
    fn sandbox_command(
        &self,
        sandbox: &FileSystemSandboxContext,
    ) -> Result<SandboxExecRequest, JSONRPCErrorError> {
        let helper = &self.runtime_paths.codex_self_exe;
        let mut helper_readable_paths = Vec::new();
        if !sandbox.use_legacy_landlock {
            helper_readable_paths.push(helper.clone());
            if let Some(linux_helper) = &self.runtime_paths.codex_linux_sandbox_exe
                && linux_helper != helper
            {
                helper_readable_paths.push(linux_helper.clone());
            }
        }

        let sandbox_manager = SandboxManager::for_file_system_helpers()
            .with_linux_sandbox_pid_namespace(self.runtime_paths.linux_sandbox_pid_namespace);
        #[cfg(target_os = "macos")]
        let sandbox_manager = sandbox_manager.with_allowed_symlinked_codex_home(
            self.runtime_paths.allowed_symlinked_codex_home.clone(),
        );
        let (selection, windows_sandbox_level) =
            crate::sandbox_selection::sandbox_selection(sandbox);
        // Requests use absolute paths. Start at the filesystem root even if the policy cwd was
        // removed, retaining its drive or share on Windows and handling opaque native paths.
        let helper_cwd = sandbox
            .cwd
            .to_abs_path()
            .map_err(|err| invalid_request(err.to_string()))?
            .ancestors()
            .last()
            .ok_or_else(|| invalid_request("filesystem sandbox cwd has no root".to_string()))?;
        let command = SandboxCommand {
            program: helper.as_os_str().to_owned(),
            args: vec![CODEX_FS_HELPER_ARG1.to_string()],
            cwd: PathUri::from_abs_path(&helper_cwd),
            env: self.helper_env.clone(),
            managed_network: None,
            additional_permissions: None,
        };
        let mut request = sandbox_manager
            .transform(SandboxTransformRequest {
                command,
                permissions: &sandbox.permissions,
                workspace_roots: Some(&sandbox.workspace_roots),
                runtime_readable_paths: &helper_readable_paths,
                selection,
                enforce_managed_network: false,
                network: None,
                environment_id: None,
                sandbox_policy_cwd: &sandbox.cwd,
                sandbox_exe: if cfg!(windows) {
                    Some(helper.as_path())
                } else {
                    self.runtime_paths.codex_linux_sandbox_exe.as_deref()
                },
                use_legacy_landlock: sandbox.use_legacy_landlock,
                windows_sandbox_level,
            })
            .map_err(|err| match err {
                codex_sandboxing::SandboxTransformError::SandboxUnavailable => invalid_request(
                    "filesystem sandbox cannot be enforced on this executor".to_string(),
                ),
                codex_sandboxing::SandboxTransformError::InvalidPolicy(err) => {
                    invalid_request(err.to_string())
                }
                err => invalid_request(format!("failed to prepare fs sandbox: {err}")),
            })?
            .into_command_request();
        request.sandbox_override = sandbox.sandbox_override;
        Ok(request)
    }
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
        // MXC needs SystemDrive to resolve platform directories, and SystemRoot and
        // LOCALAPPDATA to create the sandboxed helper process.
        || (cfg!(windows)
            && ["PATH", "SystemDrive", "SystemRoot", "LOCALAPPDATA"]
                .iter()
                .any(|allowed| key.eq_ignore_ascii_case(allowed)))
}

#[cfg(debug_assertions)]
fn bazel_bwrap_env_key_is_allowed(key: &str) -> bool {
    option_env!("BAZEL_PACKAGE").is_some() && FS_HELPER_BAZEL_BWRAP_ENV_ALLOWLIST.contains(&key)
}

#[cfg(not(debug_assertions))]
fn bazel_bwrap_env_key_is_allowed(_key: &str) -> bool {
    false
}

#[tracing::instrument(name = "fs.sandbox_execute", skip_all)]
async fn run_command(
    command: SandboxExecRequest,
    request_json: Vec<u8>,
) -> Result<FsHelperPayload, JSONRPCErrorError> {
    let mut child = spawn_command(command, ChildStdin::Piped)?;
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
    child: &mut Child,
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
    mut child: Child,
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
    child: Child,
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
    stdin: ChildStdin,
) -> Result<Child, JSONRPCErrorError> {
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
    command.envs(env);
    command.stdin(stdin);
    // A helper is a known executable: native launch errors must not retry through fork.
    command.fallback(SpawnFallback::ReturnError);
    // macOS cannot receive passed fds with close-on-exec set atomically.
    #[cfg(target_os = "macos")]
    command.descriptor_policy(DescriptorPolicy::Explicit);
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
    use codex_protocol::sandbox::SandboxOverride;
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

    use crate::ExecServerRuntimeOptions;

    use super::FileSystemSandboxRunner;
    use super::helper_env;
    use super::helper_env_from_vars;
    use super::helper_env_key_is_allowed;

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
    fn helper_env_preserves_windows_runtime_variables_without_leaking_secrets() {
        let env = helper_env_from_vars(
            [
                ("Path", r"C:\Windows\System32"),
                ("LocalAppData", r"C:\Users\test\AppData\Local"),
                ("SystemDrive", "C:"),
                ("systemROOT", r"C:\Windows"),
                ("PATH_INJECTION", "bad"),
                ("OPENAI_API_KEY", "secret"),
            ]
            .map(|(key, value)| (OsString::from(key), OsString::from(value))),
        );

        assert_eq!(
            env,
            HashMap::from([
                ("Path".to_string(), r"C:\Windows\System32".to_string()),
                (
                    "LocalAppData".to_string(),
                    r"C:\Users\test\AppData\Local".to_string()
                ),
                ("SystemDrive".to_string(), "C:".to_string()),
                ("systemROOT".to_string(), r"C:\Windows".to_string()),
            ])
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
            ExecServerRuntimeOptions::new(codex_self_exe.clone(), Some(codex_self_exe))
                .expect("runtime paths");
        let runner = FileSystemSandboxRunner::new(runtime_paths);
        let native_cwd = AbsolutePathBuf::current_dir().expect("cwd");
        let cwd = PathUri::from_abs_path(&native_cwd);
        let file_system_policy = restricted_policy(vec![
            #[cfg(windows)]
            special_entry(FileSystemSpecialPath::Root, FileSystemAccessMode::Read),
            path_entry(native_cwd, FileSystemAccessMode::Write),
        ]);
        let sandbox_context = sandbox_context_with_cwd(&file_system_policy, cwd);
        #[cfg(windows)]
        let sandbox_context = {
            let Err(error) = runner.sandbox_command(&sandbox_context) else {
                panic!("disabled Windows sandbox must not run the helper unsandboxed");
            };
            assert_eq!(
                error.message,
                "filesystem sandbox cannot be enforced on this executor"
            );
            crate::FileSystemSandboxContext {
                windows_sandbox_selection:
                    codex_file_system::WindowsSandboxSelection::RestrictedToken,
                ..sandbox_context
            }
        };

        let request = runner
            .sandbox_command(&sandbox_context)
            .expect("sandbox exec request");

        assert_eq!(request.env.get(&path_key), Some(&path));
    }

    #[test]
    fn sandbox_command_rejects_non_native_policy_cwd() {
        #[cfg(unix)]
        let foreign_uri = "file://server/share/checkout";
        #[cfg(windows)]
        let foreign_uri = "file:///usr/local/checkout";
        let cwd = PathUri::parse(foreign_uri).expect("valid foreign URI");
        let sandbox = sandbox_context_with_cwd(&FileSystemSandboxPolicy::read_only(), cwd.clone());
        let paths = ExecServerRuntimeOptions::new(
            std::env::current_exe().expect("current executable"),
            /*codex_linux_sandbox_exe*/ None,
        )
        .expect("runtime paths");

        let error = FileSystemSandboxRunner::new(paths)
            .sandbox_command(&sandbox)
            .expect_err("non-native policy cwd should be rejected");
        assert_eq!(
            error,
            crate::rpc::invalid_request(format!(
                "'{cwd}' is invalid on '{}'",
                std::env::consts::OS
            ))
        );
    }

    /// Removing the selected directory must not change permission anchoring or prevent launch.
    #[test]
    fn sandbox_exec_request_uses_filesystem_root_and_preserves_policy_cwd() {
        let codex_self_exe = std::env::current_exe().expect("current exe");
        let runtime_paths =
            ExecServerRuntimeOptions::new(codex_self_exe.clone(), Some(codex_self_exe))
                .expect("runtime paths");
        let runner = FileSystemSandboxRunner::new(runtime_paths);
        let selected = tempfile::tempdir().expect("selected directory");
        let selected_cwd = AbsolutePathBuf::from_absolute_path(selected.path()).expect("cwd");
        let root = selected_cwd.ancestors().last().expect("filesystem root");
        let policy = restricted_policy(vec![special_entry(
            FileSystemSpecialPath::Root,
            FileSystemAccessMode::Read,
        )]);
        let mut sandbox_context =
            sandbox_context_with_cwd(&policy, PathUri::from_abs_path(&selected_cwd));
        sandbox_context.sandbox_override = SandboxOverride::EscalatedSandboxWithRestrictions;
        #[cfg(windows)]
        let sandbox_context = crate::FileSystemSandboxContext {
            windows_sandbox_selection: codex_file_system::WindowsSandboxSelection::RestrictedToken,
            ..sandbox_context
        };
        selected.close().expect("remove selected directory");

        let request = runner
            .sandbox_command(&sandbox_context)
            .expect("sandbox command");

        assert_eq!(
            (
                request.cwd,
                request.sandbox_policy_cwd,
                request.sandbox_override
            ),
            (
                PathUri::from_abs_path(&root),
                PathUri::from_abs_path(&selected_cwd),
                SandboxOverride::EscalatedSandboxWithRestrictions,
            )
        );
    }

    /// Windows wrapper policy checks must keep cwd-relative denials anchored to the selection.
    #[cfg(windows)]
    #[test]
    fn sandbox_exec_request_binds_windows_relative_globs_to_policy_cwd() {
        let codex_self_exe = std::env::current_exe().expect("current exe");
        let runtime_paths =
            ExecServerRuntimeOptions::new(codex_self_exe, /*codex_linux_sandbox_exe*/ None)
                .expect("runtime paths");
        let runner = FileSystemSandboxRunner::new(runtime_paths);
        let selected = tempfile::tempdir().expect("selected directory");
        let selected_cwd = AbsolutePathBuf::from_absolute_path(selected.path()).expect("cwd");
        let cwd_uri = PathUri::from_abs_path(&selected_cwd);
        let pattern = "hidden/*.env";
        let absolute_pattern = cwd_uri
            .join(pattern)
            .expect("absolute glob")
            .inferred_native_path_string();
        let policy = restricted_policy(vec![
            special_entry(FileSystemSpecialPath::Root, FileSystemAccessMode::Read),
            FileSystemSandboxEntry::new(
                FileSystemPath::GlobPattern {
                    pattern: pattern.to_string(),
                },
                FileSystemAccessMode::Deny,
            ),
        ]);
        let sandbox_context = crate::FileSystemSandboxContext {
            windows_sandbox_selection: codex_file_system::WindowsSandboxSelection::RestrictedToken,
            ..sandbox_context_with_cwd(&policy, cwd_uri)
        };
        selected.close().expect("remove selected directory");

        let request = runner
            .sandbox_command(&sandbox_context)
            .expect("sandbox command");

        assert_eq!(request.sandbox, codex_sandboxing::SandboxType::None);
        assert_eq!(request.command[1], "--run-as-windows-sandbox");
        assert!(
            request
                .command
                .iter()
                .any(|arg| arg == "--preserve-proxy-settings")
        );
        assert_eq!(
            request
                .command
                .windows(2)
                .filter(|args| args[0] == "--workspace-root")
                .map(|args| std::path::Path::new(&args[1]))
                .collect::<Vec<_>>(),
            [selected_cwd.as_path()],
        );
        let profile = request
            .command
            .windows(2)
            .find(|args| args[0] == "--permission-profile")
            .expect("serialized helper policy");
        assert_eq!(
            serde_json::from_str::<PermissionProfile>(&profile[1])
                .expect("decode helper policy")
                .file_system_sandbox_policy()
                .entries
                .into_iter()
                .filter(|entry| matches!(entry.path, FileSystemPath::GlobPattern { .. }))
                .collect::<Vec<_>>(),
            [FileSystemSandboxEntry::new(
                FileSystemPath::GlobPattern {
                    pattern: absolute_pattern,
                },
                FileSystemAccessMode::Deny,
            )]
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_helper_binds_executable_and_workspace_grants() {
        const CHILD: &str = "CODEX_TEST_WINDOWS_HELPER_GRANTS_CHILD";
        if std::env::var_os(CHILD).is_none() {
            // Preparing the token wrapper persists capability SIDs under CODEX_HOME.
            let home = tempfile::tempdir().expect("isolated Codex home");
            let output = std::process::Command::new(std::env::current_exe().expect("test binary"))
                .args([
                    "--exact",
                    "fs_sandbox::tests::windows_helper_binds_executable_and_workspace_grants",
                ])
                .env(CHILD, "1")
                .env("CODEX_HOME", home.path())
                .output()
                .expect("run isolated Windows helper test");
            assert!(
                output.status.success()
                    && String::from_utf8_lossy(&output.stdout).contains("1 passed"),
                "isolated helper test: {output:?}"
            );
            return;
        }

        let workspace = tempfile::tempdir().expect("workspace");
        let other = tempfile::tempdir().expect("other workspace");
        let roots = [workspace.path(), other.path()].map(|path| {
            AbsolutePathBuf::from_absolute_path(path.canonicalize().expect("canonical workspace"))
                .expect("absolute workspace")
        });
        let drive = roots[0].ancestors().last().expect("workspace drive");
        // An unused drive also avoids resolving a mapped drive back into the workspace.
        let helper = ('D'..='Z')
            .rev()
            .map(|letter| std::path::PathBuf::from(format!(r"{letter}:\")))
            .find(|root| !root.exists())
            .expect("unused drive for the helper policy")
            .join(r"codex-test-runtime\helper.exe");
        let helper = AbsolutePathBuf::from_absolute_path(helper).expect("helper");
        let helper_dir = helper.parent().expect("helper directory");
        let runtime_paths = ExecServerRuntimeOptions::new(
            helper.to_path_buf(),
            /*codex_linux_sandbox_exe*/ None,
        )
        .expect("runtime paths");
        let runner = FileSystemSandboxRunner {
            runtime_paths,
            helper_env: HashMap::new(),
        };
        let policy = restricted_policy(vec![
            special_entry(FileSystemSpecialPath::Root, FileSystemAccessMode::Read),
            special_entry(
                FileSystemSpecialPath::project_roots(/*subpath*/ None),
                FileSystemAccessMode::Write,
            ),
        ]);
        for (grants, policy_cwd, wrapper_roots, writable) in [
            (&roots[..], &roots[0], &roots[..], [true, true]),
            // The wrapper encodes the command's drive root when no write roots were granted.
            (
                &roots[..0],
                &roots[0],
                std::slice::from_ref(&drive),
                [false, false],
            ),
            (&roots[1..], &roots[1], &roots[1..], [false, true]),
        ] {
            let sandbox = crate::FileSystemSandboxContext {
                workspace_roots: grants.iter().map(PathUri::from_abs_path).collect(),
                windows_sandbox_selection:
                    codex_file_system::WindowsSandboxSelection::RestrictedToken,
                ..sandbox_context_with_cwd(&policy, PathUri::from_abs_path(policy_cwd))
            };
            let request = runner
                .sandbox_command(&sandbox)
                .expect("prepare Windows helper");
            let args = &request.command;
            assert_eq!(
                args.windows(2)
                    .filter(|pair| pair[0] == "--workspace-root")
                    .map(|pair| std::path::Path::new(&pair[1]))
                    .collect::<Vec<_>>(),
                wrapper_roots
                    .iter()
                    .map(AbsolutePathBuf::as_path)
                    .collect::<Vec<_>>()
            );
            let profile = args
                .windows(2)
                .find(|pair| pair[0] == "--permission-profile")
                .expect("serialized helper policy");
            let profile: PermissionProfile =
                serde_json::from_str(&profile[1]).expect("helper policy");
            let fs = profile.file_system_sandbox_policy();
            assert_eq!(
                roots
                    .each_ref()
                    .map(|root| fs.can_write_local_path_with_cwd(root, policy_cwd)),
                writable
            );
            assert_eq!(
                fs.resolve_access_for_local_path_with_cwd(&helper, policy_cwd),
                FileSystemAccessMode::Read
            );
            assert!(!fs.can_read_local_path_with_cwd(&helper_dir, policy_cwd));
            assert!(
                !fs.can_read_local_path_with_cwd(&helper_dir.join("credentials.json"), policy_cwd)
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn helper_executable_reads_do_not_expand_workspace_or_network_access() {
        let temp = tempfile::tempdir().expect("temp dir");
        let cwd = AbsolutePathBuf::from_absolute_path(temp.path().canonicalize().expect("cwd"))
            .expect("absolute cwd");
        let helper = cwd.join("bin/codex");
        let linux_helper = cwd.join("aliases/codex-linux-sandbox");
        let paths =
            ExecServerRuntimeOptions::new(helper.to_path_buf(), Some(linux_helper.to_path_buf()))
                .expect("runtime paths");
        let policy = restricted_policy(vec![special_entry(
            FileSystemSpecialPath::project_roots(/*subpath*/ None),
            FileSystemAccessMode::Write,
        )]);
        let permissions =
            PermissionProfile::from_runtime_permissions(&policy, NetworkSandboxPolicy::Enabled);
        let mut sandbox = crate::FileSystemSandboxContext::from_permission_profile(
            permissions,
            PathUri::from_abs_path(&cwd),
        );
        let runner = FileSystemSandboxRunner::new(paths);
        let workspace = cwd.join("workspace");

        for roots in [vec![PathUri::from_abs_path(&workspace)], Vec::new()] {
            let expect_workspace_write = !roots.is_empty();
            sandbox.workspace_roots = roots;
            let command = runner.sandbox_command(&sandbox).expect("sandbox command");
            let (policy, network) = command.permission_profile.to_runtime_permissions();

            assert_eq!(network, NetworkSandboxPolicy::Restricted);
            assert!(policy.include_platform_defaults());
            assert_eq!(
                policy.can_write_local_path_with_cwd(&workspace, &cwd),
                expect_workspace_write
            );
            assert!(!policy.can_write_local_path_with_cwd(&cwd, &cwd));
            for executable in [&helper, &linux_helper] {
                let sibling = executable.parent().unwrap().join("credentials.json");
                assert!(policy.can_read_local_path_with_cwd(executable, &cwd));
                assert!(!policy.can_write_local_path_with_cwd(executable, &cwd));
                assert!(!policy.can_read_local_path_with_cwd(&sibling, &cwd));
            }
        }
    }

    fn restricted_policy(entries: Vec<FileSystemSandboxEntry>) -> FileSystemSandboxPolicy {
        FileSystemSandboxPolicy::restricted(entries)
    }

    fn sandbox_context_with_cwd(
        policy: &FileSystemSandboxPolicy,
        cwd: PathUri,
    ) -> crate::FileSystemSandboxContext {
        codex_file_system::FileSystemSandboxContext::from_permission_profile(
            PermissionProfile::from_runtime_permissions(policy, NetworkSandboxPolicy::Restricted),
            cwd,
        )
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
