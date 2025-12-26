use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use regex::Regex;
use serde_json::json;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tracing::debug;
use tracing::warn;

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::config::HookConfig;
use crate::git_info::resolve_root_git_project_for_trust;
use crate::protocol::EventMsg;
use codex_protocol::models::ResponseInputItem;

#[derive(Debug, Clone)]
struct CompiledHook {
    id: Option<String>,
    when: HashSet<String>,
    matcher: Option<Regex>,
    command: Vec<String>,
    timeout: Option<Duration>,
    include_output: bool,
    include_patch_contents: bool,
    include_mcp_arguments: bool,
}

#[derive(Debug, Default)]
pub(crate) struct HookRunner {
    hooks: Vec<CompiledHook>,
}

impl HookRunner {
    pub(crate) fn try_new(hooks: Vec<HookConfig>) -> anyhow::Result<Self> {
        let mut out = Vec::with_capacity(hooks.len());
        for (idx, hook) in hooks.into_iter().enumerate() {
            if hook.command.is_empty() {
                anyhow::bail!("hooks[{idx}].command must not be empty");
            }
            if hook.when.is_empty() {
                anyhow::bail!("hooks[{idx}].when must not be empty");
            }

            let matcher = match hook.matcher {
                Some(pattern) => Some(
                    Regex::new(&pattern)
                        .map_err(|e| anyhow::anyhow!("hooks[{idx}].matcher invalid regex: {e}"))?,
                ),
                None => None,
            };

            out.push(CompiledHook {
                id: hook.id,
                when: hook.when.into_iter().collect(),
                matcher,
                command: hook.command,
                timeout: hook.timeout_ms.map(Duration::from_millis),
                include_output: hook.include_output,
                include_patch_contents: hook.include_patch_contents,
                include_mcp_arguments: hook.include_mcp_arguments,
            });
        }
        debug!(hook_count = out.len(), "hooks configured");
        Ok(Self { hooks: out })
    }

    pub(crate) fn on_event(&self, sess: &Session, turn: &TurnContext, msg: &EventMsg) {
        let Some(kind) = hook_event_kind(msg) else {
            return;
        };

        let thread_id = sess.conversation_id().to_string();
        let hook_cwd =
            resolve_root_git_project_for_trust(&turn.cwd).unwrap_or_else(|| turn.cwd.clone());

        debug!(
            kind,
            hook_count = self.hooks.len(),
            cwd = %turn.cwd.display(),
            hook_cwd = %hook_cwd.display(),
            "hook event received"
        );

        for hook in &self.hooks {
            if !hook.when.contains(kind) {
                continue;
            }

            if let Some(matcher) = &hook.matcher
                && let Some(subject) = hook_matcher_subject(msg)
                && !matcher.is_match(subject)
            {
                continue;
            }

            let payload = match build_payload(&thread_id, turn, msg, hook, kind) {
                Ok(payload) => payload,
                Err(err) => {
                    warn!(
                        hook_id = hook.id.as_deref().unwrap_or("<none>"),
                        kind, "failed to build hook payload: {err:#}"
                    );
                    continue;
                }
            };

            let hook_id = hook.id.clone();
            let command = hook.command.clone();
            let timeout = hook.timeout;
            let kind = kind.to_string();
            let cwd = hook_cwd.clone();

            debug!(
                hook_id = hook_id.as_deref().unwrap_or("<none>"),
                kind,
                argv0 = command.first().map(String::as_str).unwrap_or("<empty>"),
                cwd = %cwd.display(),
                "spawning hook"
            );

            tokio::spawn(async move {
                if let Err(err) =
                    run_hook_command(hook_id.as_deref(), &kind, &command, &payload, &cwd, timeout)
                        .await
                {
                    warn!(
                        hook_id = hook_id.as_deref().unwrap_or("<none>"),
                        kind, "hook command failed: {err:#}"
                    );
                }
            });
        }
    }

    pub(crate) fn on_tool_call_begin(
        &self,
        sess: &Session,
        turn: &TurnContext,
        tool_name: &str,
        call_id: &str,
    ) {
        self.on_generic_tool_call_event(sess, turn, "tool.call.begin", tool_name, call_id, None);
    }

