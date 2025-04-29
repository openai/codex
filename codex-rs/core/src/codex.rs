use std::collections::HashMap;
use std::collections::HashSet;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use anyhow::Context;
use async_channel::Receiver;
use async_channel::Sender;
use codex_apply_patch::maybe_parse_apply_patch_verified;
use codex_apply_patch::print_summary;
use codex_apply_patch::AffectedPaths;
use codex_apply_patch::ApplyPatchFileChange;
use codex_apply_patch::MaybeApplyPatchVerified;
use fs_err as fs;
use futures::prelude::*;
use serde::Serialize;
use tokio::sync::oneshot;
use tokio::sync::Notify;
use tokio::task::AbortHandle;
use tracing::debug;
use tracing::info;
use tracing::trace;
use tracing::warn;

use crate::client::ModelClient;
use crate::client::Prompt;
use crate::client::ResponseEvent;
use crate::error::CodexErr;
use crate::error::Result as CodexResult;
use crate::exec::process_exec_tool_call;
use crate::exec::ExecParams;
use crate::exec::ExecToolCallOutput;
use crate::exec::SandboxType;
use crate::flags::OPENAI_STREAM_MAX_RETRIES;
use crate::models::ContentItem;
use crate::models::FunctionCallOutputPayload;
use crate::models::ResponseInputItem;
use crate::models::ResponseItem;
use crate::protocol::AskForApproval;
use crate::protocol::Event;
use crate::protocol::EventMsg;
use crate::protocol::FileChange;
use crate::protocol::InputItem;
use crate::protocol::Op;
use crate::protocol::ReviewDecision;
use crate::protocol::SandboxPolicy;
use crate::protocol::Submission;
use crate::safety::assess_command_safety;
use crate::safety::assess_patch_safety;
use crate::safety::SafetyCheck;
use crate::util::backoff;
use crate::zdr_transcript::ZdrTranscript;

/// The high-level interface to the Codex system.
/// It operates as a queue pair where you send submissions and receive events.
#[derive(Clone)]
pub struct Codex {
    tx_sub: Sender<Submission>,
    rx_event: Receiver<Event>,
    recorder: Recorder,
}

impl Codex {
    pub fn spawn(ctrl_c: Arc<Notify>) -> CodexResult<Self> {
        CodexBuilder::default().spawn(ctrl_c)
    }

    pub fn builder() -> CodexBuilder {
        CodexBuilder::default()
    }

    pub async fn submit(&self, sub: Submission) -> CodexResult<()> {
        self.recorder.record_submission(&sub);
        self.tx_sub
            .send(sub)
            .await
            .map_err(|_| CodexErr::InternalAgentDied)
    }

    pub async fn next_event(&self) -> CodexResult<Event> {
        let event = self
            .rx_event
            .recv()
            .await
            .map_err(|_| CodexErr::InternalAgentDied)?;
        self.recorder.record_event(&event);
        Ok(event)
    }
}

#[derive(Default)]
pub struct CodexBuilder {
    record_submissions: Option<PathBuf>,
    record_events: Option<PathBuf>,
}

impl CodexBuilder {
    pub fn spawn(self, ctrl_c: Arc<Notify>) -> CodexResult<Codex> {
        let (tx_sub, rx_sub) = async_channel::bounded(64);
        let (tx_event, rx_event) = async_channel::bounded(64);
        let recorder = Recorder::new(&self)?;
        tokio::spawn(submission_loop(rx_sub, tx_event, ctrl_c));
        Ok(Codex {
            tx_sub,
            rx_event,
            recorder,
        })
    }

    pub fn record_submissions(mut self, path: impl AsRef<Path>) -> Self {
        debug!("Recording submissions to {:?}", path.as_ref());
        self.record_submissions = Some(path.as_ref().to_path_buf());
        self
    }

    pub fn record_events(mut self, path: impl AsRef<Path>) -> Self {
        debug!("Recording events to {:?}", path.as_ref());
        self.record_events = Some(path.as_ref().to_path_buf());
        self
    }
}

#[derive(Clone)]
struct Recorder {
    submissions: Option<Arc<Mutex<fs::File>>>,
    events: Option<Arc<Mutex<fs::File>>>,
}

impl Recorder {
    fn new(builder: &CodexBuilder) -> CodexResult<Self> {
        let submissions = match &builder.record_submissions {
            Some(path) => {
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)?;
                }
                let f = fs::File::create(path)?;
                Some(Arc::new(Mutex::new(f)))
            }
            None => None,
        };
        let events = match &builder.record_events {
            Some(path) => {
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)?;
                }
                let f = fs::File::create(path)?;
                Some(Arc::new(Mutex::new(f)))
            }
            None => None,
        };
        Ok(Self {
            submissions,
            events,
        })
    }

    pub fn record_submission(&self, sub: &Submission) {
        let Some(f) = &self.submissions else {
            return;
        };
        let mut f = f.lock().unwrap();
        let json = serde_json::to_string(sub).expect("failed to serialize submission json");
        if let Err(e) = writeln!(f, "{json}") {
            warn!("failed to record submission: {e:#}");
        }
    }

    pub fn record_event(&self, event: &Event) {
        let Some(f) = &self.events else {
            return;
        };
        let mut f = f.lock().unwrap();
        let json = serde_json::to_string(event).expect("failed to serialize event json");
        if let Err(e) = writeln!(f, "{json}") {
            warn!("failed to record event: {e:#}");
        }
    }
}

