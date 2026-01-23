#![allow(dead_code)]

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::config::types::ProjectHookCommand;
use crate::config::types::ProjectHookEvent;
use crate::error::CodexErr;
use crate::error::Result;
use crate::error::SandboxErr;
use crate::exec::ExecParams;
use crate::exec::ExecToolCallOutput;
use crate::exec::process_exec_tool_call;
use crate::exec_env::create_env;
use crate::project_hooks::ProjectHook;
use crate::protocol::AskForApproval;
use crate::protocol::EventMsg;
use crate::protocol::ExecCommandBeginEvent;
use crate::protocol::ExecCommandEndEvent;
use crate::protocol::ExecCommandSource;
use crate::protocol::FileChange;
use crate::sandboxing::SandboxPermissions;
use crate::tools::format_exec_output_str;
use crate::user_notification::UserNotification;
use chrono::Local;
use codex_protocol::ThreadId;
use serde_json::Map;
use serde_json::Number;
use serde_json::Value;
use serde_json::json;
use shlex::split as shlex_split;
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use tracing::warn;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HookPermissionMode {
    Allow,
    Ask,
}

impl HookPermissionMode {
    fn as_str(self) -> &'static str {
        match self {
            HookPermissionMode::Allow => "allow",
            HookPermissionMode::Ask => "ask",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HookPermissionDecision {
    Allow,
    Deny,
    Ask,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HookDecision {
    Approve,
    Block,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct HookOutput {
    pub continue_processing: bool,
    pub suppress_output: bool,
    pub system_message: Option<String>,
    pub permission_decision: Option<HookPermissionDecision>,
    pub updated_input: Option<Value>,
    pub decision: Option<HookDecision>,
    pub reason: Option<String>,
}

impl Default for HookOutput {
    fn default() -> Self {
        Self {
            continue_processing: true,
            suppress_output: false,
            system_message: None,
            permission_decision: None,
            updated_input: None,
            decision: None,
            reason: None,
        }
    }
}

pub(crate) struct HookCommandContext<'a> {
    pub session: &'a Session,
    pub turn: &'a TurnContext,
    pub hook: &'a ProjectHook,
    pub payload: Value,
    pub hook_index: usize,
    pub source: &'a str,
    pub source_call_id: Option<&'a str>,
}

pub(crate) struct HookRunResult {
    pub hook_output: HookOutput,
    pub exec_output: ExecToolCallOutput,
    pub call_id: String,
}

pub(crate) struct ExecHookPayloadInput<'a> {
    pub event: ProjectHookEvent,
    pub call_id: &'a str,
    pub cwd: &'a Path,
    pub command: &'a [String],
    pub timeout_ms: Option<u64>,
    pub tool_name: Option<&'a str>,
    pub tool_use_id: Option<&'a str>,
    pub tool_result: Option<&'a Value>,
    pub output: Option<&'a ExecToolCallOutput>,
    pub changes: Option<&'a HashMap<PathBuf, FileChange>>,
}

pub(crate) fn hook_event_name(event: ProjectHookEvent) -> &'static str {
    match event {
        ProjectHookEvent::SessionStart => "SessionStart",
        ProjectHookEvent::SessionEnd => "SessionEnd",
        ProjectHookEvent::PreToolUse => "PreToolUse",
        ProjectHookEvent::PostToolUse => "PostToolUse",
        ProjectHookEvent::FileBeforeWrite => "FileBeforeWrite",
        ProjectHookEvent::FileAfterWrite => "FileAfterWrite",
        ProjectHookEvent::Stop => "Stop",
        ProjectHookEvent::SubagentStop => "SubagentStop",
        ProjectHookEvent::UserPromptSubmit => "UserPromptSubmit",
        ProjectHookEvent::PreCompact => "PreCompact",
        ProjectHookEvent::PostCompact => "PostCompact",
        ProjectHookEvent::Notification => "Notification",
    }
}

fn hook_event_slug(event: ProjectHookEvent) -> &'static str {
    match event {
        ProjectHookEvent::SessionStart => "session.start",
        ProjectHookEvent::SessionEnd => "session.end",
        ProjectHookEvent::PreToolUse => "tool.before",
        ProjectHookEvent::PostToolUse => "tool.after",
        ProjectHookEvent::FileBeforeWrite => "file.before_write",
        ProjectHookEvent::FileAfterWrite => "file.after_write",
        ProjectHookEvent::Stop => "stop",
        ProjectHookEvent::SubagentStop => "subagent.stop",
        ProjectHookEvent::UserPromptSubmit => "user.prompt_submit",
        ProjectHookEvent::PreCompact => "pre.compact",
        ProjectHookEvent::PostCompact => "post.compact",
        ProjectHookEvent::Notification => "notification",
    }
}

