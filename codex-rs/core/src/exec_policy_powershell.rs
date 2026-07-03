use codex_shell_command::powershell::PowerShellExecPolicyParse;
use codex_shell_command::powershell::PowerShellExecPolicyParseOutcome;

use super::render_shlex_command;
use crate::tools::sandboxing::ExecApprovalRequirement;

pub(super) enum PreparedPowerShell {
    Terminal(ExecApprovalRequirement),
    Parsed(ParsedPowerShell),
    Unsupported,
}

pub(super) struct ParsedPowerShell {
    commands: Vec<Vec<String>>,
}

pub(super) fn prepare(command: &[String]) -> Option<PreparedPowerShell> {
    let parsed =
        codex_shell_command::powershell::parse_powershell_command_for_exec_policy(command)?;
    prepare_classified(command, parsed)
}

pub(super) fn prepare_classified(
    command: &[String],
    parsed: PowerShellExecPolicyParse,
) -> Option<PreparedPowerShell> {
    match parsed {
        PowerShellExecPolicyParse::TrustedRuntime {
            outcome: PowerShellExecPolicyParseOutcome::Commands(commands),
        } => Some(PreparedPowerShell::Parsed(ParsedPowerShell { commands })),
        PowerShellExecPolicyParse::TrustedRuntime {
            outcome: PowerShellExecPolicyParseOutcome::Unsupported,
        } => Some(PreparedPowerShell::Unsupported),
        PowerShellExecPolicyParse::TrustedRuntime {
            outcome: PowerShellExecPolicyParseOutcome::Failed,
        } => Some(forbidden(command, "the protected PowerShell parser failed")),
        PowerShellExecPolicyParse::UntrustedRuntime {
            outcome: PowerShellExecPolicyParseOutcome::Commands(_),
        } => Some(forbidden(
            command,
            "the PowerShell runtime is not a protected system executable",
        )),
        PowerShellExecPolicyParse::UntrustedRuntime {
            outcome: PowerShellExecPolicyParseOutcome::Unsupported,
        } => Some(forbidden(
            command,
            "an untrusted PowerShell wrapper could not be inspected with the protected system parser",
        )),
        PowerShellExecPolicyParse::UntrustedRuntime {
            outcome: PowerShellExecPolicyParseOutcome::Failed,
        } => Some(forbidden(
            command,
            "the protected system parser failed while inspecting an untrusted PowerShell wrapper",
        )),
    }
}

fn forbidden(command: &[String], reason: &str) -> PreparedPowerShell {
    PreparedPowerShell::Terminal(ExecApprovalRequirement::Forbidden {
        reason: format!("`{}` rejected: {reason}", render_shlex_command(command)),
    })
}

impl ParsedPowerShell {
    pub(super) fn commands(&self) -> &[Vec<String>] {
        &self.commands
    }
}
