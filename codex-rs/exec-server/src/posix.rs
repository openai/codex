//! This is an MCP that implements an alternative `shell` tool with fine-grained privilege
//! escalation based on a per-exec() policy.
//!
//! We spawn Bash process inside a sandbox. The Bash we spawn is patched to allow us to intercept
//! every exec() call it makes by invoking a wrapper program and passing in the arguments it would
//! have passed to exec(). The Bash process (and its descendants) inherit a communication socket
//! from us, and we give its fd number in the CODEX_ESCALATE_SOCKET environment variable.
//!
//! When we intercept an exec() call, we send a message over the socket back to the main
//! MCP process. The MCP process can then decide whether to allow the exec() call to proceed
//! or to escalate privileges and run the requested command with elevated permissions. In the
//! latter case, we send a message back to the child requesting that it forward its open FDs to us.
//! We then execute the requested command on its behalf, patching in the forwarded FDs.
//!
//!
//! ### The privilege escalation flow
//!
//! Child  MCP   Bash   Escalate Helper
//!         |
//!         o----->o
//!         |      |
//!         |      o--(exec)-->o
//!         |      |           |
//!         |o<-(EscalateReq)--o
//!         ||     |           |
//!         |o--(Escalate)---->o
//!         ||     |           |
//!         |o<---------(fds)--o
//!         ||     |           |
//!   o<-----o     |           |
//!   |     ||     |           |
//!   x----->o     |           |
//!         ||     |           |
//!         |x--(exit code)--->o
//!         |      |           |
//!         |      o<--(exit)--x
//!         |      |
//!         o<-----x
//!
//! ### The non-escalation flow
//!
//!  MCP   Bash   Escalate Helper   Child
//!   |
//!   o----->o
//!   |      |
//!   |      o--(exec)-->o
//!   |      |           |
//!   |o<-(EscalateReq)--o
//!   ||     |           |
//!   |o-(RunInSandbox)->o
//!   |      |           |
//!   |      |           x--(exec)-->o
//!   |      |                       |
//!   |      o<--------------(exit)--x
//!   |      |
//!   o<-----x
//!
use anyhow::Context as _;
use anyhow::Result;
use clap::Parser;
use clap::Subcommand;
use codex_core::exec::SandboxType;
use codex_core::exec::process_exec_tool_call;
use codex_core::get_platform_sandbox;
use codex_core::protocol::SandboxPolicy;
use rmcp::ErrorData as McpError;
use rmcp::RoleServer;
use rmcp::ServerHandler;
use rmcp::ServiceExt;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::*;
use rmcp::schemars;
use rmcp::service::RequestContext;
use rmcp::tool;
use rmcp::tool_handler;
use rmcp::tool_router;
use rmcp::transport::stdio;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::io;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd as _;
use std::os::fd::OwnedFd;
use std::os::fd::RawFd;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::{self};

mod socket;

use socket::AsyncDatagramSocket;
use socket::AsyncSocket;

const ESCALATE_SOCKET_ENV_VAR: &str = "CODEX_ESCALATE_SOCKET";
const BASH_PATH_ENV_VAR: &str = "CODEX_BASH_PATH";

// C->S on the escalate socket
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
enum EscalateClientMessage {
    /// The client wants to run exec() with the given arguments.
    EscalateRequest {
        /// The absolute path to the executable to run, i.e. the first arg to exec.
        file: String,
        /// The argv, including the program name (argv[0]).
        argv: Vec<String>,
        workdir: PathBuf,
        env: HashMap<String, String>,
    },
}

