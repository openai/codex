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
use crate::mentions::build_connector_slug_counts;
use crate::mentions::build_skill_name_counts;
use crate::parse_command::parse_command;
use crate::shell::Shell;
use crate::shell::empty_shell_snapshot_receiver;
use crate::shell::get_shell_by_model_provided_path;
use crate::skills::injection::ToolMentionKind;
use crate::skills::injection::app_id_from_path;
use crate::skills::injection::extract_tool_mentions;
use crate::skills::injection::tool_kind_for_path;
use crate::tools::context::ToolPayload;

#[derive(Deserialize)]
struct ReadFileArgs {
    file_path: String,
}

#[derive(Deserialize)]
struct ExecCommandArgs {
    cmd: String,
    #[serde(default)]
    workdir: Option<String>,
    #[serde(default)]
    shell: Option<String>,
    #[serde(default = "default_login")]
    login: bool,
}

// Keeps exec-command login behavior aligned with the tool default.
fn default_login() -> bool {
    true
}

// Propagates connector selections from trusted skill file reads and tool mentions.
pub(crate) async fn maybe_update_tool_selections_from_skill_read(
    session: &Session,
    turn: &TurnContext,
    tool_name: &str,
    payload: &ToolPayload,
    response: &ResponseInputItem,
) {
    let Some(output_text) = successful_tool_output_text(response) else {
        return;
    };

    let read_paths = read_paths_for_tool_call(session, turn, tool_name, payload);
    if read_paths.is_empty() {
        return;
    }

    for path in &read_paths {
        if !session
            .is_explicitly_mentioned_skill_md_path(path.as_path())
            .await
        {
            return;
        }
    }

    let mentions = extract_tool_mentions(output_text);
    let mention_paths = mentions
        .paths()
        .map(str::to_string)
        .collect::<HashSet<String>>();
    let mention_plain_names_lower = mentions
        .plain_names()
        .map(str::to_ascii_lowercase)
        .collect::<HashSet<String>>();
    if mention_paths.is_empty() && mention_plain_names_lower.is_empty() {
        return;
    }

    let mut connector_ids = mention_paths
        .iter()
        .filter(|path| tool_kind_for_path(path) == ToolMentionKind::App)
        .filter_map(|path| app_id_from_path(path).map(str::to_string))
        .collect::<HashSet<String>>();

    let needs_additional_resolution = !mention_plain_names_lower.is_empty()
        || mention_paths
            .iter()
            .any(|path| tool_kind_for_path(path) == ToolMentionKind::Skill);
    if !needs_additional_resolution {
        maybe_merge_connector_selection(session, connector_ids).await;
        return;
    }

    let mcp_tools = session
        .services
        .mcp_connection_manager
        .read()
        .await
        .list_all_tools()
        .await;
    let connectors = connectors::accessible_connectors_from_mcp_tools(&mcp_tools);
    let connector_slug_counts = build_connector_slug_counts(&connectors);
    let skills_outcome = session
        .services
        .skills_manager
        .skills_for_cwd(&turn.cwd, false)
        .await;
    let skill_name_counts_lower =
        build_skill_name_counts(&skills_outcome.skills, &skills_outcome.disabled_paths).1;

    for connector in &connectors {
        let slug = connectors::connector_mention_slug(connector);
        let connector_count = connector_slug_counts.get(&slug).copied().unwrap_or(0);
        let skill_count = skill_name_counts_lower.get(&slug).copied().unwrap_or(0);
        if connector_count == 1 && skill_count == 0 && mention_plain_names_lower.contains(&slug) {
            connector_ids.insert(connector.id.clone());
        }
    }

    maybe_merge_connector_selection(session, connector_ids).await;
}

// Returns text output only when a function call completed without explicit failure.
fn successful_tool_output_text(response: &ResponseInputItem) -> Option<&str> {
    if let ResponseInputItem::FunctionCallOutput { output, .. } = response
        && output.success != Some(false)
    {
        return output.text_content();
    }

    None
}

// Resolves read paths from supported tool-call payload shapes.
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
            read_paths_for_shell_tool(arguments, turn)
        }
        ("shell_command", ToolPayload::Function { arguments }) => {
            read_paths_for_shell_command(arguments, session, turn)
        }
        ("exec_command", ToolPayload::Function { arguments }) => {
            read_paths_for_exec_command(arguments, session, turn)
        }
        (_, ToolPayload::LocalShell { params }) => {
            let cwd = turn.resolve_path(params.workdir.clone());
            command_read_paths(&params.command, cwd.as_path())
        }
        _ => Vec::new(),
    }
}

