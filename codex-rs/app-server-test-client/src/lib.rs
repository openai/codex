use std::collections::VecDeque;
use std::ffi::OsString;
use std::fs;
use std::fs::OpenOptions;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::net::TcpStream;
use std::path::Path;
use std::path::PathBuf;
use std::process::Child;
use std::process::ChildStdin;
use std::process::ChildStdout;
use std::process::Command;
use std::process::Stdio;
use std::thread;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use clap::ArgAction;
use clap::Parser;
use clap::Subcommand;
use codex_app_server_protocol::AddConversationListenerParams;
use codex_app_server_protocol::AddConversationSubscriptionResponse;
use codex_app_server_protocol::AskForApproval;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::CommandExecutionApprovalDecision;
use codex_app_server_protocol::CommandExecutionRequestApprovalParams;
use codex_app_server_protocol::CommandExecutionRequestApprovalResponse;
use codex_app_server_protocol::CommandExecutionStatus;
use codex_app_server_protocol::DynamicToolSpec;
use codex_app_server_protocol::FileChangeApprovalDecision;
use codex_app_server_protocol::FileChangeRequestApprovalParams;
use codex_app_server_protocol::FileChangeRequestApprovalResponse;
use codex_app_server_protocol::GetAccountRateLimitsResponse;
use codex_app_server_protocol::InitializeCapabilities;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::InitializeResponse;
use codex_app_server_protocol::InputItem;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCRequest;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::LoginChatGptCompleteNotification;
use codex_app_server_protocol::LoginChatGptResponse;
use codex_app_server_protocol::ModelListParams;
use codex_app_server_protocol::ModelListResponse;
use codex_app_server_protocol::NewConversationParams;
use codex_app_server_protocol::NewConversationResponse;
use codex_app_server_protocol::ReadOnlyAccess;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SandboxPolicy;
use codex_app_server_protocol::SendUserMessageParams;
use codex_app_server_protocol::SendUserMessageResponse;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadListParams;
use codex_app_server_protocol::ThreadListResponse;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_protocol::ThreadId;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tungstenite::Message;
use tungstenite::WebSocket;
use tungstenite::connect;
use tungstenite::stream::MaybeTlsStream;
use url::Url;
use uuid::Uuid;

const NOTIFICATIONS_TO_OPT_OUT: &[&str] = &[
    // Legacy codex/event (v1-style) deltas.
    "codex/event/agent_message_content_delta",
    "codex/event/agent_message_delta",
    "codex/event/agent_reasoning_delta",
    "codex/event/reasoning_content_delta",
    "codex/event/reasoning_raw_content_delta",
    "codex/event/exec_command_output_delta",
    // Other legacy events.
    "codex/event/exec_approval_request",
    "codex/event/exec_command_begin",
    "codex/event/exec_command_end",
    "codex/event/exec_output",
    "codex/event/item_started",
    "codex/event/item_completed",
    // v2 item deltas.
    "item/agentMessage/delta",
    "item/plan/delta",
    "item/commandExecution/outputDelta",
    "item/fileChange/outputDelta",
    "item/reasoning/summaryTextDelta",
    "item/reasoning/textDelta",
];

/// Minimal launcher that initializes the Codex app-server and logs the handshake.
#[derive(Parser)]
#[command(author = "Codex", version, about = "Bootstrap Codex app-server", long_about = None)]
struct Cli {
    /// Path to the `codex` CLI binary. When set, requests use stdio by
    /// spawning `codex app-server` as a child process.
    #[arg(long, env = "CODEX_BIN", global = true)]
    codex_bin: Option<PathBuf>,

    /// Existing websocket server URL to connect to.
    ///
    /// If neither `--codex-bin` nor `--url` is provided, defaults to
    /// `ws://127.0.0.1:4222`.
    #[arg(long, env = "CODEX_APP_SERVER_URL", global = true)]
    url: Option<String>,

    /// Forwarded to the `codex` CLI as `--config key=value`. Repeatable.
    ///
    /// Example:
    ///   `--config 'model_providers.mock.base_url="http://localhost:4010/v2"'`
    #[arg(
        short = 'c',
        long = "config",
        value_name = "key=value",
        action = ArgAction::Append,
        global = true
    )]
    config_overrides: Vec<String>,

    /// JSON array of dynamic tool specs or a single tool object.
    /// Prefix a filename with '@' to read from a file.
    ///
    /// Example:
    ///   --dynamic-tools '[{"name":"demo","description":"Demo","inputSchema":{"type":"object"}}]'
    ///   --dynamic-tools @/path/to/tools.json
    #[arg(long, value_name = "json-or-@file", global = true)]
    dynamic_tools: Option<String>,

    #[command(subcommand)]
    command: CliCommand,
}

#[derive(Subcommand)]
enum CliCommand {
    /// Start `codex app-server` on a websocket endpoint in the background.
    ///
    /// Logs are written to:
    ///   `/tmp/codex-app-server-test-client/`
    Serve {
        /// WebSocket listen URL passed to `codex app-server --listen`.
        #[arg(long, default_value = "ws://127.0.0.1:4222")]
        listen: String,
        /// Kill any process listening on the same port before starting.
        #[arg(long, default_value_t = false)]
        kill: bool,
    },
    /// Send a user message through the Codex app-server.
    SendMessage {
        /// User message to send to Codex.
        user_message: String,
    },
    /// Send a user message through the app-server V2 thread/turn APIs.
    SendMessageV2 {
        /// User message to send to Codex.
        user_message: String,
    },
    /// Resume a V2 thread by id, then send a user message.
    ResumeMessageV2 {
        /// Existing thread id to resume.
        thread_id: String,
        /// User message to send to Codex.
        user_message: String,
    },
    /// Resume a V2 thread and continuously stream notifications/events.
    ///
    /// This command does not auto-exit; stop it with SIGINT/SIGTERM/SIGKILL.
    ThreadResume {
        /// Existing thread id to resume.
        thread_id: String,
    },
    /// Start a V2 turn that elicits an ExecCommand approval.
    #[command(name = "trigger-cmd-approval")]
    TriggerCmdApproval {
        /// Optional prompt; defaults to a simple python command.
        user_message: Option<String>,
    },
    /// Start a V2 turn that elicits an ApplyPatch approval.
    #[command(name = "trigger-patch-approval")]
    TriggerPatchApproval {
        /// Optional prompt; defaults to creating a file via apply_patch.
        user_message: Option<String>,
    },
    /// Start a V2 turn that should not elicit an ExecCommand approval.
    #[command(name = "no-trigger-cmd-approval")]
    NoTriggerCmdApproval,
    /// Send two sequential V2 turns in the same thread to test follow-up behavior.
    SendFollowUpV2 {
        /// Initial user message for the first turn.
        first_message: String,
        /// Follow-up user message for the second turn.
        follow_up_message: String,
    },
    /// Trigger zsh-fork multi-subcommand approvals and assert expected approval behavior.
    #[command(name = "trigger-zsh-fork-multi-cmd-approval")]
    TriggerZshForkMultiCmdApproval {
        /// Optional prompt; defaults to an explicit `/usr/bin/true && /usr/bin/true` command.
        user_message: Option<String>,
        /// Minimum number of command-approval callbacks expected in the turn.
        #[arg(long, default_value_t = 2)]
        min_approvals: usize,
        /// One-based approval index to abort (e.g. --abort-on 2 aborts the second approval).
        #[arg(long)]
        abort_on: Option<usize>,
    },
    /// Trigger the ChatGPT login flow and wait for completion.
    TestLogin,
    /// Fetch the current account rate limits from the Codex app-server.
    GetAccountRateLimits,
    /// List the available models from the Codex app-server.
    #[command(name = "model-list")]
    ModelList,
    /// List stored threads from the Codex app-server.
    #[command(name = "thread-list")]
    ThreadList {
        /// Number of threads to return.
        #[arg(long, default_value_t = 20)]
        limit: u32,
    },
}