// C->S on the escalate socket
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
enum EscalateServerMessage {
    EscalateResponse(EscalateAction),
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
enum EscalateAction {
    RunInSandbox,
    Escalate,
}

// C->S on the super-exec socket
#[derive(Clone, Serialize, Deserialize, Debug)]
struct SuperExecMessage {
    fds: Vec<RawFd>,
}

// S->C on the super-exec socket
#[derive(Clone, Serialize, Deserialize, Debug)]
struct SuperExecResult {
    exit_code: i32,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ExecParams {
    /// The bash string to execute.
    pub command: String,
    /// The working directory to execute the command in. Must be an absolute path.
    pub workdir: String,
    /// The timeout for the command in milliseconds.
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct ExecResult {
    pub exit_code: i32,
    pub output: String,
    pub duration: Duration,
    pub timed_out: bool,
}

fn decide_escalate(file: &str, argv: &[String], _workdir: &PathBuf) -> EscalateAction {
    // TODO: execpolicy
    match (file, argv) {
        ("/opt/homebrew/bin/gh", [_, arg1, arg2, ..]) if arg1 == "issue" && arg2 == "list" => {
            EscalateAction::Escalate
        }
        _ => EscalateAction::RunInSandbox,
    }
}

async fn handle_escalate_session_with_decider<F>(
    socket: AsyncSocket,
    decider: F,
) -> anyhow::Result<()>
where
    F: Fn(&str, &[String], &PathBuf) -> EscalateAction,
{
    let EscalateClientMessage::EscalateRequest {
        file,
        argv,
        workdir,
        env,
    } = socket.receive::<EscalateClientMessage>().await?;
    let action = decider(&file, &argv, &workdir);
    tracing::debug!("decided {action:?} for {file:?} {argv:?} {workdir:?}");
    match action {
        EscalateAction::RunInSandbox => {
            socket
                .send(EscalateServerMessage::EscalateResponse(
                    EscalateAction::RunInSandbox,
                ))
                .await?;
        }
        EscalateAction::Escalate => {
            socket
                .send(EscalateServerMessage::EscalateResponse(
                    EscalateAction::Escalate,
                ))
                .await?;
            let (msg, fds) = socket
                .receive_with_fds::<SuperExecMessage>()
                .await
                .context("failed to receive SuperExecMessage")?;
            if fds.len() != msg.fds.len() {
                return Err(anyhow::anyhow!(
                    "mismatched number of fds in SuperExecMessage: {} in the message, {} from the control message",
                    msg.fds.len(),
                    fds.len()
                ));
            }

            if msg
                .fds
                .iter()
                .any(|src_fd| fds.iter().any(|dst_fd| dst_fd.as_raw_fd() == *src_fd))
            {
                return Err(anyhow::anyhow!(
                    "overlapping fds not yet supported in SuperExecMessage"
                ));
            }

            let mut command = Command::new(file);
            command
                .args(&argv[1..])
                .arg0(argv[0].clone())
                .envs(&env)
                .current_dir(&workdir)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            unsafe {
                command.pre_exec(move || {
                    for (dst_fd, src_fd) in msg.fds.iter().zip(&fds) {
                        libc::dup2(src_fd.as_raw_fd(), *dst_fd);
                    }
                    Ok(())
                });
            }
            let mut child = command.spawn()?;
            let exit_status = child.wait().await?;
            socket
                .send(SuperExecResult {
                    exit_code: exit_status.code().unwrap_or(127),
                })
                .await?;
        }
    }
    Ok(())
}

async fn handle_escalate_session(socket: AsyncSocket) -> anyhow::Result<()> {
    handle_escalate_session_with_decider(socket, decide_escalate).await
}

async fn escalate_task(socket: AsyncDatagramSocket) -> anyhow::Result<()> {
    loop {
        let (_, mut fds) = socket.receive_with_fds().await?;
        if fds.len() != 1 {
            tracing::error!("expected 1 fd in datagram handshake, got {}", fds.len());
            continue;
        }
        let stream_socket = AsyncSocket::from_fd(fds.remove(0))?;
        tokio::spawn(async move {
            if let Err(err) = handle_escalate_session(stream_socket).await {
                tracing::error!("escalate session failed: {err:?}");
            }
        });
    }
}

fn get_bash_path() -> Result<String> {
    std::env::var(BASH_PATH_ENV_VAR).context(format!("{BASH_PATH_ENV_VAR} must be set"))
}

async fn shell_exec(params: ExecParams) -> anyhow::Result<ExecResult> {
    let bash_path = get_bash_path()?;
    let ExecParams {
        command,
        workdir,
        timeout_ms,
    } = params;
    let (escalate_server, escalate_client) = AsyncDatagramSocket::pair()?;
    let client_socket = escalate_client.into_inner();
    client_socket.set_cloexec(false)?;

    let escalate_task = tokio::spawn(escalate_task(escalate_server));
    let mut env = std::env::vars().collect::<HashMap<String, String>>();
    env.insert(
        ESCALATE_SOCKET_ENV_VAR.to_string(),
        client_socket.as_raw_fd().to_string(),
    );
    env.insert(
        "BASH_EXEC_WRAPPER".to_string(),
        format!("{} escalate", std::env::current_exe()?.to_string_lossy()),
    );
    let result = process_exec_tool_call(
        codex_core::exec::ExecParams {
            command: vec![bash_path, "-c".to_string(), command],
            cwd: PathBuf::from(&workdir),
            timeout_ms,
            env,
            with_escalated_permissions: None,
            justification: None,
            arg0: None,
        },
        get_platform_sandbox().unwrap_or(SandboxType::None),
        &SandboxPolicy::ReadOnly,
        &PathBuf::from("/__NONEXISTENT__"), // This is ignored for ReadOnly
        &None,
        None,
    )
    .await?;
    escalate_task.abort();
    let result = ExecResult {
        exit_code: result.exit_code,
        output: result.aggregated_output.text,
        duration: result.duration,
        timed_out: result.timed_out,
    };
    Ok(result)
}

#[derive(Clone)]
pub struct ExecTool {
    tool_router: ToolRouter<ExecTool>,
}

#[allow(clippy::expect_used)]
#[tool_router]
impl ExecTool {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    /// Runs a shell command and returns its output. You MUST provide the workdir as an absolute path.
    #[tool]
    async fn shell(
        &self,
        _context: RequestContext<RoleServer>,
        Parameters(params): Parameters<ExecParams>,
    ) -> Result<CallToolResult, McpError> {
        let result = shell_exec(params)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(CallToolResult::success(vec![Content::json(result)?]))
    }

    #[allow(dead_code)]
    async fn prompt(
        &self,
        command: String,
        workdir: String,
        context: RequestContext<RoleServer>,
    ) -> Result<CreateElicitationResult, McpError> {
        context
            .peer
            .create_elicitation(CreateElicitationRequestParam {
                message: format!("Allow Codex to run `{command:?}` in `{workdir:?}`?"),
                requested_schema: ElicitationSchema::builder()
                    .property("dummy", PrimitiveSchema::String(StringSchema::new()))
                    .build()
                    .expect("failed to build elicitation schema"),
            })
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))
    }
}

impl Default for ExecTool {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_handler]
impl ServerHandler for ExecTool {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2025_06_18,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation::from_build_env(),
            instructions: Some(
                "This server provides a tool to execute shell commands and return their output."
                    .to_string(),
            ),
        }
    }

    async fn initialize(
        &self,
        _request: InitializeRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, McpError> {
        Ok(self.get_info())
    }
}

