use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ShellCommandToolCallParams;
use codex_protocol::models::ShellToolCallParams;
use codex_protocol::parse_command::ParsedCommand;
use serde::Deserialize;

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::connectors;
use crate::features::Feature;
use crate::mentions::build_connector_slug_counts;
use crate::mentions::build_skill_name_counts;
use crate::parse_command::parse_command;
use crate::shell::Shell;
use crate::shell::empty_shell_snapshot_receiver;
use crate::shell::get_shell_by_model_provided_path;
use crate::skills::SkillMetadata;
use crate::skills::injection::MentionRewriteContext;
use crate::skills::injection::build_mention_rewrite_context;
use crate::skills::injection::rewrite_text_mentions;
use crate::tools::context::ToolPayload;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MentionRewriteOutputKind {
    ReadFile,
    ShellFreeform,
    ShellStructured,
    PlainText,
}

#[derive(Deserialize)]
struct ReadFileToolArgs {
    file_path: String,
}

#[derive(Deserialize)]
struct UnifiedExecCommandArgs {
    cmd: String,
    #[serde(default)]
    workdir: Option<String>,
    #[serde(default)]
    shell: Option<String>,
    #[serde(default = "default_login")]
    login: bool,
}

pub(crate) async fn mention_rewrite_context_for_tool_call(
    session: &Session,
    turn: &TurnContext,
    tool_name: &str,
    payload: &ToolPayload,
) -> Option<MentionRewriteContext> {
    let read_paths = read_paths_for_tool_call(session, turn, tool_name, payload);
    mention_rewrite_context_for_read_paths(session, turn, read_paths).await
}

pub(crate) async fn mention_rewrite_context_for_read_paths(
    session: &Session,
    turn: &TurnContext,
    read_paths: Vec<PathBuf>,
) -> Option<MentionRewriteContext> {
    if read_paths.is_empty() {
        return None;
    }

    let skills_outcome = session
        .services
        .skills_manager
        .skills_for_cwd(&turn.cwd, false)
        .await;
    if skills_outcome.skills.is_empty() {
        return None;
    }

    let canonical_paths = read_paths
        .into_iter()
        .map(|path| dunce::canonicalize(&path).unwrap_or(path))
        .collect::<Vec<_>>();
    if !canonical_paths
        .iter()
        .any(|path| should_rewrite_mentions_for_path(path, &skills_outcome.skills))
    {
        return None;
    }

    let (skill_name_counts, skill_name_counts_lower) =
        build_skill_name_counts(&skills_outcome.skills, &skills_outcome.disabled_paths);
    let connectors = if turn.config.features.enabled(Feature::Apps) {
        let mcp_tools = session
            .services
            .mcp_connection_manager
            .read()
            .await
            .list_all_tools()
            .await;
        connectors::accessible_connectors_from_mcp_tools(&mcp_tools)
    } else {
        Vec::new()
    };
    let connector_slug_counts = build_connector_slug_counts(&connectors);

    Some(build_mention_rewrite_context(
        &skills_outcome.skills,
        &skills_outcome.disabled_paths,
        &skill_name_counts,
        &skill_name_counts_lower,
        &connector_slug_counts,
        &connectors,
    ))
}

pub(crate) fn rewrite_tool_response_mentions(
    response: &mut ResponseInputItem,
    tool_name: &str,
    context: &MentionRewriteContext,
) {
    let Some(kind) = output_rewrite_kind_for_tool_name(tool_name) else {
        return;
    };

    let ResponseInputItem::FunctionCallOutput { output, .. } = response else {
        return;
    };

    if output.success == Some(false) {
        return;
    }

    let Some(content) = output.text_content_mut() else {
        return;
    };

    let rewritten = rewrite_tool_output(content, kind, context);
    if rewritten != *content {
        *content = rewritten;
    }
}

pub(crate) fn command_read_paths(command: &[String], cwd: &Path) -> Vec<PathBuf> {
    parse_command(command)
        .into_iter()
        .filter_map(|parsed| match parsed {
            ParsedCommand::Read { path, .. } => {
                if path.is_absolute() {
                    Some(path)
                } else {
                    Some(cwd.join(path))
                }
            }
            _ => None,
        })
        .collect()
}

pub(crate) fn should_rewrite_mentions_for_path(path: &Path, skills: &[SkillMetadata]) -> bool {
    if path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("SKILL.md"))
    {
        return true;
    }

    skills.iter().any(|skill| {
        skill
            .path
            .parent()
            .is_some_and(|skill_dir| path.starts_with(skill_dir))
    })
}

