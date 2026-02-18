use crate::is_safe_command::is_known_safe_command;
use crate::parse_command::parse_command;
use crate::shell::get_shell_by_model_provided_path;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use codex_protocol::models::ShellCommandToolCallParams;
use codex_protocol::models::ShellToolCallParams;
use codex_protocol::parse_command::ParsedCommand;
use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

const MEMORIES_USAGE_METRIC: &str = "codex.memories.usage";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum MemoriesUsageKind {
    MemoryMd,
    MemorySummary,
    RawMemories,
    RolloutSummaries,
    Skills,
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

fn default_login() -> bool {
    true
}

impl MemoriesUsageKind {
    fn as_tag(self) -> &'static str {
        match self {
            Self::MemoryMd => "memory_md",
            Self::MemorySummary => "memory_summary",
            Self::RawMemories => "raw_memories",
            Self::RolloutSummaries => "rollout_summaries",
            Self::Skills => "skills",
        }
    }
}

pub(crate) async fn emit_metric_for_tool_read(invocation: &ToolInvocation, success: bool) {
    let kinds = memories_usage_kinds_from_invocation(invocation).await;
    if kinds.is_empty() {
        return;
    }

    let success = if success { "true" } else { "false" };
    for kind in kinds {
        invocation.turn.otel_manager.counter(
            MEMORIES_USAGE_METRIC,
            1,
            &[
                ("kind", kind.as_tag()),
                ("tool", invocation.tool_name.as_str()),
                ("success", success),
            ],
        );
    }
}

async fn memories_usage_kinds_from_invocation(
    invocation: &ToolInvocation,
) -> Vec<MemoriesUsageKind> {
    let codex_home = invocation.session.codex_home().await;
    let memories_root = crate::memories::memory_root(&codex_home);
    let Some((command, cwd)) = shell_command_for_invocation(invocation) else {
        return Vec::new();
    };
    memories_usage_kinds_from_shell_command(&command, cwd.as_path(), &memories_root)
}

fn shell_command_for_invocation(invocation: &ToolInvocation) -> Option<(Vec<String>, PathBuf)> {
    let ToolPayload::Function { arguments } = &invocation.payload else {
        return None;
    };

    match invocation.tool_name.as_str() {
        "shell" => serde_json::from_str::<ShellToolCallParams>(arguments)
            .ok()
            .map(|params| (params.command, invocation.turn.resolve_path(params.workdir))),
        "shell_command" => serde_json::from_str::<ShellCommandToolCallParams>(arguments)
            .ok()
            .map(|params| {
                let command = invocation
                    .session
                    .user_shell()
                    .derive_exec_args(&params.command, params.login.unwrap_or(true));
                (command, invocation.turn.resolve_path(params.workdir))
            }),
        "exec_command" => serde_json::from_str::<ExecCommandArgs>(arguments)
            .ok()
            .map(|params| {
                (
                    derive_exec_command_for_metrics(&params, invocation.session.user_shell()),
                    invocation.turn.resolve_path(params.workdir),
                )
            }),
        _ => None,
    }
}

fn derive_exec_command_for_metrics(
    args: &ExecCommandArgs,
    session_shell: Arc<crate::shell::Shell>,
) -> Vec<String> {
    let model_shell = args.shell.as_ref().map(|shell_path| {
        let mut shell = get_shell_by_model_provided_path(&PathBuf::from(shell_path));
        shell.shell_snapshot = crate::shell::empty_shell_snapshot_receiver();
        shell
    });
    let shell = model_shell.as_ref().unwrap_or(session_shell.as_ref());
    shell.derive_exec_args(&args.cmd, args.login)
}

fn memories_usage_kinds_from_shell_command(
    command: &[String],
    cwd: &Path,
    memories_root: &Path,
) -> Vec<MemoriesUsageKind> {
    if !is_known_safe_command(command) {
        return Vec::new();
    }

    let parsed_commands = parse_command(command);
    let mut kinds = BTreeSet::new();
    for parsed_command in parsed_commands {
        maybe_add_kinds_from_parsed_command(&mut kinds, &parsed_command, cwd, memories_root);
    }
    kinds.into_iter().collect()
}