pub fn run() -> Result<()> {
    let Cli {
        codex_bin,
        url,
        config_overrides,
        dynamic_tools,
        command,
    } = Cli::parse();

    let dynamic_tools = parse_dynamic_tools_arg(&dynamic_tools)?;

    match command {
        CliCommand::Serve { listen, kill } => {
            ensure_dynamic_tools_unused(&dynamic_tools, "serve")?;
            let codex_bin = codex_bin.unwrap_or_else(|| PathBuf::from("codex"));
            serve(&codex_bin, &config_overrides, &listen, kill)
        }
        CliCommand::SendMessage { user_message } => {
            ensure_dynamic_tools_unused(&dynamic_tools, "send-message")?;
            let endpoint = resolve_endpoint(codex_bin, url)?;
            send_message(&endpoint, &config_overrides, user_message)
        }
        CliCommand::SendMessageV2 { user_message } => {
            let endpoint = resolve_endpoint(codex_bin, url)?;
            send_message_v2_endpoint(&endpoint, &config_overrides, user_message, &dynamic_tools)
        }
        CliCommand::ResumeMessageV2 {
            thread_id,
            user_message,
        } => {
            let endpoint = resolve_endpoint(codex_bin, url)?;
            resume_message_v2(
                &endpoint,
                &config_overrides,
                thread_id,
                user_message,
                &dynamic_tools,
            )
        }
        CliCommand::ThreadResume { thread_id } => {
            ensure_dynamic_tools_unused(&dynamic_tools, "thread-resume")?;
            let endpoint = resolve_endpoint(codex_bin, url)?;
            thread_resume_follow(&endpoint, &config_overrides, thread_id)
        }
        CliCommand::TriggerCmdApproval { user_message } => {
            let endpoint = resolve_endpoint(codex_bin, url)?;
            trigger_cmd_approval(&endpoint, &config_overrides, user_message, &dynamic_tools)
        }
        CliCommand::TriggerPatchApproval { user_message } => {
            let endpoint = resolve_endpoint(codex_bin, url)?;
            trigger_patch_approval(&endpoint, &config_overrides, user_message, &dynamic_tools)
        }
        CliCommand::NoTriggerCmdApproval => {
            let endpoint = resolve_endpoint(codex_bin, url)?;
            no_trigger_cmd_approval(&endpoint, &config_overrides, &dynamic_tools)
        }
        CliCommand::SendFollowUpV2 {
            first_message,
            follow_up_message,
        } => {
            let endpoint = resolve_endpoint(codex_bin, url)?;
            send_follow_up_v2(
                &endpoint,
                &config_overrides,
                first_message,
                follow_up_message,
                &dynamic_tools,
            )
        }
        CliCommand::TriggerZshForkMultiCmdApproval {
            user_message,
            min_approvals,
            abort_on,
        } => {
            let endpoint = resolve_endpoint(codex_bin, url)?;
            trigger_zsh_fork_multi_cmd_approval(
                &endpoint,
                &config_overrides,
                user_message,
                min_approvals,
                abort_on,
                &dynamic_tools,
            )
        }
        CliCommand::TestLogin => {
            ensure_dynamic_tools_unused(&dynamic_tools, "test-login")?;
            let endpoint = resolve_endpoint(codex_bin, url)?;
            test_login(&endpoint, &config_overrides)
        }
        CliCommand::GetAccountRateLimits => {
            ensure_dynamic_tools_unused(&dynamic_tools, "get-account-rate-limits")?;
            let endpoint = resolve_endpoint(codex_bin, url)?;
            get_account_rate_limits(&endpoint, &config_overrides)
        }
        CliCommand::ModelList => {
            ensure_dynamic_tools_unused(&dynamic_tools, "model-list")?;
            let endpoint = resolve_endpoint(codex_bin, url)?;
            model_list(&endpoint, &config_overrides)
        }
        CliCommand::ThreadList { limit } => {
            ensure_dynamic_tools_unused(&dynamic_tools, "thread-list")?;
            let endpoint = resolve_endpoint(codex_bin, url)?;
            thread_list(&endpoint, &config_overrides, limit)
        }
    }
}

enum Endpoint {
    SpawnCodex(PathBuf),
    ConnectWs(String),
}

fn resolve_endpoint(codex_bin: Option<PathBuf>, url: Option<String>) -> Result<Endpoint> {
    if codex_bin.is_some() && url.is_some() {
        bail!("--codex-bin and --url are mutually exclusive");
    }
    if let Some(codex_bin) = codex_bin {
        return Ok(Endpoint::SpawnCodex(codex_bin));
    }
    if let Some(url) = url {
        return Ok(Endpoint::ConnectWs(url));
    }
    Ok(Endpoint::ConnectWs("ws://127.0.0.1:4222".to_string()))
}

