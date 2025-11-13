use anyhow::Context as _;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use clap::Parser;
use clap::Subcommand;
use codex_core::exec::SandboxType;
use codex_core::exec::process_exec_tool_call;
use codex_core::get_platform_sandbox;
use codex_core::protocol::SandboxPolicy;
use libc::c_uint;
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
use socket2::Domain;
use socket2::MaybeUninitSlice;
use socket2::MsgHdr;
use socket2::MsgHdrMut;
use socket2::Socket;
use socket2::Type;
use std::collections::HashMap;
use std::io;
use std::io::IoSlice;
use std::io::stdin;
use std::mem::MaybeUninit;
use std::os::fd::AsFd;
use std::os::fd::AsRawFd;
use std::os::fd::BorrowedFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;
use std::os::fd::RawFd;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::Interest;
use tokio::io::unix::AsyncFd;
use tokio::process::Command;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::writer::Tee;
use tracing_subscriber::{self};

// C->S
#[derive(Clone, Serialize, Deserialize, Debug)]
enum ClientMessage {
    EscalateRequest {
        file: String,
        argv: Vec<String>,
        workdir: String,
    },
}

// C->S
#[derive(Clone, Serialize, Deserialize, Debug)]
enum ServerMessage {
    EscalateResponse(EscalateAction),
}

