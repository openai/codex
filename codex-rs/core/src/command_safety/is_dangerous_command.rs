use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::SandboxPolicy;

use crate::sandboxing::SandboxPermissions;

use crate::bash::parse_shell_lc_plain_commands;
use crate::is_safe_command::is_known_safe_command;
#[cfg(windows)]
#[path = "windows_dangerous_commands.rs"]
mod windows_dangerous_commands;

pub fn requires_initial_appoval(
    policy: AskForApproval,
    sandbox_policy: &SandboxPolicy,
    command: &[String],
    sandbox_permissions: SandboxPermissions,
) -> bool {
    if is_known_safe_command(command) {
        return false;
    }
    match policy {
        AskForApproval::Never | AskForApproval::OnFailure => false,
        AskForApproval::OnRequest => {
            // In DangerFullAccess or ExternalSandbox, only prompt if the command looks dangerous.
            if matches!(
                sandbox_policy,
                SandboxPolicy::DangerFullAccess | SandboxPolicy::ExternalSandbox { .. }
            ) {
                return command_might_be_dangerous(command);
            }

            // In restricted sandboxes (ReadOnly/WorkspaceWrite), do not prompt for
            // non‑escalated, non‑dangerous commands — let the sandbox enforce
            // restrictions (e.g., block network/write) without a user prompt.
            if sandbox_permissions.requires_escalated_permissions() {
                return true;
            }
            command_might_be_dangerous(command)
        }
        AskForApproval::UnlessTrusted => !is_known_safe_command(command),
    }
}

pub fn command_might_be_dangerous(command: &[String]) -> bool {
    #[cfg(windows)]
    {
        if windows_dangerous_commands::is_dangerous_command_windows(command) {
            return true;
        }
    }

    if is_dangerous_to_call_with_exec(command) {
        return true;
    }

    // Support `bash -lc "<script>"` where the any part of the script might contain a dangerous command.
    if let Some(all_commands) = parse_shell_lc_plain_commands(command)
        && all_commands
            .iter()
            .any(|cmd| is_dangerous_to_call_with_exec(cmd))
    {
        return true;
    }

    false
}

fn is_dangerous_to_call_with_exec(command: &[String]) -> bool {
    let cmd0 = command.first().map(String::as_str);

    match cmd0 {
        Some(cmd) if cmd.ends_with("git") || cmd.ends_with("/git") => {
            matches!(command.get(1).map(String::as_str), Some("reset" | "rm"))
        }

        Some("rm") => matches!(command.get(1).map(String::as_str), Some("-f" | "-rf")),

        // for sudo <cmd> simply do the check for <cmd>
        Some("sudo") => is_dangerous_to_call_with_exec(&command[1..]),

        // ── anything else ─────────────────────────────────────────────────
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::protocol::NetworkAccess;

    fn vec_str(items: &[&str]) -> Vec<String> {
        items.iter().map(std::string::ToString::to_string).collect()
    }

    #[test]
    fn git_reset_is_dangerous() {
        assert!(command_might_be_dangerous(&vec_str(&["git", "reset"])));
    }

    #[test]
    fn bash_git_reset_is_dangerous() {
        assert!(command_might_be_dangerous(&vec_str(&[
            "bash",
            "-lc",
            "git reset --hard"
        ])));
    }

    #[test]
    fn zsh_git_reset_is_dangerous() {
        assert!(command_might_be_dangerous(&vec_str(&[
            "zsh",
            "-lc",
            "git reset --hard"
        ])));
    }

    #[test]
    fn git_status_is_not_dangerous() {
        assert!(!command_might_be_dangerous(&vec_str(&["git", "status"])));
    }

    #[test]
    fn bash_git_status_is_not_dangerous() {
        assert!(!command_might_be_dangerous(&vec_str(&[
            "bash",
            "-lc",
            "git status"
        ])));
    }

    #[test]
    fn sudo_git_reset_is_dangerous() {
        assert!(command_might_be_dangerous(&vec_str(&[
            "sudo", "git", "reset", "--hard"
        ])));
    }

    #[test]
    fn usr_bin_git_is_dangerous() {
        assert!(command_might_be_dangerous(&vec_str(&[
            "/usr/bin/git",
            "reset",
            "--hard"
        ])));
    }

    #[test]
    fn rm_rf_is_dangerous() {
        assert!(command_might_be_dangerous(&vec_str(&["rm", "-rf", "/"])));
    }

    #[test]
    fn rm_f_is_dangerous() {
        assert!(command_might_be_dangerous(&vec_str(&["rm", "-f", "/"])));
    }

    #[test]
    fn external_sandbox_only_prompts_for_dangerous_commands() {
        let external_policy = SandboxPolicy::ExternalSandbox {
            network_access: NetworkAccess::Restricted,
        };
        assert!(!requires_initial_appoval(
            AskForApproval::OnRequest,
            &external_policy,
            &vec_str(&["ls"]),
            SandboxPermissions::UseDefault,
        ));
        assert!(requires_initial_appoval(
            AskForApproval::OnRequest,
            &external_policy,
            &vec_str(&["rm", "-rf", "/"]),
            SandboxPermissions::UseDefault,
        ));
    }
}