#[derive(Parser)]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    subcommand: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Escalate(EscalateArgs),
    ShellExec(ShellExecArgs),
}

fn get_escalate_client() -> anyhow::Result<AsyncDatagramSocket> {
    // TODO: we should defensively require only calling this once, since AsyncSocket will take ownership of the fd.
    let client_fd = std::env::var(ESCALATE_SOCKET_ENV_VAR)?.parse::<i32>()?;
    if client_fd < 0 {
        return Err(anyhow::anyhow!(
            "{ESCALATE_SOCKET_ENV_VAR} is not a valid file descriptor: {client_fd}"
        ));
    }
    Ok(unsafe { AsyncDatagramSocket::from_raw_fd(client_fd) }?)
}

/// Invoked from within the sandbox to (potentially) escalate permissions.
#[derive(Parser, Debug)]
struct EscalateArgs {
    file: String,

    #[arg(trailing_var_arg = true)]
    argv: Vec<String>,
}

impl EscalateArgs {
    /// This is the escalate client. It talks to the escalate server to determine whether to exec()
    /// the command directly or to proxy to the escalate server.
    async fn run(self) -> anyhow::Result<()> {
        let EscalateArgs { file, argv } = self;
        let handshake_client = get_escalate_client()?;
        let (server, client) = AsyncSocket::pair()?;
        const HANDSHAKE_MESSAGE: [u8; 1] = [0];
        handshake_client
            .send_with_fds(&HANDSHAKE_MESSAGE, &[server.into_inner().into()])
            .await
            .context("failed to send handshake datagram")?;
        let env = std::env::vars()
            .filter(|(k, _)| {
                !matches!(
                    k.as_str(),
                    ESCALATE_SOCKET_ENV_VAR | "BASH_EXEC_WRAPPER" | BASH_PATH_ENV_VAR
                )
            })
            .collect();
        client
            .send(EscalateClientMessage::EscalateRequest {
                file: file.clone(),
                argv: argv.clone(),
                workdir: std::env::current_dir()?,
                env,
            })
            .await
            .context("failed to send EscalateRequest")?;
        let message = client.receive::<EscalateServerMessage>().await?;
        let EscalateServerMessage::EscalateResponse(action) = message;
        match action {
            EscalateAction::Escalate => {
                // TODO: maybe we should send ALL open FDs (except the escalate client)?
                let fds_to_send = [
                    unsafe { OwnedFd::from_raw_fd(io::stdin().as_raw_fd()) },
                    unsafe { OwnedFd::from_raw_fd(io::stdout().as_raw_fd()) },
                    unsafe { OwnedFd::from_raw_fd(io::stderr().as_raw_fd()) },
                ];

                // TODO: also forward signals over the super-exec socket

                client
                    .send_with_fds(
                        SuperExecMessage {
                            fds: fds_to_send.iter().map(AsRawFd::as_raw_fd).collect(),
                        },
                        &fds_to_send,
                    )
                    .await
                    .context("failed to send SuperExecMessage")?;
                let SuperExecResult { exit_code } = client.receive::<SuperExecResult>().await?;
                std::process::exit(exit_code);
            }
            EscalateAction::RunInSandbox => {
                // We avoid std::process::Command here because we want to be as transparent as
                // possible. std::os::unix::process::CommandExt has .exec() but it does some funky
                // stuff with signal masks and dup2() on its standard FDs, which we don't want.
                use std::ffi::CString;
                let file = CString::new(file).context("NUL in file")?;

                let argv_cstrs: Vec<CString> = argv
                    .iter()
                    .map(|s| CString::new(s.as_str()).context("NUL in argv"))
                    .collect::<Result<Vec<_>, _>>()?;

                let mut argv: Vec<*const libc::c_char> =
                    argv_cstrs.iter().map(|s| s.as_ptr()).collect();
                argv.push(std::ptr::null());

                unsafe {
                    libc::execv(file.as_ptr(), argv.as_ptr());
                    let err = std::io::Error::last_os_error();
                    tracing::error!("failed to execute command: {err}");
                    std::process::exit(127);
                }
            }
        }
    }
}

