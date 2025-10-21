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
use clap::Parser;
use clap::Subcommand;
use codex_app_server_protocol::AddConversationListenerParams;
use codex_app_server_protocol::AddConversationSubscriptionResponse;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::GetAccountRateLimitsResponse;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::InitializeResponse;
use codex_app_server_protocol::InputItem;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::LoginChatGptCompleteNotification;
use codex_app_server_protocol::LoginChatGptResponse;
use codex_app_server_protocol::NewConversationParams;
use codex_app_server_protocol::NewConversationResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SendUserMessageParams;
use codex_app_server_protocol::SendUserMessageResponse;
use codex_app_server_protocol::ServerNotification;
use codex_protocol::ConversationId;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
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
    /// Trigger the ChatGPT login flow and wait for completion.
    TestLogin,
    /// Fetch the current account rate limits from the Codex app-server.
    GetAccountRateLimits,
}

fn main() -> Result<()> {
    let Cli { codex_bin, command } = Cli::parse();

    match command {
        CliCommand::SendMessage { user_message } => send_message(codex_bin, user_message),
        CliCommand::TestLogin => test_login(codex_bin),
        CliCommand::GetAccountRateLimits => get_account_rate_limits(codex_bin),
    }
}

fn send_message(codex_bin: String, user_message: String) -> Result<()> {
    let mut client = CodexClient::spawn(codex_bin)?;

    let initialize = client.initialize()?;
    println!("< initialize response: {initialize:?}");

    let conversation = client.new_conversation()?;
    println!("< newConversation response: {conversation:?}");

    let subscription = client.add_conversation_listener(&conversation.conversation_id)?;
    println!("< addConversationListener response: {subscription:?}");

    let send_response = client.send_user_message(&conversation.conversation_id, &user_message)?;
    println!("< sendUserMessage response: {send_response:?}");

    client.stream_conversation(&conversation.conversation_id)?;

    client.remove_conversation_listener(subscription.subscription_id)?;

    Ok(())
}

fn test_login(codex_bin: String) -> Result<()> {
    let mut client = CodexClient::spawn(codex_bin)?;

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

fn get_account_rate_limits(codex_bin: String) -> Result<()> {
    let mut client = CodexClient::spawn(codex_bin)?;

    let initialize = client.initialize()?;
    println!("< initialize response: {initialize:?}");

    let response = client.get_account_rate_limits()?;
    println!("< account/rateLimits/read response: {response:?}");

    Ok(())
}

struct CodexClient {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    pending_notifications: VecDeque<JSONRPCNotification>,
}

impl CodexClient {
    fn spawn(codex_bin: String) -> Result<Self> {
        let mut codex_app_server = Command::new(&codex_bin)
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

    fn new_conversation(&mut self) -> Result<NewConversationResponse> {
        let request_id = self.request_id();
        let request = ClientRequest::NewConversation {
            request_id: request_id.clone(),
            params: NewConversationParams::default(),
        };

        self.send_request(request, request_id, "newConversation")
    }

    fn add_conversation_listener(
        &mut self,
        conversation_id: &ConversationId,
    ) -> Result<AddConversationSubscriptionResponse> {
        let request_id = self.request_id();
        let request = ClientRequest::AddConversationListener {
            request_id: request_id.clone(),
            params: AddConversationListenerParams {
                conversation_id: *conversation_id,
            },
        };

        self.send_request(request, request_id, "addConversationListener")
    }

    fn remove_conversation_listener(&mut self, subscription_id: Uuid) -> Result<()> {
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
        conversation_id: &ConversationId,
        message: &str,
    ) -> Result<SendUserMessageResponse> {
        let request_id = self.request_id();
        let request = ClientRequest::SendUserMessage {
            request_id: request_id.clone(),
            params: SendUserMessageParams {
                conversation_id: *conversation_id,
                items: vec![InputItem::Text {
                    text: message.to_string(),
                }],
            },
        };

        self.send_request(request, request_id, "sendUserMessage")
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

    fn stream_conversation(&mut self, conversation_id: &ConversationId) -> Result<()> {
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
                    EventMsg::TaskComplete(event) => {
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
                    ServerNotification::SessionConfigured(_) => {
                        // SessionConfigured notifications are unrelated to login; skip.
                    }
                }
            }

            // Not a server notification (likely a conversation event); keep waiting.
        }
    }

    fn extract_event(
        &self,
        notification: JSONRPCNotification,
        conversation_id: &ConversationId,
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
        let notification_conversation: ConversationId = serde_json::from_value(conversation_value)
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
                        bail!("{method} failed: {:?}", err);
                    }
                }
                JSONRPCMessage::Notification(notification) => {
                    self.pending_notifications.push_back(notification);
                }
                JSONRPCMessage::Request(_) => {
                    bail!("unexpected request from codex app-server");
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
                JSONRPCMessage::Request(_) => {
                    bail!("unexpected request from codex app-server");
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