fn serve(codex_bin: &Path, config_overrides: &[String], listen: &str, kill: bool) -> Result<()> {
    let runtime_dir = PathBuf::from("/tmp/codex-app-server-test-client");
    fs::create_dir_all(&runtime_dir)
        .with_context(|| format!("failed to create runtime dir {}", runtime_dir.display()))?;
    let log_path = runtime_dir.join("app-server.log");
    if kill {
        kill_listeners_on_same_port(listen)?;
    }

    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .with_context(|| format!("failed to open log file {}", log_path.display()))?;
    let log_file_stderr = log_file
        .try_clone()
        .with_context(|| format!("failed to clone log file handle {}", log_path.display()))?;

    let mut cmdline = format!(
        "tail -f /dev/null | RUST_BACKTRACE=full RUST_LOG=warn,codex_=trace {}",
        shell_quote(&codex_bin.display().to_string())
    );
    for override_kv in config_overrides {
        cmdline.push_str(&format!(" --config {}", shell_quote(override_kv)));
    }
    cmdline.push_str(&format!(" app-server --listen {}", shell_quote(listen)));

    let child = Command::new("nohup")
        .arg("sh")
        .arg("-c")
        .arg(cmdline)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log_file))
        .stderr(Stdio::from(log_file_stderr))
        .spawn()
        .with_context(|| format!("failed to start `{}` app-server", codex_bin.display()))?;

    let pid = child.id();

    println!("started codex app-server");
    println!("listen: {listen}");
    println!("pid: {pid} (launcher process)");
    println!("log: {}", log_path.display());

    Ok(())
}

fn kill_listeners_on_same_port(listen: &str) -> Result<()> {
    let url = Url::parse(listen).with_context(|| format!("invalid --listen URL `{listen}`"))?;
    let port = url
        .port_or_known_default()
        .with_context(|| format!("unable to infer port from --listen URL `{listen}`"))?;

    let output = Command::new("lsof")
        .arg("-nP")
        .arg(format!("-tiTCP:{port}"))
        .arg("-sTCP:LISTEN")
        .output()
        .with_context(|| format!("failed to run lsof for port {port}"))?;

    if !output.status.success() {
        return Ok(());
    }

    let pids: Vec<u32> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.trim().parse::<u32>().ok())
        .collect();

    if pids.is_empty() {
        return Ok(());
    }

    for pid in pids {
        println!("killing listener pid {pid} on port {port}");
        let pid_str = pid.to_string();
        let term_status = Command::new("kill")
            .arg(&pid_str)
            .status()
            .with_context(|| format!("failed to send SIGTERM to pid {pid}"))?;
        if !term_status.success() {
            continue;
        }
    }

    thread::sleep(Duration::from_millis(300));

    let output = Command::new("lsof")
        .arg("-nP")
        .arg(format!("-tiTCP:{port}"))
        .arg("-sTCP:LISTEN")
        .output()
        .with_context(|| format!("failed to re-check listeners on port {port}"))?;
    if !output.status.success() {
        return Ok(());
    }
    let remaining: Vec<u32> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.trim().parse::<u32>().ok())
        .collect();
    for pid in remaining {
        println!("force killing remaining listener pid {pid} on port {port}");
        let _ = Command::new("kill").arg("-9").arg(pid.to_string()).status();
    }

    Ok(())
}

fn shell_quote(input: &str) -> String {
    format!("'{}'", input.replace('\'', "'\\''"))
}

fn send_message(
    endpoint: &Endpoint,
    config_overrides: &[String],
    user_message: String,
) -> Result<()> {
    let mut client = CodexClient::connect(endpoint, config_overrides)?;

    let initialize = client.initialize()?;
    println!("< initialize response: {initialize:?}");

    let conversation = client.start_thread()?;
    println!("< newConversation response: {conversation:?}");

    let subscription = client.add_conversation_listener(&conversation.conversation_id)?;
    println!("< addConversationListener response: {subscription:?}");

    let send_response = client.send_user_message(&conversation.conversation_id, &user_message)?;
    println!("< sendUserMessage response: {send_response:?}");

    client.stream_conversation(&conversation.conversation_id)?;

    client.remove_thread_listener(subscription.subscription_id)?;

    Ok(())
}

pub fn send_message_v2(
    codex_bin: &Path,
    config_overrides: &[String],
    user_message: String,
    dynamic_tools: &Option<Vec<DynamicToolSpec>>,
) -> Result<()> {
    let endpoint = Endpoint::SpawnCodex(codex_bin.to_path_buf());
    send_message_v2_endpoint(&endpoint, config_overrides, user_message, dynamic_tools)
}

fn send_message_v2_endpoint(
    endpoint: &Endpoint,
    config_overrides: &[String],
    user_message: String,
    dynamic_tools: &Option<Vec<DynamicToolSpec>>,
) -> Result<()> {
    send_message_v2_with_policies(
        endpoint,
        config_overrides,
        user_message,
        None,
        None,
        dynamic_tools,
    )
}

fn trigger_zsh_fork_multi_cmd_approval(
    endpoint: &Endpoint,
    config_overrides: &[String],
    user_message: Option<String>,
    min_approvals: usize,
    abort_on: Option<usize>,
    dynamic_tools: &Option<Vec<DynamicToolSpec>>,
) -> Result<()> {
    if let Some(abort_on) = abort_on
        && abort_on == 0
    {
        bail!("--abort-on must be >= 1 when provided");
    }

    let default_prompt = "Run this exact command using shell command execution without rewriting or splitting it: /usr/bin/true && /usr/bin/true";
    let message = user_message.unwrap_or_else(|| default_prompt.to_string());

    let mut client = CodexClient::connect(endpoint, config_overrides)?;
    let initialize = client.initialize()?;
    println!("< initialize response: {initialize:?}");

    let thread_response = client.thread_start(ThreadStartParams {
        dynamic_tools: dynamic_tools.clone(),
        ..Default::default()
    })?;
    println!("< thread/start response: {thread_response:?}");

    client.command_approval_behavior = match abort_on {
        Some(index) => CommandApprovalBehavior::AbortOn(index),
        None => CommandApprovalBehavior::AlwaysAccept,
    };
    client.command_approval_count = 0;
    client.command_approval_item_ids.clear();
    client.command_execution_statuses.clear();
    client.last_turn_status = None;

    let mut turn_params = TurnStartParams {
        thread_id: thread_response.thread.id.clone(),
        input: vec![V2UserInput::Text {
            text: message,
            text_elements: Vec::new(),
        }],
        ..Default::default()
    };
    turn_params.approval_policy = Some(AskForApproval::OnRequest);
    turn_params.sandbox_policy = Some(SandboxPolicy::ReadOnly {
        access: ReadOnlyAccess::FullAccess,
    });

    let turn_response = client.turn_start(turn_params)?;
    println!("< turn/start response: {turn_response:?}");
    client.stream_turn(&thread_response.thread.id, &turn_response.turn.id)?;

    if client.command_approval_count < min_approvals {
        bail!(
            "expected at least {min_approvals} command approvals, got {}",
            client.command_approval_count
        );
    }
    let mut approvals_per_item = std::collections::BTreeMap::new();
    for item_id in &client.command_approval_item_ids {
        *approvals_per_item.entry(item_id.clone()).or_insert(0usize) += 1;
    }
    let max_approvals_for_one_item = approvals_per_item.values().copied().max().unwrap_or(0);
    if max_approvals_for_one_item < min_approvals {
        bail!(
            "expected at least {min_approvals} approvals for one command item, got max {max_approvals_for_one_item} with map {approvals_per_item:?}"
        );
    }

    let last_command_status = client.command_execution_statuses.last();
    if abort_on.is_none() {
        if last_command_status != Some(&CommandExecutionStatus::Completed) {
            bail!("expected completed command execution, got {last_command_status:?}");
        }
        if client.last_turn_status != Some(TurnStatus::Completed) {
            bail!(
                "expected completed turn in all-accept flow, got {:?}",
                client.last_turn_status
            );
        }
    } else if last_command_status == Some(&CommandExecutionStatus::Completed) {
        bail!(
            "expected non-completed command execution in mixed approval/decline flow, got {last_command_status:?}"
        );
    }

    println!(
        "[zsh-fork multi-approval summary] approvals={}, approvals_per_item={approvals_per_item:?}, command_statuses={:?}, turn_status={:?}",
        client.command_approval_count, client.command_execution_statuses, client.last_turn_status
    );

    Ok(())
}

