//! Cache captured shell state in executor memory. Replay through a per-launch
//! unnamed reader when the capture sandbox permits it; otherwise retain env replay.

use std::collections::HashMap;
use std::collections::VecDeque;
use std::fs::File;
use std::os::fd::AsRawFd;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use codex_exec_server_protocol::JSONRPCErrorError;
use codex_network_proxy::PROXY_ACTIVE_ENV_KEY;
use codex_network_proxy::strip_managed_proxy_env;
use codex_protocol::config_types::ShellEnvironmentPolicyInherit;
use codex_protocol::shell_environment;
use codex_shell_command::shell_detect::ShellType;
use codex_shell_command::shell_snapshot::CapturedSnapshot;
use codex_shell_command::shell_snapshot::SnapshotCaptureOptions;
use codex_shell_command::shell_snapshot::SnapshotStartup;
use codex_shell_command::shell_snapshot::snapshot_source_capture_script;
use codex_utils_path_uri::PathUri;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio::sync::OnceCell;
use tokio::time::Instant;

use crate::FileSystemSandboxContext;
use crate::local_process::shell_environment_policy;
use crate::process_sandbox::PreparedExecRequest;
use crate::protocol::ExecEnvPolicy;
use crate::protocol::ExecParams;
use crate::protocol::ShellSnapshotRequest;
use crate::rpc::internal_error;
use crate::rpc::invalid_params;
use crate::telemetry::ExecServerTelemetry;

const MAX_CACHED_SNAPSHOTS: usize = 16;
const MAX_SNAPSHOT_BYTES: usize = 512 * 1024;
// Capture also includes quoted export records and an optional pre-startup environment.
const MAX_SNAPSHOT_CAPTURE_BYTES: usize = 8 * MAX_SNAPSHOT_BYTES;
const MAX_SNAPSHOT_ENV_VALUE_BYTES: usize = 60 * 1024;
const MAX_SNAPSHOT_SCOPE_BYTES: usize = 256;
const SNAPSHOT_TIMEOUT: Duration = Duration::from_secs(10);
const SNAPSHOT_RETRY_BACKOFF: Duration = Duration::from_secs(1);
const MAX_SNAPSHOT_ATTEMPTS: usize = 3;

#[derive(Default)]
pub(crate) struct ShellSnapshotCache {
    entries: Mutex<VecDeque<CachedShellSnapshot>>,
}

struct CachedShellSnapshot {
    request: ShellSnapshotRequest,
    cwd: PathUri,
    env_policy: Option<ExecEnvPolicy>,
    sandbox: Option<FileSystemSandboxContext>,
    attempts: usize,
    // Failed captures store the earliest time another attempt may start.
    snapshot: Arc<OnceCell<Result<ShellSnapshot, Instant>>>,
}

struct ShellSnapshot {
    state: String,
    file_source: bool,
    environment: HashMap<String, String>,
}

// Keep a bounded metric label alongside the original RPC error.
type CaptureResult = Result<ShellSnapshot, (&'static str, JSONRPCErrorError)>;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum CapturePurpose {
    Execution,
    Prewarm,
}