pub(crate) fn rewrite_text_with_mentions(text: &str, context: &MentionRewriteContext) -> String {
    let mut explicit_app_paths = HashSet::new();
    rewrite_text_mentions(text, context, &mut explicit_app_paths)
}

fn read_paths_for_tool_call(
    session: &Session,
    turn: &TurnContext,
    tool_name: &str,
    payload: &ToolPayload,
) -> Vec<PathBuf> {
    match (tool_name, payload) {
        ("read_file", ToolPayload::Function { arguments }) => read_paths_for_read_file(arguments),
        ("shell", ToolPayload::Function { arguments })
        | ("container.exec", ToolPayload::Function { arguments }) => {
            command_read_paths_for_shell_tool(arguments, turn)
        }
        ("shell_command", ToolPayload::Function { arguments }) => {
            command_read_paths_for_shell_command(arguments, session, turn)
        }
        ("exec_command", ToolPayload::Function { arguments }) => {
            command_read_paths_for_unified_exec(arguments, session, turn)
        }
        (_, ToolPayload::LocalShell { params }) => {
            let cwd = turn.resolve_path(params.workdir.clone());
            command_read_paths(&params.command, &cwd)
        }
        _ => Vec::new(),
    }
}

fn read_paths_for_read_file(arguments: &str) -> Vec<PathBuf> {
    let Some(args) = parse_tool_arguments::<ReadFileToolArgs>(arguments) else {
        return Vec::new();
    };

    let path = PathBuf::from(args.file_path);
    if path.is_absolute() {
        vec![path]
    } else {
        Vec::new()
    }
}

fn command_read_paths_for_shell_tool(arguments: &str, turn: &TurnContext) -> Vec<PathBuf> {
    let Some(params) = parse_tool_arguments::<ShellToolCallParams>(arguments) else {
        return Vec::new();
    };

    let cwd = turn.resolve_path(params.workdir);
    command_read_paths(&params.command, &cwd)
}

fn command_read_paths_for_shell_command(
    arguments: &str,
    session: &Session,
    turn: &TurnContext,
) -> Vec<PathBuf> {
    let Some(params) = parse_tool_arguments::<ShellCommandToolCallParams>(arguments) else {
        return Vec::new();
    };

    let shell = session.user_shell();
    let command = shell.derive_exec_args(&params.command, params.login.unwrap_or(true));
    let cwd = turn.resolve_path(params.workdir);
    command_read_paths(&command, &cwd)
}

fn command_read_paths_for_unified_exec(
    arguments: &str,
    session: &Session,
    turn: &TurnContext,
) -> Vec<PathBuf> {
    let Some(args) = parse_tool_arguments::<UnifiedExecCommandArgs>(arguments) else {
        return Vec::new();
    };

    let workdir = args.workdir.clone().filter(|value| !value.is_empty());
    let cwd = turn.resolve_path(workdir);
    let session_shell = session.user_shell();
    let command = unified_exec_command(&args, session_shell.as_ref());
    command_read_paths(&command, &cwd)
}

fn parse_tool_arguments<T>(arguments: &str) -> Option<T>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_str(arguments).ok()
}

fn default_login() -> bool {
    true
}

fn unified_exec_command(args: &UnifiedExecCommandArgs, session_shell: &Shell) -> Vec<String> {
    let model_shell = args.shell.as_ref().map(|shell_str| {
        let mut shell = get_shell_by_model_provided_path(&PathBuf::from(shell_str));
        shell.shell_snapshot = empty_shell_snapshot_receiver();
        shell
    });

    let shell = model_shell.as_ref().unwrap_or(session_shell);
    shell.derive_exec_args(&args.cmd, args.login)
}

fn output_rewrite_kind_for_tool_name(tool_name: &str) -> Option<MentionRewriteOutputKind> {
    match tool_name {
        "read_file" => Some(MentionRewriteOutputKind::ReadFile),
        "shell_command" => Some(MentionRewriteOutputKind::ShellFreeform),
        "shell" | "container.exec" | "local_shell" => {
            Some(MentionRewriteOutputKind::ShellStructured)
        }
        "exec_command" => Some(MentionRewriteOutputKind::PlainText),
        _ => None,
    }
}

fn rewrite_tool_output(
    content: &str,
    kind: MentionRewriteOutputKind,
    context: &MentionRewriteContext,
) -> String {
    match kind {
        MentionRewriteOutputKind::ReadFile => rewrite_read_file_output_mentions(content, context),
        MentionRewriteOutputKind::ShellFreeform => {
            rewrite_shell_freeform_output_mentions(content, context)
        }
        MentionRewriteOutputKind::ShellStructured => {
            rewrite_shell_structured_output_mentions(content, context)
        }
        MentionRewriteOutputKind::PlainText => rewrite_text_with_mentions(content, context),
    }
}