pub(crate) fn build_base_hook_payload(
    session_id: &ThreadId,
    transcript_path: Option<&Path>,
    cwd: &Path,
    approval_policy: AskForApproval,
    event: ProjectHookEvent,
) -> Map<String, Value> {
    let mut payload = Map::new();
    payload.insert(
        "session_id".to_string(),
        Value::String(session_id.to_string()),
    );
    payload.insert(
        "transcript_path".to_string(),
        transcript_path.map(path_to_value).unwrap_or(Value::Null),
    );
    payload.insert("cwd".to_string(), path_to_value(cwd));
    payload.insert(
        "permission_mode".to_string(),
        Value::String(permission_mode(approval_policy).as_str().to_string()),
    );
    payload.insert(
        "hook_event_name".to_string(),
        Value::String(hook_event_name(event).to_string()),
    );
    payload
}

pub(crate) fn build_exec_hook_payload(
    base: Map<String, Value>,
    input: ExecHookPayloadInput<'_>,
) -> Value {
    let mut payload = base;
    insert_event(&mut payload, input.event);
    payload.insert(
        "call_id".to_string(),
        Value::String(input.call_id.to_string()),
    );
    payload.insert("cwd".to_string(), path_to_value(input.cwd));
    payload.insert(
        "command".to_string(),
        Value::Array(input.command.iter().cloned().map(Value::String).collect()),
    );
    if let Some(timeout_ms) = input.timeout_ms {
        payload.insert(
            "timeout_ms".to_string(),
            Value::Number(Number::from(timeout_ms)),
        );
    }
    if let Some(tool_name) = input.tool_name {
        payload.insert(
            "tool_name".to_string(),
            Value::String(tool_name.to_string()),
        );
    }
    if let Some(tool_use_id) = input.tool_use_id {
        payload.insert(
            "tool_use_id".to_string(),
            Value::String(tool_use_id.to_string()),
        );
    }
    if let Some(tool_result) = input.tool_result {
        payload.insert("tool_result".to_string(), tool_result.clone());
    }
    if let Some(output) = input.output {
        payload.insert(
            "exit_code".to_string(),
            Value::Number(Number::from(output.exit_code as i64)),
        );
        payload.insert(
            "duration_ms".to_string(),
            Value::Number(Number::from(output.duration.as_millis() as u64)),
        );
        payload.insert("timed_out".to_string(), Value::Bool(output.timed_out));
        payload.insert(
            "stdout".to_string(),
            Value::String(output.stdout.text.clone()),
        );
        payload.insert(
            "stderr".to_string(),
            Value::String(output.stderr.text.clone()),
        );
        payload.insert("success".to_string(), Value::Bool(output.exit_code == 0));
    }
    if let Some(changes) = input.changes {
        match serde_json::to_value(changes) {
            Ok(value) => {
                payload.insert("changes".to_string(), value);
            }
            Err(err) => {
                warn!("failed to serialize hook changes: {err}");
            }
        }
    }
    Value::Object(payload)
}

pub(crate) fn build_user_prompt_hook_payload(
    base: Map<String, Value>,
    event: ProjectHookEvent,
    user_prompt: &str,
) -> Value {
    let mut payload = base;
    insert_event(&mut payload, event);
    payload.insert(
        "user_prompt".to_string(),
        Value::String(user_prompt.to_string()),
    );
    Value::Object(payload)
}

pub(crate) fn build_stop_hook_payload(
    base: Map<String, Value>,
    event: ProjectHookEvent,
    reason: &str,
    details: &Value,
) -> Value {
    let mut payload = base;
    insert_event(&mut payload, event);
    payload.insert("reason".to_string(), Value::String(reason.to_string()));
    payload.insert("details".to_string(), details.clone());
    Value::Object(payload)
}

pub(crate) fn build_compact_hook_payload(
    base: Map<String, Value>,
    event: ProjectHookEvent,
    reason: &str,
) -> Value {
    let mut payload = base;
    insert_event(&mut payload, event);
    payload.insert("reason".to_string(), Value::String(reason.to_string()));
    Value::Object(payload)
}

pub(crate) fn build_notification_hook_payload(
    base: Map<String, Value>,
    event: ProjectHookEvent,
    notification: &UserNotification,
) -> Value {
    let mut payload = base;
    insert_event(&mut payload, event);
    match serde_json::to_value(notification) {
        Ok(value) => {
            payload.insert("notification".to_string(), value);
        }
        Err(err) => {
            warn!("failed to serialize notification payload: {err}");
        }
    }
    Value::Object(payload)
}

pub(crate) fn parse_hook_output_from_exec(output: &ExecToolCallOutput) -> HookOutput {
    parse_hook_output_from_text(
        output.stdout.text.as_str(),
        output.stderr.text.as_str(),
        output.exit_code,
    )
}

pub(crate) fn parse_hook_output_from_text(
    stdout: &str,
    stderr: &str,
    exit_code: i32,
) -> HookOutput {
    let mut output = HookOutput::default();
    if let Some(value) = parse_json_value(stdout).or_else(|| parse_json_value(stderr)) {
        apply_hook_output_value(&mut output, &value);
    }

    if exit_code == 2 {
        output.continue_processing = false;
        if output.system_message.is_none() {
            let trimmed = stderr.trim();
            if !trimmed.is_empty() {
                output.system_message = Some(trimmed.to_string());
            }
        }
    }
    output
}

