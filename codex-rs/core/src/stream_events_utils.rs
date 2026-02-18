use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use codex_protocol::config_types::ModeKind;
use codex_protocol::items::TurnItem;
use serde::Deserialize;
use tokio_util::sync::CancellationToken;

use crate::analytics_client::InvokeType;
use crate::analytics_client::SkillInvocation;
use crate::analytics_client::TrackEventsContext;
use crate::analytics_client::skill_id_for_local_skill;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::error::CodexErr;
use crate::error::Result;
use crate::function_tool::FunctionCallError;
use crate::git_info::collect_git_info;
use crate::git_info::get_git_repo_root;
use crate::parse_turn_item;
use crate::proposed_plan_parser::strip_proposed_plan_blocks;
use crate::skills::SkillMetadata;
use crate::tools::parallel::ToolCallRuntime;
use crate::tools::router::ToolRouter;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;
use futures::Future;
use tracing::debug;
use tracing::instrument;

/// Handle a completed output item from the model stream, recording it and
/// queuing any tool execution futures. This records items immediately so
/// history and rollout stay in sync even if the turn is later cancelled.
pub(crate) type InFlightFuture<'f> =
    Pin<Box<dyn Future<Output = Result<ResponseInputItem>> + Send + 'f>>;

#[derive(Default)]
pub(crate) struct OutputItemResult {
    pub last_agent_message: Option<String>,
    pub needs_follow_up: bool,
    pub tool_future: Option<InFlightFuture<'static>>,
}

#[derive(Clone)]
struct ImplicitSkillCandidate {
    invocation: SkillInvocation,
    skill_id: String,
}

#[derive(Default)]
struct ImplicitSkillDetector {
    by_scripts_dir: HashMap<PathBuf, ImplicitSkillCandidate>,
    by_skill_doc_path: HashMap<PathBuf, ImplicitSkillCandidate>,
}

pub(crate) struct ImplicitInvocationContext {
    detector: ImplicitSkillDetector,
    seen_skill_ids: HashSet<String>,
    tracking: TrackEventsContext,
}

#[derive(Deserialize)]
struct ShellCommandDetectionArgs {
    command: String,
    workdir: Option<String>,
}

#[derive(Deserialize)]
struct ExecCommandDetectionArgs {
    cmd: String,
    workdir: Option<String>,
}

pub(crate) async fn build_implicit_invocation_context(
    skills: Vec<SkillMetadata>,
    tracking: TrackEventsContext,
) -> Option<ImplicitInvocationContext> {
    if skills.is_empty() {
        return None;
    }

    let mut detector = ImplicitSkillDetector::default();
    for skill in skills {
        let invocation = SkillInvocation {
            skill_name: skill.name,
            skill_scope: skill.scope,
            skill_path: skill.path,
            invoke_type: InvokeType::Implicit,
        };
        let repo_root = get_git_repo_root(invocation.skill_path.as_path());
        let repo_url = if let Some(root) = repo_root.as_ref() {
            collect_git_info(root)
                .await
                .and_then(|info| info.repository_url)
        } else {
            None
        };
        let skill_id = skill_id_for_local_skill(
            repo_url.as_deref(),
            repo_root.as_deref(),
            invocation.skill_path.as_path(),
            invocation.skill_name.as_str(),
        );
        let candidate = ImplicitSkillCandidate {
            invocation,
            skill_id,
        };

        let skill_doc_path = normalize_path(candidate.invocation.skill_path.as_path());
        detector
            .by_skill_doc_path
            .insert(skill_doc_path, candidate.clone());

        if let Some(skill_dir) = candidate.invocation.skill_path.parent() {
            let scripts_dir = normalize_path(&skill_dir.join("scripts"));
            detector.by_scripts_dir.insert(scripts_dir, candidate);
        }
    }

    Some(ImplicitInvocationContext {
        detector,
        seen_skill_ids: HashSet::new(),
        tracking,
    })
}

pub(crate) struct HandleOutputCtx<'a> {
    pub sess: Arc<Session>,
    pub turn_context: Arc<TurnContext>,
    pub tool_runtime: ToolCallRuntime,
    pub cancellation_token: CancellationToken,
    pub implicit_invocation_context: Option<&'a mut ImplicitInvocationContext>,
}