fn rewrite_read_file_output_mentions(content: &str, context: &MentionRewriteContext) -> String {
    let mut explicit_app_paths = HashSet::new();
    let mut changed = false;
    let rewritten_lines = content
        .split('\n')
        .map(|line| {
            let rewritten =
                rewrite_prefixed_line_for_mentions(line, context, &mut explicit_app_paths);
            if rewritten != line {
                changed = true;
            }
            rewritten
        })
        .collect::<Vec<_>>();

    if changed {
        rewritten_lines.join("\n")
    } else {
        content.to_string()
    }
}

fn rewrite_prefixed_line_for_mentions(
    line: &str,
    context: &MentionRewriteContext,
    explicit_app_paths: &mut HashSet<String>,
) -> String {
    let Some((prefix, line_text)) = line.split_once(": ") else {
        return line.to_string();
    };

    let rewritten = rewrite_text_mentions(line_text, context, explicit_app_paths);
    if rewritten == line_text {
        line.to_string()
    } else {
        format!("{prefix}: {rewritten}")
    }
}

fn rewrite_shell_freeform_output_mentions(
    content: &str,
    context: &MentionRewriteContext,
) -> String {
    let Some((prefix, output)) = content.split_once("\nOutput:\n") else {
        return rewrite_text_with_mentions(content, context);
    };

    let rewritten = rewrite_text_with_mentions(output, context);
    if rewritten == output {
        content.to_string()
    } else {
        format!("{prefix}\nOutput:\n{rewritten}")
    }
}

