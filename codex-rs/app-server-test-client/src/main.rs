use std::collections::VecDeque;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
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
use codex_app_server_protocol::FileChangeApprovalDecision;
use codex_app_server_protocol::FileChangeRequestApprovalParams;
use codex_app_server_protocol::FileChangeRequestApprovalResponse;
use codex_app_server_protocol::GetAccountRateLimitsResponse;
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
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SandboxPolicy;
use codex_app_server_protocol::SendUserMessageParams;
use codex_app_server_protocol::SendUserMessageResponse;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
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
use uuid::Uuid;

/// Minimal launcher that initializes the Codex app-server and logs the handshake.
#[derive(Parser)]
#[command(author = "Codex", version, about = "Bootstrap Codex app-server", long_about = None)]
struct Cli {
    /// Path to the `codex` CLI binary.
    #[arg(long, env = "CODEX_BIN", default_value = "codex")]
    codex_bin: String,

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

    #[command(subcommand)]
    command: CliCommand,
}

#[derive(Subcommand)]
enum CliCommand {
    /// Send a user message through the Codex app-server.
    SendMessage {
        /// User message to send to Codex.
        #[arg()]
        user_message: String,
    },
    /// Send a user message through the app-server V2 thread/turn APIs.
    SendMessageV2 {
        /// User message to send to Codex.
        #[arg()]
        user_message: String,
    },
    /// Start a V2 turn that elicits an ExecCommand approval.
    #[command(name = "trigger-cmd-approval")]
    TriggerCmdApproval {
        /// Optional prompt; defaults to a simple python command.
        #[arg()]
        user_message: Option<String>,
    },
    /// Start a V2 turn that elicits an ApplyPatch approval.
    #[command(name = "trigger-patch-approval")]
    TriggerPatchApproval {
        /// Optional prompt; defaults to creating a file via apply_patch.
        #[arg()]
        user_message: Option<String>,
    },
    /// Start a V2 turn that should not elicit an ExecCommand approval.
    #[command(name = "no-trigger-cmd-approval")]
    NoTriggerCmdApproval,
    /// Send two sequential V2 turns in the same thread to test follow-up behavior.
    SendFollowUpV2 {
        /// Initial user message for the first turn.
        #[arg()]
        first_message: String,
        /// Follow-up user message for the second turn.
        #[arg()]
        follow_up_message: String,
    },
    /// Trigger the ChatGPT login flow and wait for completion.
    TestLogin,
    /// Fetch the current account rate limits from the Codex app-server.
    GetAccountRateLimits,
    /// List the available models from the Codex app-server.
    #[command(name = "model-list")]
    ModelList,
}

fn main() -> Result<()> {
    let Cli {
        codex_bin,
        config_overrides,
        command,
    } = Cli::parse();

    match command {
        CliCommand::SendMessage { user_message } => {
            send_message(&codex_bin, &config_overrides, user_message)
        }
        CliCommand::SendMessageV2 { user_message } => {
            send_message_v2(&codex_bin, &config_overrides, user_message)
        }
        CliCommand::TriggerCmdApproval { user_message } => {
            trigger_cmd_approval(&codex_bin, &config_overrides, user_message)
        }
        CliCommand::TriggerPatchApproval { user_message } => {
            trigger_patch_approval(&codex_bin, &config_overrides, user_message)
        }
        CliCommand::NoTriggerCmdApproval => no_trigger_cmd_approval(&codex_bin, &config_overrides),
        CliCommand::SendFollowUpV2 {
            first_message,
            follow_up_message,
        } => send_follow_up_v2(
            &codex_bin,
            &config_overrides,
            first_message,
            follow_up_message,
        ),
        CliCommand::TestLogin => test_login(&codex_bin, &config_overrides),
        CliCommand::GetAccountRateLimits => get_account_rate_limits(&codex_bin, &config_overrides),
        CliCommand::ModelList => model_list(&codex_bin, &config_overrides),
    }
}