#[instrument(level = "trace", skip_all)]
pub(crate) async fn handle_output_item_done(
    ctx: &mut HandleOutputCtx<'_>,
    item: ResponseItem,
    previously_active_item: Option<TurnItem>,
) -> Result<OutputItemResult> {
    let mut output = OutputItemResult::default();
    let plan_mode = ctx.turn_context.collaboration_mode.mode == ModeKind::Plan;

    match ToolRouter::build_tool_call(ctx.sess.as_ref(), item.clone()).await {
        // The model emitted a tool call; log it, persist the item immediately, and queue the tool execution.
        Ok(Some(call)) => {
            let payload_preview = call.payload.log_payload().into_owned();
            tracing::info!(
                thread_id = %ctx.sess.conversation_id,
                "ToolCall: {} {}",
                call.tool_name,
                payload_preview
            );

            maybe_emit_implicit_skill_invocation(ctx, &item).await;

            ctx.sess
                .record_conversation_items(&ctx.turn_context, std::slice::from_ref(&item))
                .await;

            let cancellation_token = ctx.cancellation_token.child_token();
            let tool_future: InFlightFuture<'static> = Box::pin(
                ctx.tool_runtime
                    .clone()
                    .handle_tool_call(call, cancellation_token),
            );

            output.needs_follow_up = true;
            output.tool_future = Some(tool_future);
        }
        // No tool call: convert messages/reasoning into turn items and mark them as complete.
        Ok(None) => {
            if let Some(turn_item) = handle_non_tool_response_item(&item, plan_mode).await {
                if previously_active_item.is_none() {
                    ctx.sess
                        .emit_turn_item_started(&ctx.turn_context, &turn_item)
                        .await;
                }

                ctx.sess
                    .emit_turn_item_completed(&ctx.turn_context, turn_item)
                    .await;
            }

            ctx.sess
                .record_conversation_items(&ctx.turn_context, std::slice::from_ref(&item))
                .await;
            let last_agent_message = last_assistant_message_from_item(&item, plan_mode);

            output.last_agent_message = last_agent_message;
        }
        // Guardrail: the model issued a LocalShellCall without an id; surface the error back into history.
        Err(FunctionCallError::MissingLocalShellCallId) => {
            let msg = "LocalShellCall without call_id or id";
            ctx.turn_context
                .otel_manager
                .log_tool_failed("local_shell", msg);
            tracing::error!(msg);

            let response = ResponseInputItem::FunctionCallOutput {
                call_id: String::new(),
                output: FunctionCallOutputPayload {
                    body: FunctionCallOutputBody::Text(msg.to_string()),
                    ..Default::default()
                },
            };
            ctx.sess
                .record_conversation_items(&ctx.turn_context, std::slice::from_ref(&item))
                .await;
            if let Some(response_item) = response_input_to_response_item(&response) {
                ctx.sess
                    .record_conversation_items(
                        &ctx.turn_context,
                        std::slice::from_ref(&response_item),
                    )
                    .await;
            }

            output.needs_follow_up = true;
        }
        // The tool request should be answered directly (or was denied); push that response into the transcript.
        Err(FunctionCallError::RespondToModel(message)) => {
            let response = ResponseInputItem::FunctionCallOutput {
                call_id: String::new(),
                output: FunctionCallOutputPayload {
                    body: FunctionCallOutputBody::Text(message),
                    ..Default::default()
                },
            };
            ctx.sess
                .record_conversation_items(&ctx.turn_context, std::slice::from_ref(&item))
                .await;
            if let Some(response_item) = response_input_to_response_item(&response) {
                ctx.sess
                    .record_conversation_items(
                        &ctx.turn_context,
                        std::slice::from_ref(&response_item),
                    )
                    .await;
            }

            output.needs_follow_up = true;
        }
        // A fatal error occurred; surface it back into history.
        Err(FunctionCallError::Fatal(message)) => {
            return Err(CodexErr::Fatal(message));
        }
    }

    Ok(output)
}