#[derive(Clone, Serialize, Deserialize, Debug)]
enum EscalateAction {
    RunInSandbox,
    Escalate,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
struct SuperExecMessage {
    fds: Vec<RawFd>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
struct SuperExecResult {
    exit_code: i32,
}

#[derive(Debug)]
struct ReceivedMessage {
    data: Vec<u8>,
    #[allow(dead_code)]
    fds: Vec<OwnedFd>,
}

#[derive(Debug)]
// TODO: just make this arguments to send_message
struct SentMessage<'a> {
    data: Vec<u8>,
    fds: Vec<BorrowedFd<'a>>,
}

/*

#[derive(Clone)]
struct DgramSocket {
    pub fd: RawFd,
}
impl DgramSocket {
    fn new(fd: RawFd) -> Self {
        Self { fd }
    }

    async fn recv_message(&self) -> anyhow::Result<ReceivedMessage> {
        use tokio::io::unix::AsyncFd;

        const MAX_MESSAGE_SIZE: usize = 64 * 1024;
        const MAX_FDS_PER_MESSAGE: usize = 16;
        let async_fd = AsyncFd::new(self.fd)?;
        let mut buffer = vec![0u8; MAX_MESSAGE_SIZE];
        let control_capacity = unsafe {
            libc::CMSG_SPACE((MAX_FDS_PER_MESSAGE * std::mem::size_of::<RawFd>()) as _) as usize
        };

        let (received, fds) = async_fd
            .async_io(Interest::READABLE, |fd| {
                let mut iov = libc::iovec {
                    iov_base: buffer.as_mut_ptr() as *mut libc::c_void,
                    iov_len: buffer.len(),
                };
                let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
                msg.msg_iov = &mut iov;
                msg.msg_iovlen = 1;
                let mut control = vec![0u8; control_capacity];
                msg.msg_control = control.as_mut_ptr() as *mut libc::c_void;
                msg.msg_controllen = control.len() as _;

                let received = unsafe { libc::recvmsg(*fd, &mut msg, 0) };
                if received < 0 {
                    return Err(std::io::Error::last_os_error());
                }

                let mut received_fds = Vec::new();
                let mut cmsg = unsafe { libc::CMSG_FIRSTHDR(&msg) };
                while !cmsg.is_null() {
                    unsafe {
                        if (*cmsg).cmsg_level == libc::SOL_SOCKET
                            && (*cmsg).cmsg_type == libc::SCM_RIGHTS
                        {
                            let data_ptr = libc::CMSG_DATA(cmsg) as *const RawFd;
                            let payload_len = ((*cmsg).cmsg_len as usize)
                                .saturating_sub(libc::CMSG_LEN(0) as usize);
                            let fd_count = payload_len / std::mem::size_of::<RawFd>();
                            let fd_slice = std::slice::from_raw_parts(data_ptr, fd_count);
                            received_fds.extend_from_slice(fd_slice);
                        }
                        cmsg = libc::CMSG_NXTHDR(&msg, cmsg);
                    }
                }

                if msg.msg_flags & libc::MSG_TRUNC != 0 {
                    for fd in &received_fds {
                        unsafe {
                            libc::close(*fd);
                        }
                    }
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "truncated datagram on escalate socket",
                    ));
                }
                if msg.msg_flags & libc::MSG_CTRUNC != 0 {
                    for fd in &received_fds {
                        unsafe {
                            libc::close(*fd);
                        }
                    }
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "truncated control message on escalate socket",
                    ));
                }

                Ok((received as usize, received_fds))
            })
            .await?;
        buffer.truncate(received);
        let fds = fds
            .into_iter()
            .map(|fd| unsafe { OwnedFd::from_raw_fd(fd) })
            .collect();
        Ok(ReceivedMessage { data: buffer, fds })
    }

    async fn send_message(&self, data: &[u8]) -> anyhow::Result<()> {
        use std::os::unix::io::RawFd;
        use tokio::io::unix::AsyncFd;
        let fd = self.fd as RawFd;
        let async_fd = AsyncFd::new(fd)?;
        async_fd
            .async_io(Interest::WRITABLE, |fd| {
                let mut iov = libc::iovec {
                    iov_base: data.as_ptr() as *mut libc::c_void,
                    iov_len: data.len(),
                };
                let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
                msg.msg_iov = &mut iov;
                msg.msg_iovlen = 1;

                let written = unsafe { libc::sendmsg(*fd, &msg, 0) };
                if written < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                if written as usize != data.len() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::WriteZero,
                        "partial datagram send on escalate socket",
                    ));
                }
                Ok(())
            })
            .await?;
        Ok(())
    }

    async fn send_message_with_fds(&self, data: &[u8], fds: &[RawFd]) -> anyhow::Result<()> {
        use tokio::io::unix::AsyncFd;

        if fds.is_empty() {
            return self.send_message(data).await;
        }

        let fd = self.fd;
        let async_fd = AsyncFd::new(fd)?;
        async_fd
            .async_io(Interest::WRITABLE, |fd| {
                let mut iov = libc::iovec {
                    iov_base: data.as_ptr() as *mut libc::c_void,
                    iov_len: data.len(),
                };
                let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
                msg.msg_iov = &mut iov;
                msg.msg_iovlen = 1;

                let fd_bytes = std::mem::size_of_val(fds);
                let control_len = unsafe { libc::CMSG_SPACE(fd_bytes as _) as usize };
                let mut control = vec![0u8; control_len];
                msg.msg_control = control.as_mut_ptr() as *mut libc::c_void;
                msg.msg_controllen = control_len as _;

                let cmsg = unsafe { libc::CMSG_FIRSTHDR(&msg) };
                if cmsg.is_null() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "failed to build control message",
                    ));
                }

                unsafe {
                    (*cmsg).cmsg_level = libc::SOL_SOCKET;
                    (*cmsg).cmsg_type = libc::SCM_RIGHTS;
                    (*cmsg).cmsg_len = libc::CMSG_LEN(fd_bytes as _) as _;
                    let data_ptr = libc::CMSG_DATA(cmsg) as *mut RawFd;
                    std::ptr::copy_nonoverlapping(fds.as_ptr(), data_ptr, fds.len());
                }

                let written = unsafe { libc::sendmsg(*fd, &msg, 0) };
                if written < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                if written as usize != data.len() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::WriteZero,
                        "partial datagram send on escalate socket",
                    ));
                }
                Ok(())
            })
            .await?;
        Ok(())
    }
}

impl Drop for DgramSocket {
    fn drop(&mut self) {
        unsafe {
            let _ = libc::close(self.fd);
        }
    }
}
*/

fn assume_init(buf: &[MaybeUninit<u8>]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(buf.as_ptr().cast(), buf.len()) }
}
fn bytes_to_string(buf: &[MaybeUninit<u8>]) -> String {
    let bytes = assume_init(buf);
    String::from_utf8_lossy(bytes).to_string()
}
fn control_space_for_fds(count: usize) -> usize {
    unsafe { libc::CMSG_SPACE((count * size_of::<RawFd>()) as _) as usize }
}
fn extract_fds(control: &mut [MaybeUninit<u8>], len: usize) -> std::io::Result<Vec<OwnedFd>> {
    if len == 0 {
        return Ok(Vec::new());
    }
    let mut fds = Vec::new();
    let mut hdr: libc::msghdr = unsafe { std::mem::zeroed() };
    hdr.msg_control = control.as_mut_ptr().cast();
    hdr.msg_controllen = len as _;

    let mut cmsg = unsafe { libc::CMSG_FIRSTHDR(&hdr) };
    while !cmsg.is_null() {
        let level = unsafe { (*cmsg).cmsg_level };
        let ty = unsafe { (*cmsg).cmsg_type };
        if level == libc::SOL_SOCKET && ty == libc::SCM_RIGHTS {
            let data_ptr = unsafe { libc::CMSG_DATA(cmsg).cast::<RawFd>() };
            let fd_count: usize = {
                let cmsg_data_len =
                    unsafe { (*cmsg).cmsg_len as usize } - unsafe { libc::CMSG_LEN(0) as usize };
                cmsg_data_len / size_of::<RawFd>()
            };
            for i in 0..fd_count {
                let fd = unsafe { data_ptr.add(i).read() };
                fds.push(unsafe { OwnedFd::from_raw_fd(fd) });
            }
        }
        cmsg = unsafe { libc::CMSG_NXTHDR(&hdr, cmsg) };
    }
    Ok(fds)
}
const MAX_FDS_PER_MESSAGE: usize = 16;
const MAX_MESSAGE_SIZE: usize = 64 * 1024;
fn receive_message(socket: &Socket) -> std::io::Result<ReceivedMessage> {
    let mut data = [MaybeUninit::<u8>::uninit(); MAX_MESSAGE_SIZE];
    let mut control = vec![MaybeUninit::<u8>::uninit(); control_space_for_fds(MAX_FDS_PER_MESSAGE)];
    let (received, control_len) = {
        let mut bufs = [MaybeUninitSlice::new(&mut data)];
        let mut msg = MsgHdrMut::new()
            .with_buffers(&mut bufs)
            .with_control(&mut control);
        let received = socket.recvmsg(&mut msg, 0)?;
        (received, msg.control_len())
    };

    let message = assume_init(&data[..received]).to_vec();
    let fds = extract_fds(&mut control, control_len)?;
    Ok(ReceivedMessage { data: message, fds })
}
fn send_message(
    socket: &Socket,
    SentMessage { data, fds }: SentMessage<'_>,
) -> std::io::Result<()> {
    if fds.len() > MAX_FDS_PER_MESSAGE {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("too many fds: {}", fds.len()),
        ));
    }
    let mut control = vec![0u8; control_space_for_fds(fds.len())];
    unsafe {
        let cmsg = control.as_mut_ptr().cast::<libc::cmsghdr>();
        (*cmsg).cmsg_len = libc::CMSG_LEN(size_of::<RawFd>() as c_uint * fds.len() as c_uint) as _;
        (*cmsg).cmsg_level = libc::SOL_SOCKET;
        (*cmsg).cmsg_type = libc::SCM_RIGHTS;
        let data_ptr = libc::CMSG_DATA(cmsg).cast::<RawFd>();
        for (i, fd) in fds.iter().enumerate() {
            data_ptr.add(i).write(fd.as_raw_fd());
        }
    }

    let payload = [IoSlice::new(&data)];
    let msg = MsgHdr::new().with_buffers(&payload).with_control(&control);
    socket.sendmsg(&msg, 0)?;
    Ok(())
}

