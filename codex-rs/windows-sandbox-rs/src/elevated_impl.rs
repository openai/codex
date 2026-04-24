use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

pub struct ElevatedSandboxCaptureRequest<'a> {
    pub policy_json_or_preset: &'a str,
    pub sandbox_policy_cwd: &'a Path,
    pub codex_home: &'a Path,
    pub command: Vec<String>,
    pub cwd: &'a Path,
    pub env_map: HashMap<String, String>,
    pub timeout_ms: Option<u64>,
    pub use_private_desktop: bool,
    pub proxy_enforced: bool,
    pub read_roots_override: Option<&'a [PathBuf]>,
    pub write_roots_override: Option<&'a [PathBuf]>,
    pub deny_write_paths_override: &'a [PathBuf],
}

mod windows_impl {
    use super::ElevatedSandboxCaptureRequest;
    use crate::acl::allow_null_device;
    use crate::cap::load_or_create_cap_sids;
    use crate::env::ensure_non_interactive_pager;
    use crate::env::inherit_path_env;
    use crate::env::normalize_null_device_env;
    use crate::identity::require_logon_sandbox_creds;
    use crate::ipc_framed::Message;
    use crate::ipc_framed::OutputStream;
    use crate::ipc_framed::SpawnRequest;
    use crate::ipc_framed::decode_bytes;
    use crate::ipc_framed::read_frame;
    use crate::logging::log_failure;
    use crate::logging::log_start;
    use crate::logging::log_success;
    use crate::policy::SandboxPolicy;
    use crate::policy::parse_policy;
    use crate::runner_client::spawn_runner_transport;
    use crate::token::convert_string_sid_to_sid;
    use anyhow::Result;
    use std::collections::HashMap;
    use std::path::Path;
    use std::path::PathBuf;

    /// Ensures the parent directory of a path exists before writing to it.
    /// Walks upward from `start` to locate the git worktree root, following gitfile redirects.
    fn find_git_root(start: &Path) -> Option<PathBuf> {
        let mut cur = dunce::canonicalize(start).ok()?;
        loop {
            let marker = cur.join(".git");
            if marker.is_dir() {
                return Some(cur);
            }
            if marker.is_file() {
                if let Ok(txt) = std::fs::read_to_string(&marker)
                    && let Some(rest) = txt.trim().strip_prefix("gitdir:")
                {
                    let gitdir = rest.trim();
                    let resolved = if Path::new(gitdir).is_absolute() {
                        PathBuf::from(gitdir)
                    } else {
                        cur.join(gitdir)
                    };
                    return resolved.parent().map(Path::to_path_buf).or(Some(cur));
                }
                return Some(cur);
            }
            let parent = cur.parent()?;
            if parent == cur {
                return None;
            }
            cur = parent.to_path_buf();
        }
    }

    /// Creates the sandbox user's Codex home directory if it does not already exist.
    fn ensure_codex_home_exists(p: &Path) -> Result<()> {
        std::fs::create_dir_all(p)?;
        Ok(())
    }

    /// Adds a git safe.directory entry to the environment when running inside a repository.
    /// git will not otherwise allow the Sandbox user to run git commands on the repo directory
    /// which is owned by the primary user.
    fn inject_git_safe_directory(
        env_map: &mut HashMap<String, String>,
        cwd: &Path,
        _logs_base_dir: Option<&Path>,
    ) {
        if let Some(git_root) = find_git_root(cwd) {
            let mut cfg_count: usize = env_map
                .get("GIT_CONFIG_COUNT")
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(0);
            let git_path = git_root.to_string_lossy().replace("\\\\", "/");
            env_map.insert(
                format!("GIT_CONFIG_KEY_{cfg_count}"),
                "safe.directory".to_string(),
            );
            env_map.insert(format!("GIT_CONFIG_VALUE_{cfg_count}"), git_path);
            cfg_count += 1;
            env_map.insert("GIT_CONFIG_COUNT".to_string(), cfg_count.to_string());
        }
    }

    pub use crate::windows_impl::CaptureResult;