fn rewrite_shell_structured_output_mentions(
    content: &str,
    context: &MentionRewriteContext,
) -> String {
    let Ok(mut payload) = serde_json::from_str::<serde_json::Value>(content) else {
        return content.to_string();
    };

    let Some(output) = payload.get("output").and_then(serde_json::Value::as_str) else {
        return content.to_string();
    };

    let rewritten = rewrite_text_with_mentions(output, context);
    if rewritten == output {
        return content.to_string();
    }

    let Some(output_field) = payload.get_mut("output") else {
        return content.to_string();
    };
    *output_field = serde_json::Value::String(rewritten);
    serde_json::to_string(&payload).unwrap_or_else(|_| content.to_string())
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::Path;
    use std::path::PathBuf;

    use codex_app_server_protocol::AppInfo;
    use codex_protocol::models::FunctionCallOutputPayload;
    use codex_protocol::models::ResponseInputItem;
    use codex_protocol::protocol::SkillScope;
    use pretty_assertions::assert_eq;

    use crate::mentions::build_connector_slug_counts;
    use crate::mentions::build_skill_name_counts;
    use crate::skills::SkillMetadata;
    use crate::skills::injection::build_mention_rewrite_context;

    use super::command_read_paths;
    use super::rewrite_tool_response_mentions;
    use super::should_rewrite_mentions_for_path;

    fn make_skill(name: &str, path: &str) -> SkillMetadata {
        SkillMetadata {
            name: name.to_string(),
            description: format!("{name} skill"),
            short_description: None,
            interface: None,
            dependencies: None,
            policy: None,
            path: PathBuf::from(path),
            scope: SkillScope::User,
        }
    }

    fn make_connector(id: &str, name: &str) -> AppInfo {
        AppInfo {
            id: id.to_string(),
            name: name.to_string(),
            description: None,
            logo_url: None,
            logo_url_dark: None,
            distribution_channel: None,
            install_url: None,
            is_accessible: true,
        }
    }

    fn make_mention_rewrite_context() -> super::MentionRewriteContext {
        let skills = vec![make_skill("beta-skill", "/tmp/skills/beta/SKILL.md")];
        let disabled_paths = HashSet::new();
        let (skill_name_counts, skill_name_counts_lower) =
            build_skill_name_counts(&skills, &disabled_paths);
        let connectors = vec![make_connector("github-id", "GitHub")];
        let connector_slug_counts = build_connector_slug_counts(&connectors);
        build_mention_rewrite_context(
            &skills,
            &disabled_paths,
            &skill_name_counts,
            &skill_name_counts_lower,
            &connector_slug_counts,
            &connectors,
        )
    }

    #[test]
    fn should_rewrite_mentions_for_skill_paths() {
        let skills = vec![make_skill("alpha-skill", "/tmp/skills/alpha/SKILL.md")];

        assert_eq!(
            true,
            should_rewrite_mentions_for_path(Path::new("/tmp/skills/alpha/SKILL.md"), &skills)
        );
        assert_eq!(
            true,
            should_rewrite_mentions_for_path(
                Path::new("/tmp/skills/alpha/references/secondary_context.md"),
                &skills,
            )
        );
        assert_eq!(
            true,
            should_rewrite_mentions_for_path(Path::new("/tmp/random/SKILL.md"), &skills)
        );
        assert_eq!(
            false,
            should_rewrite_mentions_for_path(Path::new("/tmp/random/README.md"), &skills)
        );
    }

    #[test]
    fn command_read_paths_extracts_reads_from_shell_command() {
        let command = vec![
            "bash".to_string(),
            "-lc".to_string(),
            "cat SKILL.md".to_string(),
        ];
        let paths = command_read_paths(&command, Path::new("/tmp/skills/alpha"));

        assert_eq!(paths, vec![PathBuf::from("/tmp/skills/alpha/SKILL.md")]);
    }

    #[test]
    fn rewrite_tool_response_mentions_rewrites_read_file_lines() {
        let mut response = ResponseInputItem::FunctionCallOutput {
            call_id: "call-1".to_string(),
            output: FunctionCallOutputPayload::from_text(
                "L7: use $beta-skill and $GitHub".to_string(),
            ),
        };

        rewrite_tool_response_mentions(&mut response, "read_file", &make_mention_rewrite_context());

        let ResponseInputItem::FunctionCallOutput { output, .. } = response else {
            panic!("expected function_call_output");
        };
        assert_eq!(
            output.text_content(),
            Some(
                "L7: use [$beta-skill](skill:///tmp/skills/beta/SKILL.md) and [$GitHub](app://github-id)"
            )
        );
    }

    #[test]
    fn rewrite_tool_response_mentions_rewrites_shell_freeform_output() {
        let mut response = ResponseInputItem::FunctionCallOutput {
            call_id: "call-1".to_string(),
            output: FunctionCallOutputPayload::from_text(
                "Exit code: 0\nWall time: 0.1 seconds\nOutput:\nuse $beta-skill and $GitHub"
                    .to_string(),
            ),
        };

        rewrite_tool_response_mentions(
            &mut response,
            "shell_command",
            &make_mention_rewrite_context(),
        );

        let ResponseInputItem::FunctionCallOutput { output, .. } = response else {
            panic!("expected function_call_output");
        };
        assert_eq!(
            output.text_content(),
            Some(
                "Exit code: 0\nWall time: 0.1 seconds\nOutput:\nuse [$beta-skill](skill:///tmp/skills/beta/SKILL.md) and [$GitHub](app://github-id)"
            )
        );
    }

    #[test]
    fn rewrite_tool_response_mentions_rewrites_shell_structured_output() {
        let mut response = ResponseInputItem::FunctionCallOutput {
            call_id: "call-1".to_string(),
            output: FunctionCallOutputPayload::from_text(
                serde_json::json!({
                    "output": "use $beta-skill and $GitHub",
                    "metadata": {
                        "exit_code": 0,
                        "duration_seconds": 0.1
                    }
                })
                .to_string(),
            ),
        };

        rewrite_tool_response_mentions(&mut response, "shell", &make_mention_rewrite_context());

        let ResponseInputItem::FunctionCallOutput { output, .. } = response else {
            panic!("expected function_call_output");
        };
        let Some(text) = output.text_content() else {
            panic!("expected text output");
        };
        let payload: serde_json::Value =
            serde_json::from_str(text).expect("valid structured shell output");

        assert_eq!(
            payload.get("output"),
            Some(&serde_json::Value::String(
                "use [$beta-skill](skill:///tmp/skills/beta/SKILL.md) and [$GitHub](app://github-id)"
                    .to_string(),
            ))
        );
    }

    #[test]
    fn rewrite_tool_response_mentions_skips_failed_tool_output() {
        let mut response = ResponseInputItem::FunctionCallOutput {
            call_id: "call-1".to_string(),
            output: FunctionCallOutputPayload {
                body: codex_protocol::models::FunctionCallOutputBody::Text(
                    "use $beta-skill".to_string(),
                ),
                success: Some(false),
            },
        };

        rewrite_tool_response_mentions(&mut response, "read_file", &make_mention_rewrite_context());

        let ResponseInputItem::FunctionCallOutput { output, .. } = response else {
            panic!("expected function_call_output");
        };
        assert_eq!(output.text_content(), Some("use $beta-skill"));
    }
}