fn resume_message_v2(
    endpoint: &Endpoint,
    config_overrides: &[String],
    thread_id: String,
    user_message: String,
    dynamic_tools: &Option<Vec<DynamicToolSpec>>,
) -> Result<()> {
    ensure_dynamic_tools_unused(dynamic_tools, "resume-message-v2")?;

    let mut client = CodexClient::connect(endpoint, config_overrides)?;

    let initialize = client.initialize()?;
    println!("< initialize response: {initialize:?}");

    let resume_response = client.thread_resume(ThreadResumeParams {
        thread_id,
        ..Default::default()
    })?;
    println!("< thread/resume response: {resume_response:?}");

    let turn_response = client.turn_start(TurnStartParams {
        thread_id: resume_response.thread.id.clone(),
        input: vec![V2UserInput::Text {
            text: user_message,
            text_elements: Vec::new(),
        }],
        ..Default::default()
    })?;
    println!("< turn/start response: {turn_response:?}");

    client.stream_turn(&resume_response.thread.id, &turn_response.turn.id)?;

    Ok(())
}

fn thread_resume_follow(
    endpoint: &Endpoint,
    config_overrides: &[String],
    thread_id: String,
) -> Result<()> {
    let mut client = CodexClient::connect(endpoint, config_overrides)?;

    let initialize = client.initialize()?;
    println!("< initialize response: {initialize:?}");

    let resume_response = client.thread_resume(ThreadResumeParams {
        thread_id,
        ..Default::default()
    })?;
    println!("< thread/resume response: {resume_response:?}");
    println!("< streaming notifications until process is terminated");

    client.stream_notifications_forever()
}

fn trigger_cmd_approval(
    endpoint: &Endpoint,
    config_overrides: &[String],
    user_message: Option<String>,
    dynamic_tools: &Option<Vec<DynamicToolSpec>>,
) -> Result<()> {
    let default_prompt =
        "Run `touch /tmp/should-trigger-approval` so I can confirm the file exists.";
    let message = user_message.unwrap_or_else(|| default_prompt.to_string());
    send_message_v2_with_policies(
        endpoint,
        config_overrides,
        message,
        Some(AskForApproval::OnRequest),
        Some(SandboxPolicy::ReadOnly {
            access: ReadOnlyAccess::FullAccess,
        }),
        dynamic_tools,
    )
}

fn trigger_patch_approval(
    endpoint: &Endpoint,
    config_overrides: &[String],
    user_message: Option<String>,
    dynamic_tools: &Option<Vec<DynamicToolSpec>>,
) -> Result<()> {
    let default_prompt =
        "Create a file named APPROVAL_DEMO.txt containing a short hello message using apply_patch.";
    let message = user_message.unwrap_or_else(|| default_prompt.to_string());
    send_message_v2_with_policies(
        endpoint,
        config_overrides,
        message,
        Some(AskForApproval::OnRequest),
        Some(SandboxPolicy::ReadOnly {
            access: ReadOnlyAccess::FullAccess,
        }),
        dynamic_tools,
    )
}

fn no_trigger_cmd_approval(
    endpoint: &Endpoint,
    config_overrides: &[String],
    dynamic_tools: &Option<Vec<DynamicToolSpec>>,
) -> Result<()> {
    let prompt = "Run `touch should_not_trigger_approval.txt`";
    send_message_v2_with_policies(
        endpoint,
        config_overrides,
        prompt.to_string(),
        None,
        None,
        dynamic_tools,
    )
}

fn send_message_v2_with_policies(
    endpoint: &Endpoint,
    config_overrides: &[String],
    user_message: String,
    approval_policy: Option<AskForApproval>,
    sandbox_policy: Option<SandboxPolicy>,
    dynamic_tools: &Option<Vec<DynamicToolSpec>>,
) -> Result<()> {
    let mut client = CodexClient::connect(endpoint, config_overrides)?;

    let initialize = client.initialize()?;
    println!("< initialize response: {initialize:?}");

    let thread_response = client.thread_start(ThreadStartParams {
        dynamic_tools: dynamic_tools.clone(),
        ..Default::default()
    })?;
    println!("< thread/start response: {thread_response:?}");
    let mut turn_params = TurnStartParams {
        thread_id: thread_response.thread.id.clone(),
        input: vec![V2UserInput::Text {
            text: user_message,
            // Test client sends plain text without UI element ranges.
            text_elements: Vec::new(),
        }],
        ..Default::default()
    };
    turn_params.approval_policy = approval_policy;
    turn_params.sandbox_policy = sandbox_policy;

    let turn_response = client.turn_start(turn_params)?;
    println!("< turn/start response: {turn_response:?}");

    client.stream_turn(&thread_response.thread.id, &turn_response.turn.id)?;

    Ok(())
}

fn send_follow_up_v2(
    endpoint: &Endpoint,
    config_overrides: &[String],
    first_message: String,
    follow_up_message: String,
    dynamic_tools: &Option<Vec<DynamicToolSpec>>,
) -> Result<()> {
    let mut client = CodexClient::connect(endpoint, config_overrides)?;

    let initialize = client.initialize()?;
    println!("< initialize response: {initialize:?}");

    let thread_response = client.thread_start(ThreadStartParams {
        dynamic_tools: dynamic_tools.clone(),
        ..Default::default()
    })?;
    println!("< thread/start response: {thread_response:?}");

    let first_turn_params = TurnStartParams {
        thread_id: thread_response.thread.id.clone(),
        input: vec![V2UserInput::Text {
            text: first_message,
            // Test client sends plain text without UI element ranges.
            text_elements: Vec::new(),
        }],
        ..Default::default()
    };
    let first_turn_response = client.turn_start(first_turn_params)?;
    println!("< turn/start response (initial): {first_turn_response:?}");
    client.stream_turn(&thread_response.thread.id, &first_turn_response.turn.id)?;

    let follow_up_params = TurnStartParams {
        thread_id: thread_response.thread.id.clone(),
        input: vec![V2UserInput::Text {
            text: follow_up_message,
            // Test client sends plain text without UI element ranges.
            text_elements: Vec::new(),
        }],
        ..Default::default()
    };
    let follow_up_response = client.turn_start(follow_up_params)?;
    println!("< turn/start response (follow-up): {follow_up_response:?}");
    client.stream_turn(&thread_response.thread.id, &follow_up_response.turn.id)?;

    Ok(())
}