struct EscalateSocket {
    server: Socket,
    client: Socket,
}

impl EscalateSocket {
    fn open() -> anyhow::Result<EscalateSocket> {
        let (server, client) = Socket::pair(Domain::UNIX, Type::DGRAM, None)?;
        client.set_cloexec(false)?;
        let socket = EscalateSocket { server, client };
        Ok(socket)
    }
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ExecParams {
    pub command: String,
    pub workdir: String,
    pub timeout_ms: Option<u64>,
    pub with_escalated_permissions: Option<bool>,
    pub justification: Option<String>,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct ExecResult {
    pub exit_code: i32,
    pub output: String,
    pub duration: Duration,
    pub timed_out: bool,
}

fn decide_escalate(file: &str, argv: &[String], workdir: &str) -> EscalateAction {
    if file == "/bin/echo" {
        EscalateAction::Escalate
    } else {
        EscalateAction::RunInSandbox
    }
}

async fn super_exec_task(
    socket: Socket,
    file: String,
    argv: Vec<String>,
    workdir: String,
) -> anyhow::Result<()> {
    socket.set_nonblocking(true)?;
    let server_async_fd = AsyncFd::new(socket)?;
    let msg = server_async_fd
        .async_io(Interest::READABLE, receive_message)
        .await
        .context("failed to receive message")?;
    let message: SuperExecMessage = serde_json::from_slice(&msg.data)?;
    assert_eq!(msg.fds.len(), message.fds.len());

    assert!(
        message
            .fds
            .iter()
            .all(|src_fd| !msg.fds.iter().any(|dst_fd| dst_fd.as_raw_fd() == *src_fd)),
        "TODO: handle overlapping fds"
    );

    let mut child = unsafe {
        Command::new(file)
            .args(&argv[1..])
            .arg0(argv[0].clone())
            .current_dir(workdir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .pre_exec(move || {
                for (dst_fd, src_fd) in message.fds.iter().zip(&msg.fds) {
                    libc::dup2(src_fd.as_raw_fd(), *dst_fd);
                }
                Ok(())
            })
            .spawn()
            .context("failed to spawn command")?
    };

    let exit_status = child.wait().await?;
    let result = SuperExecResult {
        exit_code: exit_status.code().unwrap_or(127),
    };
    server_async_fd
        .async_io(Interest::WRITABLE, |server| {
            send_message(
                server,
                SentMessage {
                    data: serde_json::to_vec(&result)?,
                    fds: vec![],
                },
            )
        })
        .await
        .context("failed to receive message")?;

    Ok(())
}

async fn escalate_task(socket: Socket) -> anyhow::Result<()> {
    socket.set_nonblocking(true)?;
    let server_async_fd = AsyncFd::new(socket)?;
    loop {
        let msg = server_async_fd
            .async_io(Interest::READABLE, receive_message)
            .await
            .context("failed to receive message")?;

        let message: ClientMessage = serde_json::from_slice(&msg.data)?;
        let ClientMessage::EscalateRequest {
            file,
            argv,
            workdir,
        } = message;
        /*
        let response = context
            .peer
            .create_elicitation(CreateElicitationRequestParam {
                message: format!("Allow Codex to run `{command:?}` in `{workdir:?}`?"),
                requested_schema: ElicitationSchema::builder()
                    .property("dummy", PrimitiveSchema::String(StringSchema::new()))
                    .build()
                    .expect("failed to build elicitation schema"),
            })
            .await
            .expect("failed to create elicitation");
        match response.action {
            ElicitationAction::Accept => {
                let response_message =
                    ServerMessage::EscalateResponse(EscalateAction::EscalateRequest);
                escalate_socket
                    .server
                    .send_message(
                        &serde_json::to_vec(&response_message)
                            .expect("failed to serialize response message"),
                    )
                    .await
                    .expect("failed to send message");
            }
            ElicitationAction::Decline => {
                let response_message =
                    ServerMessage::EscalateResponse(EscalateAction::RunInSandbox);
                escalate_socket
                    .server
                    .send_message(
                        &serde_json::to_vec(&response_message)
                            .expect("failed to serialize response message"),
                    )
                    .await
                    .expect("failed to send message");
            }
            ElicitationAction::Cancel => {
                todo!("kill the task probably");
            }
        }
        */
        let action = decide_escalate(&file, &argv, &workdir);
        tracing::debug!("decided {action:?} for {file:?} {argv:?} {workdir:?}");
        match action {
            EscalateAction::RunInSandbox => {
                server_async_fd
                    .async_io(Interest::WRITABLE, |server| {
                        server.send(&serde_json::to_vec(&ServerMessage::EscalateResponse(
                            EscalateAction::RunInSandbox,
                        ))?)
                    })
                    .await
                    .context("failed to send message")?;
            }
            EscalateAction::Escalate => {
                let (super_exec_server, super_exec_client) =
                    Socket::pair(Domain::UNIX, Type::DGRAM, None)
                        .context("failed to create socket pair")?;
                tokio::spawn(super_exec_task(super_exec_server, file, argv, workdir));
                server_async_fd
                    .async_io(Interest::WRITABLE, |server| {
                        let data = serde_json::to_vec(&ServerMessage::EscalateResponse(
                            EscalateAction::Escalate,
                        ))?;
                        send_message(
                            server,
                            SentMessage {
                                data,
                                fds: vec![super_exec_client.as_fd()],
                            },
                        )
                    })
                    .await
                    .context("failed to send message")?;
            }
        }
    }
}

async fn shell_exec(params: ExecParams) -> anyhow::Result<ExecResult> {
    let ExecParams {
        command,
        workdir,
        timeout_ms,
        with_escalated_permissions,
        justification,
    } = params;
    let escalate_socket = EscalateSocket::open()?;

    let client_fd = escalate_socket.client.as_raw_fd();
    let escalate_task = tokio::spawn(escalate_task(escalate_socket.server));
    let result = process_exec_tool_call(
        codex_core::exec::ExecParams {
            command: vec![
                "/users/nornagon/code/bash/bash".to_string(),
                "-c".to_string(),
                command,
            ],
            cwd: PathBuf::from(&workdir),
            timeout_ms,
            env: {
                let mut env = HashMap::new();
                env.insert("CODEX_ESCALATE_SOCKET".to_string(), client_fd.to_string());
                let current_exe = std::env::current_exe()?;
                env.insert(
                    "BASH_EXEC_WRAPPER".to_string(),
                    format!("{} escalate", current_exe.to_string_lossy()),
                );
                env
            },
            with_escalated_permissions,
            justification,
            arg0: None,
        },
        get_platform_sandbox().unwrap_or(SandboxType::None),
        &SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![std::env::current_dir()?.to_path_buf()],
            network_access: false,
            exclude_tmpdir_env_var: false,
            exclude_slash_tmp: false,
        },
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

    #[tool(description = "Runs a shell command and returns its output.")]
    async fn shell(
        &self,
        context: RequestContext<RoleServer>,
        Parameters(ExecParams {
            command,
            workdir,
            timeout_ms,
            with_escalated_permissions,
            justification,
        }): Parameters<ExecParams>,
    ) -> Result<CallToolResult, McpError> {
        #[allow(clippy::expect_used)]
        let escalate_socket = EscalateSocket::open().expect("failed to open escalate socket");

        let client_fd = escalate_socket.client.as_raw_fd();
        #[allow(clippy::expect_used)]
        let escalate_task = tokio::spawn(escalate_task(escalate_socket.server));
        let result = process_exec_tool_call(
            codex_core::exec::ExecParams {
                command: vec![
                    "/users/nornagon/code/bash/bash".to_string(),
                    "-lc".to_string(),
                    command,
                ],
                cwd: PathBuf::from(&workdir),
                timeout_ms,
                env: {
                    let mut env = HashMap::new();
                    env.insert("CODEX_ESCALATE_SOCKET".to_string(), client_fd.to_string());
                    let current_exe =
                        std::env::current_exe().expect("failed to get current process path");
                    env.insert(
                        "BASH_EXEC_WRAPPER".to_string(),
                        format!("{} escalate", current_exe.to_string_lossy()),
                    );
                    env
                },
                with_escalated_permissions,
                justification,
                arg0: None,
            },
            get_platform_sandbox().unwrap_or(SandboxType::None),
            &SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![
                    std::env::current_dir().expect("failed to get current directory"),
                ],
                network_access: false,
                exclude_tmpdir_env_var: false,
                exclude_slash_tmp: false,
            },
            &PathBuf::from("/__NONEXISTENT__"), // This is ignored for ReadOnly
            &None,
            None,
        )
        .await
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        escalate_task.abort();
        let result = ExecResult {
            exit_code: result.exit_code,
            output: result.aggregated_output.text,
            duration: result.duration,
            timed_out: result.timed_out,
        };
        Ok(CallToolResult::success(vec![Content::json(result)?]))
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
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            server_info: Implementation::from_build_env(),
            instructions: Some("This server provides counter tools and prompts. Tools: increment, decrement, get_value, say_hello, echo, sum. Prompts: example_prompt (takes a message), counter_analysis (analyzes counter state with a goal).".to_string()),
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

#[derive(Parser, Debug)]
struct EscalateArgs {
    file: String,

    #[arg(trailing_var_arg = true)]
    argv: Vec<String>,
}

#[derive(Parser, Debug)]
struct ShellExecArgs {
    command: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();
    #[allow(clippy::expect_used)]
    if let Some(subcommand) = cli.subcommand {
        match subcommand {
            Commands::Escalate(args) => {
                let client_fd = std::env::var("CODEX_ESCALATE_SOCKET")
                    .expect("CODEX_ESCALATE_SOCKET is not set")
                    .parse::<i32>()
                    .expect("CODEX_ESCALATE_SOCKET is not a valid integer");
                let client = unsafe { Socket::from_raw_fd(client_fd) };
                let message = ClientMessage::EscalateRequest {
                    file: args.file.clone(),
                    argv: args.argv.clone(),
                    workdir: std::env::current_dir()
                        .expect("failed to get current directory")
                        .to_string_lossy()
                        .to_string(),
                };
                let message = serde_json::to_vec(&message).expect("failed to serialize message");
                client.send(&message)?;
                let ReceivedMessage { data, mut fds } = receive_message(&client)?;
                let message: ServerMessage =
                    serde_json::from_slice(&data).expect("failed to deserialize message");
                let ServerMessage::EscalateResponse(action) = message;
                match action {
                    EscalateAction::Escalate => {
                        if fds.len() != 1 {
                            bail!("expected 1 fd, got {}", fds.len());
                        }
                        let fd = fds.remove(0);
                        let escalate_socket = Socket::from(fd);
                        let all_fds = [
                            io::stdin().as_raw_fd(),
                            io::stdout().as_raw_fd(),
                            io::stderr().as_raw_fd(),
                        ];
                        let fds: Vec<BorrowedFd> = all_fds
                            .iter()
                            .copied()
                            .filter(|&fd| fd >= 0)
                            .map(|fd| unsafe { BorrowedFd::borrow_raw(fd) })
                            .collect();
                        let message = SuperExecMessage {
                            fds: fds.iter().map(AsRawFd::as_raw_fd).collect(),
                        };
                        let data = serde_json::to_vec(&message)?;
                        send_message(&escalate_socket, SentMessage { data, fds })?;
                        let ReceivedMessage { data, .. } = receive_message(&escalate_socket)?;
                        let message: SuperExecResult =
                            serde_json::from_slice(&data).expect("failed to deserialize message");
                        let SuperExecResult { exit_code } = message;
                        std::process::exit(exit_code);
                    }
                    EscalateAction::RunInSandbox => {
                        use std::ffi::CString;
                        let file = CString::new(args.file.as_str()).expect("NUL in file");

                        let argv_cstrs: Vec<CString> = args
                            .argv
                            .iter()
                            .map(|s| CString::new(s.as_str()).expect("NUL in argv"))
                            .collect();

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
            Commands::ShellExec(args) => {
                let result = shell_exec(ExecParams {
                    command: args.command.clone(),
                    workdir: std::env::current_dir()
                        .expect("failed to get current directory")
                        .to_string_lossy()
                        .to_string(),
                    timeout_ms: None,
                    with_escalated_permissions: None,
                    justification: None,
                })
                .await?;
                println!("{result:?}");
                std::process::exit(result.exit_code);
            }
        }
    }

    tracing::info!("Starting MCP server");
    let service = ExecTool::new().serve(stdio()).await.inspect_err(|e| {
        tracing::error!("serving error: {:?}", e);
    })?;

    service.waiting().await?;
    Ok(())
}
