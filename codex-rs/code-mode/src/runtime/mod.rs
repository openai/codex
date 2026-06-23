mod callbacks;
mod globals;
mod module_loader;
mod timers;
mod value;

use std::collections::HashMap;
use std::fmt;
use std::sync::OnceLock;
use std::sync::mpsc as std_mpsc;
use std::thread;

use codex_code_mode_protocol::CodeModeToolKind;
use codex_code_mode_protocol::EnabledToolMetadata;
use codex_code_mode_protocol::ExecuteRequest;
use codex_code_mode_protocol::FunctionCallOutputContentItem;
use codex_code_mode_protocol::enabled_tool_metadata;
use codex_protocol::ToolName;
use serde_json::Value as JsonValue;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

const EXIT_SENTINEL: &str = "__codex_code_mode_exit__";

#[derive(Debug)]
pub(crate) enum RuntimeCommand {
    ToolResponse { id: String, result: JsonValue },
    ToolError { id: String, error_text: String },
    TimeoutFired { id: u64 },
    ObservePendingFrontier,
    Terminate,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum PendingRuntimeMode {
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "used by the stacked continuing cell actor")
    )]
    Continue,
    PauseUntilResumed,
}

#[derive(Debug)]
pub(crate) enum RuntimeControlCommand {
    Continue,
    Resume,
    Terminate,
}

#[derive(Debug)]
pub(crate) enum RuntimeEvent {
    Started,
    Pending,
    ContentItem(FunctionCallOutputContentItem),
    YieldRequested,
    ToolCall {
        id: String,
        name: ToolName,
        kind: CodeModeToolKind,
        input: Option<JsonValue>,
    },
    Notify {
        call_id: String,
        text: String,
    },
    Result {
        stored_value_writes: HashMap<String, JsonValue>,
        error_text: Option<String>,
    },
}

pub(crate) struct RuntimeThread {
    completion_rx: oneshot::Receiver<()>,
    join_handle: Option<thread::JoinHandle<()>>,
}

impl RuntimeThread {
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "used by the stacked continuing cell actor")
    )]
    pub(crate) async fn wait(&mut self) {
        let _ = (&mut self.completion_rx).await;
    }

    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "used by the stacked continuing cell actor")
    )]
    pub(crate) fn join_finished(&mut self) -> Result<(), RuntimeThreadError> {
        let Some(join_handle) = self.join_handle.take() else {
            return Ok(());
        };
        join_handle.join().map_err(|_| RuntimeThreadError::Panicked)
    }

    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "used by the stacked continuing cell actor")
    )]
    pub(crate) fn join_pending(&self) -> bool {
        self.join_handle.is_some()
    }

    pub(crate) fn detach(mut self) {
        drop(self.join_handle.take());
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeThreadError {
    Panicked,
}

impl fmt::Display for RuntimeThreadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Panicked => formatter.write_str("code mode runtime thread panicked"),
        }
    }
}

impl std::error::Error for RuntimeThreadError {}

struct RuntimeCompletionGuard(Option<oneshot::Sender<()>>);

impl Drop for RuntimeCompletionGuard {
    fn drop(&mut self) {
        if let Some(completion_tx) = self.0.take() {
            let _ = completion_tx.send(());
        }
    }
}

pub(crate) fn spawn_runtime_thread(run: impl FnOnce() + Send + 'static) -> RuntimeThread {
    let (completion_tx, completion_rx) = oneshot::channel();
    let join_handle = thread::spawn(move || {
        let _completion_guard = RuntimeCompletionGuard(Some(completion_tx));
        run();
    });
    RuntimeThread {
        completion_rx,
        join_handle: Some(join_handle),
    }
}

pub(crate) fn spawn_runtime(
    stored_values: HashMap<String, JsonValue>,
    request: ExecuteRequest,
    event_tx: mpsc::UnboundedSender<RuntimeEvent>,
    pending_mode: PendingRuntimeMode,
) -> Result<
    (
        std_mpsc::Sender<RuntimeCommand>,
        std_mpsc::Sender<RuntimeControlCommand>,
        v8::IsolateHandle,
    ),
    String,
> {
    let (command_tx, control_tx, isolate_handle, runtime_thread) =
        spawn_owned_runtime(stored_values, request, event_tx, pending_mode)?;
    runtime_thread.detach();
    Ok((command_tx, control_tx, isolate_handle))
}