async fn maybe_emit_implicit_skill_invocation(ctx: &mut HandleOutputCtx<'_>, item: &ResponseItem) {
    let Some(implicit) = ctx.implicit_invocation_context.as_deref_mut() else {
        return;
    };
    let Some(candidate) =
        detect_implicit_skill_invocation(&implicit.detector, ctx.turn_context.as_ref(), item)
    else {
        return;
    };
    if !implicit.seen_skill_ids.insert(candidate.skill_id) {
        return;
    }

    let skill_name = candidate.invocation.skill_name.as_str();
    ctx.turn_context.otel_manager.counter(
        "codex.skill.injected",
        1,
        &[
            ("status", "ok"),
            ("skill", skill_name),
            ("invoke_type", "implicit"),
        ],
    );
    ctx.sess
        .services
        .analytics_events_client
        .track_skill_invocations(implicit.tracking.clone(), vec![candidate.invocation]);
}

fn detect_implicit_skill_invocation(
    detector: &ImplicitSkillDetector,
    turn_context: &TurnContext,
    item: &ResponseItem,
) -> Option<ImplicitSkillCandidate> {
    let ResponseItem::FunctionCall {
        name, arguments, ..
    } = item
    else {
        return None;
    };
    let (command, workdir) = parse_implicit_detection_command(name, arguments)?;
    let workdir = turn_context.resolve_path(workdir);
    let workdir = normalize_path(workdir.as_path());
    let tokens = tokenize_command(command.as_str());

    if let Some(candidate) = detect_skill_script_run(detector, tokens.as_slice(), workdir.as_path())
    {
        return Some(candidate);
    }

    if let Some(candidate) = detect_skill_doc_read(detector, tokens.as_slice(), workdir.as_path()) {
        return Some(candidate);
    }

    None
}

fn parse_implicit_detection_command(
    tool_name: &str,
    arguments: &str,
) -> Option<(String, Option<String>)> {
    match tool_name {
        "shell_command" => serde_json::from_str::<ShellCommandDetectionArgs>(arguments)
            .ok()
            .map(|args| (args.command, args.workdir)),
        "exec_command" => serde_json::from_str::<ExecCommandDetectionArgs>(arguments)
            .ok()
            .map(|args| (args.cmd, args.workdir)),
        _ => None,
    }
}

fn tokenize_command(command: &str) -> Vec<String> {
    shlex::split(command).unwrap_or_else(|| {
        command
            .split_whitespace()
            .map(std::string::ToString::to_string)
            .collect()
    })
}

fn script_run_token(tokens: &[String]) -> Option<&str> {
    const RUNNERS: [&str; 10] = [
        "python", "python3", "bash", "zsh", "sh", "node", "deno", "ruby", "perl", "pwsh",
    ];
    const SCRIPT_EXTENSIONS: [&str; 7] = [".py", ".sh", ".js", ".ts", ".rb", ".pl", ".ps1"];

    let Some(runner_token) = tokens.first() else {
        return None;
    };
    let runner = command_basename(runner_token).to_ascii_lowercase();
    let runner = runner.strip_suffix(".exe").unwrap_or(&runner);
    if !RUNNERS.contains(&runner) {
        return None;
    }

    let mut script_token: Option<&str> = None;
    for token in tokens.iter().skip(1) {
        if token == "--" {
            continue;
        }
        if token.starts_with('-') {
            continue;
        }
        script_token = Some(token.as_str());
        break;
    }
    let script_token = script_token?;
    if SCRIPT_EXTENSIONS
        .iter()
        .any(|extension| script_token.to_ascii_lowercase().ends_with(extension))
    {
        return Some(script_token);
    }

    None
}

fn detect_skill_script_run(
    detector: &ImplicitSkillDetector,
    tokens: &[String],
    workdir: &Path,
) -> Option<ImplicitSkillCandidate> {
    let script_token = script_run_token(tokens)?;
    let script_path = Path::new(script_token);
    let script_path = if script_path.is_absolute() {
        script_path.to_path_buf()
    } else {
        workdir.join(script_path)
    };
    let script_path = normalize_path(script_path.as_path());

    for ancestor in script_path.ancestors() {
        if let Some(candidate) = detector.by_scripts_dir.get(ancestor) {
            return Some(candidate.clone());
        }
    }

    None
}

