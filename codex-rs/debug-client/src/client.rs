use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::process::Child;
use std::process::ChildStdin;
use std::process::ChildStdout;
use std::process::Command;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering;
use std::sync::mpsc::Sender;

use anyhow::Context;
use anyhow::Result;
use codex_app_server_protocol::AskForApproval;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ClientNotification;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::CommandExecutionApprovalDecision;
use codex_app_server_protocol::FileChangeApprovalDecision;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCRequest;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadListParams;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::UserInput;
use serde::Serialize;

use crate::output::Output;
use crate::reader::start_reader;
use crate::state::PendingRequest;
use crate::state::ReaderEvent;
use crate::state::State;

pub struct AppServerClient {
    child: Child,
    stdin: Arc<Mutex<Option<ChildStdin>>>,
    stdout: Option<BufReader<ChildStdout>>,
    next_request_id: AtomicI64,
    state: Arc<Mutex<State>>,
    output: Output,
    filtered_output: bool,
}

impl AppServerClient {
    pub fn spawn(
        codex_bin: &str,
        config_overrides: &[String],
        output: Output,
        filtered_output: bool,
    ) -> Result<Self> {
        let mut cmd = Command::new(codex_bin);
        for override_kv in config_overrides {
            cmd.arg("--config").arg(override_kv);
        }

        let mut child = cmd
            .arg("app-server")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .with_context(|| format!("failed to start `{codex_bin}` app-server"))?;

        let stdin = child
            .stdin
            .take()
            .context("codex app-server stdin unavailable")?;
        let stdout = child
            .stdout
            .take()
            .context("codex app-server stdout unavailable")?;

        Ok(Self {
            child,
            stdin: Arc::new(Mutex::new(Some(stdin))),
            stdout: Some(BufReader::new(stdout)),
            next_request_id: AtomicI64::new(1),
            state: Arc::new(Mutex::new(State::default())),
            output,
            filtered_output,
        })
    }

    pub fn initialize(&mut self) -> Result<()> {
        let request_id = self.next_request_id();
        let request = ClientRequest::Initialize {
            request_id: request_id.clone(),
            params: codex_app_server_protocol::InitializeParams {
                client_info: ClientInfo {
                    name: "debug-client".to_string(),
                    title: Some("Debug Client".to_string()),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                },
            },
        };

        self.send(&request)?;
        let response = self.read_until_response(&request_id)?;
        let _parsed: codex_app_server_protocol::InitializeResponse =
            serde_json::from_value(response.result).context("decode initialize response")?;
        let initialized = ClientNotification::Initialized;
        self.send(&initialized)?;
        Ok(())
    }

    pub fn start_thread(&mut self, params: ThreadStartParams) -> Result<String> {
        let request_id = self.next_request_id();
        let request = ClientRequest::ThreadStart {
            request_id: request_id.clone(),
            params,
        };
        self.send(&request)?;
        let response = self.read_until_response(&request_id)?;
        let parsed: ThreadStartResponse =
            serde_json::from_value(response.result).context("decode thread/start response")?;
        let thread_id = parsed.thread.id;
        self.set_thread_id(thread_id.clone());
        Ok(thread_id)
    }

    pub fn resume_thread(&mut self, params: ThreadResumeParams) -> Result<String> {
        let request_id = self.next_request_id();
        let request = ClientRequest::ThreadResume {
            request_id: request_id.clone(),
            params,
        };
        self.send(&request)?;
        let response = self.read_until_response(&request_id)?;
        let parsed: ThreadResumeResponse =
            serde_json::from_value(response.result).context("decode thread/resume response")?;
        let thread_id = parsed.thread.id;
        self.set_thread_id(thread_id.clone());
        Ok(thread_id)
    }