pub(crate) fn spawn_owned_runtime(
    stored_values: HashMap<String, JsonValue>,
    request: ExecuteRequest,
    event_tx: mpsc::UnboundedSender<RuntimeEvent>,
    pending_mode: PendingRuntimeMode,
) -> Result<
    (
        std_mpsc::Sender<RuntimeCommand>,
        std_mpsc::Sender<RuntimeControlCommand>,
        v8::IsolateHandle,
        RuntimeThread,
    ),
    String,
> {
    initialize_v8()?;

    let (command_tx, command_rx) = std_mpsc::channel();
    let (control_tx, control_rx) = std_mpsc::channel();
    let runtime_command_tx = command_tx.clone();
    let (isolate_handle_tx, isolate_handle_rx) = std_mpsc::sync_channel(1);
    let enabled_tools = request
        .enabled_tools
        .iter()
        .map(enabled_tool_metadata)
        .collect::<Vec<_>>();
    let config = RuntimeConfig {
        tool_call_id: request.tool_call_id,
        enabled_tools,
        source: request.source,
        stored_values,
    };

    let runtime_thread = spawn_runtime_thread(move || {
        run_runtime(
            config,
            RuntimeChannels {
                event_tx,
                command_rx,
                control_rx,
                isolate_handle_tx,
                runtime_command_tx,
            },
            pending_mode,
        );
    });

    let isolate_handle = isolate_handle_rx
        .recv()
        .map_err(|_| "failed to initialize code mode runtime".to_string())?;
    Ok((command_tx, control_tx, isolate_handle, runtime_thread))
}

#[derive(Clone)]
struct RuntimeConfig {
    tool_call_id: String,
    enabled_tools: Vec<EnabledToolMetadata>,
    source: String,
    stored_values: HashMap<String, JsonValue>,
}

struct RuntimeChannels {
    event_tx: mpsc::UnboundedSender<RuntimeEvent>,
    command_rx: std_mpsc::Receiver<RuntimeCommand>,
    control_rx: std_mpsc::Receiver<RuntimeControlCommand>,
    isolate_handle_tx: std_mpsc::SyncSender<v8::IsolateHandle>,
    runtime_command_tx: std_mpsc::Sender<RuntimeCommand>,
}

pub(super) struct RuntimeState {
    event_tx: mpsc::UnboundedSender<RuntimeEvent>,
    pending_tool_calls: HashMap<String, v8::Global<v8::PromiseResolver>>,
    pending_timeouts: HashMap<u64, timers::ScheduledTimeout>,
    stored_values: HashMap<String, JsonValue>,
    stored_value_writes: HashMap<String, JsonValue>,
    enabled_tools: Vec<EnabledToolMetadata>,
    next_tool_call_id: u64,
    next_timeout_id: u64,
    tool_call_id: String,
    runtime_command_tx: std_mpsc::Sender<RuntimeCommand>,
    exit_requested: bool,
}

pub(super) enum CompletionState {
    Pending,
    Completed {
        stored_value_writes: HashMap<String, JsonValue>,
        error_text: Option<String>,
    },
}

fn initialize_v8() -> Result<(), String> {
    static PLATFORM: OnceLock<Result<v8::SharedRef<v8::Platform>, String>> = OnceLock::new();

    match PLATFORM.get_or_init(|| {
        v8::icu::set_common_data_77(deno_core_icudata::ICU_DATA)
            .map_err(|error_code| format!("failed to initialize ICU data: {error_code}"))?;
        let platform = v8::new_default_platform(0, false).make_shared();
        v8::V8::initialize_platform(platform.clone());
        v8::V8::initialize();
        Ok(platform)
    }) {
        Ok(_) => Ok(()),
        Err(error_text) => Err(error_text.clone()),
    }
}