/// Context for an initialized model agent
///
/// A session has at most 1 running task at a time, and can be interrupted by user input.
struct Session {
    client: ModelClient,
    tx_event: Sender<Event>,
    ctrl_c: Arc<Notify>,

    instructions: Option<String>,
    approval_policy: AskForApproval,
    sandbox_policy: SandboxPolicy,
    writable_roots: Mutex<Vec<PathBuf>>,

    state: Mutex<State>,
}

/// Mutable state of the agent
#[derive(Default)]
struct State {
    approved_commands: HashSet<Vec<String>>,
    current_task: Option<AgentTask>,
    previous_response_id: Option<String>,
    pending_approvals: HashMap<String, oneshot::Sender<ReviewDecision>>,
    pending_input: Vec<ResponseInputItem>,
    zdr_transcript: Option<ZdrTranscript>,
}

impl Session {
    pub fn set_task(&self, task: AgentTask) {
        let mut state = self.state.lock().unwrap();
        if let Some(current_task) = state.current_task.take() {
            current_task.abort();
        }
        state.current_task = Some(task);
    }

    pub fn remove_task(&self, sub_id: &str) {
        let mut state = self.state.lock().unwrap();
        if let Some(task) = &state.current_task {
            if task.sub_id == sub_id {
                state.current_task.take();
            }
        }
    }

    pub async fn request_command_approval(
        &self,
        sub_id: String,
        command: Vec<String>,
        cwd: PathBuf,
        reason: Option<String>,
    ) -> oneshot::Receiver<ReviewDecision> {
        let (tx_approve, rx_approve) = oneshot::channel();
        let event = Event {
            id: sub_id.clone(),
            msg: EventMsg::ExecApprovalRequest {
                command,
                cwd,
                reason,
            },
        };
        let _ = self.tx_event.send(event).await;
        {
            let mut state = self.state.lock().unwrap();
            state.pending_approvals.insert(sub_id, tx_approve);
        }
        rx_approve
    }

    pub async fn request_patch_approval(
        &self,
        sub_id: String,
        changes: &HashMap<PathBuf, ApplyPatchFileChange>,
        reason: Option<String>,
        grant_root: Option<PathBuf>,
    ) -> oneshot::Receiver<ReviewDecision> {
        let (tx_approve, rx_approve) = oneshot::channel();
        let event = Event {
            id: sub_id.clone(),
            msg: EventMsg::ApplyPatchApprovalRequest {
                changes: convert_apply_patch_to_protocol(changes),
                reason,
                grant_root,
            },
        };
        let _ = self.tx_event.send(event).await;
        {
            let mut state = self.state.lock().unwrap();
            state.pending_approvals.insert(sub_id, tx_approve);
        }
        rx_approve
    }

    pub fn notify_approval(&self, sub_id: &str, decision: ReviewDecision) {
        let mut state = self.state.lock().unwrap();
        if let Some(tx_approve) = state.pending_approvals.remove(sub_id) {
            tx_approve.send(decision).ok();
        }
    }

    pub fn add_approved_command(&self, cmd: Vec<String>) {
        let mut state = self.state.lock().unwrap();
        state.approved_commands.insert(cmd);
    }

    async fn notify_exec_command_begin(
        &self,
        sub_id: &str,
        call_id: &str,
        command: Vec<String>,
        cwd: Option<String>,
    ) {
        let cwd = cwd
            .or_else(|| {
                std::env::current_dir()
                    .ok()
                    .map(|p| p.to_string_lossy().to_string())
            })
            .unwrap_or_else(|| "<unknown cwd>".to_string());
        let event = Event {
            id: sub_id.to_string(),
            msg: EventMsg::ExecCommandBegin {
                call_id: call_id.to_string(),
                command,
                cwd,
            },
        };
        let _ = self.tx_event.send(event).await;
    }

    async fn notify_exec_command_end(
        &self,
        sub_id: &str,
        call_id: &str,
        stdout: &str,
        stderr: &str,
        exit_code: i32,
    ) {
        const MAX_STREAM_OUTPUT: usize = 5 * 1024; // 5KiB
        let event = Event {
            id: sub_id.to_string(),
            // Because stdout and stderr could each be up to 100 KiB, we send
            // truncated versions.
            msg: EventMsg::ExecCommandEnd {
                call_id: call_id.to_string(),
                stdout: stdout.chars().take(MAX_STREAM_OUTPUT).collect(),
                stderr: stderr.chars().take(MAX_STREAM_OUTPUT).collect(),
                exit_code,
            },
        };
        let _ = self.tx_event.send(event).await;
    }

    /// Helper that emits a BackgroundEvent with the given message. This keeps
    /// the call‑sites terse so adding more diagnostics does not clutter the
    /// core agent logic.
    async fn notify_background_event(&self, sub_id: &str, message: impl Into<String>) {
        let event = Event {
            id: sub_id.to_string(),
            msg: EventMsg::BackgroundEvent {
                message: message.into(),
            },
        };
        let _ = self.tx_event.send(event).await;
    }

    /// Returns the input if there was no task running to inject into
    pub fn inject_input(&self, input: Vec<InputItem>) -> Result<(), Vec<InputItem>> {
        let mut state = self.state.lock().unwrap();
        if state.current_task.is_some() {
            state.pending_input.push(input.into());
            Ok(())
        } else {
            Err(input)
        }
    }