pub(crate) async fn run_hook_command(ctx: HookCommandContext<'_>) -> Result<HookRunResult> {
    let event = ctx.hook.event;
    let event_slug = hook_event_slug(event);
    let event_trigger = event_slug.replace('.', "_");
    let call_id = format!(
        "{source}_hook_{event_trigger}_{index}",
        source = ctx.source,
        index = ctx.hook_index
    );

    let command = hook_command_args(&ctx.hook.run);
    if command.is_empty() {
        return Err(CodexErr::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "hook command args are empty",
        )));
    }

    let mut env = create_env(&ctx.turn.shell_environment_policy);
    if let Some(overrides) = ctx.hook.env.as_ref() {
        env.extend(overrides.clone());
    }

    let payload_json = match serde_json::to_string(&ctx.payload) {
        Ok(payload) => payload,
        Err(err) => {
            warn!("failed to serialize hook payload: {err}");
            "{}".to_string()
        }
    };

    env.insert("CODE_HOOK_EVENT".to_string(), event_slug.to_string());
    env.insert("CODE_HOOK_TRIGGER".to_string(), event_trigger);
    env.insert("CODE_HOOK_CALL_ID".to_string(), call_id.clone());
    env.insert("CODE_HOOK_SUB_ID".to_string(), ctx.turn.sub_id.clone());
    env.insert("CODE_HOOK_INDEX".to_string(), ctx.hook_index.to_string());
    env.insert("CODE_HOOK_PAYLOAD".to_string(), payload_json);
    env.insert(
        "CODE_SESSION_CWD".to_string(),
        ctx.turn.cwd.to_string_lossy().into_owned(),
    );
    if let Some(name) = ctx.hook.name.as_deref() {
        env.insert("CODE_HOOK_NAME".to_string(), name.to_string());
    }
    if let Some(source_call_id) = ctx.source_call_id {
        env.insert(
            "CODE_HOOK_SOURCE_CALL_ID".to_string(),
            source_call_id.to_string(),
        );
    }

    let cwd = ctx
        .hook
        .resolved_cwd
        .as_ref()
        .map(codex_utils_absolute_path::AbsolutePathBuf::to_path_buf)
        .unwrap_or_else(|| ctx.turn.cwd.clone());

    let exec_params = ExecParams {
        command: command.clone(),
        cwd: cwd.clone(),
        expiration: ctx.hook.timeout_ms.into(),
        env,
        sandbox_permissions: SandboxPermissions::UseDefault,
        justification: None,
        arg0: None,
    };

    if !ctx.hook.run_in_background {
        let label = hook_display_label(event, ctx.hook.name.as_deref());
        ctx.session
            .send_event(
                ctx.turn,
                EventMsg::ExecCommandBegin(ExecCommandBeginEvent {
                    call_id: call_id.clone(),
                    process_id: None,
                    turn_id: ctx.turn.sub_id.clone(),
                    command: vec![label],
                    cwd: cwd.clone(),
                    parsed_cmd: Vec::new(),
                    source: ExecCommandSource::Agent,
                    interaction_input: None,
                }),
            )
            .await;
    }

    let exec_output = match process_exec_tool_call(
        exec_params,
        &ctx.turn.sandbox_policy,
        &ctx.turn.cwd,
        &ctx.turn.codex_linux_sandbox_exe,
        None,
    )
    .await
    {
        Ok(output) => output,
        Err(CodexErr::Sandbox(SandboxErr::Timeout { output }))
        | Err(CodexErr::Sandbox(SandboxErr::Denied { output })) => *output,
        Err(err) => return Err(err),
    };

    if !ctx.hook.run_in_background {
        let label = hook_display_label(event, ctx.hook.name.as_deref());
        ctx.session
            .send_event(
                ctx.turn,
                EventMsg::ExecCommandEnd(ExecCommandEndEvent {
                    call_id: call_id.clone(),
                    process_id: None,
                    turn_id: ctx.turn.sub_id.clone(),
                    command: vec![label],
                    cwd: cwd.clone(),
                    parsed_cmd: Vec::new(),
                    source: ExecCommandSource::Agent,
                    interaction_input: None,
                    stdout: exec_output.stdout.text.clone(),
                    stderr: exec_output.stderr.text.clone(),
                    aggregated_output: exec_output.aggregated_output.text.clone(),
                    exit_code: exec_output.exit_code,
                    duration: exec_output.duration,
                    formatted_output: format_exec_output_str(
                        &exec_output,
                        ctx.turn.truncation_policy,
                    ),
                }),
            )
            .await;
    }

    let hook_output = parse_hook_output_from_exec(&exec_output);
    append_hook_log(
        ctx.turn,
        ctx.hook,
        event,
        &call_id,
        ctx.source_call_id,
        &ctx.payload,
        &exec_output,
    );

    Ok(HookRunResult {
        hook_output,
        exec_output,
        call_id,
    })
}

