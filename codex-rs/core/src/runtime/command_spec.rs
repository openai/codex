use crate::orchestrator::ToolError;
use crate::sandboxing::CommandSpec;
use std::collections::HashMap;
use std::path::PathBuf;

/// Shared helper to construct a CommandSpec from a tokenized command line.
/// Validates that at least a program is present.
pub(crate) fn build_command_spec(
    command: &[String],
    cwd: &PathBuf,
    env: &HashMap<String, String>,
    timeout_ms: Option<u64>,
    with_escalated_permissions: Option<bool>,
    justification: Option<String>,
) -> Result<CommandSpec, ToolError> {
    let (program, args) = command
        .split_first()
        .ok_or_else(|| ToolError::Rejected("command args are empty".to_string()))?;
    Ok(CommandSpec {
        program: program.clone(),
        args: args.to_vec(),
        cwd: cwd.clone(),
        env: env.clone(),
        timeout_ms,
        with_escalated_permissions,
        justification,
    })
}