    pub fn get_pending_input(&self) -> Vec<ResponseInputItem> {
        let mut state = self.state.lock().unwrap();
        if state.pending_input.is_empty() {
            Vec::with_capacity(0)
        } else {
            let mut ret = Vec::new();
            std::mem::swap(&mut ret, &mut state.pending_input);
            ret
        }
    }

    pub fn abort(&self) {
        info!("Aborting existing session");
        let mut state = self.state.lock().unwrap();
        state.pending_approvals.clear();
        state.pending_input.clear();
        if let Some(task) = state.current_task.take() {
            task.abort();
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.abort();
    }
}

impl State {
    pub fn partial_clone(&self) -> Self {
        Self {
            approved_commands: self.approved_commands.clone(),
            previous_response_id: self.previous_response_id.clone(),
            zdr_transcript: self.zdr_transcript.clone(),
            ..Default::default()
        }
    }
}

/// A series of Turns in response to user input.
struct AgentTask {
    sess: Arc<Session>,
    sub_id: String,
    handle: AbortHandle,
}

impl AgentTask {
    fn spawn(sess: Arc<Session>, sub_id: String, input: Vec<InputItem>) -> Self {
        let handle =
            tokio::spawn(run_task(Arc::clone(&sess), sub_id.clone(), input)).abort_handle();
        Self {
            sess,
            sub_id,
            handle,
        }
    }