fn run_runtime(config: RuntimeConfig, channels: RuntimeChannels, pending_mode: PendingRuntimeMode) {
    let RuntimeChannels {
        event_tx,
        command_rx,
        control_rx,
        isolate_handle_tx,
        runtime_command_tx,
    } = channels;
    let isolate = &mut v8::Isolate::new(v8::CreateParams::default());
    let isolate_handle = isolate.thread_safe_handle();
    if isolate_handle_tx.send(isolate_handle).is_err() {
        return;
    }
    isolate.set_host_import_module_dynamically_callback(module_loader::dynamic_import_callback);

    v8::scope!(let scope, isolate);
    let context = v8::Context::new(scope, Default::default());
    let scope = &mut v8::ContextScope::new(scope, context);

    scope.set_slot(RuntimeState {
        event_tx: event_tx.clone(),
        pending_tool_calls: HashMap::new(),
        pending_timeouts: HashMap::new(),
        stored_values: config.stored_values,
        stored_value_writes: HashMap::new(),
        enabled_tools: config.enabled_tools,
        next_tool_call_id: 1,
        next_timeout_id: 1,
        tool_call_id: config.tool_call_id,
        runtime_command_tx,
        exit_requested: false,
    });

    if let Err(error_text) = globals::install_globals(scope) {
        send_result(&event_tx, HashMap::new(), Some(error_text));
        return;
    }

    let _ = event_tx.send(RuntimeEvent::Started);

    let pending_promise = match module_loader::evaluate_main_module(scope, &config.source) {
        Ok(pending_promise) => pending_promise,
        Err(error_text) => {
            capture_scope_send_error(scope, &event_tx, Some(error_text));
            return;
        }
    };

    match module_loader::completion_state(scope, pending_promise.as_ref()) {
        CompletionState::Completed {
            stored_value_writes,
            error_text,
        } => {
            send_result(&event_tx, stored_value_writes, error_text);
            return;
        }
        CompletionState::Pending => {}
    }

    let mut pending_promise = pending_promise;
    while let Some(command) =
        next_runtime_command(&event_tx, &command_rx, &control_rx, pending_mode)
    {
        match command {
            RuntimeCommand::Terminate => break,
            RuntimeCommand::ToolResponse { id, result } => {
                if let Err(error_text) =
                    module_loader::resolve_tool_response(scope, &id, Ok(result))
                {
                    capture_scope_send_error(scope, &event_tx, Some(error_text));
                    return;
                }
            }
            RuntimeCommand::ToolError { id, error_text } => {
                if let Err(runtime_error) =
                    module_loader::resolve_tool_response(scope, &id, Err(error_text))
                {
                    capture_scope_send_error(scope, &event_tx, Some(runtime_error));
                    return;
                }
            }
            RuntimeCommand::TimeoutFired { id } => {
                if let Err(runtime_error) = timers::invoke_timeout_callback(scope, id) {
                    capture_scope_send_error(scope, &event_tx, Some(runtime_error));
                    return;
                }
            }
            RuntimeCommand::ObservePendingFrontier => {}
        }

        scope.perform_microtask_checkpoint();
        match module_loader::completion_state(scope, pending_promise.as_ref()) {
            CompletionState::Completed {
                stored_value_writes,
                error_text,
            } => {
                send_result(&event_tx, stored_value_writes, error_text);
                return;
            }
            CompletionState::Pending => {}
        }

        if let Some(promise) = pending_promise.as_ref() {
            let promise = v8::Local::new(scope, promise);
            if promise.state() != v8::PromiseState::Pending {
                pending_promise = None;
            }
        }
    }
}

fn next_runtime_command(
    event_tx: &mpsc::UnboundedSender<RuntimeEvent>,
    command_rx: &std_mpsc::Receiver<RuntimeCommand>,
    control_rx: &std_mpsc::Receiver<RuntimeControlCommand>,
    pending_mode: PendingRuntimeMode,
) -> Option<RuntimeCommand> {
    loop {
        match command_rx.try_recv() {
            Ok(command) => return Some(command),
            Err(std_mpsc::TryRecvError::Disconnected) => return None,
            Err(std_mpsc::TryRecvError::Empty) => {}
        }

        let _ = event_tx.send(RuntimeEvent::Pending);
        match pending_mode {
            PendingRuntimeMode::Continue => return command_rx.recv().ok(),
            PendingRuntimeMode::PauseUntilResumed => match control_rx.recv().ok()? {
                RuntimeControlCommand::Continue => return command_rx.recv().ok(),
                RuntimeControlCommand::Resume => continue,
                RuntimeControlCommand::Terminate => return Some(RuntimeCommand::Terminate),
            },
        }
    }
}

fn capture_scope_send_error(
    scope: &mut v8::PinScope<'_, '_>,
    event_tx: &mpsc::UnboundedSender<RuntimeEvent>,
    error_text: Option<String>,
) {
    let stored_value_writes = scope
        .get_slot::<RuntimeState>()
        .map(|state| state.stored_value_writes.clone())
        .unwrap_or_default();

    send_result(event_tx, stored_value_writes, error_text);
}