    pub(crate) fn on_tool_call_end(
        &self,
        sess: &Session,
        turn: &TurnContext,
        tool_name: &str,
        call_id: &str,
        response: &ResponseInputItem,
    ) {
        let (success, error) = tool_call_success_and_error(response);
        self.on_generic_tool_call_event(
            sess,
            turn,
            "tool.call.end",
            tool_name,
            call_id,
            Some((success, error)),
        );
    }

    pub(crate) fn on_tool_call_fatal(
        &self,
        sess: &Session,
        turn: &TurnContext,
        tool_name: &str,
        call_id: &str,
        error: &str,
    ) {
        self.on_generic_tool_call_event(
            sess,
            turn,
            "tool.call.end",
            tool_name,
            call_id,
            Some((Some(false), Some(error.to_string()))),
        );
    }

    fn on_generic_tool_call_event(
        &self,
        sess: &Session,
        turn: &TurnContext,
        kind: &'static str,
        tool_name: &str,
        call_id: &str,
        end_fields: Option<(Option<bool>, Option<String>)>,
    ) {
        let thread_id = sess.conversation_id().to_string();
        let hook_cwd =
            resolve_root_git_project_for_trust(&turn.cwd).unwrap_or_else(|| turn.cwd.clone());

        debug!(
            kind,
            tool_name,
            call_id,
            hook_count = self.hooks.len(),
            cwd = %turn.cwd.display(),
            hook_cwd = %hook_cwd.display(),
            "hook tool-call event received"
        );

        for hook in &self.hooks {
            if !hook.when.contains(kind) {
                continue;
            }

            if let Some(matcher) = &hook.matcher
                && !matcher.is_match(tool_name)
            {
                continue;
            }

            let mut payload = json!({
                "type": kind,
                "thread_id": &thread_id,
                "turn_id": &turn.sub_id,
                "cwd": &turn.cwd,
                "tool_name": tool_name,
                "call_id": call_id,
                "hook": {
                    "id": hook.id.as_deref(),
                }
            });

            if let Some((success, error)) = &end_fields
                && let Some(obj) = payload.as_object_mut()
            {
                obj.insert("success".to_string(), json!(success));
                if let Some(error) = error {
                    obj.insert("error".to_string(), json!(error));
                }
            }

            let payload = match serde_json::to_string(&payload) {
                Ok(payload) => payload,
                Err(err) => {
                    warn!(
                        hook_id = hook.id.as_deref().unwrap_or("<none>"),
                        kind, "failed to serialize hook payload: {err:#}"
                    );
                    continue;
                }
            };

            let hook_id = hook.id.clone();
            let command = hook.command.clone();
            let timeout = hook.timeout;
            let cwd = hook_cwd.clone();

            debug!(
                hook_id = hook_id.as_deref().unwrap_or("<none>"),
                kind,
                tool_name,
                call_id,
                argv0 = command.first().map(String::as_str).unwrap_or("<empty>"),
                cwd = %cwd.display(),
                "spawning hook"
            );

            tokio::spawn(async move {
                if let Err(err) =
                    run_hook_command(hook_id.as_deref(), kind, &command, &payload, &cwd, timeout)
                        .await
                {
                    warn!(
                        hook_id = hook_id.as_deref().unwrap_or("<none>"),
                        kind, "hook command failed: {err:#}"
                    );
                }
            });
        }
    }
}

fn tool_call_success_and_error(response: &ResponseInputItem) -> (Option<bool>, Option<String>) {
    match response {
        ResponseInputItem::FunctionCallOutput { output, .. } => {
            let success = output.success;
            let error = match success {
                Some(false) => Some(output.content.clone()),
                _ => None,
            };
            (success, error)
        }
        ResponseInputItem::McpToolCallOutput { result, .. } => match result {
            Ok(result) => (Some(!result.is_error.unwrap_or(false)), None),
            Err(err) => (Some(false), Some(err.clone())),
        },
        ResponseInputItem::CustomToolCallOutput { .. } => (None, None),
        _ => (None, None),
    }
}