    fn abort(self) {
        if !self.handle.is_finished() {
            self.handle.abort();
            let event = Event {
                id: self.sub_id,
                msg: EventMsg::Error {
                    message: "Turn interrupted".to_string(),
                },
            };
            let tx_event = self.sess.tx_event.clone();
            tokio::spawn(async move {
                tx_event.send(event).await.ok();
            });
        }
    }
}

async fn submission_loop(
    rx_sub: Receiver<Submission>,
    tx_event: Sender<Event>,
    ctrl_c: Arc<Notify>,
) {
    let mut sess: Option<Arc<Session>> = None;
    // shorthand - send an event when there is no active session
    let send_no_session_event = |sub_id: String| async {
        let event = Event {
            id: sub_id,
            msg: EventMsg::Error {
                message: "No session initialized, expected 'ConfigureSession' as first Op"
                    .to_string(),
            },
        };
        tx_event.send(event).await.ok();
    };

    loop {
        let interrupted = ctrl_c.notified();
        let sub = tokio::select! {
            res = rx_sub.recv() => match res {
                Ok(sub) => sub,
                Err(_) => break,
            },
            _ = interrupted => {
                if let Some(sess) = sess.as_ref(){
                    sess.abort();
                }
                continue;
            },
        };

        debug!(?sub, "Submission");
        match sub.op {
            Op::Interrupt => {
                let sess = match sess.as_ref() {
                    Some(sess) => sess,
                    None => {
                        send_no_session_event(sub.id).await;
                        continue;
                    }
                };
                sess.abort();
            }
            Op::ConfigureSession {
                model,
                instructions,
                approval_policy,
                sandbox_policy,
                disable_response_storage,
            } => {
                info!(model, "Configuring session");
                let client = ModelClient::new(model.clone());

                // abort any current running session and clone its state
                let state = match sess.take() {
                    Some(sess) => {
                        sess.abort();
                        sess.state.lock().unwrap().partial_clone()
                    }
                    None => State {
                        zdr_transcript: if disable_response_storage {
                            Some(ZdrTranscript::new())
                        } else {
                            None
                        },
                        ..Default::default()
                    },
                };

                // update session
                sess = Some(Arc::new(Session {
                    client,
                    tx_event: tx_event.clone(),
                    ctrl_c: Arc::clone(&ctrl_c),
                    instructions,
                    approval_policy,
                    sandbox_policy,
                    writable_roots: Mutex::new(get_writable_roots()),
                    state: Mutex::new(state),
                }));

                // ack
                let event = Event {
                    id: sub.id,
                    msg: EventMsg::SessionConfigured { model },
                };
                if tx_event.send(event).await.is_err() {
                    return;
                }
            }
            Op::UserInput { items } => {
                let sess = match sess.as_ref() {
                    Some(sess) => sess,
                    None => {
                        send_no_session_event(sub.id).await;
                        continue;
                    }
                };

                // attempt to inject input into current task
                if let Err(items) = sess.inject_input(items) {
                    // no current task, spawn a new one
                    let task = AgentTask::spawn(Arc::clone(sess), sub.id, items);
                    sess.set_task(task);
                }
            }
            Op::ExecApproval { id, decision } => {
                let sess = match sess.as_ref() {
                    Some(sess) => sess,
                    None => {
                        send_no_session_event(sub.id).await;
                        continue;
                    }
                };
                match decision {
                    ReviewDecision::Abort => {
                        sess.abort();
                    }
                    other => sess.notify_approval(&id, other),
                }
            }
            Op::PatchApproval { id, decision } => {
                let sess = match sess.as_ref() {
                    Some(sess) => sess,
                    None => {
                        send_no_session_event(sub.id).await;
                        continue;
                    }
                };
                match decision {
                    ReviewDecision::Abort => {
                        sess.abort();
                    }
                    other => sess.notify_approval(&id, other),
                }
            }
        }
    }
    debug!("Agent loop exited");
}

async fn run_task(sess: Arc<Session>, sub_id: String, input: Vec<InputItem>) {
    if input.is_empty() {
        return;
    }
    let event = Event {
        id: sub_id.clone(),
        msg: EventMsg::TaskStarted,
    };
    if sess.tx_event.send(event).await.is_err() {
        return;
    }

    let mut pending_response_input: Vec<ResponseInputItem> = vec![ResponseInputItem::from(input)];
    loop {
        let mut net_new_turn_input = pending_response_input
            .drain(..)
            .map(ResponseItem::from)
            .collect::<Vec<_>>();

        // Note that pending_input would be something like a message the user
        // submitted through the UI while the model was running. Though the UI
        // may support this, the model might not.
        let pending_input = sess.get_pending_input().into_iter().map(ResponseItem::from);
        net_new_turn_input.extend(pending_input);

        let turn_input: Vec<ResponseItem> =
            if let Some(transcript) = sess.state.lock().unwrap().zdr_transcript.as_mut() {
                // If we are using ZDR, we need to send the transcript with every turn.
                let mut full_transcript = transcript.contents();
                full_transcript.extend(net_new_turn_input.clone());
                transcript.record_items(net_new_turn_input);
                full_transcript
            } else {
                net_new_turn_input
            };

        match run_turn(&sess, sub_id.clone(), turn_input).await {
            Ok(turn_output) => {
                let (items, responses): (Vec<_>, Vec<_>) = turn_output
                    .into_iter()
                    .map(|p| (p.item, p.response))
                    .unzip();
                let responses = responses
                    .into_iter()
                    .flatten()
                    .collect::<Vec<ResponseInputItem>>();

                // Only attempt to take the lock if there is something to record.
                if !items.is_empty() {
                    if let Some(transcript) = sess.state.lock().unwrap().zdr_transcript.as_mut() {
                        transcript.record_items(items);
                    }
                }

                if responses.is_empty() {
                    debug!("Turn completed");
                    break;
                }

                pending_response_input = responses;
            }
            Err(e) => {
                info!("Turn error: {e:#}");
                let event = Event {
                    id: sub_id.clone(),
                    msg: EventMsg::Error {
                        message: e.to_string(),
                    },
                };
                sess.tx_event.send(event).await.ok();
                return;
            }
        }
    }
    sess.remove_task(&sub_id);
    let event = Event {
        id: sub_id,
        msg: EventMsg::TaskComplete,
    };
    sess.tx_event.send(event).await.ok();
}

async fn run_turn(
    sess: &Session,
    sub_id: String,
    input: Vec<ResponseItem>,
) -> CodexResult<Vec<ProcessedResponseItem>> {
    // Decide whether to use server-side storage (previous_response_id) or disable it
    let (prev_id, store, is_first_turn) = {
        let state = sess.state.lock().unwrap();
        let is_first_turn = state.previous_response_id.is_none();
        let store = state.zdr_transcript.is_none();
        let prev_id = if store {
            state.previous_response_id.clone()
        } else {
            // When using ZDR, the Reponses API may send previous_response_id
            // back, but trying to use it results in a 400.
            None
        };
        (prev_id, store, is_first_turn)
    };

    let instructions = if is_first_turn {
        sess.instructions.clone()
    } else {
        None
    };
    let prompt = Prompt {
        input,
        prev_id,
        instructions,
        store,
    };

    let mut retries = 0;
    loop {
        match try_run_turn(sess, &sub_id, &prompt).await {
            Ok(output) => return Ok(output),
            Err(CodexErr::Interrupted) => return Err(CodexErr::Interrupted),
            Err(e) => {
                if retries < *OPENAI_STREAM_MAX_RETRIES {
                    retries += 1;
                    let delay = backoff(retries);
                    warn!(
                        "stream disconnected - retrying turn ({retries}/{} in {delay:?})...",
                        *OPENAI_STREAM_MAX_RETRIES
                    );

                    // Surface retry information to any UI/front‑end so the
                    // user understands what is happening instead of staring
                    // at a seemingly frozen screen.
                    sess.notify_background_event(
                        &sub_id,
                        format!(
                            "stream error: {e}; retrying {retries}/{} in {:?}…",
                            *OPENAI_STREAM_MAX_RETRIES, delay
                        ),
                    )
                    .await;

                    tokio::time::sleep(delay).await;
                } else {
                    return Err(e);
                }
            }
        }
    }
}

/// When the model is prompted, it returns a stream of events. Some of these
/// events map to a `ResponseItem`. A `ResponseItem` may need to be
/// "handled" such that it produces a `ResponseInputItem` that needs to be
/// sent back to the model on the next turn.
struct ProcessedResponseItem {
    item: ResponseItem,
    response: Option<ResponseInputItem>,
}

async fn try_run_turn(
    sess: &Session,
    sub_id: &str,
    prompt: &Prompt,
) -> CodexResult<Vec<ProcessedResponseItem>> {
    let mut stream = sess.client.clone().stream(prompt).await?;

    // Buffer all the incoming messages from the stream first, then execute them.
    // If we execute a function call in the middle of handling the stream, it can time out.
    let mut input = Vec::new();
    while let Some(event) = stream.next().await {
        input.push(event?);
    }

    let mut output = Vec::new();
    for event in input {
        match event {
            ResponseEvent::OutputItemDone(item) => {
                let response = handle_response_item(sess, sub_id, item.clone()).await?;
                output.push(ProcessedResponseItem { item, response });
            }
            ResponseEvent::Completed { response_id } => {
                let mut state = sess.state.lock().unwrap();
                state.previous_response_id = Some(response_id);
                break;
            }
        }
    }
    Ok(output)
}

async fn handle_response_item(
    sess: &Session,
    sub_id: &str,
    item: ResponseItem,
) -> CodexResult<Option<ResponseInputItem>> {
    debug!(?item, "Output item");
    let mut output = None;
    match item {
        ResponseItem::Message { content, .. } => {
            for item in content {
                if let ContentItem::OutputText { text } = item {
                    let event = Event {
                        id: sub_id.to_string(),
                        msg: EventMsg::AgentMessage { message: text },
                    };
                    sess.tx_event.send(event).await.ok();
                }
            }
        }
        ResponseItem::FunctionCall {
            name,
            arguments,
            call_id,
        } => {
            output = Some(
                handle_function_call(sess, sub_id.to_string(), name, arguments, call_id).await,
            );
        }
        ResponseItem::FunctionCallOutput { .. } => {
            debug!("unexpected FunctionCallOutput from stream");
        }
        ResponseItem::Other => (),
    }
    Ok(output)
}

async fn handle_function_call(
    sess: &Session,
    sub_id: String,
    name: String,
    arguments: String,
    call_id: String,
) -> ResponseInputItem {
    match name.as_str() {
        "container.exec" | "shell" => {
            // parse command
            let params = match serde_json::from_str::<ExecParams>(&arguments) {
                Ok(v) => v,
                Err(e) => {
                    // allow model to re-sample
                    let output = ResponseInputItem::FunctionCallOutput {
                        call_id,
                        output: crate::models::FunctionCallOutputPayload {
                            content: format!("failed to parse function arguments: {e}"),
                            success: None,
                        },
                    };
                    return output;
                }
            };

            // check if this was a patch, and apply it if so
            match maybe_parse_apply_patch_verified(&params.command) {
                MaybeApplyPatchVerified::Body(changes) => {
                    return apply_patch(sess, sub_id, call_id, changes).await;
                }
                MaybeApplyPatchVerified::CorrectnessError(parse_error) => {
                    // It looks like an invocation of `apply_patch`, but we
                    // could not resolve it into a patch that would apply
                    // cleanly. Return to model for resample.
                    return ResponseInputItem::FunctionCallOutput {
                        call_id,
                        output: FunctionCallOutputPayload {
                            content: format!("error: {parse_error:#}"),
                            success: None,
                        },
                    };
                }
                MaybeApplyPatchVerified::ShellParseError(error) => {
                    trace!("Failed to parse shell command, {error}");
                }
                MaybeApplyPatchVerified::NotApplyPatch => (),
            }

            // this was not a valid patch, execute command
            let repo_root = std::env::current_dir().expect("no current dir");
            let workdir: PathBuf = params
                .workdir
                .as_ref()
                .map(PathBuf::from)
                .unwrap_or(repo_root.clone());

            // safety checks
            let safety = {
                let state = sess.state.lock().unwrap();
                assess_command_safety(
                    &params.command,
                    sess.approval_policy,
                    sess.sandbox_policy,
                    &state.approved_commands,
                )
            };
            let sandbox_type = match safety {
                SafetyCheck::AutoApprove { sandbox_type } => sandbox_type,
                SafetyCheck::AskUser => {
                    let rx_approve = sess
                        .request_command_approval(
                            sub_id.clone(),
                            params.command.clone(),
                            workdir.clone(),
                            None,
                        )
                        .await;
                    match rx_approve.await.unwrap_or_default() {
                        ReviewDecision::Approved => (),
                        ReviewDecision::ApprovedForSession => {
                            sess.add_approved_command(params.command.clone());
                        }
                        ReviewDecision::Denied | ReviewDecision::Abort => {
                            return ResponseInputItem::FunctionCallOutput {
                                call_id,
                                output: crate::models::FunctionCallOutputPayload {
                                    content: "exec command rejected by user".to_string(),
                                    success: None,
                                },
                            };
                        }
                    }
                    // No sandboxing is applied because the user has given
                    // explicit approval. Often, we end up in this case because
                    // the command cannot be run in a sandbox, such as
                    // installing a new dependency that requires network access.
                    SandboxType::None
                }
                SafetyCheck::Reject { reason } => {
                    return ResponseInputItem::FunctionCallOutput {
                        call_id,
                        output: crate::models::FunctionCallOutputPayload {
                            content: format!("exec command rejected: {reason}"),
                            success: None,
                        },
                    };
                }
            };

            sess.notify_exec_command_begin(
                &sub_id,
                &call_id,
                params.command.clone(),
                params.workdir.clone(),
            )
            .await;

            let roots_snapshot = { sess.writable_roots.lock().unwrap().clone() };

            let output_result = process_exec_tool_call(
                params.clone(),
                sandbox_type,
                &roots_snapshot,
                sess.ctrl_c.clone(),
                sess.sandbox_policy,
            )
            .await;

            match output_result {
                Ok(output) => {
                    let ExecToolCallOutput {
                        exit_code,
                        stdout,
                        stderr,
                        duration,
                    } = output;

                    sess.notify_exec_command_end(&sub_id, &call_id, &stdout, &stderr, exit_code)
                        .await;

                    let is_success = exit_code == 0;
                    let content = format_exec_output(
                        if is_success { &stdout } else { &stderr },
                        exit_code,
                        duration,
                    );

                    ResponseInputItem::FunctionCallOutput {
                        call_id,
                        output: FunctionCallOutputPayload {
                            content,
                            success: Some(is_success),
                        },
                    }
                }
                Err(CodexErr::Sandbox(e)) => {
                    // Early out if the user never wants to be asked for approval; just return to the model immediately
                    if sess.approval_policy == AskForApproval::Never {
                        return ResponseInputItem::FunctionCallOutput {
                            call_id,
                            output: FunctionCallOutputPayload {
                                content: format!(
                                    "failed in sandbox {:?} with execution error: {e}",
                                    sandbox_type
                                ),
                                success: Some(false),
                            },
                        };
                    }

                    // Ask the user to retry without sandbox
                    sess.notify_background_event(&sub_id, format!("Execution failed: {e}"))
                        .await;

                    let rx_approve = sess
                        .request_command_approval(
                            sub_id.clone(),
                            params.command.clone(),
                            workdir,
                            Some("command failed; retry without sandbox?".to_string()),
                        )
                        .await;

                    match rx_approve.await.unwrap_or_default() {
                        ReviewDecision::Approved | ReviewDecision::ApprovedForSession => {
                            // Persist this command as pre‑approved for the
                            // remainder of the session so future
                            // executions skip the sandbox directly.
                            // TODO(ragona): Isn't this a bug? It always saves the command in an | fork?
                            sess.add_approved_command(params.command.clone());
                            // Inform UI we are retrying without sandbox.
                            sess.notify_background_event(
                                &sub_id,
                                "retrying command without sandbox",
                            )
                            .await;

                            // Emit a fresh Begin event so progress bars reset.
                            let retry_call_id = format!("{call_id}-retry");
                            sess.notify_exec_command_begin(
                                &sub_id,
                                &retry_call_id,
                                params.command.clone(),
                                params.workdir.clone(),
                            )
                            .await;

                            let retry_roots = { sess.writable_roots.lock().unwrap().clone() };

                            // This is an escalated retry; the policy will not be
                            // examined and the sandbox has been set to `None`.
                            let retry_output_result = process_exec_tool_call(
                                params.clone(),
                                SandboxType::None,
                                &retry_roots,
                                sess.ctrl_c.clone(),
                                sess.sandbox_policy,
                            )
                            .await;

                            match retry_output_result {
                                Ok(retry_output) => {
                                    let ExecToolCallOutput {
                                        exit_code,
                                        stdout,
                                        stderr,
                                        duration,
                                    } = retry_output;

                                    sess.notify_exec_command_end(
                                        &sub_id,
                                        &retry_call_id,
                                        &stdout,
                                        &stderr,
                                        exit_code,
                                    )
                                    .await;

                                    let is_success = exit_code == 0;
                                    let content = format_exec_output(
                                        if is_success { &stdout } else { &stderr },
                                        exit_code,
                                        duration,
                                    );

                                    ResponseInputItem::FunctionCallOutput {
                                        call_id,
                                        output: FunctionCallOutputPayload {
                                            content,
                                            success: Some(is_success),
                                        },
                                    }
                                }
                                Err(e) => {
                                    // Handle retry failure
                                    ResponseInputItem::FunctionCallOutput {
                                        call_id,
                                        output: FunctionCallOutputPayload {
                                            content: format!("retry failed: {e}"),
                                            success: None,
                                        },
                                    }
                                }
                            }
                        }
                        ReviewDecision::Denied | ReviewDecision::Abort => {
                            // Fall through to original failure handling.
                            ResponseInputItem::FunctionCallOutput {
                                call_id,
                                output: FunctionCallOutputPayload {
                                    content: "exec command rejected by user".to_string(),
                                    success: None,
                                },
                            }
                        }
                    }
                }
                Err(e) => {
                    // Handle non-sandbox errors
                    ResponseInputItem::FunctionCallOutput {
                        call_id,
                        output: FunctionCallOutputPayload {
                            content: format!("execution error: {e}"),
                            success: None,
                        },
                    }
                }
            }
        }
        _ => {
            // Unknown function: reply with structured failure so the model can adapt.
            ResponseInputItem::FunctionCallOutput {
                call_id,
                output: crate::models::FunctionCallOutputPayload {
                    content: format!("unsupported call: {}", name),
                    success: None,
                },
            }
        }
    }
}

async fn apply_patch(
    sess: &Session,
    sub_id: String,
    call_id: String,
    changes: HashMap<PathBuf, ApplyPatchFileChange>,
) -> ResponseInputItem {
    let writable_roots_snapshot = {
        let guard = sess.writable_roots.lock().unwrap();
        guard.clone()
    };

    let auto_approved =
        match assess_patch_safety(&changes, sess.approval_policy, &writable_roots_snapshot) {
            SafetyCheck::AutoApprove { .. } => true,
            SafetyCheck::AskUser => {
                // Compute a readable summary of path changes to include in the
                // approval request so the user can make an informed decision.
                let rx_approve = sess
                    .request_patch_approval(sub_id.clone(), &changes, None, None)
                    .await;
                match rx_approve.await.unwrap_or_default() {
                    ReviewDecision::Approved | ReviewDecision::ApprovedForSession => false,
                    ReviewDecision::Denied | ReviewDecision::Abort => {
                        return ResponseInputItem::FunctionCallOutput {
                            call_id,
                            output: FunctionCallOutputPayload {
                                content: "patch rejected by user".to_string(),
                                success: Some(false),
                            },
                        };
                    }
                }
            }
            SafetyCheck::Reject { reason } => {
                return ResponseInputItem::FunctionCallOutput {
                    call_id,
                    output: FunctionCallOutputPayload {
                        content: format!("patch rejected: {reason}"),
                        success: Some(false),
                    },
                };
            }
        };

    // Verify write permissions before touching the filesystem.
    let writable_snapshot = { sess.writable_roots.lock().unwrap().clone() };

    if let Some(offending) = first_offending_path(&changes, &writable_snapshot) {
        let root = offending.parent().unwrap_or(&offending).to_path_buf();

        let reason = Some(format!(
            "grant write access to {} for this session",
            root.display()
        ));

        let rx = sess
            .request_patch_approval(sub_id.clone(), &changes, reason.clone(), Some(root.clone()))
            .await;

        if !matches!(
            rx.await.unwrap_or_default(),
            ReviewDecision::Approved | ReviewDecision::ApprovedForSession
        ) {
            return ResponseInputItem::FunctionCallOutput {
                call_id,
                output: FunctionCallOutputPayload {
                    content: "patch rejected by user".to_string(),
                    success: Some(false),
                },
            };
        }

        // user approved, extend writable roots for this session
        sess.writable_roots.lock().unwrap().push(root);
    }

    let _ = sess
        .tx_event
        .send(Event {
            id: sub_id.clone(),
            msg: EventMsg::PatchApplyBegin {
                call_id: call_id.clone(),
                auto_approved,
                changes: convert_apply_patch_to_protocol(&changes),
            },
        })
        .await;

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    // Enforce writable roots. If a write is blocked, collect offending root
    // and prompt the user to extend permissions.
    let mut result = apply_changes_from_apply_patch_and_report(&changes, &mut stdout, &mut stderr);

    if let Err(err) = &result {
        if err.kind() == std::io::ErrorKind::PermissionDenied {
            // Determine first offending path.
            let offending_opt = changes.iter().find_map(|(path, change)| {
                let path_ref = match change {
                    ApplyPatchFileChange::Add { .. } => path,
                    ApplyPatchFileChange::Delete => path,
                    ApplyPatchFileChange::Update { .. } => path,
                };

                // Reuse safety normalisation logic: treat absolute path.
                let abs = if path_ref.is_absolute() {
                    path_ref.clone()
                } else {
                    std::env::current_dir().unwrap_or_default().join(path_ref)
                };

                let writable = {
                    let roots = sess.writable_roots.lock().unwrap();
                    roots.iter().any(|root| abs.starts_with(root))
                };
                if writable {
                    None
                } else {
                    Some(path_ref.clone())
                }
            });

            if let Some(offending) = offending_opt {
                let root = offending.parent().unwrap_or(&offending).to_path_buf();

                let reason = Some(format!(
                    "grant write access to {} for this session",
                    root.display()
                ));
                let rx = sess
                    .request_patch_approval(
                        sub_id.clone(),
                        &changes,
                        reason.clone(),
                        Some(root.clone()),
                    )
                    .await;
                if matches!(
                    rx.await.unwrap_or_default(),
                    ReviewDecision::Approved | ReviewDecision::ApprovedForSession
                ) {
                    // Extend writable roots.
                    sess.writable_roots.lock().unwrap().push(root);
                    stdout.clear();
                    stderr.clear();
                    result = apply_changes_from_apply_patch_and_report(
                        &changes,
                        &mut stdout,
                        &mut stderr,
                    );
                }
            }
        }
    }

    // Emit PatchApplyEnd event.
    let success_flag = result.is_ok();
    let _ = sess
        .tx_event
        .send(Event {
            id: sub_id.clone(),
            msg: EventMsg::PatchApplyEnd {
                call_id: call_id.clone(),
                stdout: String::from_utf8_lossy(&stdout).to_string(),
                stderr: String::from_utf8_lossy(&stderr).to_string(),
                success: success_flag,
            },
        })
        .await;

    match result {
        Ok(_) => ResponseInputItem::FunctionCallOutput {
            call_id,
            output: FunctionCallOutputPayload {
                content: String::from_utf8_lossy(&stdout).to_string(),
                success: None,
            },
        },
        Err(e) => ResponseInputItem::FunctionCallOutput {
            call_id,
            output: FunctionCallOutputPayload {
                content: format!("error: {e:#}, stderr: {}", String::from_utf8_lossy(&stderr)),
                success: Some(false),
            },
        },
    }
}

/// Return the first path in `hunks` that is NOT under any of the
/// `writable_roots` (after normalising). If all paths are acceptable,
/// returns None.
fn first_offending_path(
    changes: &HashMap<PathBuf, ApplyPatchFileChange>,
    writable_roots: &[PathBuf],
) -> Option<PathBuf> {
    let cwd = std::env::current_dir().unwrap_or_default();

    for (path, change) in changes {
        let candidate = match change {
            ApplyPatchFileChange::Add { .. } => path,
            ApplyPatchFileChange::Delete => path,
            ApplyPatchFileChange::Update { move_path, .. } => move_path.as_ref().unwrap_or(path),
        };

        let abs = if candidate.is_absolute() {
            candidate.clone()
        } else {
            cwd.join(candidate)
        };

        let mut allowed = false;
        for root in writable_roots {
            let root_abs = if root.is_absolute() {
                root.clone()
            } else {
                cwd.join(root)
            };
            if abs.starts_with(&root_abs) {
                allowed = true;
                break;
            }
        }

        if !allowed {
            return Some(candidate.clone());
        }
    }
    None
}

fn convert_apply_patch_to_protocol(
    changes: &HashMap<PathBuf, ApplyPatchFileChange>,
) -> HashMap<PathBuf, FileChange> {
    let mut result = HashMap::with_capacity(changes.len());
    for (path, change) in changes {
        let protocol_change = match change {
            ApplyPatchFileChange::Add { content } => FileChange::Add {
                content: content.clone(),
            },
            ApplyPatchFileChange::Delete => FileChange::Delete,
            ApplyPatchFileChange::Update {
                unified_diff,
                move_path,
                new_content: _new_content,
            } => FileChange::Update {
                unified_diff: unified_diff.clone(),
                move_path: move_path.clone(),
            },
        };
        result.insert(path.clone(), protocol_change);
    }
    result
}

fn apply_changes_from_apply_patch_and_report(
    changes: &HashMap<PathBuf, ApplyPatchFileChange>,
    stdout: &mut impl std::io::Write,
    stderr: &mut impl std::io::Write,
) -> std::io::Result<()> {
    match apply_changes_from_apply_patch(changes) {
        Ok(affected_paths) => {
            print_summary(&affected_paths, stdout)?;
        }
        Err(err) => {
            writeln!(stderr, "{err:?}")?;
        }
    }

    Ok(())
}

fn apply_changes_from_apply_patch(
    changes: &HashMap<PathBuf, ApplyPatchFileChange>,
) -> anyhow::Result<AffectedPaths> {
    let mut added: Vec<PathBuf> = Vec::new();
    let mut modified: Vec<PathBuf> = Vec::new();
    let mut deleted: Vec<PathBuf> = Vec::new();

    for (path, change) in changes {
        match change {
            ApplyPatchFileChange::Add { content } => {
                if let Some(parent) = path.parent() {
                    if !parent.as_os_str().is_empty() {
                        std::fs::create_dir_all(parent).with_context(|| {
                            format!("Failed to create parent directories for {}", path.display())
                        })?;
                    }
                }
                std::fs::write(path, content)
                    .with_context(|| format!("Failed to write file {}", path.display()))?;
                added.push(path.clone());
            }
            ApplyPatchFileChange::Delete => {
                std::fs::remove_file(path)
                    .with_context(|| format!("Failed to delete file {}", path.display()))?;
                deleted.push(path.clone());
            }
            ApplyPatchFileChange::Update {
                unified_diff: _unified_diff,
                move_path,
                new_content,
            } => {
                if let Some(move_path) = move_path {
                    if let Some(parent) = move_path.parent() {
                        if !parent.as_os_str().is_empty() {
                            std::fs::create_dir_all(parent).with_context(|| {
                                format!(
                                    "Failed to create parent directories for {}",
                                    move_path.display()
                                )
                            })?;
                        }
                    }

                    std::fs::rename(path, move_path)
                        .with_context(|| format!("Failed to rename file {}", path.display()))?;
                    std::fs::write(move_path, new_content)?;
                    modified.push(move_path.clone());
                    deleted.push(path.clone());
                } else {
                    std::fs::write(path, new_content)?;
                    modified.push(path.clone());
                }
            }
        }
    }

    Ok(AffectedPaths {
        added,
        modified,
        deleted,
    })
}

fn get_writable_roots() -> Vec<PathBuf> {
    let mut writable_roots = Vec::new();
    if cfg!(target_os = "macos") {
        // On macOS, $TMPDIR is private to the user.
        writable_roots.push(std::env::temp_dir());

        // Allow pyenv to update its shims directory. Without this, any tool
        // that happens to be managed by `pyenv` will fail with an error like:
        //
        //   pyenv: cannot rehash: $HOME/.pyenv/shims isn't writable
        //
        // which is emitted every time `pyenv` tries to run `rehash` (for
        // example, after installing a new Python package that drops an entry
        // point). Although the sandbox is intentionally read‑only by default,
        // writing to the user's local `pyenv` directory is safe because it
        // is already user‑writable and scoped to the current user account.
        if let Ok(home_dir) = std::env::var("HOME") {
            let pyenv_dir = PathBuf::from(home_dir).join(".pyenv");
            writable_roots.push(pyenv_dir);
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        writable_roots.push(cwd);
    }

    writable_roots
}

/// Exec output is a pre-serialized JSON payload
fn format_exec_output(output: &str, exit_code: i32, duration: std::time::Duration) -> String {
    #[derive(Serialize)]
    struct ExecMetadata {
        exit_code: i32,
        duration_seconds: f32,
    }

    #[derive(Serialize)]
    struct ExecOutput<'a> {
        output: &'a str,
        metadata: ExecMetadata,
    }

    // round to 1 decimal place
    let duration_seconds = ((duration.as_secs_f32()) * 10.0).round() / 10.0;

    let payload = ExecOutput {
        output,
        metadata: ExecMetadata {
            exit_code,
            duration_seconds,
        },
    };

    serde_json::to_string(&payload).expect("serialize ExecOutput")
}