impl ShellSnapshotCache {
    #[tracing::instrument(
        name = "codex.exec_server.process_prepare_shell_snapshot",
        skip_all,
        fields(
            process.id = crate::process_telemetry::trace_process_id(params.process_id.as_str()),
            snapshot_requested = params.shell_snapshot.is_some(),
        ),
    )]
    pub(crate) async fn prepare(
        &self,
        params: &ExecParams,
        prepared: &mut PreparedExecRequest,
        telemetry: &ExecServerTelemetry,
        purpose: CapturePurpose,
    ) -> Result<Option<File>, JSONRPCErrorError> {
        let Some(request) = params.shell_snapshot.as_ref() else {
            return Ok(None);
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
            return Ok(None);
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

        let (snapshot, attempt) = {
            let mut entries = self.entries.lock().await;
            let position = entries.iter().position(|entry| {
                &entry.request == request
                    && entry.cwd == params.cwd
                    && entry.env_policy == params.env_policy
                    && entry.sandbox == params.sandbox
            });
            let cached = position.and_then(|position| {
                let mut entry = entries.remove(position)?;
                // Share each failed attempt during backoff. After the retry
                // budget is exhausted, keep falling back until eviction.
                if purpose == CapturePurpose::Execution
                    && entry.attempts < MAX_SNAPSHOT_ATTEMPTS
                    && let Some(Err(retry_at)) = entry.snapshot.get()
                    && Instant::now() >= *retry_at
                {
                    entry.attempts += 1;
                    entry.snapshot = Arc::new(OnceCell::new());
                }
                let snapshot = Arc::clone(&entry.snapshot);
                let attempt = entry.attempts;
                entries.push_back(entry);
                Some((snapshot, attempt))
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
                    attempts: 1,
                    snapshot: Arc::clone(&snapshot),
                };
                entries.push_back(entry);
                if entries.len() > MAX_CACHED_SNAPSHOTS {
                    entries.pop_front();
                }

                (snapshot, 1)
            }
        };
        let capture = async {
            let attempt = attempt.to_string();
            let purpose = match purpose {
                CapturePurpose::Execution => "execution",
                CapturePurpose::Prewarm => "prewarm",
            };
            let started_at = std::time::Instant::now();
            let result = capture_snapshot(params, prepared, shell_type).await;
            telemetry.shell_snapshot_captured(
                started_at.elapsed(),
                result.as_ref().map(|_| ()).map_err(|(reason, _)| *reason),
                &[
                    ("purpose", purpose),
                    ("attempt", &attempt),
                    ("shell", request.shell.name.as_str()),
                    ("sandbox", prepared.sandbox.as_metric_tag()),
                ],
            );
            result.map_err(|(_, error)| error)
        };
        let snapshot = match purpose {
            CapturePurpose::Execution => {
                snapshot
                    .get_or_init(|| async {
                        capture.await.map_err(|err| {
                            tracing::warn!("failed to capture shell snapshot: {err:?}");
                            Instant::now() + SNAPSHOT_RETRY_BACKOFF
                        })
                    })
                    .await
            }
            CapturePurpose::Prewarm => {
                // Leave the cell uninitialized on failure: a waiting real command
                // can capture immediately, without spending its retry budget.
                snapshot
                    .get_or_try_init(|| async { capture.await.map(Ok) })
                    .await?
            }
        };
        let Ok(snapshot) = snapshot else {
            return Ok(None);
        };
        if purpose == CapturePurpose::Prewarm {
            return Ok(None);
        }

        // POSIX sh cannot portably close arbitrary descriptors above 9. Keep its
        // existing replay until the launcher supports remapping child descriptors.
        let reader = if snapshot.file_source {
            let state = snapshot.state.clone();
            match tokio::task::spawn_blocking(move || {
                crate::shell_snapshot_file::materialize(shell_type, &state)
            })
            .await
            {
                Ok(Ok(reader)) => Some(reader),
                error => {
                    tracing::warn!(
                        ?error,
                        "cannot prepare shell snapshot transport; using normal startup"
                    );
                    return Ok(None);
                }
            }
        } else {
            None
        };

        let request_overrides = params
            .env
            .iter()
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
        prepared
            .env
            .retain(|name, _| !shell_environment::is_non_inheritable_env_var(name));

        let restore = if let Some(reader) = &reader {
            format!(". /dev/fd/{}", reader.as_raw_fd())
        } else {
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
            format!("eval \"unset {state_variables}\n{state_expansion}\"")
        };
        let shell_start = prepared.command.len() - params.argv.len();
        // Automatic startup files run before the restoration script and could
        // reintroduce environment variables that the snapshot already filtered.
        let (shell_flag, startup) = match shell_type {
            ShellType::Bash => ("-pc", "set +o privileged\n"),
            ShellType::Zsh => ("-fc", "setopt RCS\n"),
            ShellType::Sh => ("-c", ""),
            ShellType::PowerShell | ShellType::Cmd => unreachable!(),
        };
        prepared.command[shell_start + 1] = shell_flag.to_string();
        prepared.command[shell_start + 2] = format!(
            "{startup}if ! {restore} >/dev/null; then printf 'failed to restore shell snapshot\\n' >&2; fi\n{}",
            params.argv[2]
        );

        Ok(reader)
    }
}