fn hook_event_kind(msg: &EventMsg) -> Option<&'static str> {
    match msg {
        EventMsg::TaskStarted(_) => Some("turn.begin"),
        EventMsg::TaskComplete(_) => Some("turn.end"),
        EventMsg::ExecCommandBegin(_) => Some("tool.exec.begin"),
        EventMsg::ExecCommandEnd(_) => Some("tool.exec.end"),
        EventMsg::PatchApplyBegin(_) => Some("tool.apply_patch.begin"),
        EventMsg::PatchApplyEnd(_) => Some("tool.apply_patch.end"),
        EventMsg::McpToolCallBegin(_) => Some("tool.mcp.begin"),
        EventMsg::McpToolCallEnd(_) => Some("tool.mcp.end"),
        EventMsg::WebSearchBegin(_) => Some("web_search.begin"),
        EventMsg::WebSearchEnd(_) => Some("web_search.end"),
        _ => None,
    }
}

fn hook_matcher_subject(msg: &EventMsg) -> Option<&str> {
    match msg {
        EventMsg::ExecCommandBegin(e) => Some(exec_source_label(e.source)),
        EventMsg::ExecCommandEnd(e) => Some(exec_source_label(e.source)),
        EventMsg::PatchApplyBegin(_) | EventMsg::PatchApplyEnd(_) => Some("apply_patch"),
        EventMsg::McpToolCallBegin(e) => Some(e.invocation.tool.as_str()),
        EventMsg::McpToolCallEnd(e) => Some(e.invocation.tool.as_str()),
        _ => None,
    }
}

fn exec_source_label(source: crate::protocol::ExecCommandSource) -> &'static str {
    match source {
        crate::protocol::ExecCommandSource::Agent => "shell",
        crate::protocol::ExecCommandSource::UserShell => "user_shell",
        crate::protocol::ExecCommandSource::UnifiedExecStartup
        | crate::protocol::ExecCommandSource::UnifiedExecInteraction => "unified_exec",
    }
}

fn build_payload(
    thread_id: &str,
    turn: &TurnContext,
    msg: &EventMsg,
    hook: &CompiledHook,
    kind: &str,
) -> anyhow::Result<String> {
    let mut base = json!({
        "type": kind,
        "thread_id": thread_id,
        "turn_id": &turn.sub_id,
        "cwd": &turn.cwd,
        "hook": {
            "id": hook.id.as_deref(),
        }
    });

    let obj = base
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("hook payload must be an object"))?;

    match msg {
        EventMsg::TaskStarted(e) => {
            obj.insert(
                "model_context_window".to_string(),
                json!(e.model_context_window),
            );
        }
        EventMsg::TaskComplete(e) => {
            obj.insert(
                "last_agent_message".to_string(),
                json!(&e.last_agent_message),
            );
        }
        EventMsg::ExecCommandBegin(e) => {
            obj.insert("tool_name".to_string(), json!(exec_source_label(e.source)));
            obj.insert("call_id".to_string(), json!(&e.call_id));
            obj.insert("process_id".to_string(), json!(&e.process_id));
            obj.insert("command".to_string(), json!(&e.command));
            obj.insert("source".to_string(), json!(e.source));
            obj.insert("cwd".to_string(), json!(&e.cwd));
        }
        EventMsg::ExecCommandEnd(e) => {
            obj.insert("tool_name".to_string(), json!(exec_source_label(e.source)));
            obj.insert("call_id".to_string(), json!(&e.call_id));
            obj.insert("process_id".to_string(), json!(&e.process_id));
            obj.insert("command".to_string(), json!(&e.command));
            obj.insert("source".to_string(), json!(e.source));
            obj.insert("cwd".to_string(), json!(&e.cwd));
            obj.insert("exit_code".to_string(), json!(e.exit_code));
            obj.insert(
                "duration_ms".to_string(),
                json!(e.duration.as_millis().min(u128::from(u64::MAX)) as u64),
            );
            if hook.include_output {
                obj.insert("stdout".to_string(), json!(&e.stdout));
                obj.insert("stderr".to_string(), json!(&e.stderr));
                obj.insert("aggregated_output".to_string(), json!(&e.aggregated_output));
                obj.insert("formatted_output".to_string(), json!(&e.formatted_output));
            }
        }
        EventMsg::PatchApplyBegin(e) => {
            obj.insert("tool_name".to_string(), json!("apply_patch"));
            obj.insert("call_id".to_string(), json!(&e.call_id));
            obj.insert("auto_approved".to_string(), json!(e.auto_approved));
            obj.insert(
                "change_summaries".to_string(),
                json!(summarize_changes(&e.changes)),
            );
            if hook.include_patch_contents {
                obj.insert("changes".to_string(), json!(&e.changes));
            }
        }
        EventMsg::PatchApplyEnd(e) => {
            obj.insert("tool_name".to_string(), json!("apply_patch"));
            obj.insert("call_id".to_string(), json!(&e.call_id));
            obj.insert("success".to_string(), json!(e.success));
            obj.insert(
                "change_summaries".to_string(),
                json!(summarize_changes(&e.changes)),
            );
            if hook.include_patch_contents {
                obj.insert("changes".to_string(), json!(&e.changes));
            }
        }
        EventMsg::McpToolCallBegin(e) => {
            obj.insert("tool_name".to_string(), json!(&e.invocation.tool));
            obj.insert("call_id".to_string(), json!(&e.call_id));
            obj.insert("mcp_server".to_string(), json!(&e.invocation.server));
            if hook.include_mcp_arguments {
                obj.insert("arguments".to_string(), json!(&e.invocation.arguments));
            }
        }
        EventMsg::McpToolCallEnd(e) => {
            obj.insert("tool_name".to_string(), json!(&e.invocation.tool));
            obj.insert("call_id".to_string(), json!(&e.call_id));
            obj.insert("mcp_server".to_string(), json!(&e.invocation.server));
            obj.insert(
                "duration_ms".to_string(),
                json!(e.duration.as_millis().min(u128::from(u64::MAX)) as u64),
            );
            obj.insert("success".to_string(), json!(e.is_success()));
            if hook.include_mcp_arguments {
                obj.insert("arguments".to_string(), json!(&e.invocation.arguments));
            }
            if let Err(err) = &e.result {
                obj.insert("error".to_string(), json!(err));
            }
        }
        EventMsg::WebSearchBegin(e) => {
            obj.insert("call_id".to_string(), json!(&e.call_id));
        }
        EventMsg::WebSearchEnd(e) => {
            obj.insert("call_id".to_string(), json!(&e.call_id));
            obj.insert("query".to_string(), json!(&e.query));
        }
        _ => anyhow::bail!("unexpected event for kind={kind}"),
    }

    Ok(serde_json::to_string(&base)?)
}