/// Debugging command to emulate an MCP "shell" tool call.
#[derive(Parser, Debug)]
struct ShellExecArgs {
    command: String,
}

#[tokio::main]
pub async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    if let Some(subcommand) = cli.subcommand {
        match subcommand {
            Commands::Escalate(args) => {
                args.run().await?;
            }
            Commands::ShellExec(args) => {
                let result = shell_exec(ExecParams {
                    command: args.command.clone(),
                    workdir: std::env::current_dir()
                        .context("failed to get current directory")?
                        .to_string_lossy()
                        .to_string(),
                    timeout_ms: None,
                })
                .await?;
                println!("{result:?}");
                std::process::exit(result.exit_code);
            }
        }
    }

    // Fail early if the bash path is not set.
    let _ = get_bash_path()?;

    tracing::info!("Starting MCP server");
    let service = ExecTool::new().serve(stdio()).await.inspect_err(|e| {
        tracing::error!("serving error: {:?}", e);
    })?;

    service.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::collections::HashMap;
    use std::path::PathBuf;

    #[tokio::test]
    async fn handle_escalate_session_respects_run_in_sandbox_decision() -> anyhow::Result<()> {
        let (server, client) = AsyncSocket::pair()?;
        let server_task = tokio::spawn(handle_escalate_session_with_decider(
            server,
            |_file, _argv, _workdir| EscalateAction::RunInSandbox,
        ));

        client
            .send(EscalateClientMessage::EscalateRequest {
                file: "/bin/echo".to_string(),
                argv: vec!["echo".to_string()],
                workdir: PathBuf::from("/tmp"),
                env: HashMap::new(),
            })
            .await?;

        let response = client.receive::<EscalateServerMessage>().await?;
        assert_eq!(
            EscalateServerMessage::EscalateResponse(EscalateAction::RunInSandbox),
            response
        );
        server_task.await?
    }

    #[tokio::test]
    async fn handle_escalate_session_executes_escalated_command() -> anyhow::Result<()> {
        let (server, client) = AsyncSocket::pair()?;
        let server_task = tokio::spawn(handle_escalate_session_with_decider(
            server,
            |_file, _argv, _workdir| EscalateAction::Escalate,
        ));

        client
            .send(EscalateClientMessage::EscalateRequest {
                file: "/bin/sh".to_string(),
                argv: vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    r#"if [ "$KEY" = VALUE ]; then exit 42; else exit 1; fi"#.to_string(),
                ],
                workdir: std::env::current_dir()?,
                env: HashMap::from([("KEY".to_string(), "VALUE".to_string())]),
            })
            .await?;

        let response = client.receive::<EscalateServerMessage>().await?;
        assert_eq!(
            EscalateServerMessage::EscalateResponse(EscalateAction::Escalate),
            response
        );

        client
            .send_with_fds(SuperExecMessage { fds: Vec::new() }, &[])
            .await?;

        let result = client.receive::<SuperExecResult>().await?;
        assert_eq!(42, result.exit_code);

        server_task.await?
    }
}
