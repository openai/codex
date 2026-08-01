use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::flat_tool_name;
use crate::tools::handlers::unified_exec::ExecCommandArgs;
use codex_memories_read::usage::MEMORIES_USAGE_METRIC;
use codex_memories_read::usage::memories_usage_kinds_from_command;
use codex_protocol::models::ShellCommandToolCallParams;
// copybara:strip-for-public begin
use codex_analytics::INTERNAL_TOOL_INPUT_COMMAND_NAMES;
use codex_analytics::INTERNAL_TOOL_INPUT_FLAG_NAMES;
use codex_analytics::INTERNAL_TOOL_INPUT_MAX_COMMANDS;
use codex_analytics::INTERNAL_TOOL_INPUT_MAX_FLAGS;
use codex_analytics::INTERNAL_TOOL_INPUT_SUBCOMMAND_NAMES;
use codex_shell_command::bash::parse_shell_lc_plain_commands;
use codex_shell_command::bash::parse_shell_script_into_commands;
use serde_json::Value;
// copybara:strip-for-public end

pub(crate) fn emit_metric_for_tool_read(invocation: &ToolInvocation, success: bool) {
    let Some(command) = shell_script_for_invocation(invocation) else {
        return;
    };

    let success = if success { "true" } else { "false" };
    let tool_name = flat_tool_name(&invocation.tool_name);
    for kind in memories_usage_kinds_from_command(&command) {
        invocation.turn.session_telemetry.counter(
            MEMORIES_USAGE_METRIC,
            /*inc*/ 1,
            &[
                ("kind", kind.as_tag()),
                ("tool", tool_name.as_ref()),
                ("success", success),
            ],
        );
    }
}

pub(crate) fn shell_script_for_invocation(invocation: &ToolInvocation) -> Option<String> {
    let ToolPayload::Function { arguments } = &invocation.payload else {
        return None;
    };

    match (
        invocation.tool_name.namespace.as_deref(),
        invocation.tool_name.name.as_str(),
    ) {
        (None, "shell_command") => serde_json::from_str::<ShellCommandToolCallParams>(arguments)
            .ok()
            .map(|params| params.command),
        (None, "exec_command") => serde_json::from_str::<ExecCommandArgs>(arguments)
            .ok()
            .map(|params| params.cmd),
        (Some(_), _) | (None, _) => None,
    }
}
// copybara:strip-for-public begin
const MAX_INTERNAL_TOOL_SCRIPT_BYTES: usize = 16 * 1024;

fn empty_internal_tool_metadata() -> Value {
    Value::Object(serde_json::Map::new())
}

fn known_internal_tool_name(names: &'static str, candidate: &str) -> Option<&'static str> {
    names
        .split_ascii_whitespace()
        .find(|name| *name == candidate)
}

fn known_internal_tool_flag(word: &str) -> Option<&'static str> {
    let flag_name = word.split_once('=').map_or(word, |(name, _)| name);
    if let Some(flag) = known_internal_tool_name(INTERNAL_TOOL_INPUT_FLAG_NAMES, flag_name) {
        return Some(flag);
    }

    if word.starts_with("--") {
        return None;
    }
    INTERNAL_TOOL_INPUT_FLAG_NAMES
        .split_ascii_whitespace()
        .filter(|flag| flag.len() == 2)
        .find(|flag| word.starts_with(flag) && word.len() > flag.len())
}

fn internal_tool_command_metadata(words: Vec<String>) -> Option<Value> {
    let executable = words.first()?.rsplit(['/', '\\']).next()?;
    let command = known_internal_tool_name(INTERNAL_TOOL_INPUT_COMMAND_NAMES, executable)?;
    let mut metadata = serde_json::Map::new();
    metadata.insert("command".to_string(), Value::String(command.to_string()));

    let subcommand = matches!(
        command,
        "cargo" | "docker" | "gh" | "git" | "just" | "kubectl" | "npm" | "pnpm" | "yarn"
    )
    .then(|| words.get(1))
    .flatten()
    .and_then(|word| known_internal_tool_name(INTERNAL_TOOL_INPUT_SUBCOMMAND_NAMES, word));
    if let Some(subcommand) = subcommand {
        metadata.insert(
            "subcommand".to_string(),
            Value::String(subcommand.to_string()),
        );
    }

    let mut flags = Vec::new();
    for word in words.iter().skip(1) {
        if word == "--" {
            break;
        }
        if let Some(flag) = known_internal_tool_flag(word) {
            flags.push(Value::String(flag.to_string()));
            if flags.len() > INTERNAL_TOOL_INPUT_MAX_FLAGS {
                return None;
            }
        }
    }
    metadata.insert("flags".to_string(), Value::Array(flags));
    Some(Value::Object(metadata))
}

fn internal_tool_commands_metadata(commands: Vec<Vec<String>>) -> Value {
    if commands.is_empty() || commands.len() > INTERNAL_TOOL_INPUT_MAX_COMMANDS {
        return empty_internal_tool_metadata();
    }

    let commands = commands
        .into_iter()
        .filter_map(internal_tool_command_metadata)
        .collect::<Vec<_>>();
    if commands.is_empty() {
        return empty_internal_tool_metadata();
    }
    serde_json::json!({ "commands": commands })
}

pub(crate) fn shell_command_metadata(script: &str) -> Value {
    if script.len() > MAX_INTERNAL_TOOL_SCRIPT_BYTES {
        return empty_internal_tool_metadata();
    }
    parse_shell_script_into_commands(script)
        .map(internal_tool_commands_metadata)
        .unwrap_or_else(empty_internal_tool_metadata)
}

pub(crate) fn shell_command_metadata_for_argv(argv: &[String]) -> Value {
    if argv
        .iter()
        .fold(0usize, |bytes, word| bytes.saturating_add(word.len()))
        > MAX_INTERNAL_TOOL_SCRIPT_BYTES
    {
        return empty_internal_tool_metadata();
    }
    parse_shell_lc_plain_commands(argv)
        .map(internal_tool_commands_metadata)
        .unwrap_or_else(empty_internal_tool_metadata)
}

pub(crate) fn shell_command_metadata_for_invocation(invocation: &ToolInvocation) -> Value {
    let ToolPayload::Function { arguments } = &invocation.payload else {
        return empty_internal_tool_metadata();
    };
    if arguments.len() > MAX_INTERNAL_TOOL_SCRIPT_BYTES {
        return empty_internal_tool_metadata();
    }
    shell_script_for_invocation(invocation)
        .map(|script| shell_command_metadata(&script))
        .unwrap_or_else(empty_internal_tool_metadata)
}

// copybara:strip-for-public end