fn detect_skill_doc_read(
    detector: &ImplicitSkillDetector,
    tokens: &[String],
    workdir: &Path,
) -> Option<ImplicitSkillCandidate> {
    if !command_reads_file(tokens) {
        return None;
    }

    for token in tokens.iter().skip(1) {
        if token.starts_with('-') {
            continue;
        }
        let path = Path::new(token);
        let candidate_path = if path.is_absolute() {
            normalize_path(path)
        } else {
            normalize_path(&workdir.join(path))
        };
        if let Some(candidate) = detector.by_skill_doc_path.get(&candidate_path) {
            return Some(candidate.clone());
        }
    }

    None
}

fn command_reads_file(tokens: &[String]) -> bool {
    const READERS: [&str; 8] = ["cat", "sed", "head", "tail", "less", "more", "bat", "awk"];
    let Some(program) = tokens.first() else {
        return false;
    };
    let program = command_basename(program).to_ascii_lowercase();
    READERS.contains(&program.as_str())
}

fn command_basename(command: &str) -> String {
    Path::new(command)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(command)
        .to_string()
}

fn normalize_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

pub(crate) async fn handle_non_tool_response_item(
    item: &ResponseItem,
    plan_mode: bool,
) -> Option<TurnItem> {
    debug!(?item, "Output item");

    match item {
        ResponseItem::Message { .. }
        | ResponseItem::Reasoning { .. }
        | ResponseItem::WebSearchCall { .. } => {
            let mut turn_item = parse_turn_item(item)?;
            if plan_mode && let TurnItem::AgentMessage(agent_message) = &mut turn_item {
                let combined = agent_message
                    .content
                    .iter()
                    .map(|entry| match entry {
                        codex_protocol::items::AgentMessageContent::Text { text } => text.as_str(),
                    })
                    .collect::<String>();
                let stripped = strip_proposed_plan_blocks(&combined);
                agent_message.content =
                    vec![codex_protocol::items::AgentMessageContent::Text { text: stripped }];
            }
            Some(turn_item)
        }
        ResponseItem::FunctionCallOutput { .. } | ResponseItem::CustomToolCallOutput { .. } => {
            debug!("unexpected tool output from stream");
            None
        }
        _ => None,
    }
}

