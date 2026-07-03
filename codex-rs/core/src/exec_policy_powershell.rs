use codex_shell_command::powershell::PowerShellExecPolicyParse;

use super::render_shlex_command;
use crate::tools::sandboxing::ExecApprovalRequirement;

pub(super) enum PreparedPowerShell {
    Terminal(ExecApprovalRequirement),
    Parsed(ParsedPowerShell),
}

pub(super) struct ParsedPowerShell {
    commands: Vec<Vec<String>>,
}

pub(super) fn prepare(command: &[String]) -> Option<PreparedPowerShell> {
    match codex_shell_command::powershell::parse_powershell_command_for_exec_policy(command)? {
        PowerShellExecPolicyParse::TrustedRuntime {
            commands: Some(commands),
        } if !commands.is_empty() => {
            Some(PreparedPowerShell::Parsed(ParsedPowerShell { commands }))
        }
        PowerShellExecPolicyParse::TrustedRuntime { .. } => Some(forbidden(
            command,
            "the PowerShell script could not be inspected",
        )),
        PowerShellExecPolicyParse::UntrustedRuntime {
            commands: Some(commands),
        } if !commands.is_empty() => Some(forbidden(
            command,
            "the PowerShell runtime is not a protected system executable",
        )),
        PowerShellExecPolicyParse::UntrustedRuntime { .. } => Some(forbidden(
            command,
            "an untrusted PowerShell wrapper could not be inspected with the protected system parser",
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