// Extracts the absolute file path from a read_file tool call.
fn read_paths_for_read_file(arguments: &str) -> Vec<PathBuf> {
    let Some(args) = parse_tool_arguments::<ReadFileArgs>(arguments) else {
        return Vec::new();
    };

    let path = PathBuf::from(args.file_path);
    if path.is_absolute() {
        vec![path]
    } else {
        Vec::new()
    }
}

// Parses shell tool parameters and returns paths read by the resulting command.
fn read_paths_for_shell_tool(arguments: &str, turn: &TurnContext) -> Vec<PathBuf> {
    let Some(params) = parse_tool_arguments::<ShellToolCallParams>(arguments) else {
        return Vec::new();
    };

    let cwd = turn.resolve_path(params.workdir);
    command_read_paths(&params.command, cwd.as_path())
}

// Parses shell_command payloads and returns files read by the derived shell command.
fn read_paths_for_shell_command(
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
    command_read_paths(&command, cwd.as_path())
}

// Parses exec_command payloads and returns files read by the derived command vector.
fn read_paths_for_exec_command(
    arguments: &str,
    session: &Session,
    turn: &TurnContext,
) -> Vec<PathBuf> {
    let Some(args) = parse_tool_arguments::<ExecCommandArgs>(arguments) else {
        return Vec::new();
    };

    let workdir = args.workdir.clone().filter(|value| !value.is_empty());
    let cwd = turn.resolve_path(workdir);
    let session_shell = session.user_shell();
    let command = exec_command_vector(&args, session_shell.as_ref());
    command_read_paths(&command, cwd.as_path())
}

// Deserializes tool-call JSON arguments into a typed payload.
fn parse_tool_arguments<T>(arguments: &str) -> Option<T>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_str(arguments).ok()
}

// Builds the command vector used by exec_command, honoring model-provided shell overrides.
fn exec_command_vector(args: &ExecCommandArgs, session_shell: &Shell) -> Vec<String> {
    let model_shell = args.shell.as_ref().map(|shell_str| {
        let mut shell = get_shell_by_model_provided_path(&PathBuf::from(shell_str));
        shell.shell_snapshot = empty_shell_snapshot_receiver();
        shell
    });
    let shell = model_shell.as_ref().unwrap_or(session_shell);

    shell.derive_exec_args(&args.cmd, args.login)
}

// Extracts read operations from a parsed command and resolves paths against cwd.
// Returns no paths unless every parsed segment is a read operation.
fn command_read_paths(command: &[String], cwd: &Path) -> Vec<PathBuf> {
    let parsed_commands = parse_command(command);
    if parsed_commands.is_empty() {
        return Vec::new();
    }

    let mut paths = Vec::with_capacity(parsed_commands.len());
    for parsed in parsed_commands {
        let ParsedCommand::Read { path, .. } = parsed else {
            return Vec::new();
        };
        if path.is_absolute() {
            paths.push(path);
        } else {
            paths.push(cwd.join(path));
        }
    }

    paths
}