pub(crate) fn last_assistant_message_from_item(
    item: &ResponseItem,
    plan_mode: bool,
) -> Option<String> {
    if let ResponseItem::Message { role, content, .. } = item
        && role == "assistant"
    {
        let combined = content
            .iter()
            .filter_map(|ci| match ci {
                codex_protocol::models::ContentItem::OutputText { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<String>();
        if combined.is_empty() {
            return None;
        }
        return if plan_mode {
            let stripped = strip_proposed_plan_blocks(&combined);
            (!stripped.trim().is_empty()).then_some(stripped)
        } else {
            Some(combined)
        };
    }
    None
}

pub(crate) fn response_input_to_response_item(input: &ResponseInputItem) -> Option<ResponseItem> {
    match input {
        ResponseInputItem::FunctionCallOutput { call_id, output } => {
            Some(ResponseItem::FunctionCallOutput {
                call_id: call_id.clone(),
                output: output.clone(),
            })
        }
        ResponseInputItem::CustomToolCallOutput { call_id, output } => {
            Some(ResponseItem::CustomToolCallOutput {
                call_id: call_id.clone(),
                output: output.clone(),
            })
        }
        ResponseInputItem::McpToolCallOutput { call_id, result } => {
            let output = match result {
                Ok(call_tool_result) => FunctionCallOutputPayload::from(call_tool_result),
                Err(err) => FunctionCallOutputPayload {
                    body: FunctionCallOutputBody::Text(err.clone()),
                    success: Some(false),
                },
            };
            Some(ResponseItem::FunctionCallOutput {
                call_id: call_id.clone(),
                output,
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::ImplicitSkillCandidate;
    use super::ImplicitSkillDetector;
    use super::InvokeType;
    use super::SkillInvocation;
    use super::detect_skill_doc_read;
    use super::detect_skill_script_run;
    use super::normalize_path;
    use super::script_run_token;
    use pretty_assertions::assert_eq;
    use std::collections::HashMap;
    use std::path::Path;
    use std::path::PathBuf;

    #[test]
    fn script_run_detection_matches_runner_plus_extension() {
        let tokens = vec![
            "python3".to_string(),
            "-u".to_string(),
            "scripts/fetch_comments.py".to_string(),
        ];

        assert_eq!(script_run_token(&tokens).is_some(), true);
    }

    #[test]
    fn script_run_detection_excludes_python_c() {
        let tokens = vec![
            "python3".to_string(),
            "-c".to_string(),
            "print(1)".to_string(),
        ];

        assert_eq!(script_run_token(&tokens).is_some(), false);
    }

    #[test]
    fn skill_doc_read_detection_matches_absolute_path() {
        let skill_doc_path = PathBuf::from("/tmp/skill-test/SKILL.md");
        let normalized_skill_doc_path = normalize_path(skill_doc_path.as_path());
        let invocation = SkillInvocation {
            skill_name: "test-skill".to_string(),
            skill_scope: codex_protocol::protocol::SkillScope::User,
            skill_path: skill_doc_path,
            invoke_type: InvokeType::Implicit,
        };
        let candidate = ImplicitSkillCandidate {
            invocation,
            skill_id: "skill-id".to_string(),
        };

        let detector = ImplicitSkillDetector {
            by_scripts_dir: HashMap::new(),
            by_skill_doc_path: HashMap::from([(normalized_skill_doc_path, candidate)]),
        };

        let tokens = vec![
            "cat".to_string(),
            "/tmp/skill-test/SKILL.md".to_string(),
            "|".to_string(),
            "head".to_string(),
        ];
        let found = detect_skill_doc_read(&detector, &tokens, Path::new("/tmp"));

        assert_eq!(
            found.map(|value| value.skill_id),
            Some("skill-id".to_string())
        );
    }

    #[test]
    fn skill_script_run_detection_matches_relative_path_from_skill_root() {
        let skill_doc_path = PathBuf::from("/tmp/skill-test/SKILL.md");
        let scripts_dir = normalize_path(Path::new("/tmp/skill-test/scripts"));
        let invocation = SkillInvocation {
            skill_name: "test-skill".to_string(),
            skill_scope: codex_protocol::protocol::SkillScope::User,
            skill_path: skill_doc_path,
            invoke_type: InvokeType::Implicit,
        };
        let candidate = ImplicitSkillCandidate {
            invocation,
            skill_id: "skill-id".to_string(),
        };

        let detector = ImplicitSkillDetector {
            by_scripts_dir: HashMap::from([(scripts_dir, candidate)]),
            by_skill_doc_path: HashMap::new(),
        };
        let tokens = vec![
            "python3".to_string(),
            "scripts/fetch_comments.py".to_string(),
        ];

        let found = detect_skill_script_run(&detector, &tokens, Path::new("/tmp/skill-test"));

        assert_eq!(
            found.map(|value| value.skill_id),
            Some("skill-id".to_string())
        );
    }

    #[test]
    fn skill_script_run_detection_matches_absolute_path_from_any_workdir() {
        let skill_doc_path = PathBuf::from("/tmp/skill-test/SKILL.md");
        let scripts_dir = normalize_path(Path::new("/tmp/skill-test/scripts"));
        let invocation = SkillInvocation {
            skill_name: "test-skill".to_string(),
            skill_scope: codex_protocol::protocol::SkillScope::User,
            skill_path: skill_doc_path,
            invoke_type: InvokeType::Implicit,
        };
        let candidate = ImplicitSkillCandidate {
            invocation,
            skill_id: "skill-id".to_string(),
        };

        let detector = ImplicitSkillDetector {
            by_scripts_dir: HashMap::from([(scripts_dir, candidate)]),
            by_skill_doc_path: HashMap::new(),
        };
        let tokens = vec![
            "python3".to_string(),
            "/tmp/skill-test/scripts/fetch_comments.py".to_string(),
        ];

        let found = detect_skill_script_run(&detector, &tokens, Path::new("/tmp/other"));

        assert_eq!(
            found.map(|value| value.skill_id),
            Some("skill-id".to_string())
        );
    }
}