fn apply_hook_output_value(output: &mut HookOutput, value: &Value) {
    let Some(root) = value.as_object() else {
        return;
    };

    if let Some(continue_processing) = root.get("continue").and_then(Value::as_bool) {
        output.continue_processing = continue_processing;
    }
    if let Some(suppress_output) = root.get("suppressOutput").and_then(Value::as_bool) {
        output.suppress_output = suppress_output;
    }
    if let Some(system_message) = root.get("systemMessage").and_then(Value::as_str) {
        output.system_message = Some(system_message.to_string());
    }

    if let Some(permission_value) = hook_specific_value(root, "permissionDecision") {
        output.permission_decision = parse_permission_decision(permission_value);
    }
    if let Some(updated_input) = hook_specific_value(root, "updatedInput") {
        output.updated_input = Some(updated_input.clone());
    }
    if let Some(decision) = root.get("decision").and_then(Value::as_str) {
        output.decision = parse_decision(decision);
    }
    if let Some(reason) = root.get("reason").and_then(Value::as_str) {
        output.reason = Some(reason.to_string());
    }
}

fn hook_specific_value<'a>(root: &'a Map<String, Value>, key: &str) -> Option<&'a Value> {
    root.get("hookSpecificOutput")
        .and_then(Value::as_object)
        .and_then(|hook_specific| hook_specific.get(key))
        .or_else(|| root.get(key))
}

fn parse_permission_decision(value: &Value) -> Option<HookPermissionDecision> {
    let decision = value.as_str()?.to_ascii_lowercase();
    match decision.as_str() {
        "allow" | "approve" => Some(HookPermissionDecision::Allow),
        "deny" | "block" => Some(HookPermissionDecision::Deny),
        "ask" | "confirm" => Some(HookPermissionDecision::Ask),
        _ => None,
    }
}

fn parse_decision(value: &str) -> Option<HookDecision> {
    match value.to_ascii_lowercase().as_str() {
        "approve" => Some(HookDecision::Approve),
        "block" => Some(HookDecision::Block),
        _ => None,
    }
}

fn parse_json_value(text: &str) -> Option<Value> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    serde_json::from_str(trimmed).ok()
}

fn insert_event(payload: &mut Map<String, Value>, event: ProjectHookEvent) {
    payload.insert(
        "event".to_string(),
        Value::String(hook_event_slug(event).to_string()),
    );
}

fn permission_mode(approval_policy: AskForApproval) -> HookPermissionMode {
    match approval_policy {
        AskForApproval::OnFailure | AskForApproval::Never => HookPermissionMode::Allow,
        AskForApproval::OnRequest | AskForApproval::UnlessTrusted => HookPermissionMode::Ask,
    }
}

fn path_to_value(path: &Path) -> Value {
    Value::String(path.to_string_lossy().into_owned())
}

fn hook_command_args(command: &ProjectHookCommand) -> Vec<String> {
    match command {
        ProjectHookCommand::List(args) => args.clone(),
        ProjectHookCommand::String(raw) => {
            shlex_split(raw).unwrap_or_else(|| raw.split_whitespace().map(str::to_string).collect())
        }
    }
}

fn hook_display_label(event: ProjectHookEvent, name: Option<&str>) -> String {
    let event_name = hook_event_name(event);
    match name {
        Some(name) if !name.is_empty() => format!("Hook: {event_name} ({name})"),
        _ => format!("Hook: {event_name}"),
    }
}

fn append_hook_log(
    turn: &TurnContext,
    hook: &ProjectHook,
    event: ProjectHookEvent,
    call_id: &str,
    source_call_id: Option<&str>,
    payload: &Value,
    exec_output: &ExecToolCallOutput,
) {
    let timestamp = Local::now().to_rfc3339();
    let entry = json!({
        "timestamp": timestamp,
        "hook_event": hook_event_slug(event),
        "hook_event_name": hook_event_name(event),
        "hook_name": hook.name,
        "call_id": call_id,
        "source_call_id": source_call_id,
        "context": payload,
        "exit_code": exec_output.exit_code,
        "timed_out": exec_output.timed_out,
    });

    let entry_text = match serde_json::to_string(&entry) {
        Ok(text) => text,
        Err(err) => {
            warn!("failed to serialize hook log entry: {err}");
            return;
        }
    };

    let log_path = hook_log_path(turn);
    if let Some(parent) = log_path.parent()
        && let Err(err) = std::fs::create_dir_all(parent)
    {
        warn!(
            "failed to create hook log directory {}: {err}",
            parent.display()
        );
        return;
    }

    match OpenOptions::new().create(true).append(true).open(&log_path) {
        Ok(mut file) => {
            if let Err(err) = writeln!(file, "{entry_text}") {
                warn!("failed to write hook log entry: {err}");
            }
        }
        Err(err) => {
            warn!("failed to open hook log file {}: {err}", log_path.display());
        }
    }
}