fn test_login(endpoint: &Endpoint, config_overrides: &[String]) -> Result<()> {
    let mut client = CodexClient::connect(endpoint, config_overrides)?;

    let initialize = client.initialize()?;
    println!("< initialize response: {initialize:?}");

    let login_response = client.login_chat_gpt()?;
    println!("< loginChatGpt response: {login_response:?}");
    println!(
        "Open the following URL in your browser to continue:\n{}",
        login_response.auth_url
    );

    let completion = client.wait_for_login_completion(&login_response.login_id)?;
    println!("< loginChatGptComplete notification: {completion:?}");

    if completion.success {
        println!("Login succeeded.");
        Ok(())
    } else {
        bail!(
            "login failed: {}",
            completion
                .error
                .as_deref()
                .unwrap_or("unknown error from loginChatGptComplete")
        );
    }
}

fn get_account_rate_limits(endpoint: &Endpoint, config_overrides: &[String]) -> Result<()> {
    let mut client = CodexClient::connect(endpoint, config_overrides)?;

    let initialize = client.initialize()?;
    println!("< initialize response: {initialize:?}");

    let response = client.get_account_rate_limits()?;
    println!("< account/rateLimits/read response: {response:?}");

    Ok(())
}

fn model_list(endpoint: &Endpoint, config_overrides: &[String]) -> Result<()> {
    let mut client = CodexClient::connect(endpoint, config_overrides)?;

    let initialize = client.initialize()?;
    println!("< initialize response: {initialize:?}");

    let response = client.model_list(ModelListParams::default())?;
    println!("< model/list response: {response:?}");

    Ok(())
}

fn thread_list(endpoint: &Endpoint, config_overrides: &[String], limit: u32) -> Result<()> {
    let mut client = CodexClient::connect(endpoint, config_overrides)?;

    let initialize = client.initialize()?;
    println!("< initialize response: {initialize:?}");

    let response = client.thread_list(ThreadListParams {
        cursor: None,
        limit: Some(limit),
        sort_key: None,
        model_providers: None,
        source_kinds: None,
        archived: None,
        cwd: None,
    })?;
    println!("< thread/list response: {response:?}");

    Ok(())
}

fn ensure_dynamic_tools_unused(
    dynamic_tools: &Option<Vec<DynamicToolSpec>>,
    command: &str,
) -> Result<()> {
    if dynamic_tools.is_some() {
        bail!(
            "dynamic tools are only supported for v2 thread/start; remove --dynamic-tools for {command} or use send-message-v2"
        );
    }
    Ok(())
}

fn parse_dynamic_tools_arg(dynamic_tools: &Option<String>) -> Result<Option<Vec<DynamicToolSpec>>> {
    let Some(raw_arg) = dynamic_tools.as_deref() else {
        return Ok(None);
    };

    let raw_json = if let Some(path) = raw_arg.strip_prefix('@') {
        fs::read_to_string(Path::new(path))
            .with_context(|| format!("read dynamic tools file {path}"))?
    } else {
        raw_arg.to_string()
    };

    let value: Value = serde_json::from_str(&raw_json).context("parse dynamic tools JSON")?;
    let tools = match value {
        Value::Array(_) => serde_json::from_value(value).context("decode dynamic tools array")?,
        Value::Object(_) => vec![serde_json::from_value(value).context("decode dynamic tool")?],
        _ => bail!("dynamic tools JSON must be an object or array"),
    };

    Ok(Some(tools))
}

enum ClientTransport {
    Stdio {
        child: Child,
        stdin: Option<ChildStdin>,
        stdout: BufReader<ChildStdout>,
    },
    WebSocket {
        url: String,
        socket: Box<WebSocket<MaybeTlsStream<TcpStream>>>,
    },
}

struct CodexClient {
    transport: ClientTransport,
    pending_notifications: VecDeque<JSONRPCNotification>,
    command_approval_behavior: CommandApprovalBehavior,
    command_approval_count: usize,
    command_approval_item_ids: Vec<String>,
    command_execution_statuses: Vec<CommandExecutionStatus>,
    last_turn_status: Option<TurnStatus>,
}

#[derive(Debug, Clone, Copy)]
enum CommandApprovalBehavior {
    AlwaysAccept,
    AbortOn(usize),
}

impl CodexClient {
    fn connect(endpoint: &Endpoint, config_overrides: &[String]) -> Result<Self> {
        match endpoint {
            Endpoint::SpawnCodex(codex_bin) => Self::spawn_stdio(codex_bin, config_overrides),
            Endpoint::ConnectWs(url) => Self::connect_websocket(url),
        }
    }

    fn spawn_stdio(codex_bin: &Path, config_overrides: &[String]) -> Result<Self> {
        let codex_bin_display = codex_bin.display();
        let mut cmd = Command::new(codex_bin);
        if let Some(codex_bin_parent) = codex_bin.parent() {
            let mut path = OsString::from(codex_bin_parent.as_os_str());
            if let Some(existing_path) = std::env::var_os("PATH") {
                path.push(":");
                path.push(existing_path);
            }
            cmd.env("PATH", path);
        }
        for override_kv in config_overrides {
            cmd.arg("--config").arg(override_kv);
        }
        let mut codex_app_server = cmd
            .arg("app-server")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .with_context(|| format!("failed to start `{codex_bin_display}` app-server"))?;

        let stdin = codex_app_server
            .stdin
            .take()
            .context("codex app-server stdin unavailable")?;
        let stdout = codex_app_server
            .stdout
            .take()
            .context("codex app-server stdout unavailable")?;

        Ok(Self {
            transport: ClientTransport::Stdio {
                child: codex_app_server,
                stdin: Some(stdin),
                stdout: BufReader::new(stdout),
            },
            pending_notifications: VecDeque::new(),
            command_approval_behavior: CommandApprovalBehavior::AlwaysAccept,
            command_approval_count: 0,
            command_approval_item_ids: Vec::new(),
            command_execution_statuses: Vec::new(),
            last_turn_status: None,
        })
    }