fn maybe_add_kinds_from_parsed_command(
    kinds: &mut BTreeSet<MemoriesUsageKind>,
    parsed_command: &ParsedCommand,
    cwd: &Path,
    memories_root: &Path,
) {
    match parsed_command {
        ParsedCommand::Read { path, .. } => {
            let resolved_path = if path.is_absolute() {
                path.clone()
            } else {
                cwd.join(path)
            };
            if let Some(path) = resolved_path.to_str() {
                maybe_add_kind_from_path(kinds, path, memories_root);
            }
        }
        ParsedCommand::Search { path, .. } => {
            if let Some(path) = path {
                let path = Path::new(path);
                let resolved_path = if path.is_absolute() {
                    path.to_path_buf()
                } else {
                    cwd.join(path)
                };
                if let Some(path) = resolved_path.to_str() {
                    maybe_add_kind_from_path(kinds, path, memories_root);
                }
            }
        }
        ParsedCommand::ListFiles { .. } => {}
        ParsedCommand::Unknown { .. } => {}
    }
}

fn maybe_add_kind_from_path(
    kinds: &mut BTreeSet<MemoriesUsageKind>,
    raw_path: &str,
    memories_root: &Path,
) {
    let trimmed = raw_path.trim_matches(|c: char| {
        matches!(
            c,
            '"' | '\'' | '(' | ')' | '[' | ']' | '{' | '}' | ',' | ';'
        )
    });
    let Some(kind) = classify_memories_usage_path(trimmed, memories_root) else {
        return;
    };
    kinds.insert(kind);
}

fn classify_memories_usage_path(raw_path: &str, memories_root: &Path) -> Option<MemoriesUsageKind> {
    let normalized_root = normalize_path(memories_root);
    let normalized_path = normalize_path(Path::new(raw_path));
    if !normalized_path.starts_with(&normalized_root) {
        return None;
    }

    let relative = normalized_path.strip_prefix(&normalized_root).ok()?;
    if relative == Path::new("MEMORY.md") {
        return Some(MemoriesUsageKind::MemoryMd);
    }
    if relative == Path::new("memory_summary.md") {
        return Some(MemoriesUsageKind::MemorySummary);
    }
    if relative == Path::new("raw_memories.md") {
        return Some(MemoriesUsageKind::RawMemories);
    }
    if relative.starts_with("rollout_summaries") {
        return Some(MemoriesUsageKind::RolloutSummaries);
    }
    if relative.starts_with("skills") {
        return Some(MemoriesUsageKind::Skills);
    }
    None
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::MemoriesUsageKind;
    use super::memories_usage_kinds_from_shell_command;
    use crate::is_safe_command::is_known_safe_command;
    use pretty_assertions::assert_eq;
    use std::path::Path;

    #[test]
    fn matches_safe_shell_cat_command() {
        let root = Path::new("/tmp/codex_home/memories");
        let command = vec![
            "bash".to_string(),
            "-lc".to_string(),
            "cat /tmp/codex_home/memories/memory_summary.md".to_string(),
        ];
        assert!(is_known_safe_command(&command));

        let kinds = memories_usage_kinds_from_shell_command(&command, Path::new("/tmp"), root);
        assert_eq!(kinds, vec![MemoriesUsageKind::MemorySummary]);
    }

    #[test]
    fn matches_safe_shell_sed_command() {
        let root = Path::new("/tmp/codex_home/memories");
        let command = vec![
            "bash".to_string(),
            "-lc".to_string(),
            "sed -n '1,120p' /tmp/codex_home/memories/MEMORY.md".to_string(),
        ];
        assert!(is_known_safe_command(&command));

        let kinds = memories_usage_kinds_from_shell_command(&command, Path::new("/tmp"), root);
        assert_eq!(kinds, vec![MemoriesUsageKind::MemoryMd]);
    }
}