fn send_result(
    event_tx: &mpsc::UnboundedSender<RuntimeEvent>,
    stored_value_writes: HashMap<String, JsonValue>,
    error_text: Option<String>,
) {
    let _ = event_tx.send(RuntimeEvent::Result {
        stored_value_writes,
        error_text,
    });
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::time::Duration;

    use pretty_assertions::assert_eq;
    use tokio::sync::mpsc;

    use super::ExecuteRequest;
    use super::PendingRuntimeMode;
    use super::RuntimeCommand;
    use super::RuntimeControlCommand;
    use super::RuntimeEvent;
    use super::spawn_owned_runtime;
    use super::spawn_runtime;
    use crate::FunctionCallOutputContentItem;

    fn execute_request(source: &str) -> ExecuteRequest {
        ExecuteRequest {
            tool_call_id: "call_1".to_string(),
            enabled_tools: Vec::new(),
            source: source.to_string(),
            yield_time_ms: Some(1),
            max_output_tokens: None,
        }
    }

    #[tokio::test]
    async fn owned_runtime_joins_after_completion() {
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let (_runtime_tx, _runtime_control_tx, _runtime_terminate_handle, mut runtime_thread) =
            spawn_owned_runtime(
                HashMap::new(),
                execute_request(r#"text("runtime output");"#),
                event_tx,
                PendingRuntimeMode::Continue,
            )
            .unwrap();

        assert!(matches!(event_rx.recv().await, Some(RuntimeEvent::Started)));
        assert!(runtime_thread.join_pending());
        assert!(matches!(
            event_rx.recv().await,
            Some(RuntimeEvent::ContentItem(
                FunctionCallOutputContentItem::InputText { .. }
            ))
        ));
        assert!(matches!(
            event_rx.recv().await,
            Some(RuntimeEvent::Result { .. })
        ));
        assert!(event_rx.recv().await.is_none());

        runtime_thread.wait().await;
        assert_eq!(runtime_thread.join_finished(), Ok(()));
        assert!(!runtime_thread.join_pending());
    }

    #[tokio::test]
    async fn terminate_execution_stops_cpu_bound_module() {
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let (_runtime_tx, _runtime_control_tx, runtime_terminate_handle) = spawn_runtime(
            HashMap::new(),
            execute_request("while (true) {}"),
            event_tx,
            PendingRuntimeMode::Continue,
        )
        .unwrap();

        let started_event = tokio::time::timeout(Duration::from_secs(1), event_rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(started_event, RuntimeEvent::Started));

        assert!(runtime_terminate_handle.terminate_execution());

        let result_event = tokio::time::timeout(Duration::from_secs(1), event_rx.recv())
            .await
            .unwrap()
            .unwrap();
        let RuntimeEvent::Result { error_text, .. } = result_event else {
            panic!("expected runtime result after termination");
        };
        assert!(error_text.is_some());

        assert!(
            tokio::time::timeout(Duration::from_secs(1), event_rx.recv())
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn pending_mode_freezes_runtime_commands_until_resume() {
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let (runtime_tx, runtime_control_tx, _runtime_terminate_handle) = spawn_runtime(
            HashMap::new(),
            execute_request(
                r#"
await new Promise((resolve) => setTimeout(resolve, 60_000));
text("after");
await new Promise(() => {});
"#,
            ),
            event_tx,
            PendingRuntimeMode::PauseUntilResumed,
        )
        .unwrap();

        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(1), event_rx.recv())
                .await
                .unwrap()
                .unwrap(),
            RuntimeEvent::Started
        ));
        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(1), event_rx.recv())
                .await
                .unwrap()
                .unwrap(),
            RuntimeEvent::Pending
        ));

        runtime_tx
            .send(RuntimeCommand::TimeoutFired { id: 1 })
            .unwrap();
        assert!(
            tokio::time::timeout(Duration::from_secs(1), event_rx.recv())
                .await
                .is_err()
        );

        runtime_control_tx
            .send(RuntimeControlCommand::Resume)
            .unwrap();

        let content_event = tokio::time::timeout(Duration::from_secs(1), event_rx.recv())
            .await
            .unwrap()
            .unwrap();
        let RuntimeEvent::ContentItem(FunctionCallOutputContentItem::InputText { text }) =
            content_event
        else {
            panic!("expected resumed runtime output");
        };
        assert_eq!(text, "after");
        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(1), event_rx.recv())
                .await
                .unwrap()
                .unwrap(),
            RuntimeEvent::Pending
        ));

        runtime_control_tx
            .send(RuntimeControlCommand::Terminate)
            .unwrap();
    }
}