    fn connect_websocket(url: &str) -> Result<Self> {
        let parsed = Url::parse(url).with_context(|| format!("invalid websocket URL `{url}`"))?;
        let (socket, _response) = connect(parsed.as_str()).with_context(|| {
            format!(
                "failed to connect to websocket app-server at `{url}`; if no server is running, start one with `codex-app-server-test-client serve --listen {url}`"
            )
        })?;
        Ok(Self {
            transport: ClientTransport::WebSocket {
                url: url.to_string(),
                socket: Box::new(socket),
            },
            pending_notifications: VecDeque::new(),
            command_approval_behavior: CommandApprovalBehavior::AlwaysAccept,
            command_approval_count: 0,
            command_approval_item_ids: Vec::new(),
            command_execution_statuses: Vec::new(),
            last_turn_status: None,
        })
    }

    fn initialize(&mut self) -> Result<InitializeResponse> {
        let request_id = self.request_id();
        let request = ClientRequest::Initialize {
            request_id: request_id.clone(),
            params: InitializeParams {
                client_info: ClientInfo {
                    name: "codex-toy-app-server".to_string(),
                    title: Some("Codex Toy App Server".to_string()),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                },
                capabilities: Some(InitializeCapabilities {
                    experimental_api: true,
                    opt_out_notification_methods: Some(
                        NOTIFICATIONS_TO_OPT_OUT
                            .iter()
                            .map(|method| (*method).to_string())
                            .collect(),
                    ),
                }),
            },
        };

        let response: InitializeResponse = self.send_request(request, request_id, "initialize")?;

        // Complete the initialize handshake.
        let initialized = JSONRPCMessage::Notification(JSONRPCNotification {
            method: "initialized".to_string(),
            params: None,
        });
        self.write_jsonrpc_message(initialized)?;

        Ok(response)
    }

    fn start_thread(&mut self) -> Result<NewConversationResponse> {
        let request_id = self.request_id();
        let request = ClientRequest::NewConversation {
            request_id: request_id.clone(),
            params: NewConversationParams::default(),
        };

        self.send_request(request, request_id, "newConversation")
    }

    fn add_conversation_listener(
        &mut self,
        conversation_id: &ThreadId,
    ) -> Result<AddConversationSubscriptionResponse> {
        let request_id = self.request_id();
        let request = ClientRequest::AddConversationListener {
            request_id: request_id.clone(),
            params: AddConversationListenerParams {
                conversation_id: *conversation_id,
                experimental_raw_events: false,
            },
        };

        self.send_request(request, request_id, "addConversationListener")
    }

    fn remove_thread_listener(&mut self, subscription_id: Uuid) -> Result<()> {
        let request_id = self.request_id();
        let request = ClientRequest::RemoveConversationListener {
            request_id: request_id.clone(),
            params: codex_app_server_protocol::RemoveConversationListenerParams { subscription_id },
        };

        self.send_request::<codex_app_server_protocol::RemoveConversationSubscriptionResponse>(
            request,
            request_id,
            "removeConversationListener",
        )?;

        Ok(())
    }

    fn send_user_message(
        &mut self,
        conversation_id: &ThreadId,
        message: &str,
    ) -> Result<SendUserMessageResponse> {
        let request_id = self.request_id();
        let request = ClientRequest::SendUserMessage {
            request_id: request_id.clone(),
            params: SendUserMessageParams {
                conversation_id: *conversation_id,
                items: vec![InputItem::Text {
                    text: message.to_string(),
                    // Test client sends plain text without UI element ranges.
                    text_elements: Vec::new(),
                }],
            },
        };

        self.send_request(request, request_id, "sendUserMessage")
    }

    fn thread_start(&mut self, params: ThreadStartParams) -> Result<ThreadStartResponse> {
        let request_id = self.request_id();
        let request = ClientRequest::ThreadStart {
            request_id: request_id.clone(),
            params,
        };

        self.send_request(request, request_id, "thread/start")
    }

    fn thread_resume(&mut self, params: ThreadResumeParams) -> Result<ThreadResumeResponse> {
        let request_id = self.request_id();
        let request = ClientRequest::ThreadResume {
            request_id: request_id.clone(),
            params,
        };

        self.send_request(request, request_id, "thread/resume")
    }

    fn turn_start(&mut self, params: TurnStartParams) -> Result<TurnStartResponse> {
        let request_id = self.request_id();
        let request = ClientRequest::TurnStart {
            request_id: request_id.clone(),
            params,
        };

        self.send_request(request, request_id, "turn/start")
    }

    fn login_chat_gpt(&mut self) -> Result<LoginChatGptResponse> {
        let request_id = self.request_id();
        let request = ClientRequest::LoginChatGpt {
            request_id: request_id.clone(),
            params: None,
        };

        self.send_request(request, request_id, "loginChatGpt")
    }

    fn get_account_rate_limits(&mut self) -> Result<GetAccountRateLimitsResponse> {
        let request_id = self.request_id();
        let request = ClientRequest::GetAccountRateLimits {
            request_id: request_id.clone(),
            params: None,
        };

        self.send_request(request, request_id, "account/rateLimits/read")
    }

    fn model_list(&mut self, params: ModelListParams) -> Result<ModelListResponse> {
        let request_id = self.request_id();
        let request = ClientRequest::ModelList {
            request_id: request_id.clone(),
            params,
        };

        self.send_request(request, request_id, "model/list")
    }

    fn thread_list(&mut self, params: ThreadListParams) -> Result<ThreadListResponse> {
        let request_id = self.request_id();
        let request = ClientRequest::ThreadList {
            request_id: request_id.clone(),
            params,
        };

        self.send_request(request, request_id, "thread/list")
    }

    fn stream_conversation(&mut self, conversation_id: &ThreadId) -> Result<()> {
        loop {
            let notification = self.next_notification()?;

            if !notification.method.starts_with("codex/event/") {
                continue;
            }

            if let Some(event) = self.extract_event(notification, conversation_id)? {
                match &event.msg {
                    EventMsg::AgentMessage(event) => {
                        println!("{}", event.message);
                    }
                    EventMsg::AgentMessageDelta(event) => {
                        print!("{}", event.delta);
                        std::io::stdout().flush().ok();
                    }
                    EventMsg::TurnComplete(event) => {
                        println!("\n[task complete: {event:?}]");
                        break;
                    }
                    EventMsg::TurnAborted(event) => {
                        println!("\n[turn aborted: {:?}]", event.reason);
                        break;
                    }
                    EventMsg::Error(event) => {
                        println!("[error] {event:?}");
                    }
                    _ => {
                        println!("[UNKNOWN EVENT] {:?}", event.msg);
                    }
                }
            }
        }

        Ok(())
    }