// Merges connector selection only when at least one connector ID was discovered.
async fn maybe_merge_connector_selection(session: &Session, connector_ids: HashSet<String>) {
    if !connector_ids.is_empty() {
        session.merge_connector_selection(connector_ids).await;
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::PathBuf;

    use codex_protocol::models::FunctionCallOutputPayload;
    use codex_protocol::models::ResponseInputItem;
    use codex_protocol::protocol::SkillScope;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::maybe_update_tool_selections_from_skill_read;
    use super::read_paths_for_tool_call;
    use crate::codex::make_session_and_context;
    use crate::skills::SkillMetadata;
    use crate::tools::context::ToolPayload;

    // Builds minimal skill metadata for skill mention propagation tests.
    fn skill(path: &str) -> SkillMetadata {
        SkillMetadata {
            name: "alpha".to_string(),
            description: "alpha".to_string(),
            short_description: None,
            interface: None,
            dependencies: None,
            policy: None,
            path: PathBuf::from(path),
            scope: SkillScope::User,
        }
    }

    // Builds a successful function-call output item with plain text content.
    fn output(text: &str) -> ResponseInputItem {
        ResponseInputItem::FunctionCallOutput {
            call_id: "call-1".to_string(),
            output: FunctionCallOutputPayload::from_text(text.to_string()),
        }
    }

    #[tokio::test]
    // Verifies explicit skill reads can promote mentioned app connectors into selection state.
    async fn explicitly_mentioned_read_file_output_updates_connector_selection() {
        let (session, turn) = make_session_and_context().await;
        session
            .record_explicitly_mentioned_skill_sources(&[skill("/tmp/skills/alpha/SKILL.md")])
            .await;

        let payload = ToolPayload::Function {
            arguments: json!({
                "file_path": "/tmp/skills/alpha/SKILL.md",
                "offset": 1,
                "limit": 200
            })
            .to_string(),
        };

        maybe_update_tool_selections_from_skill_read(
            &session,
            &turn,
            "read_file",
            &payload,
            &output("L1: use [$calendar](app://calendar)"),
        )
        .await;

        assert_eq!(
            session.get_connector_selection().await,
            HashSet::from(["calendar".to_string()])
        );
    }

    #[tokio::test]
    // Verifies non-explicit skill reads do not affect connector selection.
    async fn non_explicitly_mentioned_read_file_output_is_ignored() {
        let (session, turn) = make_session_and_context().await;
        let payload = ToolPayload::Function {
            arguments: json!({
                "file_path": "/tmp/skills/alpha/SKILL.md",
                "offset": 1,
                "limit": 200
            })
            .to_string(),
        };

        maybe_update_tool_selections_from_skill_read(
            &session,
            &turn,
            "read_file",
            &payload,
            &output("L1: use [$calendar](app://calendar)"),
        )
        .await;

        assert_eq!(session.get_connector_selection().await, HashSet::new());
    }

    #[tokio::test]
    // Verifies nested skill references are ignored unless that nested skill was explicitly mentioned.
    async fn nested_skill_read_output_is_ignored_without_explicit_user_mention() {
        let (session, turn) = make_session_and_context().await;
        session
            .record_explicitly_mentioned_skill_sources(&[skill("/tmp/skills/alpha/SKILL.md")])
            .await;

        let alpha_payload = ToolPayload::Function {
            arguments: json!({
                "file_path": "/tmp/skills/alpha/SKILL.md",
                "offset": 1,
                "limit": 200
            })
            .to_string(),
        };
        maybe_update_tool_selections_from_skill_read(
            &session,
            &turn,
            "read_file",
            &alpha_payload,
            &output("L1: use [$beta](skill:///tmp/skills/beta/SKILL.md)"),
        )
        .await;

        let beta_payload = ToolPayload::Function {
            arguments: json!({
                "file_path": "/tmp/skills/beta/SKILL.md",
                "offset": 1,
                "limit": 200
            })
            .to_string(),
        };
        maybe_update_tool_selections_from_skill_read(
            &session,
            &turn,
            "read_file",
            &beta_payload,
            &output("L1: use [$calendar](app://calendar)"),
        )
        .await;

        assert_eq!(session.get_connector_selection().await, HashSet::new());
    }

    #[tokio::test]
    // Verifies shell_command payload parsing captures SKILL.md read paths.
    async fn shell_command_read_path_is_detected() {
        let (session, turn) = make_session_and_context().await;
        let payload = ToolPayload::Function {
            arguments: json!({
                "command": "cat /tmp/skills/alpha/SKILL.md",
                "workdir": "/tmp",
                "timeout_ms": 1_000
            })
            .to_string(),
        };

        let paths = read_paths_for_tool_call(&session, &turn, "shell_command", &payload);

        assert_eq!(paths, vec![PathBuf::from("/tmp/skills/alpha/SKILL.md")]);
    }

    #[tokio::test]
    // Verifies shell command parsing fails closed when any segment is not a read.
    async fn shell_command_with_non_read_segment_returns_no_paths() {
        let (session, turn) = make_session_and_context().await;
        let payload = ToolPayload::Function {
            arguments: json!({
                "command": "cat /tmp/skills/alpha/SKILL.md; echo done",
                "workdir": "/tmp",
                "timeout_ms": 1_000
            })
            .to_string(),
        };

        let paths = read_paths_for_tool_call(&session, &turn, "shell_command", &payload);

        assert_eq!(paths, Vec::<PathBuf>::new());
    }
}