fn hook_log_path(turn: &TurnContext) -> PathBuf {
    let base = std::env::var("CODE_SESSION_CWD")
        .map(PathBuf::from)
        .or_else(|_| std::env::current_dir())
        .unwrap_or_else(|_| turn.cwd.clone());
    base.join(".codex").join("logs").join("hooks.log")
}

/// Context for running exec event hooks (tool before/after, file before/after).
pub(crate) struct ExecHookContext<'a> {
    pub session: &'a Session,
    pub turn: &'a TurnContext,
    pub call_id: &'a str,
    pub tool_name: Option<&'a str>,
    pub tool_use_id: Option<&'a str>,
}

pub(crate) struct ToolHookContext<'a> {
    pub session: &'a Session,
    pub turn: &'a TurnContext,
    pub call_id: &'a str,
    pub tool_name: &'a str,
    pub command: &'a [String],
    pub cwd: &'a Path,
    pub timeout_ms: Option<u64>,
}

/// Result from running exec hooks.
#[derive(Debug, Clone, Default)]
pub(crate) struct ExecHookResult {
    pub continue_processing: bool,
    pub permission_decision: Option<HookPermissionDecision>,
    pub updated_input: Option<Value>,
    pub system_message: Option<String>,
}

impl ExecHookResult {
    fn from_hook_output(output: &HookOutput) -> Self {
        Self {
            continue_processing: output.continue_processing,
            permission_decision: output.permission_decision,
            updated_input: output.updated_input.clone(),
            system_message: output.system_message.clone(),
        }
    }

    fn merge(&mut self, other: &HookOutput) {
        // If any hook says stop, stop
        if !other.continue_processing {
            self.continue_processing = false;
        }
        // Last permission decision wins
        if other.permission_decision.is_some() {
            self.permission_decision = other.permission_decision;
        }
        // Last updated input wins
        if other.updated_input.is_some() {
            self.updated_input = other.updated_input.clone();
        }
        // Append system messages
        if let Some(msg) = &other.system_message {
            match &mut self.system_message {
                Some(existing) => {
                    existing.push('\n');
                    existing.push_str(msg);
                }
                None => {
                    self.system_message = Some(msg.clone());
                }
            }
        }
    }
}

/// Convenience function to run tool.before hooks.
pub(crate) async fn run_pre_tool_hooks(ctx: &ToolHookContext<'_>) -> Result<ExecHookResult> {
    let exec_ctx = ExecHookContext {
        session: ctx.session,
        turn: ctx.turn,
        call_id: ctx.call_id,
        tool_name: Some(ctx.tool_name),
        tool_use_id: Some(ctx.call_id),
    };
    let input = ExecHookPayloadInput {
        event: ProjectHookEvent::PreToolUse,
        call_id: ctx.call_id,
        cwd: ctx.cwd,
        command: ctx.command,
        timeout_ms: ctx.timeout_ms,
        tool_name: Some(ctx.tool_name),
        tool_use_id: Some(ctx.call_id),
        tool_result: None,
        output: None,
        changes: None,
    };
    run_hooks_for_exec_event(&exec_ctx, ProjectHookEvent::PreToolUse, input).await
}

/// Convenience function to run tool.after hooks.
pub(crate) async fn run_post_tool_hooks(
    ctx: &ToolHookContext<'_>,
    output: &ExecToolCallOutput,
) -> Result<ExecHookResult> {
    let exec_ctx = ExecHookContext {
        session: ctx.session,
        turn: ctx.turn,
        call_id: ctx.call_id,
        tool_name: Some(ctx.tool_name),
        tool_use_id: Some(ctx.call_id),
    };
    let input = ExecHookPayloadInput {
        event: ProjectHookEvent::PostToolUse,
        call_id: ctx.call_id,
        cwd: ctx.cwd,
        command: ctx.command,
        timeout_ms: ctx.timeout_ms,
        tool_name: Some(ctx.tool_name),
        tool_use_id: Some(ctx.call_id),
        tool_result: None,
        output: Some(output),
        changes: None,
    };
    run_hooks_for_exec_event(&exec_ctx, ProjectHookEvent::PostToolUse, input).await
}

/// Convenience function to run file.before_write hooks.
pub(crate) async fn run_file_before_write_hooks(
    session: &Session,
    turn: &TurnContext,
    call_id: &str,
    tool_name: &str,
    cwd: &Path,
    changes: &HashMap<PathBuf, FileChange>,
) -> Result<ExecHookResult> {
    let ctx = ExecHookContext {
        session,
        turn,
        call_id,
        tool_name: Some(tool_name),
        tool_use_id: Some(call_id),
    };
    let command: Vec<String> = vec![];
    let input = ExecHookPayloadInput {
        event: ProjectHookEvent::FileBeforeWrite,
        call_id,
        cwd,
        command: &command,
        timeout_ms: None,
        tool_name: Some(tool_name),
        tool_use_id: Some(call_id),
        tool_result: None,
        output: None,
        changes: Some(changes),
    };
    run_hooks_for_exec_event(&ctx, ProjectHookEvent::FileBeforeWrite, input).await
}