fn send_message(codex_bin: &str, config_overrides: &[String], user_message: String) -> Result<()> {
    let mut client = CodexClient::spawn(codex_bin, config_overrides)?;

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

fn send_message_v2(
    codex_bin: &str,
    config_overrides: &[String],
    user_message: String,
) -> Result<()> {
    send_message_v2_with_policies(codex_bin, config_overrides, user_message, None, None)
}

fn trigger_cmd_approval(
    codex_bin: &str,
    config_overrides: &[String],
    user_message: Option<String>,
) -> Result<()> {
    let default_prompt =
        "Run `touch /tmp/should-trigger-approval` so I can confirm the file exists.";
    let message = user_message.unwrap_or_else(|| default_prompt.to_string());
    send_message_v2_with_policies(
        codex_bin,
        config_overrides,
        message,
        Some(AskForApproval::OnRequest),
        Some(SandboxPolicy::ReadOnly),
    )
}

fn trigger_patch_approval(
    codex_bin: &str,
    config_overrides: &[String],
    user_message: Option<String>,
) -> Result<()> {
    let default_prompt =
        "Create a file named APPROVAL_DEMO.txt containing a short hello message using apply_patch.";
    let message = user_message.unwrap_or_else(|| default_prompt.to_string());
    send_message_v2_with_policies(
        codex_bin,
        config_overrides,
        message,
        Some(AskForApproval::OnRequest),
        Some(SandboxPolicy::ReadOnly),
    )
}

fn no_trigger_cmd_approval(codex_bin: &str, config_overrides: &[String]) -> Result<()> {
    let prompt = "Run `touch should_not_trigger_approval.txt`";
    send_message_v2_with_policies(codex_bin, config_overrides, prompt.to_string(), None, None)
}

fn send_message_v2_with_policies(
    codex_bin: &str,
    config_overrides: &[String],
    user_message: String,
    approval_policy: Option<AskForApproval>,
    sandbox_policy: Option<SandboxPolicy>,
) -> Result<()> {
    let mut client = CodexClient::spawn(codex_bin, config_overrides)?;

    let initialize = client.initialize()?;
    println!("< initialize response: {initialize:?}");

    let thread_response = client.thread_start(ThreadStartParams::default())?;
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
    codex_bin: &str,
    config_overrides: &[String],
    first_message: String,
    follow_up_message: String,
) -> Result<()> {
    let mut client = CodexClient::spawn(codex_bin, config_overrides)?;

    let initialize = client.initialize()?;
    println!("< initialize response: {initialize:?}");

    let thread_response = client.thread_start(ThreadStartParams::default())?;
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

fn test_login(codex_bin: &str, config_overrides: &[String]) -> Result<()> {
    let mut client = CodexClient::spawn(codex_bin, config_overrides)?;

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

fn get_account_rate_limits(codex_bin: &str, config_overrides: &[String]) -> Result<()> {
    let mut client = CodexClient::spawn(codex_bin, config_overrides)?;

    let initialize = client.initialize()?;
    println!("< initialize response: {initialize:?}");

    let response = client.get_account_rate_limits()?;
    println!("< account/rateLimits/read response: {response:?}");

    Ok(())
}

fn model_list(codex_bin: &str, config_overrides: &[String]) -> Result<()> {
    let mut client = CodexClient::spawn(codex_bin, config_overrides)?;

    let initialize = client.initialize()?;
    println!("< initialize response: {initialize:?}");

    let response = client.model_list(ModelListParams::default())?;
    println!("< model/list response: {response:?}");

    Ok(())
}

struct CodexClient {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    pending_notifications: VecDeque<JSONRPCNotification>,
}

impl CodexClient {
    fn spawn(codex_bin: &str, config_overrides: &[String]) -> Result<Self> {
        let mut cmd = Command::new(codex_bin);
        for override_kv in config_overrides {
            cmd.arg("--config").arg(override_kv);
        }
        let mut codex_app_server = cmd
            .arg("app-server")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .with_context(|| format!("failed to start `{codex_bin}` app-server"))?;

        let stdin = codex_app_server
            .stdin
            .take()
            .context("codex app-server stdin unavailable")?;
        let stdout = codex_app_server
            .stdout
            .take()
            .context("codex app-server stdout unavailable")?;

        Ok(Self {
            child: codex_app_server,
            stdin: Some(stdin),
            stdout: BufReader::new(stdout),
            pending_notifications: VecDeque::new(),
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
            },
        };

        self.send_request(request, request_id, "initialize")
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
                    println!("< item completed: {:?}", payload.item);
                }
                ServerNotification::TurnCompleted(payload) => {
                    if payload.turn.id == turn_id {
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

        if let Some(stdin) = self.stdin.as_mut() {
            writeln!(stdin, "{request_json}")?;
            stdin
                .flush()
                .context("failed to flush request to codex app-server")?;
        } else {
            bail!("codex app-server stdin closed");
        }

        Ok(())
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
            let mut response_line = String::new();
            let bytes = self
                .stdout
                .read_line(&mut response_line)
                .context("failed to read from codex app-server")?;

            if bytes == 0 {
                bail!("codex app-server closed stdout");
            }

            let trimmed = response_line.trim();
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
            reason,
            command,
            cwd,
            command_actions,
            proposed_execpolicy_amendment,
        } = params;

        println!(
            "\n< commandExecution approval requested for thread {thread_id}, turn {turn_id}, item {item_id}"
        );
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

        let response = CommandExecutionRequestApprovalResponse {
            decision: CommandExecutionApprovalDecision::Accept,
        };
        self.send_server_request_response(request_id, &response)?;
        println!("< approved commandExecution request for item {item_id}");
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

        if let Some(stdin) = self.stdin.as_mut() {
            writeln!(stdin, "{payload}")?;
            stdin
                .flush()
                .context("failed to flush response to codex app-server")?;
            return Ok(());
        }

        bail!("codex app-server stdin closed")
    }
}

fn print_multiline_with_prefix(prefix: &str, payload: &str) {
    for line in payload.lines() {
        println!("{prefix}{line}");
    }
}

impl Drop for CodexClient {
    fn drop(&mut self) {
        let _ = self.stdin.take();

        if let Ok(Some(status)) = self.child.try_wait() {
            println!("[codex app-server exited: {status}]");
            return;
        }

        thread::sleep(Duration::from_millis(100));

        if let Ok(Some(status)) = self.child.try_wait() {
            println!("[codex app-server exited: {status}]");
            return;
        }

        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