    pub fn request_thread_start(&self, params: ThreadStartParams) -> Result<RequestId> {
        let request_id = self.next_request_id();
        self.track_pending(request_id.clone(), PendingRequest::Start);
        let request = ClientRequest::ThreadStart {
            request_id: request_id.clone(),
            params,
        };
        self.send(&request)?;
        Ok(request_id)
    }

    pub fn request_thread_resume(&self, params: ThreadResumeParams) -> Result<RequestId> {
        let request_id = self.next_request_id();
        self.track_pending(request_id.clone(), PendingRequest::Resume);
        let request = ClientRequest::ThreadResume {
            request_id: request_id.clone(),
            params,
        };
        self.send(&request)?;
        Ok(request_id)
    }

    pub fn request_thread_list(&self, cursor: Option<String>) -> Result<RequestId> {
        let request_id = self.next_request_id();
        self.track_pending(request_id.clone(), PendingRequest::List);
        let request = ClientRequest::ThreadList {
            request_id: request_id.clone(),
            params: ThreadListParams {
                cursor,
                limit: None,
                model_providers: None,
            },
        };
        self.send(&request)?;
        Ok(request_id)
    }

    pub fn send_turn(&self, thread_id: &str, text: String) -> Result<RequestId> {
        let request_id = self.next_request_id();
        let request = ClientRequest::TurnStart {
            request_id: request_id.clone(),
            params: TurnStartParams {
                thread_id: thread_id.to_string(),
                input: vec![UserInput::Text {
                    text,
                    // Plain text conversion has no UI element ranges.
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            },
        };
        self.send(&request)?;
        Ok(request_id)
    }

    pub fn start_reader(
        &mut self,
        events: Sender<ReaderEvent>,
        auto_approve: bool,
        filtered_output: bool,
    ) -> Result<()> {
        let stdout = self.stdout.take().context("reader already started")?;
        start_reader(
            stdout,
            Arc::clone(&self.stdin),
            Arc::clone(&self.state),
            events,
            self.output.clone(),
            auto_approve,
            filtered_output,
        );
        Ok(())
    }

    pub fn thread_id(&self) -> Option<String> {
        let state = self.state.lock().expect("state lock poisoned");
        state.thread_id.clone()
    }

    pub fn set_thread_id(&self, thread_id: String) {
        let mut state = self.state.lock().expect("state lock poisoned");
        state.thread_id = Some(thread_id);
        self.remember_thread_locked(&mut state);
    }

    pub fn use_thread(&self, thread_id: String) -> bool {
        let mut state = self.state.lock().expect("state lock poisoned");
        let known = state.known_threads.iter().any(|id| id == &thread_id);
        state.thread_id = Some(thread_id);
        self.remember_thread_locked(&mut state);
        known
    }

    pub fn shutdown(&mut self) {
        if let Ok(mut stdin) = self.stdin.lock() {
            let _ = stdin.take();
        }
        let _ = self.child.wait();
    }

    fn track_pending(&self, request_id: RequestId, kind: PendingRequest) {
        let mut state = self.state.lock().expect("state lock poisoned");
        state.pending.insert(request_id, kind);
    }

    fn remember_thread_locked(&self, state: &mut State) {
        if let Some(thread_id) = state.thread_id.as_ref()
            && !state.known_threads.iter().any(|id| id == thread_id)
        {
            state.known_threads.push(thread_id.clone());
        }
    }

    fn next_request_id(&self) -> RequestId {
        let id = self.next_request_id.fetch_add(1, Ordering::SeqCst);
        RequestId::Integer(id)
    }

    fn send<T: Serialize>(&self, value: &T) -> Result<()> {
        let json = serde_json::to_string(value).context("serialize message")?;
        let mut line = json;
        line.push('\n');
        let mut stdin = self.stdin.lock().expect("stdin lock poisoned");
        let Some(stdin) = stdin.as_mut() else {
            anyhow::bail!("stdin already closed");
        };
        stdin.write_all(line.as_bytes()).context("write message")?;
        stdin.flush().context("flush message")?;
        Ok(())
    }

    fn read_until_response(&mut self, request_id: &RequestId) -> Result<JSONRPCResponse> {
        let stdin = Arc::clone(&self.stdin);
        let output = self.output.clone();
        let reader = self.stdout.as_mut().context("stdout missing")?;
        let mut buffer = String::new();

        loop {
            buffer.clear();
            let bytes = reader
                .read_line(&mut buffer)
                .context("read server output")?;
            if bytes == 0 {
                anyhow::bail!("server closed stdout while awaiting response {request_id:?}");
            }

            let line = buffer.trim_end_matches(['\n', '\r']);
            if !line.is_empty() && !self.filtered_output {
                let _ = output.server_line(line);
            }

            let message = match serde_json::from_str::<JSONRPCMessage>(line) {
                Ok(message) => message,
                Err(_) => continue,
            };

            match message {
                JSONRPCMessage::Response(response) => {
                    if &response.id == request_id {
                        return Ok(response);
                    }
                }
                JSONRPCMessage::Request(request) => {
                    let _ = handle_server_request(request, &stdin);
                }
                _ => {}
            }
        }
    }
}

fn handle_server_request(
    request: JSONRPCRequest,
    stdin: &Arc<Mutex<Option<ChildStdin>>>,
) -> Result<()> {
    let Ok(server_request) = codex_app_server_protocol::ServerRequest::try_from(request) else {
        return Ok(());
    };

    match server_request {
        codex_app_server_protocol::ServerRequest::CommandExecutionRequestApproval {
            request_id,
            ..
        } => {
            let response = codex_app_server_protocol::CommandExecutionRequestApprovalResponse {
                decision: CommandExecutionApprovalDecision::Decline,
            };
            send_jsonrpc_response(stdin, request_id, response)
        }
        codex_app_server_protocol::ServerRequest::FileChangeRequestApproval {
            request_id, ..
        } => {
            let response = codex_app_server_protocol::FileChangeRequestApprovalResponse {
                decision: FileChangeApprovalDecision::Decline,
            };
            send_jsonrpc_response(stdin, request_id, response)
        }
        _ => Ok(()),
    }
}

fn send_jsonrpc_response<T: Serialize>(
    stdin: &Arc<Mutex<Option<ChildStdin>>>,
    request_id: RequestId,
    response: T,
) -> Result<()> {
    let result = serde_json::to_value(response).context("serialize response")?;
    let message = JSONRPCMessage::Response(JSONRPCResponse {
        id: request_id,
        result,
    });
    send_with_stdin(stdin, &message)
}

fn send_with_stdin<T: Serialize>(stdin: &Arc<Mutex<Option<ChildStdin>>>, value: &T) -> Result<()> {
    let json = serde_json::to_string(value).context("serialize message")?;
    let mut line = json;
    line.push('\n');
    let mut stdin = stdin.lock().expect("stdin lock poisoned");
    let Some(stdin) = stdin.as_mut() else {
        anyhow::bail!("stdin already closed");
    };
    stdin.write_all(line.as_bytes()).context("write message")?;
    stdin.flush().context("flush message")?;
    Ok(())
}

pub fn build_thread_start_params(
    approval_policy: AskForApproval,
    model: Option<String>,
    model_provider: Option<String>,
    cwd: Option<String>,
) -> ThreadStartParams {
    ThreadStartParams {
        model,
        model_provider,
        cwd,
        approval_policy: Some(approval_policy),
        experimental_raw_events: false,
        ..Default::default()
    }
}

pub fn build_thread_resume_params(
    thread_id: String,
    approval_policy: AskForApproval,
    model: Option<String>,
    model_provider: Option<String>,
    cwd: Option<String>,
) -> ThreadResumeParams {
    ThreadResumeParams {
        thread_id,
        model,
        model_provider,
        cwd,
        approval_policy: Some(approval_policy),
        ..Default::default()
    }
}