    /// Launches the command runner under the sandbox user and captures its output.
    #[allow(clippy::too_many_arguments)]
    pub fn run_windows_sandbox_capture(
        request: ElevatedSandboxCaptureRequest<'_>,
    ) -> Result<CaptureResult> {
        let ElevatedSandboxCaptureRequest {
            policy_json_or_preset,
            sandbox_policy_cwd,
            codex_home,
            command,
            cwd,
            mut env_map,
            timeout_ms,
            use_private_desktop,
            proxy_enforced,
            read_roots_override,
            write_roots_override,
            deny_write_paths_override,
        } = request;
        let policy = parse_policy(policy_json_or_preset)?;
        normalize_null_device_env(&mut env_map);
        ensure_non_interactive_pager(&mut env_map);
        inherit_path_env(&mut env_map);
        inject_git_safe_directory(&mut env_map, cwd, None);
        // Use a temp-based log dir that the sandbox user can write.
        let sandbox_base = codex_home.join(".sandbox");
        ensure_codex_home_exists(&sandbox_base)?;

        let logs_base_dir: Option<&Path> = Some(sandbox_base.as_path());
        log_start(&command, logs_base_dir);
        let sandbox_creds = require_logon_sandbox_creds(
            &policy,
            sandbox_policy_cwd,
            cwd,
            &env_map,
            codex_home,
            read_roots_override,
            write_roots_override,
            deny_write_paths_override,
            proxy_enforced,
        )?;
        // Build capability SID for ACL grants.
        if matches!(
            &policy,
            SandboxPolicy::DangerFullAccess | SandboxPolicy::ExternalSandbox { .. }
        ) {
            anyhow::bail!("DangerFullAccess and ExternalSandbox are not supported for sandboxing")
        }
        let caps = load_or_create_cap_sids(codex_home)?;
        let (psid_to_use, cap_sids) = match &policy {
            SandboxPolicy::ReadOnly { .. } => {
                #[allow(clippy::unwrap_used)]
                let psid = unsafe { convert_string_sid_to_sid(&caps.readonly).unwrap() };
                (psid, vec![caps.readonly])
            }
            SandboxPolicy::WorkspaceWrite { .. } => {
                #[allow(clippy::unwrap_used)]
                let psid = unsafe { convert_string_sid_to_sid(&caps.workspace).unwrap() };
                (
                    psid,
                    vec![
                        caps.workspace,
                        crate::cap::workspace_cap_sid_for_cwd(codex_home, cwd)?,
                    ],
                )
            }
            SandboxPolicy::DangerFullAccess | SandboxPolicy::ExternalSandbox { .. } => {
                unreachable!("DangerFullAccess handled above")
            }
        };

        unsafe {
            allow_null_device(psid_to_use);
        }

        (|| -> Result<CaptureResult> {
            let spawn_request = SpawnRequest {
                command: command.clone(),
                cwd: cwd.to_path_buf(),
                env: env_map.clone(),
                policy_json_or_preset: policy_json_or_preset.to_string(),
                sandbox_policy_cwd: sandbox_policy_cwd.to_path_buf(),
                codex_home: sandbox_base.clone(),
                real_codex_home: codex_home.to_path_buf(),
                cap_sids,
                timeout_ms,
                tty: false,
                stdin_open: false,
                use_private_desktop,
            };
            let mut transport =
                spawn_runner_transport(codex_home, cwd, &sandbox_creds, logs_base_dir)?;
            transport.send_spawn_request(spawn_request)?;
            transport.read_spawn_ready()?;
            let (pipe_write, mut pipe_read) = transport.into_files();
            drop(pipe_write);

            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            let (exit_code, timed_out) = loop {
                let msg = read_frame(&mut pipe_read)?
                    .ok_or_else(|| anyhow::anyhow!("runner pipe closed before exit"))?;
                match msg.message {
                    Message::SpawnReady { .. } => {}
                    Message::Output { payload } => {
                        let bytes = decode_bytes(&payload.data_b64)?;
                        match payload.stream {
                            OutputStream::Stdout => stdout.extend_from_slice(&bytes),
                            OutputStream::Stderr => stderr.extend_from_slice(&bytes),
                        }
                    }
                    Message::Exit { payload } => break (payload.exit_code, payload.timed_out),
                    Message::Error { payload } => {
                        return Err(anyhow::anyhow!("runner error: {}", payload.message));
                    }
                    other => {
                        return Err(anyhow::anyhow!(
                            "unexpected runner message during capture: {other:?}"
                        ));
                    }
                }
            };

            if exit_code == 0 {
                log_success(&command, logs_base_dir);
            } else {
                log_failure(&command, &format!("exit code {exit_code}"), logs_base_dir);
            }

            Ok(CaptureResult {
                exit_code,
                stdout,
                stderr,
                timed_out,
            })
        })()
    }

    #[cfg(test)]
    mod tests {
        use crate::policy::SandboxPolicy;

        fn workspace_policy(network_access: bool) -> SandboxPolicy {
            SandboxPolicy::WorkspaceWrite {
                writable_roots: Vec::new(),
                read_only_access: Default::default(),
                network_access,
                exclude_tmpdir_env_var: false,
                exclude_slash_tmp: false,
            }
        }

        #[test]
        fn applies_network_block_when_access_is_disabled() {
            assert!(!workspace_policy(/*network_access*/ false).has_full_network_access());
        }

        #[test]
        fn skips_network_block_when_access_is_allowed() {
            assert!(workspace_policy(/*network_access*/ true).has_full_network_access());
        }

        #[test]
        fn applies_network_block_for_read_only() {
            assert!(!SandboxPolicy::new_read_only_policy().has_full_network_access());
        }
    }
}

#[cfg(target_os = "windows")]
pub use windows_impl::run_windows_sandbox_capture;

#[cfg(not(target_os = "windows"))]
mod stub {
    use super::ElevatedSandboxCaptureRequest;
    use anyhow::Result;
    use anyhow::bail;

    #[derive(Debug, Default)]
    pub struct CaptureResult {
        pub exit_code: i32,
        pub stdout: Vec<u8>,
        pub stderr: Vec<u8>,
        pub timed_out: bool,
    }

    /// Stub implementation for non-Windows targets; sandboxing only works on Windows.
    #[allow(clippy::too_many_arguments)]
    pub fn run_windows_sandbox_capture(
        _request: ElevatedSandboxCaptureRequest<'_>,
    ) -> Result<CaptureResult> {
        bail!("Windows sandbox is only available on Windows")
    }
}

#[cfg(not(target_os = "windows"))]
pub use stub::run_windows_sandbox_capture;