/// Convenience function to run file.after_write hooks.
pub(crate) async fn run_file_after_write_hooks(
    session: &Session,
    turn: &TurnContext,
    call_id: &str,
    tool_name: &str,
    cwd: &Path,
    changes: &HashMap<PathBuf, FileChange>,
    output: &ExecToolCallOutput,
) -> Result<ExecHookResult> {
    let ctx = ExecHookContext {
        session,
        turn,
        call_id,
        tool_name: Some(tool_name),
        tool_use_id: Some(call_id),
    };
    let command: Vec<String> = vec![];
    let input = ExecHookPayloadInput {
        event: ProjectHookEvent::FileAfterWrite,
        call_id,
        cwd,
        command: &command,
        timeout_ms: None,
        tool_name: Some(tool_name),
        tool_use_id: Some(call_id),
        tool_result: None,
        output: Some(output),
        changes: Some(changes),
    };
    run_hooks_for_exec_event(&ctx, ProjectHookEvent::FileAfterWrite, input).await
}

/// Run hooks for an exec event (tool.before, tool.after, file.before_write, file.after_write).
pub(crate) async fn run_hooks_for_exec_event(
    ctx: &ExecHookContext<'_>,
    event: ProjectHookEvent,
    input: ExecHookPayloadInput<'_>,
) -> Result<ExecHookResult> {
    let hooks = ctx.session.get_hooks_for_event(event).await;

    if hooks.is_empty() {
        return Ok(ExecHookResult {
            continue_processing: true,
            ..Default::default()
        });
    }

    let base_payload = build_base_hook_payload(
        &ctx.session.conversation_id(),
        ctx.session.rollout_path().await.as_deref(),
        &ctx.turn.cwd,
        ctx.turn.approval_policy,
        event,
    );
    let payload = build_exec_hook_payload(base_payload, input);

    let mut result = ExecHookResult {
        continue_processing: true,
        ..Default::default()
    };

    for (index, hook) in hooks.iter().enumerate() {
        let hook_ctx = HookCommandContext {
            session: ctx.session,
            turn: ctx.turn,
            hook,
            payload: payload.clone(),
            hook_index: index,
            source: ctx.call_id,
            source_call_id: Some(ctx.call_id),
        };

        match run_hook_command(hook_ctx).await {
            Ok(run_result) => {
                if index == 0 {
                    result = ExecHookResult::from_hook_output(&run_result.hook_output);
                } else {
                    result.merge(&run_result.hook_output);
                }

                // Exit code 2 stops further hook processing
                if !run_result.hook_output.continue_processing {
                    break;
                }
            }
            Err(err) => {
                warn!("hook execution failed: {err}");
            }
        }
    }

    Ok(result)
}

/// Result from running user prompt submit hooks.
#[derive(Debug, Clone, Default)]
pub(crate) struct UserPromptHookResult {
    pub continue_processing: bool,
    pub updated_prompt: Option<String>,
    pub system_message: Option<String>,
}

/// Run user.prompt_submit hooks before spawning a task.
/// Returns the (potentially rewritten) prompt and whether to continue.
pub(crate) async fn run_user_prompt_submit_hooks(
    session: &Session,
    turn: &TurnContext,
    user_prompt: &str,
) -> Result<UserPromptHookResult> {
    let hooks = session
        .get_hooks_for_event(ProjectHookEvent::UserPromptSubmit)
        .await;

    if hooks.is_empty() {
        return Ok(UserPromptHookResult {
            continue_processing: true,
            updated_prompt: None,
            system_message: None,
        });
    }

    let base_payload = build_base_hook_payload(
        &session.conversation_id(),
        session.rollout_path().await.as_deref(),
        &turn.cwd,
        turn.approval_policy,
        ProjectHookEvent::UserPromptSubmit,
    );
    let payload = build_user_prompt_hook_payload(
        base_payload,
        ProjectHookEvent::UserPromptSubmit,
        user_prompt,
    );

    let mut result = UserPromptHookResult {
        continue_processing: true,
        updated_prompt: None,
        system_message: None,
    };

    for (index, hook) in hooks.iter().enumerate() {
        let hook_ctx = HookCommandContext {
            session,
            turn,
            hook,
            payload: payload.clone(),
            hook_index: index,
            source: "prompt_submit",
            source_call_id: None,
        };

        match run_hook_command(hook_ctx).await {
            Ok(run_result) => {
                // Handle continue_processing
                if !run_result.hook_output.continue_processing {
                    result.continue_processing = false;
                }

                // Handle updated input (prompt rewrite)
                if let Some(updated) = &run_result.hook_output.updated_input {
                    if let Some(new_prompt) = updated.as_str() {
                        result.updated_prompt = Some(new_prompt.to_string());
                    } else if let Some(obj) = updated.as_object() {
                        // Also support {"prompt": "..."} format
                        if let Some(prompt_val) = obj.get("prompt").and_then(Value::as_str) {
                            result.updated_prompt = Some(prompt_val.to_string());
                        }
                    }
                }

                // Append system messages
                if let Some(msg) = &run_result.hook_output.system_message {
                    match &mut result.system_message {
                        Some(existing) => {
                            existing.push('\n');
                            existing.push_str(msg);
                        }
                        None => {
                            result.system_message = Some(msg.clone());
                        }
                    }
                }

                // Exit code 2 stops further hook processing
                if !run_result.hook_output.continue_processing {
                    break;
                }
            }
            Err(err) => {
                warn!("user prompt submit hook execution failed: {err}");
            }
        }
    }

    Ok(result)
}

