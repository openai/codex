use anyhow::Context;
use anyhow::Result;
use base64::Engine as _;
use clap::Parser;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncReadExt as _;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
#[cfg(unix)]
use tokio::net::UnixListener;
#[cfg(unix)]
use tokio::net::UnixStream;
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use uuid::Uuid;

const JSONRPC_VERSION: &str = "2.0";
const METHOD_ZSH_INITIALIZE: &str = "zsh/initialize";
const METHOD_ZSH_EXEC_START: &str = "zsh/execStart";
const METHOD_ZSH_EXEC_STDIN: &str = "zsh/execStdin";
const METHOD_ZSH_EXEC_RESIZE: &str = "zsh/execResize";
const METHOD_ZSH_EXEC_INTERRUPT: &str = "zsh/execInterrupt";
const METHOD_ZSH_SHUTDOWN: &str = "zsh/shutdown";
const METHOD_ZSH_REQUEST_APPROVAL: &str = "zsh/requestApproval";
const METHOD_ZSH_EVENT_EXEC_STARTED: &str = "zsh/event/execStarted";
const METHOD_ZSH_EVENT_EXEC_STDOUT: &str = "zsh/event/execStdout";
const METHOD_ZSH_EVENT_EXEC_STDERR: &str = "zsh/event/execStderr";
const METHOD_ZSH_EVENT_EXEC_EXITED: &str = "zsh/event/execExited";
const EXEC_WRAPPER_ENV_VAR: &str = "EXEC_WRAPPER";
const SIDECAREXEC_WRAPPER_MODE_ENV_VAR: &str = "CODEX_ZSH_SIDECAR_WRAPPER_MODE";
const SIDECAREXEC_WRAPPER_SOCKET_ENV_VAR: &str = "CODEX_ZSH_SIDECAR_WRAPPER_SOCKET";
const SIDECAREXEC_WRAPPER_ORIGIN_ENV_VAR: &str = "CODEX_ZSH_SIDECAR_WRAPPER_ORIGIN";

#[derive(Parser, Debug)]
struct Args {
    #[arg(long)]
    zsh_path: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(untagged)]
enum JsonRpcId {
    Number(i64),
    String(String),
}

#[derive(Debug, Clone, Serialize)]
struct JsonRpcRequest<T> {
    jsonrpc: &'static str,
    id: JsonRpcId,
    method: &'static str,
    params: T,
}

#[derive(Debug, Clone, Serialize)]
struct JsonRpcSuccess<T> {
    jsonrpc: &'static str,
    id: JsonRpcId,
    result: T,
}

#[derive(Debug, Clone, Serialize)]
struct JsonRpcErrorResponse {
    jsonrpc: &'static str,
    id: JsonRpcId,
    error: JsonRpcError,
}

#[derive(Debug, Clone, Serialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

