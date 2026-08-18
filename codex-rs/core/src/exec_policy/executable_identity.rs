use super::ExecApprovalRequest;
#[cfg(windows)]
use super::ExecPolicyCommandOrigin;
#[cfg(windows)]
use super::ExecPolicyCommands;
use super::ExecPolicyManager;
use super::commands_for_exec_policy;
use crate::shell::Shell;
use crate::tools::sandboxing::ExecApprovalRequirement;
#[cfg(windows)]
use codex_shell_command::powershell::extract_powershell_command;
#[cfg(windows)]
use codex_shell_command::powershell::parse_powershell_script_into_plain_commands;
use codex_tools::UnifiedExecShellMode;
use std::path::Path;

impl ExecPolicyManager {
    pub(crate) async fn create_exec_approval_requirement_for_shell(
        &self,
        mut request: ExecApprovalRequest<'_>,
        configured_shell: &Shell,
        shell_mode: &UnifiedExecShellMode,
    ) -> ExecApprovalRequirement {
        let command = request.command;
        let executable = shell_approval_command(command, configured_shell, shell_mode);
        if executable.len() == command.len() {
            return self
                .create_exec_approval_requirement_for_command(request)
                .await;
        }

        #[cfg(windows)]
        let mut policy_commands = match extract_powershell_command(command) {
            Some((_, script)) => ExecPolicyCommands {
                commands: parse_powershell_script_into_plain_commands(script)
                    .unwrap_or_else(|| vec![command.to_vec()]),
                used_complex_parsing: false,
                command_origin: ExecPolicyCommandOrigin::PowerShell,
            },
            None => commands_for_exec_policy(command),
        };

        #[cfg(not(windows))]
        let mut policy_commands = commands_for_exec_policy(command);

        // Evaluate the executable alongside its apparent commands. Inner
        // commands can add restrictions, but cannot grant the executable trust.
        policy_commands.commands.insert(0, executable.to_vec());
        request.command = executable;
        self.create_exec_approval_requirement_for_parsed_commands(request, policy_commands)
            .await
    }
}

fn shell_approval_command<'a>(
    command: &'a [String],
    configured_shell: &Shell,
    shell_mode: &UnifiedExecShellMode,
) -> &'a [String] {
    let Some(executable) = command.first() else {
        return command;
    };
    let executable_path = Path::new(executable);

    #[cfg(windows)]
    let is_system_shell = std::env::var_os("SystemRoot").is_some_and(|system_root| {
        let system_directory = Path::new(&system_root).join("System32");
        let powershell_directory = system_directory.join("WindowsPowerShell").join("v1.0");
        executable_path.parent().is_some_and(|parent| {
            parent
                .as_os_str()
                .eq_ignore_ascii_case(system_directory.as_os_str())
                || parent
                    .as_os_str()
                    .eq_ignore_ascii_case(powershell_directory.as_os_str())
        })
    });

    #[cfg(not(windows))]
    let is_system_shell = executable_path
        .parent()
        .is_some_and(|parent| parent == Path::new("/bin") || parent == Path::new("/usr/bin"));

    #[cfg(windows)]
    let is_configured_shell = executable_path
        .as_os_str()
        .eq_ignore_ascii_case(configured_shell.shell_path.as_os_str());

    #[cfg(not(windows))]
    let is_configured_shell = executable_path == configured_shell.shell_path.as_path();

    if is_configured_shell
        || is_system_shell
        || matches!(shell_mode, UnifiedExecShellMode::ZshFork(_))
    {
        command
    } else {
        // An unfamiliar executable can ignore its arguments, so evaluate the
        // executable separately from any restrictions on its apparent command.
        std::slice::from_ref(executable)
    }
}

#[cfg(test)]
#[path = "executable_identity_tests.rs"]
mod tests;