/// Run session.start hooks once a session is initialized.
pub(crate) async fn run_session_start_hooks(session: &Session, turn: &TurnContext) -> Result<()> {
    let hooks = session
        .get_hooks_for_event(ProjectHookEvent::SessionStart)
        .await;

    if hooks.is_empty() {
        return Ok(());
    }

    let mut base_payload = build_base_hook_payload(
        &session.conversation_id(),
        session.rollout_path().await.as_deref(),
        &turn.cwd,
        turn.approval_policy,
        ProjectHookEvent::SessionStart,
    );
    insert_event(&mut base_payload, ProjectHookEvent::SessionStart);
    let payload = Value::Object(base_payload);

    for (index, hook) in hooks.iter().enumerate() {
        let hook_ctx = HookCommandContext {
            session,
            turn,
            hook,
            payload: payload.clone(),
            hook_index: index,
            source: "session_start",
            source_call_id: None,
        };

        if let Err(err) = run_hook_command(hook_ctx).await {
            warn!("session start hook execution failed: {err}");
        }
    }

    Ok(())
}

/// Run session.end hooks when a session is shutting down.
pub(crate) async fn run_session_end_hooks(session: &Session, turn: &TurnContext) -> Result<()> {
    let hooks = session
        .get_hooks_for_event(ProjectHookEvent::SessionEnd)
        .await;

    if hooks.is_empty() {
        return Ok(());
    }

    let mut base_payload = build_base_hook_payload(
        &session.conversation_id(),
        session.rollout_path().await.as_deref(),
        &turn.cwd,
        turn.approval_policy,
        ProjectHookEvent::SessionEnd,
    );
    insert_event(&mut base_payload, ProjectHookEvent::SessionEnd);
    let payload = Value::Object(base_payload);

    for (index, hook) in hooks.iter().enumerate() {
        let hook_ctx = HookCommandContext {
            session,
            turn,
            hook,
            payload: payload.clone(),
            hook_index: index,
            source: "session_end",
            source_call_id: None,
        };

        if let Err(err) = run_hook_command(hook_ctx).await {
            warn!("session end hook execution failed: {err}");
        }
    }

    Ok(())
}

/// Result from running stop hooks.
#[derive(Debug, Clone, Default)]
pub(crate) struct StopHookResult {
    pub decision: Option<HookDecision>,
    pub reason: Option<String>,
}

/// Run stop hooks when a session/task is ending.
pub(crate) async fn run_stop_hooks(
    session: &Session,
    turn: &TurnContext,
    stop_reason: &str,
    details: &Value,
) -> Result<StopHookResult> {
    let hooks = session.get_hooks_for_event(ProjectHookEvent::Stop).await;

    if hooks.is_empty() {
        return Ok(StopHookResult::default());
    }

    let base_payload = build_base_hook_payload(
        &session.conversation_id(),
        session.rollout_path().await.as_deref(),
        &turn.cwd,
        turn.approval_policy,
        ProjectHookEvent::Stop,
    );
    let payload =
        build_stop_hook_payload(base_payload, ProjectHookEvent::Stop, stop_reason, details);

    let mut result = StopHookResult::default();

    for (index, hook) in hooks.iter().enumerate() {
        let hook_ctx = HookCommandContext {
            session,
            turn,
            hook,
            payload: payload.clone(),
            hook_index: index,
            source: "stop",
            source_call_id: None,
        };

        match run_hook_command(hook_ctx).await {
            Ok(run_result) => {
                // Last decision wins
                if run_result.hook_output.decision.is_some() {
                    result.decision = run_result.hook_output.decision;
                }
                if run_result.hook_output.reason.is_some() {
                    result.reason = run_result.hook_output.reason.clone();
                }

                // Exit code 2 stops further hook processing
                if !run_result.hook_output.continue_processing {
                    break;
                }
            }
            Err(err) => {
                warn!("stop hook execution failed: {err}");
            }
        }
    }

    Ok(result)
}