#[derive(Debug, Clone, Serialize)]
struct JsonRpcNotification<T> {
    jsonrpc: &'static str,
    method: &'static str,
    params: T,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Host -> sidecar parameters for `zsh/execStart`.
///
/// This defines the top-level command invocation the sidecar should run and
/// the execution environment to apply before zsh begins intercepting nested
/// `execve` calls via wrapper mode.
struct ExecStartParams {
    exec_id: String,
    command: Vec<String>,
    cwd: String,
    #[serde(default)]
    env: Option<HashMap<String, String>>,
    #[serde(default)]
    approval_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExecInterruptParams {
    exec_id: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum ApprovalDecision {
    Approved,
    ApprovedForSession,
    ApprovedExecpolicyAmendment,
    Denied,
    Abort,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RequestApprovalParams {
    approval_id: String,
    exec_id: String,
    command: Vec<String>,
    cwd: String,
    reason: Option<String>,
    proposed_execpolicy_amendment: Option<ExecPolicyAmendmentProposal>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequestApprovalResult {
    decision: ApprovalDecision,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExecPolicyAmendmentProposal {
    command_prefix: Vec<String>,
    rationale: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct InitializeResult {
    protocol_version: u32,
    capabilities: Capabilities,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Capabilities {
    interactive_pty: bool,
}

#[derive(Debug, Serialize)]
struct EmptyResult {}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExecStartedEvent {
    exec_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExecChunkEvent {
    exec_id: String,
    chunk_base64: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExecExitedEvent {
    exec_id: String,
    exit_code: i32,
    signal: Option<String>,
    timed_out: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
/// Wrapper-mode request sent over the Unix socket for each intercepted execve.
///
/// Unlike `ExecStartParams`, this payload is produced by the wrapper subprocess
/// (invoked by patched zsh via `EXEC_WRAPPER`) and consumed by the sidecar to
/// decide whether host approval is required for this specific subcommand.
struct WrapperExecRequest {
    file: String,
    argv: Vec<String>,
    cwd: String,
    origin: Option<WrapperExecOrigin>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
/// Execution origin metadata provided by the patched zsh fork.
///
/// This is used to suppress approvals for shell startup/bootstrap commands
/// while still enforcing approvals for model/user command execution.
enum WrapperExecOrigin {
    /// Command was spawned while evaluating login-shell startup files
    /// (for example `.zprofile` / `.zlogin`) before user command execution.
    LoginStartup,
    /// Command was spawned while evaluating non-login rc startup files
    /// (for example `.zshrc`) before user command execution.
    RcStartup,
    /// Command comes from the top-level user-provided command string.
    /// This is the primary approval surface for shell-tool requests.
    UserCommand,
    /// Command is a nested child command derived from user command evaluation
    /// (for example a command reached through shell operators or subshells).
    Subcommand,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
/// Sidecar decision returned to wrapper mode for an intercepted execve.
///
/// `Run` continues execution of the target program.
/// `Deny` blocks execution and returns a failure to the shell process.
struct WrapperExecResponse {
    action: WrapperExecAction,
    reason: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum WrapperExecAction {
    Run,
    Deny,
}

/// Runs the zsh sidecar entrypoint.
///
/// Behavior:
/// - In normal mode, speaks JSON-RPC over stdio with the host (core).
/// - In wrapper mode (`CODEX_ZSH_SIDECAR_WRAPPER_MODE`), handles a single
///   exec-wrapper callback request and exits.
#[tokio::main]
async fn main() -> Result<()> {
    if std::env::var_os(SIDECAREXEC_WRAPPER_MODE_ENV_VAR).is_some() {
        return run_exec_wrapper_mode();
    }

    tracing_subscriber::fmt()
        .with_env_filter("warn")
        .with_writer(std::io::stderr)
        .init();
    let _args = Args::parse();
    let mut stdout = tokio::io::stdout();

    let stdin = tokio::io::stdin();
    let mut lines = BufReader::new(stdin).lines();

    loop {
        let Some(line) = lines.next_line().await.context("read stdin")? else {
            break;
        };

        if line.trim().is_empty() {
            continue;
        }

        let value: JsonValue = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(err) => {
                tracing::warn!("invalid JSON-RPC input: {err}");
                continue;
            }
        };

        let Some(id_value) = value.get("id") else {
            continue;
        };
        let id: JsonRpcId = match serde_json::from_value(id_value.clone()) {
            Ok(v) => v,
            Err(err) => {
                tracing::warn!("invalid request id: {err}");
                continue;
            }
        };

        let method = value
            .get("method")
            .and_then(JsonValue::as_str)
            .unwrap_or_default();

        match method {
            METHOD_ZSH_INITIALIZE => {
                write_json_line(
                    &mut stdout,
                    &JsonRpcSuccess {
                        jsonrpc: JSONRPC_VERSION,
                        id,
                        result: InitializeResult {
                            protocol_version: 1,
                            capabilities: Capabilities {
                                interactive_pty: false,
                            },
                        },
                    },
                )
                .await?;
            }
            METHOD_ZSH_EXEC_START => {
                let params: ExecStartParams = match parse_params(&value) {
                    Ok(p) => p,
                    Err(message) => {
                        write_json_line(
                            &mut stdout,
                            &JsonRpcErrorResponse {
                                jsonrpc: JSONRPC_VERSION,
                                id,
                                error: JsonRpcError {
                                    code: -32602,
                                    message,
                                },
                            },
                        )
                        .await?;
                        continue;
                    }
                };

                if params.command.is_empty() {
                    write_json_line(
                        &mut stdout,
                        &JsonRpcErrorResponse {
                            jsonrpc: JSONRPC_VERSION,
                            id,
                            error: JsonRpcError {
                                code: -32602,
                                message: "execStart.command is empty".to_string(),
                            },
                        },
                    )
                    .await?;
                    continue;
                }

                let mut cmd = Command::new(&params.command[0]);
                if params.command.len() > 1 {
                    cmd.args(&params.command[1..]);
                }
                cmd.current_dir(&params.cwd);
                cmd.stdin(Stdio::null());
                cmd.stdout(Stdio::piped());
                cmd.stderr(Stdio::piped());
                cmd.kill_on_drop(true);
                cmd.env_clear();
                if let Some(env) = params.env.as_ref() {
                    cmd.envs(env);
                }
                #[cfg(unix)]
                let wrapper_socket_path = {
                    params
                        .env
                        .as_ref()
                        .and_then(|env| env.get(SIDECAREXEC_WRAPPER_SOCKET_ENV_VAR))
                        .map(std::path::PathBuf::from)
                        .unwrap_or_else(|| {
                            let socket_id = Uuid::new_v4().as_simple().to_string();
                            let temp_dir = std::env::temp_dir();
                            let canonical_temp_dir = temp_dir.canonicalize().unwrap_or(temp_dir);
                            canonical_temp_dir.join(format!("czs-{}.sock", &socket_id[..12]))
                        })
                };
                #[cfg(unix)]
                let listener = {
                    let _ = std::fs::remove_file(&wrapper_socket_path);
                    UnixListener::bind(&wrapper_socket_path).with_context(|| {
                        format!("bind wrapper socket at {}", wrapper_socket_path.display())
                    })?
                };
                #[cfg(unix)]
                {
                    cmd.env(
                        SIDECAREXEC_WRAPPER_SOCKET_ENV_VAR,
                        wrapper_socket_path.to_string_lossy().to_string(),
                    );
                    let wrapper_path =
                        std::env::current_exe().context("resolve current sidecar binary path")?;
                    cmd.env(
                        EXEC_WRAPPER_ENV_VAR,
                        wrapper_path.to_string_lossy().to_string(),
                    );
                }
                cmd.env(SIDECAREXEC_WRAPPER_MODE_ENV_VAR, "1");

                let mut child = match cmd.spawn() {
                    Ok(c) => c,
                    Err(err) => {
                        #[cfg(unix)]
                        {
                            let _ = std::fs::remove_file(&wrapper_socket_path);
                        }
                        write_json_line(
                            &mut stdout,
                            &JsonRpcErrorResponse {
                                jsonrpc: JSONRPC_VERSION,
                                id,
                                error: JsonRpcError {
                                    code: -32000,
                                    message: format!("failed to spawn command: {err}"),
                                },
                            },
                        )
                        .await?;
                        continue;
                    }
                };

                let exec_id = params.exec_id.clone();

                write_json_line(
                    &mut stdout,
                    &JsonRpcSuccess {
                        jsonrpc: JSONRPC_VERSION,
                        id,
                        result: EmptyResult {},
                    },
                )
                .await?;
                write_json_line(
                    &mut stdout,
                    &JsonRpcNotification {
                        jsonrpc: JSONRPC_VERSION,
                        method: METHOD_ZSH_EVENT_EXEC_STARTED,
                        params: ExecStartedEvent {
                            exec_id: exec_id.clone(),
                        },
                    },
                )
                .await?;

                let mut tasks = JoinSet::new();
                let (stream_tx, mut stream_rx) =
                    mpsc::unbounded_channel::<(&'static str, Vec<u8>)>();
                if let Some(mut out) = child.stdout.take() {
                    let tx = stream_tx.clone();
                    tasks.spawn(async move {
                        let mut buf = [0_u8; 8192];
                        loop {
                            let read = match out.read(&mut buf).await {
                                Ok(0) => break,
                                Ok(n) => n,
                                Err(err) => {
                                    tracing::warn!("stdout read error: {err}");
                                    break;
                                }
                            };
                            let _ = tx.send((METHOD_ZSH_EVENT_EXEC_STDOUT, buf[..read].to_vec()));
                        }
                    });
                }
                if let Some(mut err) = child.stderr.take() {
                    let tx = stream_tx.clone();
                    tasks.spawn(async move {
                        let mut buf = [0_u8; 8192];
                        loop {
                            let read = match err.read(&mut buf).await {
                                Ok(0) => break,
                                Ok(n) => n,
                                Err(err) => {
                                    tracing::warn!("stderr read error: {err}");
                                    break;
                                }
                            };
                            let _ = tx.send((METHOD_ZSH_EVENT_EXEC_STDERR, buf[..read].to_vec()));
                        }
                    });
                }
                drop(stream_tx);

                let wait = child.wait();
                tokio::pin!(wait);
                let mut child_exit = None;

                #[cfg(unix)]
                while child_exit.is_none() || !stream_rx.is_closed() {
                    tokio::select! {
                        result = &mut wait, if child_exit.is_none() => {
                            child_exit = Some(result.context("wait for command exit")?);
                        }
                        stream = stream_rx.recv(), if !stream_rx.is_closed() => {
                            if let Some((method, chunk)) = stream {
                                write_json_line(
                                    &mut stdout,
                                    &JsonRpcNotification {
                                        jsonrpc: JSONRPC_VERSION,
                                        method,
                                        params: ExecChunkEvent {
                                            exec_id: exec_id.clone(),
                                            chunk_base64: base64::engine::general_purpose::STANDARD.encode(chunk),
                                        },
                                    }
                                ).await?;
                            }
                        }
                        accept_result = listener.accept() => {
                            let (stream, _) = match accept_result {
                                Ok(pair) => pair,
                                Err(err) => {
                                    tracing::warn!("failed to accept wrapper request: {err}");
                                    continue;
                                }
                            };
                            handle_wrapper_request(
                                &mut stdout,
                                &mut lines,
                                stream,
                                exec_id.clone(),
                                params.approval_reason.clone(),
                            ).await?;
                        }
                    }
                }
                #[cfg(not(unix))]
                while child_exit.is_none() || !stream_rx.is_closed() {
                    tokio::select! {
                        result = &mut wait, if child_exit.is_none() => {
                            child_exit = Some(result.context("wait for command exit")?);
                        }
                        stream = stream_rx.recv(), if !stream_rx.is_closed() => {
                            if let Some((method, chunk)) = stream {
                                write_json_line(
                                    &mut stdout,
                                    &JsonRpcNotification {
                                        jsonrpc: JSONRPC_VERSION,
                                        method,
                                        params: ExecChunkEvent {
                                            exec_id: exec_id.clone(),
                                            chunk_base64: base64::engine::general_purpose::STANDARD.encode(chunk),
                                        },
                                    }
                                ).await?;
                            }
                        }
                    }
                }

                while tasks.join_next().await.is_some() {}
                #[cfg(unix)]
                {
                    let _ = std::fs::remove_file(&wrapper_socket_path);
                }

                let status = child_exit.context("missing child exit status")?;
                let exit_code = status.code().unwrap_or(-1);
                #[cfg(unix)]
                let signal = {
                    use std::os::unix::process::ExitStatusExt;
                    status.signal().map(|sig: i32| sig.to_string())
                };
                #[cfg(not(unix))]
                let signal = None;
                write_json_line(
                    &mut stdout,
                    &JsonRpcNotification {
                        jsonrpc: JSONRPC_VERSION,
                        method: METHOD_ZSH_EVENT_EXEC_EXITED,
                        params: ExecExitedEvent {
                            exec_id: exec_id.clone(),
                            exit_code,
                            signal,
                            timed_out: Some(false),
                        },
                    },
                )
                .await?;
            }
            METHOD_ZSH_EXEC_INTERRUPT => {
                let params: ExecInterruptParams = match parse_params(&value) {
                    Ok(p) => p,
                    Err(message) => {
                        write_json_line(
                            &mut stdout,
                            &JsonRpcErrorResponse {
                                jsonrpc: JSONRPC_VERSION,
                                id,
                                error: JsonRpcError {
                                    code: -32602,
                                    message,
                                },
                            },
                        )
                        .await?;
                        continue;
                    }
                };
                write_json_line(
                    &mut stdout,
                    &JsonRpcErrorResponse {
                        jsonrpc: JSONRPC_VERSION,
                        id,
                        error: JsonRpcError {
                            code: -32002,
                            message: format!("unknown exec id: {}", params.exec_id),
                        },
                    },
                )
                .await?;
            }
            METHOD_ZSH_EXEC_STDIN | METHOD_ZSH_EXEC_RESIZE => {
                write_json_line(
                    &mut stdout,
                    &JsonRpcErrorResponse {
                        jsonrpc: JSONRPC_VERSION,
                        id,
                        error: JsonRpcError {
                            code: -32004,
                            message: "method not supported in sidecar phase 1".to_string(),
                        },
                    },
                )
                .await?;
            }
            METHOD_ZSH_SHUTDOWN => {
                write_json_line(
                    &mut stdout,
                    &JsonRpcSuccess {
                        jsonrpc: JSONRPC_VERSION,
                        id,
                        result: EmptyResult {},
                    },
                )
                .await?;
                break;
            }
            _ => {
                write_json_line(
                    &mut stdout,
                    &JsonRpcErrorResponse {
                        jsonrpc: JSONRPC_VERSION,
                        id,
                        error: JsonRpcError {
                            code: -32601,
                            message: format!("unknown method: {method}"),
                        },
                    },
                )
                .await?;
            }
        }
    }

    Ok(())
}

#[cfg(unix)]
/// Runs exec-wrapper mode for a single intercepted execve.
///
/// The patched zsh process invokes the sidecar binary as `EXEC_WRAPPER`.
/// This mode forwards the intercepted exec metadata over the wrapper socket,
/// waits for host policy (`run` or `deny`), and then either executes the
/// target program or exits with failure.
fn run_exec_wrapper_mode() -> Result<()> {
    use std::io::Read;
    use std::io::Write;
    use std::os::unix::net::UnixStream as StdUnixStream;

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        anyhow::bail!("exec wrapper mode requires target executable path");
    }
    let file = args[1].clone();
    let argv = if args.len() > 2 {
        args[2..].to_vec()
    } else {
        vec![file.clone()]
    };
    let cwd = std::env::current_dir()
        .context("resolve wrapper cwd")?
        .to_string_lossy()
        .to_string();
    let socket_path = std::env::var(SIDECAREXEC_WRAPPER_SOCKET_ENV_VAR)
        .context("missing wrapper socket path env var")?;

    let mut stream = StdUnixStream::connect(&socket_path)
        .with_context(|| format!("connect to wrapper socket at {socket_path}"))?;
    let request = WrapperExecRequest {
        file: file.clone(),
        argv: argv.clone(),
        cwd,
        origin: wrapper_exec_origin_from_env(),
    };
    let encoded = serde_json::to_string(&request).context("serialize wrapper request")?;
    stream
        .write_all(encoded.as_bytes())
        .context("write wrapper request")?;
    stream
        .write_all(b"\n")
        .context("write wrapper request newline")?;
    stream
        .shutdown(std::net::Shutdown::Write)
        .context("shutdown wrapper write")?;

    let mut response_buf = String::new();
    stream
        .read_to_string(&mut response_buf)
        .context("read wrapper response")?;
    let response: WrapperExecResponse =
        serde_json::from_str(response_buf.trim()).context("parse wrapper response")?;

    if response.action == WrapperExecAction::Deny {
        if let Some(reason) = response.reason {
            eprintln!("Execution denied: {reason}");
        } else {
            eprintln!("Execution denied");
        }
        std::process::exit(1);
    }

    let mut command = std::process::Command::new(&file);
    if argv.len() > 1 {
        command.args(&argv[1..]);
    }
    let status = command.status().context("spawn wrapped executable")?;
    std::process::exit(status.code().unwrap_or(1));
}

#[cfg(not(unix))]
fn run_exec_wrapper_mode() -> Result<()> {
    anyhow::bail!("exec wrapper mode is only supported on unix");
}

async fn wait_for_approval_result(
    lines: &mut tokio::io::Lines<BufReader<tokio::io::Stdin>>,
    expected_id: JsonRpcId,
) -> Result<ApprovalDecision> {
    loop {
        let Some(line) = lines.next_line().await.context("read stdin")? else {
            anyhow::bail!("stdin closed while waiting for approval response");
        };
        if line.trim().is_empty() {
            continue;
        }

        let value: JsonValue =
            serde_json::from_str(&line).context("parse approval response JSON-RPC message")?;
        let Some(id_value) = value.get("id") else {
            continue;
        };
        let id: JsonRpcId = serde_json::from_value(id_value.clone())
            .context("parse approval response JSON-RPC id")?;
        if id != expected_id {
            tracing::warn!("ignoring unexpected JSON-RPC message while waiting for approval");
            continue;
        }

        if let Some(error) = value.get("error") {
            let message = error
                .get("message")
                .and_then(JsonValue::as_str)
                .unwrap_or("unknown host approval callback error");
            anyhow::bail!("host rejected approval callback: {message}");
        }

        let result: RequestApprovalResult = serde_json::from_value(
            value
                .get("result")
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("missing approval callback result"))?,
        )
        .context("parse approval callback result")?;
        return Ok(result.decision);
    }
}

#[cfg(unix)]
async fn handle_wrapper_request(
    stdout: &mut tokio::io::Stdout,
    lines: &mut tokio::io::Lines<BufReader<tokio::io::Stdin>>,
    mut stream: UnixStream,
    exec_id: String,
    approval_reason: Option<String>,
) -> Result<()> {
    let mut request_buf = Vec::new();
    stream
        .read_to_end(&mut request_buf)
        .await
        .context("read wrapper request from socket")?;
    let request_line = String::from_utf8(request_buf).context("decode wrapper request as utf-8")?;
    let request: WrapperExecRequest =
        serde_json::from_str(request_line.trim()).context("parse wrapper request payload")?;

    if should_skip_host_approval_for_request(&request) {
        return write_json_line(
            &mut stream,
            &WrapperExecResponse {
                action: WrapperExecAction::Run,
                reason: None,
            },
        )
        .await;
    }

    let command_for_approval = if request.argv.is_empty() {
        vec![request.file.clone()]
    } else {
        request.argv.clone()
    };
    let approval_callback_id = JsonRpcId::String(format!("approval-cb-{}", Uuid::new_v4()));
    let approval_id = Uuid::new_v4().to_string();
    let approval_request = JsonRpcRequest {
        jsonrpc: JSONRPC_VERSION,
        id: approval_callback_id.clone(),
        method: METHOD_ZSH_REQUEST_APPROVAL,
        params: RequestApprovalParams {
            approval_id,
            exec_id: exec_id.clone(),
            command: command_for_approval,
            cwd: request.cwd,
            reason: approval_reason,
            proposed_execpolicy_amendment: None,
        },
    };
    write_json_line(stdout, &approval_request).await?;
    let decision = wait_for_approval_result(lines, approval_callback_id).await?;

    let response = match decision {
        ApprovalDecision::Approved
        | ApprovalDecision::ApprovedForSession
        | ApprovalDecision::ApprovedExecpolicyAmendment => WrapperExecResponse {
            action: WrapperExecAction::Run,
            reason: None,
        },
        ApprovalDecision::Denied => WrapperExecResponse {
            action: WrapperExecAction::Deny,
            reason: Some("command denied by host approval policy".to_string()),
        },
        ApprovalDecision::Abort => WrapperExecResponse {
            action: WrapperExecAction::Deny,
            reason: Some("command aborted by host approval policy".to_string()),
        },
    };
    write_json_line(&mut stream, &response).await
}

fn wrapper_exec_origin_from_env() -> Option<WrapperExecOrigin> {
    let raw = std::env::var(SIDECAREXEC_WRAPPER_ORIGIN_ENV_VAR).ok()?;
    parse_wrapper_exec_origin(raw.as_str())
}

fn parse_wrapper_exec_origin(raw: &str) -> Option<WrapperExecOrigin> {
    // These numeric values are emitted by the patched zsh fork via:
    // `CODEX_ZSH_SIDECAR_WRAPPER_ORIGIN=<int>`.
    //
    // Mapping comes from zsh's `enum exec_wrapper_origin`:
    // 0 = user command
    // 1 = login startup (.zprofile/.zlogin/etc.)
    // 2 = rc startup (.zshrc/.zshenv/etc.)
    // 3 = subcommand
    match raw {
        "0" => Some(WrapperExecOrigin::UserCommand),
        "1" => Some(WrapperExecOrigin::LoginStartup),
        "2" => Some(WrapperExecOrigin::RcStartup),
        "3" => Some(WrapperExecOrigin::Subcommand),
        _ => None,
    }
}

/// Decide whether a wrapper-intercepted execve should bypass host approval.
///
/// Rationale:
/// - We intentionally suppress startup-file commands (`login_startup`, `rc_startup`)
///   so clients only see approvals for model-generated command execution, not shell bootstrap.
/// - We also suppress internal zsh re-exec handoffs (`zsh -c` / `zsh -fc`) because
///   they are implementation details and not actionable user-facing approvals.
/// - All other commands remain approval-eligible and are surfaced to host policy.
fn should_skip_host_approval_for_request(request: &WrapperExecRequest) -> bool {
    if matches!(
        request.origin,
        Some(WrapperExecOrigin::LoginStartup | WrapperExecOrigin::RcStartup)
    ) {
        return true;
    }

    let request_file = PathBuf::from(&request.file);
    let request_file_name = request_file
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let argv_program_name = request
        .argv
        .first()
        .map(PathBuf::from)
        .and_then(|path| path.file_name().map(std::borrow::ToOwned::to_owned))
        .and_then(|name| name.to_str().map(str::to_owned))
        .unwrap_or_default();
    let looks_like_zsh_exec = request_file_name == "zsh" || argv_program_name == "zsh";
    let has_command_string_flag = request
        .argv
        .iter()
        .filter(|arg| arg.starts_with('-'))
        .any(|arg| arg.chars().skip(1).any(|ch| ch == 'c'));
    looks_like_zsh_exec && has_command_string_flag
}

fn parse_params<T: for<'de> Deserialize<'de>>(value: &JsonValue) -> std::result::Result<T, String> {
    let params = value
        .get("params")
        .cloned()
        .ok_or_else(|| "missing params".to_string())?;
    serde_json::from_value(params).map_err(|err| format!("invalid params: {err}"))
}

async fn write_json_line<W: tokio::io::AsyncWrite + Unpin, T: Serialize>(
    writer: &mut W,
    message: &T,
) -> Result<()> {
    let encoded = serde_json::to_string(message).context("serialize JSON-RPC message")?;
    writer
        .write_all(encoded.as_bytes())
        .await
        .context("write message")?;
    writer.write_all(b"\n").await.context("write newline")?;
    writer.flush().await.context("flush message")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wrapper_request(
        origin: Option<WrapperExecOrigin>,
        file: &str,
        argv: &[&str],
    ) -> WrapperExecRequest {
        WrapperExecRequest {
            file: file.to_string(),
            argv: argv.iter().map(ToString::to_string).collect(),
            cwd: "/tmp".to_string(),
            origin,
        }
    }

    #[test]
    fn skips_host_approval_for_login_startup_origin() {
        let request = wrapper_request(
            Some(WrapperExecOrigin::LoginStartup),
            "/usr/bin/echo",
            &["/usr/bin/echo", "hi"],
        );
        assert!(should_skip_host_approval_for_request(&request));
    }

    #[test]
    fn skips_host_approval_for_rc_startup_origin() {
        let request = wrapper_request(
            Some(WrapperExecOrigin::RcStartup),
            "/usr/bin/echo",
            &["/usr/bin/echo", "hi"],
        );
        assert!(should_skip_host_approval_for_request(&request));
    }

    #[test]
    fn does_not_skip_host_approval_for_user_command_origin() {
        let request = wrapper_request(
            Some(WrapperExecOrigin::UserCommand),
            "/usr/bin/true",
            &["/usr/bin/true"],
        );
        assert!(!should_skip_host_approval_for_request(&request));
    }

    #[test]
    fn skips_host_approval_for_internal_zsh_reexec_without_origin() {
        let request = wrapper_request(None, "/Users/test/bin/zsh", &["zsh", "-fc", "true"]);
        assert!(should_skip_host_approval_for_request(&request));
    }

    #[test]
    fn parses_numeric_wrapper_origin_values() {
        assert_eq!(
            parse_wrapper_exec_origin("1"),
            Some(WrapperExecOrigin::LoginStartup)
        );
        assert_eq!(
            parse_wrapper_exec_origin("2"),
            Some(WrapperExecOrigin::RcStartup)
        );
        assert_eq!(
            parse_wrapper_exec_origin("0"),
            Some(WrapperExecOrigin::UserCommand)
        );
    }
}