    fn wait_for_login_completion(
        &mut self,
        expected_login_id: &Uuid,
    ) -> Result<LoginChatGptCompleteNotification> {
        loop {
            let notification = self.next_notification()?;

            if let Ok(server_notification) = ServerNotification::try_from(notification) {
                match server_notification {
                    ServerNotification::LoginChatGptComplete(completion) => {
                        if &completion.login_id == expected_login_id {
                            return Ok(completion);
                        }

                        println!(
                            "[ignoring loginChatGptComplete for unexpected login_id: {}]",
                            completion.login_id
                        );
                    }
                    ServerNotification::AuthStatusChange(status) => {
                        println!("< authStatusChange notification: {status:?}");
                    }
                    ServerNotification::AccountRateLimitsUpdated(snapshot) => {
                        println!("< accountRateLimitsUpdated notification: {snapshot:?}");
                    }
                    ServerNotification::SessionConfigured(_) => {
                        // SessionConfigured notifications are unrelated to login; skip.
                    }
                    _ => {}
                }
            }

            // Not a server notification (likely a conversation event); keep waiting.
        }
    }

    fn stream_turn(&mut self, thread_id: &str, turn_id: &str) -> Result<()> {
        loop {
            let notification = self.next_notification()?;

            let Ok(server_notification) = ServerNotification::try_from(notification) else {
                continue;
            };

            match server_notification {
                ServerNotification::ThreadStarted(payload) => {
                    if payload.thread.id == thread_id {
                        println!("< thread/started notification: {:?}", payload.thread);
                    }
                }
                ServerNotification::TurnStarted(payload) => {
                    if payload.turn.id == turn_id {
                        println!("< turn/started notification: {:?}", payload.turn.status);
                    }
                }
                ServerNotification::AgentMessageDelta(delta) => {
                    print!("{}", delta.delta);
                    std::io::stdout().flush().ok();
                }
                ServerNotification::CommandExecutionOutputDelta(delta) => {
                    print!("{}", delta.delta);
                    std::io::stdout().flush().ok();
                }
                ServerNotification::TerminalInteraction(delta) => {
                    println!("[stdin sent: {}]", delta.stdin);
                    std::io::stdout().flush().ok();
                }
                ServerNotification::ItemStarted(payload) => {
                    println!("\n< item started: {:?}", payload.item);
                }
                ServerNotification::ItemCompleted(payload) => {
                    if let ThreadItem::CommandExecution { status, .. } = payload.item.clone() {
                        self.command_execution_statuses.push(status);
                    }
                    println!("< item completed: {:?}", payload.item);
                }
                ServerNotification::TurnCompleted(payload) => {
                    if payload.turn.id == turn_id {
                        self.last_turn_status = Some(payload.turn.status.clone());
                        println!("\n< turn/completed notification: {:?}", payload.turn.status);
                        if payload.turn.status == TurnStatus::Failed
                            && let Some(error) = payload.turn.error
                        {
                            println!("[turn error] {}", error.message);
                        }
                        break;
                    }
                }
                ServerNotification::McpToolCallProgress(payload) => {
                    println!("< MCP tool progress: {}", payload.message);
                }
                _ => {
                    println!("[UNKNOWN SERVER NOTIFICATION] {server_notification:?}");
                }
            }
        }

        Ok(())
    }

    fn stream_notifications_forever(&mut self) -> Result<()> {
        loop {
            let _ = self.next_notification()?;
        }
    }

    fn extract_event(
        &self,
        notification: JSONRPCNotification,
        conversation_id: &ThreadId,
    ) -> Result<Option<Event>> {
        let params = notification
            .params
            .context("event notification missing params")?;

        let mut map = match params {
            Value::Object(map) => map,
            other => bail!("unexpected params shape: {other:?}"),
        };

        let conversation_value = map
            .remove("conversationId")
            .context("event missing conversationId")?;
        let notification_conversation: ThreadId = serde_json::from_value(conversation_value)
            .context("conversationId was not a valid UUID")?;

        if &notification_conversation != conversation_id {
            return Ok(None);
        }

        let event_value = Value::Object(map);
        let event: Event =
            serde_json::from_value(event_value).context("failed to decode event payload")?;
        Ok(Some(event))
    }

    fn send_request<T>(
        &mut self,
        request: ClientRequest,
        request_id: RequestId,
        method: &str,
    ) -> Result<T>
    where
        T: DeserializeOwned,
    {
        self.write_request(&request)?;
        self.wait_for_response(request_id, method)
    }

    fn write_request(&mut self, request: &ClientRequest) -> Result<()> {
        let request_json = serde_json::to_string(request)?;
        let request_pretty = serde_json::to_string_pretty(request)?;
        print_multiline_with_prefix("> ", &request_pretty);
        self.write_payload(&request_json)
    }

    fn wait_for_response<T>(&mut self, request_id: RequestId, method: &str) -> Result<T>
    where
        T: DeserializeOwned,
    {
        loop {
            let message = self.read_jsonrpc_message()?;

            match message {
                JSONRPCMessage::Response(JSONRPCResponse { id, result }) => {
                    if id == request_id {
                        return serde_json::from_value(result)
                            .with_context(|| format!("{method} response missing payload"));
                    }
                }
                JSONRPCMessage::Error(err) => {
                    if err.id == request_id {
                        bail!("{method} failed: {err:?}");
                    }
                }
                JSONRPCMessage::Notification(notification) => {
                    self.pending_notifications.push_back(notification);
                }
                JSONRPCMessage::Request(request) => {
                    self.handle_server_request(request)?;
                }
            }
        }
    }

    fn next_notification(&mut self) -> Result<JSONRPCNotification> {
        if let Some(notification) = self.pending_notifications.pop_front() {
            return Ok(notification);
        }

        loop {
            let message = self.read_jsonrpc_message()?;

            match message {
                JSONRPCMessage::Notification(notification) => return Ok(notification),
                JSONRPCMessage::Response(_) | JSONRPCMessage::Error(_) => {
                    // No outstanding requests, so ignore stray responses/errors for now.
                    continue;
                }
                JSONRPCMessage::Request(request) => {
                    self.handle_server_request(request)?;
                }
            }
        }
    }