/// Run subagent.stop hooks when a subagent is ending.
pub(crate) async fn run_subagent_stop_hooks(
    session: &Session,
    turn: &TurnContext,
    stop_reason: &str,
    details: &Value,
) -> Result<StopHookResult> {
    let hooks = session
        .get_hooks_for_event(ProjectHookEvent::SubagentStop)
        .await;

    if hooks.is_empty() {
        return Ok(StopHookResult::default());
    }

    let base_payload = build_base_hook_payload(
        &session.conversation_id(),
        session.rollout_path().await.as_deref(),
        &turn.cwd,
        turn.approval_policy,
        ProjectHookEvent::SubagentStop,
    );
    let payload = build_stop_hook_payload(
        base_payload,
        ProjectHookEvent::SubagentStop,
        stop_reason,
        details,
    );

    let mut result = StopHookResult::default();

    for (index, hook) in hooks.iter().enumerate() {
        let hook_ctx = HookCommandContext {
            session,
            turn,
            hook,
            payload: payload.clone(),
            hook_index: index,
            source: "subagent_stop",
            source_call_id: None,
        };

        match run_hook_command(hook_ctx).await {
            Ok(run_result) => {
                // Last decision wins
                if run_result.hook_output.decision.is_some() {
                    result.decision = run_result.hook_output.decision;
                }
                if run_result.hook_output.reason.is_some() {
                    result.reason = run_result.hook_output.reason.clone();
                }

                // Exit code 2 stops further hook processing
                if !run_result.hook_output.continue_processing {
                    break;
                }
            }
            Err(err) => {
                warn!("subagent stop hook execution failed: {err}");
            }
        }
    }

    Ok(result)
}

/// Run pre.compact hooks before compaction.
pub(crate) async fn run_pre_compact_hooks(
    session: &Session,
    turn: &TurnContext,
    reason: &str,
) -> Result<()> {
    let hooks = session
        .get_hooks_for_event(ProjectHookEvent::PreCompact)
        .await;

    if hooks.is_empty() {
        return Ok(());
    }

    let base_payload = build_base_hook_payload(
        &session.conversation_id(),
        session.rollout_path().await.as_deref(),
        &turn.cwd,
        turn.approval_policy,
        ProjectHookEvent::PreCompact,
    );
    let payload = build_compact_hook_payload(base_payload, ProjectHookEvent::PreCompact, reason);

    for (index, hook) in hooks.iter().enumerate() {
        let hook_ctx = HookCommandContext {
            session,
            turn,
            hook,
            payload: payload.clone(),
            hook_index: index,
            source: "pre_compact",
            source_call_id: None,
        };

        if let Err(err) = run_hook_command(hook_ctx).await {
            warn!("pre compact hook execution failed: {err}");
        }
    }

    Ok(())
}

/// Run post.compact hooks after compaction.
pub(crate) async fn run_post_compact_hooks(
    session: &Session,
    turn: &TurnContext,
    reason: &str,
) -> Result<()> {
    let hooks = session
        .get_hooks_for_event(ProjectHookEvent::PostCompact)
        .await;

    if hooks.is_empty() {
        return Ok(());
    }

    let base_payload = build_base_hook_payload(
        &session.conversation_id(),
        session.rollout_path().await.as_deref(),
        &turn.cwd,
        turn.approval_policy,
        ProjectHookEvent::PostCompact,
    );
    let payload = build_compact_hook_payload(base_payload, ProjectHookEvent::PostCompact, reason);

    for (index, hook) in hooks.iter().enumerate() {
        let hook_ctx = HookCommandContext {
            session,
            turn,
            hook,
            payload: payload.clone(),
            hook_index: index,
            source: "post_compact",
            source_call_id: None,
        };

        if let Err(err) = run_hook_command(hook_ctx).await {
            warn!("post compact hook execution failed: {err}");
        }
    }

    Ok(())
}

/// Run notification hooks when a user notification is sent.
pub(crate) async fn run_notification_hooks(
    session: &Session,
    turn: &TurnContext,
    notification: &UserNotification,
) -> Result<()> {
    let hooks = session
        .get_hooks_for_event(ProjectHookEvent::Notification)
        .await;

    if hooks.is_empty() {
        return Ok(());
    }

    let base_payload = build_base_hook_payload(
        &session.conversation_id(),
        session.rollout_path().await.as_deref(),
        &turn.cwd,
        turn.approval_policy,
        ProjectHookEvent::Notification,
    );
    let payload =
        build_notification_hook_payload(base_payload, ProjectHookEvent::Notification, notification);

    for (index, hook) in hooks.iter().enumerate() {
        let hook_ctx = HookCommandContext {
            session,
            turn,
            hook,
            payload: payload.clone(),
            hook_index: index,
            source: "notification",
            source_call_id: None,
        };

        if let Err(err) = run_hook_command(hook_ctx).await {
            warn!("notification hook execution failed: {err}");
        }
    }

    Ok(())
}
