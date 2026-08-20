use std::collections::HashMap;
use std::collections::VecDeque;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use codex_exec_server_protocol::JSONRPCErrorError;
use codex_network_proxy::PROXY_ACTIVE_ENV_KEY;
use codex_network_proxy::strip_managed_proxy_env;
use codex_protocol::config_types::ShellEnvironmentPolicyInherit;
use codex_protocol::shell_environment;
use codex_shell_command::shell_detect::ShellType;
use codex_shell_command::shell_snapshot::snapshot_state_and_environment_script;
use codex_utils_path_uri::PathUri;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio::sync::OnceCell;

use crate::FileSystemSandboxContext;
use crate::local_process::shell_environment_policy;
use crate::process_sandbox::PreparedExecRequest;
use crate::protocol::ExecEnvPolicy;
use crate::protocol::ExecParams;
use crate::protocol::ShellSnapshotRequest;
use crate::rpc::internal_error;
use crate::rpc::invalid_params;

const MAX_CACHED_SNAPSHOTS: usize = 16;
const MAX_SNAPSHOT_BYTES: usize = 512 * 1024;
const MAX_SNAPSHOT_ENV_VALUE_BYTES: usize = 60 * 1024;
const MAX_SNAPSHOT_SCOPE_BYTES: usize = 256;
const SNAPSHOT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Default)]
pub(crate) struct ShellSnapshotCache {
    entries: Mutex<VecDeque<CachedShellSnapshot>>,
}

struct CachedShellSnapshot {
    request: ShellSnapshotRequest,
    cwd: PathUri,
    env_policy: Option<ExecEnvPolicy>,
    sandbox: Option<FileSystemSandboxContext>,
    snapshot: Arc<OnceCell<Option<ShellSnapshot>>>,
}

struct ShellSnapshot {
    state: String,
    environment: HashMap<String, String>,
}

impl ShellSnapshotCache {
    pub(crate) async fn prepare(
        &self,
        params: &ExecParams,
        prepared: &mut PreparedExecRequest,
    ) -> Result<(), JSONRPCErrorError> {
        let Some(request) = params.shell_snapshot.as_ref() else {
            return Ok(());
        };
        if request.scope_id.is_empty() || request.scope_id.len() > MAX_SNAPSHOT_SCOPE_BYTES {
            return Err(invalid_params(format!(
                "shell snapshot scope must be non-empty and at most {MAX_SNAPSHOT_SCOPE_BYTES} bytes"
            )));
        }

        if params.argv.len() < 3
            || params.argv[0] != request.shell.path
            || params.argv[1] != "-lc"
            || !prepared.command.ends_with(&params.argv)
        {
            return Ok(());
        }

        let shell_type = match request.shell.name.as_str() {
            "bash" => ShellType::Bash,
            "zsh" => ShellType::Zsh,
            "sh" => ShellType::Sh,
            name => {
                return Err(invalid_params(format!(
                    "shell snapshots are unsupported for shell `{name}`"
                )));
            }
        };

        let snapshot = {
            let mut entries = self.entries.lock().await;
            let position = entries.iter().position(|entry| {
                &entry.request == request
                    && entry.cwd == params.cwd
                    && entry.env_policy == params.env_policy
                    && entry.sandbox == params.sandbox
            });
            let cached = position.and_then(|position| {
                let entry = entries.remove(position)?;
                let snapshot = Arc::clone(&entry.snapshot);
                entries.push_back(entry);
                Some(snapshot)
            });
            if let Some(snapshot) = cached {
                snapshot
            } else {
                let snapshot = Arc::new(OnceCell::new());
                let entry = CachedShellSnapshot {
                    request: request.clone(),
                    cwd: params.cwd.clone(),
                    env_policy: params.env_policy.clone(),
                    sandbox: params.sandbox.clone(),
                    snapshot: Arc::clone(&snapshot),
                };
                entries.push_back(entry);
                if entries.len() > MAX_CACHED_SNAPSHOTS {
                    entries.pop_front();
                }

                snapshot
            }
        };
        let Some(snapshot) = snapshot
            .get_or_init(|| async {
                match capture_snapshot(params, prepared, shell_type).await {
                    Ok(snapshot) => Some(snapshot),
                    Err(err) => {
                        tracing::warn!("failed to capture shell snapshot: {err:?}");
                        None
                    }
                }
            })
            .await
        else {
            return Ok(());
        };

        let restore_snapshot_path = snapshot.environment.contains_key("PATH")
            && params
                .env_policy
                .as_ref()
                .is_none_or(|policy| !policy.r#set.contains_key("PATH"));
        let request_overrides = params
            .env
            .iter()
            .filter(|(name, _)| name.as_str() != "PATH" || !restore_snapshot_path)
            .map(|(name, value)| {
                (
                    name.clone(),
                    prepared.env.get(name).unwrap_or(value).clone(),
                )
            })
            .collect::<HashMap<_, _>>();
        prepared.env.extend(
            snapshot
                .environment
                .iter()
                .map(|(name, value)| (name.clone(), value.clone())),
        );
        prepared.env.extend(request_overrides);
        if restore_snapshot_path && let Some(path) = prepared.env.get_mut("PATH") {
            for entry in &request.runtime_path_prepends {
                if entry.is_empty() {
                    continue;
                }
                *path = std::iter::once(entry.as_str())
                    .chain(
                        path.split(':')
                            .filter(|existing| !existing.is_empty() && *existing != entry),
                    )
                    .collect::<Vec<_>>()
                    .join(":");
            }
        }
        prepared
            .env
            .retain(|name, _| !shell_environment::is_non_inheritable_env_var(name));

        let mut state = snapshot.state.as_str();
        let mut state_variables = Vec::new();
        while !state.is_empty() {
            let mut end = state.len().min(MAX_SNAPSHOT_ENV_VALUE_BYTES);
            while !state.is_char_boundary(end) {
                end -= 1;
            }
            let (chunk, remaining) = state.split_at(end);
            let name = format!("__CODEX_SHELL_SNAPSHOT_STATE_{}", state_variables.len());
            prepared.env.insert(name.clone(), chunk.to_string());
            state_variables.push(name);
            state = remaining;
        }
        let state_expansion = state_variables
            .iter()
            .map(|name| format!("${{{name}}}"))
            .collect::<String>();
        let state_variables = state_variables.join(" ");
        let shell_start = prepared.command.len() - params.argv.len();
        prepared.command[shell_start + 1] = "-c".to_string();
        prepared.command[shell_start + 2] = format!(
            "if ! eval \"unset {state_variables}\n{state_expansion}\" >/dev/null; then printf 'failed to restore shell snapshot\\n' >&2; fi\n{}",
            params.argv[2]
        );

        Ok(())
    }
}

async fn capture_snapshot(
    params: &ExecParams,
    prepared: &PreparedExecRequest,
    shell_type: ShellType,
) -> Result<ShellSnapshot, JSONRPCErrorError> {
    let script = snapshot_state_and_environment_script(shell_type)
        .ok_or_else(|| invalid_params("unsupported shell snapshot script".to_string()))?;
    let shell_start = prepared.command.len() - params.argv.len();
    let mut argv = prepared.command.clone();
    argv[shell_start + 2] = script;
    let (program, args) = argv
        .split_first()
        .ok_or_else(|| internal_error("missing shell snapshot command".to_string()))?;

    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(prepared.cwd.as_path())
        .env_clear()
        .envs(&prepared.env)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    if let Some(arg0) = &prepared.arg0 {
        command.arg0(arg0);
    }
    let mut child = command
        .spawn()
        .map_err(|err| internal_error(format!("cannot capture shell snapshot: {err}")))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| internal_error("missing shell snapshot output".to_string()))?;
    let capture = async {
        let mut output = Vec::new();
        stdout
            .take((MAX_SNAPSHOT_BYTES + 1) as u64)
            .read_to_end(&mut output)
            .await
            .map_err(|err| internal_error(format!("cannot read shell snapshot: {err}")))?;
        if output.len() > MAX_SNAPSHOT_BYTES {
            return Err(internal_error(format!(
                "shell snapshot exceeds {MAX_SNAPSHOT_BYTES} bytes"
            )));
        }
        let status = child
            .wait()
            .await
            .map_err(|err| internal_error(format!("cannot finish shell snapshot: {err}")))?;
        if !status.success() {
            return Err(internal_error(format!(
                "shell snapshot capture exited with {status}"
            )));
        }
        Ok(output)
    };
    let output = tokio::time::timeout(SNAPSHOT_TIMEOUT, capture)
        .await
        .map_err(|_| internal_error("shell snapshot capture timed out".to_string()))??;