    fn read_jsonrpc_message(&mut self) -> Result<JSONRPCMessage> {
        loop {
            let raw = self.read_payload()?;
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                continue;
            }

            let parsed: Value =
                serde_json::from_str(trimmed).context("response was not valid JSON-RPC")?;
            let pretty = serde_json::to_string_pretty(&parsed)?;
            print_multiline_with_prefix("< ", &pretty);
            let message: JSONRPCMessage = serde_json::from_value(parsed)
                .context("response was not a valid JSON-RPC message")?;
            return Ok(message);
        }
    }

    fn request_id(&self) -> RequestId {
        RequestId::String(Uuid::new_v4().to_string())
    }

    fn handle_server_request(&mut self, request: JSONRPCRequest) -> Result<()> {
        let server_request = ServerRequest::try_from(request)
            .context("failed to deserialize ServerRequest from JSONRPCRequest")?;

        match server_request {
            ServerRequest::CommandExecutionRequestApproval { request_id, params } => {
                self.handle_command_execution_request_approval(request_id, params)?;
            }
            ServerRequest::FileChangeRequestApproval { request_id, params } => {
                self.approve_file_change_request(request_id, params)?;
            }
            other => {
                bail!("received unsupported server request: {other:?}");
            }
        }

        Ok(())
    }

    fn handle_command_execution_request_approval(
        &mut self,
        request_id: RequestId,
        params: CommandExecutionRequestApprovalParams,
    ) -> Result<()> {
        let CommandExecutionRequestApprovalParams {
            thread_id,
            turn_id,
            item_id,
            approval_id,
            reason,
            command,
            cwd,
            command_actions,
            proposed_execpolicy_amendment,
        } = params;

        println!(
            "\n< commandExecution approval requested for thread {thread_id}, turn {turn_id}, item {item_id}, approval {}",
            approval_id.as_deref().unwrap_or("<none>")
        );
        self.command_approval_count += 1;
        self.command_approval_item_ids.push(item_id.clone());
        if let Some(reason) = reason.as_deref() {
            println!("< reason: {reason}");
        }
        if let Some(command) = command.as_deref() {
            println!("< command: {command}");
        }
        if let Some(cwd) = cwd.as_ref() {
            println!("< cwd: {}", cwd.display());
        }
        if let Some(command_actions) = command_actions.as_ref()
            && !command_actions.is_empty()
        {
            println!("< command actions: {command_actions:?}");
        }
        if let Some(execpolicy_amendment) = proposed_execpolicy_amendment.as_ref() {
            println!("< proposed execpolicy amendment: {execpolicy_amendment:?}");
        }

        let decision = match self.command_approval_behavior {
            CommandApprovalBehavior::AlwaysAccept => CommandExecutionApprovalDecision::Accept,
            CommandApprovalBehavior::AbortOn(index) if self.command_approval_count == index => {
                CommandExecutionApprovalDecision::Cancel
            }
            CommandApprovalBehavior::AbortOn(_) => CommandExecutionApprovalDecision::Accept,
        };
        let response = CommandExecutionRequestApprovalResponse {
            decision: decision.clone(),
        };
        self.send_server_request_response(request_id, &response)?;
        println!(
            "< commandExecution decision for approval #{} on item {item_id}: {:?}",
            self.command_approval_count, decision
        );
        Ok(())
    }

    fn approve_file_change_request(
        &mut self,
        request_id: RequestId,
        params: FileChangeRequestApprovalParams,
    ) -> Result<()> {
        let FileChangeRequestApprovalParams {
            thread_id,
            turn_id,
            item_id,
            reason,
            grant_root,
        } = params;

        println!(
            "\n< fileChange approval requested for thread {thread_id}, turn {turn_id}, item {item_id}"
        );
        if let Some(reason) = reason.as_deref() {
            println!("< reason: {reason}");
        }
        if let Some(grant_root) = grant_root.as_deref() {
            println!("< grant root: {}", grant_root.display());
        }

        let response = FileChangeRequestApprovalResponse {
            decision: FileChangeApprovalDecision::Accept,
        };
        self.send_server_request_response(request_id, &response)?;
        println!("< approved fileChange request for item {item_id}");
        Ok(())
    }

    fn send_server_request_response<T>(&mut self, request_id: RequestId, response: &T) -> Result<()>
    where
        T: Serialize,
    {
        let message = JSONRPCMessage::Response(JSONRPCResponse {
            id: request_id,
            result: serde_json::to_value(response)?,
        });
        self.write_jsonrpc_message(message)
    }

    fn write_jsonrpc_message(&mut self, message: JSONRPCMessage) -> Result<()> {
        let payload = serde_json::to_string(&message)?;
        let pretty = serde_json::to_string_pretty(&message)?;
        print_multiline_with_prefix("> ", &pretty);
        self.write_payload(&payload)
    }

    fn write_payload(&mut self, payload: &str) -> Result<()> {
        match &mut self.transport {
            ClientTransport::Stdio { stdin, .. } => {
                if let Some(stdin) = stdin.as_mut() {
                    writeln!(stdin, "{payload}")?;
                    stdin
                        .flush()
                        .context("failed to flush payload to codex app-server")?;
                    return Ok(());
                }
                bail!("codex app-server stdin closed")
            }
            ClientTransport::WebSocket { socket, url } => {
                socket
                    .send(Message::Text(payload.to_string().into()))
                    .with_context(|| format!("failed to write websocket message to `{url}`"))?;
                Ok(())
            }
        }
    }

    fn read_payload(&mut self) -> Result<String> {
        match &mut self.transport {
            ClientTransport::Stdio { stdout, .. } => {
                let mut response_line = String::new();
                let bytes = stdout
                    .read_line(&mut response_line)
                    .context("failed to read from codex app-server")?;
                if bytes == 0 {
                    bail!("codex app-server closed stdout");
                }
                Ok(response_line)
            }
            ClientTransport::WebSocket { socket, url } => loop {
                let frame = socket
                    .read()
                    .with_context(|| format!("failed to read websocket message from `{url}`"))?;
                match frame {
                    Message::Text(text) => return Ok(text.to_string()),
                    Message::Binary(_) | Message::Ping(_) | Message::Pong(_) => continue,
                    Message::Close(_) => {
                        bail!("websocket app-server at `{url}` closed the connection")
                    }
                    Message::Frame(_) => continue,
                }
            },
        }
    }
}

fn print_multiline_with_prefix(prefix: &str, payload: &str) {
    for line in payload.lines() {
        println!("{prefix}{line}");
    }
}

impl Drop for CodexClient {
    fn drop(&mut self) {
        let ClientTransport::Stdio { child, stdin, .. } = &mut self.transport else {
            return;
        };

        let _ = stdin.take();

        if let Ok(Some(status)) = child.try_wait() {
            println!("[codex app-server exited: {status}]");
            return;
        }

        thread::sleep(Duration::from_millis(100));

        if let Ok(Some(status)) = child.try_wait() {
            println!("[codex app-server exited: {status}]");
            return;
        }

        let _ = child.kill();
        let _ = child.wait();
    }
}