fn summarize_changes(
    changes: &std::collections::HashMap<std::path::PathBuf, crate::protocol::FileChange>,
) -> Vec<serde_json::Value> {
    let mut out = Vec::with_capacity(changes.len());
    for (path, change) in changes {
        let action = match change {
            crate::protocol::FileChange::Add { .. } => "add",
            crate::protocol::FileChange::Delete { .. } => "delete",
            crate::protocol::FileChange::Update { .. } => "update",
        };
        out.push(json!({ "path": path, "action": action }));
    }
    out
}

async fn run_hook_command(
    hook_id: Option<&str>,
    kind: &str,
    command: &[String],
    payload: &str,
    cwd: &PathBuf,
    timeout: Option<Duration>,
) -> anyhow::Result<()> {
    let mut cmd = Command::new(&command[0]);
    if command.len() > 1 {
        cmd.args(&command[1..]);
    }
    cmd.current_dir(cwd);
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| {
        anyhow::anyhow!(
            "failed to spawn hook command (id={}, kind={}, argv0={}): {e}",
            hook_id.unwrap_or("<none>"),
            kind,
            command[0]
        )
    })?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(payload.as_bytes()).await?;
    }

    let output = if let Some(timeout) = timeout {
        tokio::time::timeout(timeout, child.wait_with_output())
            .await
            .map_err(|_| {
                anyhow::anyhow!(
                    "hook timeout after {}ms (id={}, kind={}, argv0={})",
                    timeout.as_millis(),
                    hook_id.unwrap_or("<none>"),
                    kind,
                    command[0]
                )
            })??
    } else {
        child.wait_with_output().await?
    };

    if output.status.success() {
        return Ok(());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    anyhow::bail!(
        "hook exited non-zero (status={:?}); stdout={} stderr={}",
        output.status.code(),
        truncate(&stdout, 8 * 1024),
        truncate(&stderr, 8 * 1024),
    );
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut out = s[..max].to_string();
    out.push_str("…(truncated)");
    out
}