    parse_snapshot(&output, params.env_policy.as_ref())
}

fn parse_snapshot(
    output: &[u8],
    env_policy: Option<&ExecEnvPolicy>,
) -> Result<ShellSnapshot, JSONRPCErrorError> {
    let separator = output
        .iter()
        .position(|byte| *byte == 0)
        .ok_or_else(|| internal_error("shell snapshot is missing its environment".to_string()))?;
    let state = &output[..separator];
    let marker = b"# Snapshot file";
    let start = state
        .windows(marker.len())
        .position(|window| window == marker)
        .ok_or_else(|| internal_error("shell snapshot is missing its state marker".to_string()))?;
    let state = std::str::from_utf8(&state[start..])
        .map_err(|err| internal_error(format!("shell snapshot state is not UTF-8: {err}")))?;

    let mut environment = output[separator + 1..]
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
        .filter_map(|entry| {
            let (name, value) = std::str::from_utf8(entry).ok()?.split_once('=')?;
            Some((name.to_string(), value.to_string()))
        })
        .collect::<HashMap<_, _>>();
    if environment.contains_key(PROXY_ACTIVE_ENV_KEY) {
        strip_managed_proxy_env(&mut environment);
    }
    let mut environment = match env_policy {
        Some(policy) => {
            let mut policy = shell_environment_policy(policy);
            policy.inherit = ShellEnvironmentPolicyInherit::All;
            shell_environment::create_env_from_vars(environment, &policy, /*thread_id*/ None)
        }
        None => environment,
    };
    environment.remove("PWD");
    environment.remove("OLDPWD");
    environment.retain(|name, _| !shell_environment::is_non_inheritable_env_var(name));

    Ok(ShellSnapshot {
        state: state.to_string(),
        environment,
    })
}

#[cfg(test)]
#[path = "shell_snapshot_tests.rs"]
mod tests;