#[tracing::instrument(
    name = "codex.exec_server.process_capture_shell_snapshot",
    skip_all,
    fields(process.id = crate::process_telemetry::trace_process_id(params.process_id.as_str())),
)]
async fn capture_snapshot(
    params: &ExecParams,
    prepared: &PreparedExecRequest,
    shell_type: ShellType,
) -> CaptureResult {
    let mut script = snapshot_source_capture_script(
        shell_type,
        SnapshotCaptureOptions {
            startup: SnapshotStartup::Interactive,
            declarations: false,
            environment: true,
        },
    )
    .ok_or_else(|| {
        (
            "unsupported_shell",
            invalid_params("unsupported shell snapshot script".to_string()),
        )
    })?;
    // Probe under the exact capture sandbox, including its /proc fallback and
    // read policy. Keep cached env replay when descriptor paths cannot be opened.
    let probe = if shell_type == ShellType::Sh {
        None
    } else {
        tokio::task::spawn_blocking(move || crate::shell_snapshot_file::materialize(shell_type, ""))
            .await
            .ok()
            .and_then(Result::ok)
    };
    const SOURCE_MARKER: &str = "CODEX_SNAPSHOT_SOURCE";
    script = if let Some(probe) = &probe {
        let fd = probe.as_raw_fd();
        // Automatic startup has already run: do not call exec/unset here,
        // which may be user functions. Capture exit closes this harmless probe.
        format!(
            "if case '' in '') ;; esac 2>/dev/null </dev/fd/{fd}; then printf '\\0%s\\0%s\\0' '{SOURCE_MARKER}' 1; else printf '\\0%s\\0%s\\0' '{SOURCE_MARKER}' 0; fi\n{script}"
        )
    } else {
        format!("printf '\\0%s\\0%s\\0' '{SOURCE_MARKER}' 0\n{script}")
    };
    let shell_start = prepared.command.len() - params.argv.len();
    let mut argv = prepared.command.clone();
    argv[shell_start + 2] = script;
    let (program, args) = argv.split_first().ok_or_else(|| {
        (
            "missing_command",
            internal_error("missing shell snapshot command".to_string()),
        )
    })?;

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
    if let Some(probe) = &probe {
        let fd = probe.as_raw_fd();
        // SAFETY: the caller owns the CLOEXEC reader through spawn. This only
        // changes the child's descriptor table, without allocating after fork.
        unsafe {
            command.pre_exec(move || {
                if libc::fcntl(fd, libc::F_SETFD, 0) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
    let mut child = command.spawn().map_err(|err| {
        (
            "spawn_failed",
            internal_error(format!("cannot capture shell snapshot: {err}")),
        )
    })?;
    drop(probe);
    let stdout = child.stdout.take().ok_or_else(|| {
        (
            "missing_output",
            internal_error("missing shell snapshot output".to_string()),
        )
    })?;
    let capture = async {
        let mut output = Vec::new();
        stdout
            .take((MAX_SNAPSHOT_CAPTURE_BYTES + 1) as u64)
            .read_to_end(&mut output)
            .await
            .map_err(|err| {
                (
                    "read_failed",
                    internal_error(format!("cannot read shell snapshot: {err}")),
                )
            })?;
        if output.len() > MAX_SNAPSHOT_CAPTURE_BYTES {
            return Err((
                "too_large",
                internal_error(format!(
                    "shell snapshot capture exceeds {MAX_SNAPSHOT_CAPTURE_BYTES} bytes"
                )),
            ));
        }
        let status = child.wait().await.map_err(|err| {
            (
                "wait_failed",
                internal_error(format!("cannot finish shell snapshot: {err}")),
            )
        })?;
        if !status.success() {
            return Err((
                "nonzero_exit",
                internal_error(format!("shell snapshot capture exited with {status}")),
            ));
        }
        Ok(output)
    };
    let output = tokio::time::timeout(SNAPSHOT_TIMEOUT, capture)
        .await
        .map_err(|_| {
            (
                "timeout",
                internal_error("shell snapshot capture timed out".to_string()),
            )
        })??;

    let marker = format!("\0{SOURCE_MARKER}\0");
    let marker_end = output
        .windows(marker.len())
        .position(|part| part == marker.as_bytes())
        .map(|index| index + marker.len())
        .ok_or_else(|| {
            (
                "invalid_capture",
                internal_error("missing snapshot source probe".to_string()),
            )
        })?;
    let (flag, captured) = output[marker_end..].split_at_checked(2).ok_or_else(|| {
        (
            "invalid_capture",
            internal_error("incomplete snapshot source probe".to_string()),
        )
    })?;
    let mut snapshot = parse_snapshot(shell_type, captured, params.env_policy.as_ref())?;
    snapshot.file_source = flag == b"1\0";
    Ok(snapshot)
}

fn parse_snapshot(
    shell_type: ShellType,
    output: &[u8],
    env_policy: Option<&ExecEnvPolicy>,
) -> CaptureResult {
    let captured = CapturedSnapshot::parse(shell_type, output).ok_or_else(|| {
        (
            "invalid_capture",
            internal_error("invalid shell snapshot capture".to_string()),
        )
    })?;
    let state = captured.render_state();
    if state.len().saturating_add(captured.environment.len()) > MAX_SNAPSHOT_BYTES {
        return Err((
            "too_large",
            internal_error(format!("shell snapshot exceeds {MAX_SNAPSHOT_BYTES} bytes")),
        ));
    }

    let mut environment = captured
        .environment
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
        state,
        file_source: false,
        environment,
    })
}

#[cfg(test)]
#[path = "shell_snapshot_tests.rs"]
mod tests;
