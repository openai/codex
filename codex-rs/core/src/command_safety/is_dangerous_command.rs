use crate::bash::parse_shell_lc_plain_commands;
#[cfg(windows)]
#[path = "windows_dangerous_commands.rs"]
mod windows_dangerous_commands;

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
            match command.get(1).map(String::as_str) {
                Some("reset" | "rm") => true,
                Some("branch") => git_branch_is_delete(command),
                _ => false,
            }
        }

        Some("rm") => matches!(command.get(1).map(String::as_str), Some("-f" | "-rf")),

        // for sudo <cmd> simply do the check for <cmd>
        Some("sudo") => is_dangerous_to_call_with_exec(&command[1..]),

        // ── anything else ─────────────────────────────────────────────────
        _ => false,
    }
}

fn git_branch_is_delete(command: &[String]) -> bool {
    command.iter().skip(2).any(|arg| {
        matches!(arg.as_str(), "-d" | "-D" | "--delete")
            || arg.starts_with("--delete=")
            || (arg.starts_with("-d") && arg != "-d")
            || (arg.starts_with("-D") && arg != "-D")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn vec_str(items: &[&str]) -> Vec<String> {
        items.iter().map(std::string::ToString::to_string).collect()
    }

    #[test]
    fn git_reset_is_dangerous() {
        assert_eq!(
            command_might_be_dangerous(&vec_str(&["git", "reset"])),
            true
        );
    }

    #[test]
    fn bash_git_reset_is_dangerous() {
        assert_eq!(
            command_might_be_dangerous(&vec_str(&["bash", "-lc", "git reset --hard"])),
            true
        );
    }

    #[test]
    fn zsh_git_reset_is_dangerous() {
        assert_eq!(
            command_might_be_dangerous(&vec_str(&["zsh", "-lc", "git reset --hard"])),
            true
        );
    }

    #[test]
    fn git_status_is_not_dangerous() {
        assert_eq!(
            command_might_be_dangerous(&vec_str(&["git", "status"])),
            false
        );
    }

    #[test]
    fn bash_git_status_is_not_dangerous() {
        assert_eq!(
            command_might_be_dangerous(&vec_str(&["bash", "-lc", "git status"])),
            false
        );
    }

    #[test]
    fn sudo_git_reset_is_dangerous() {
        assert_eq!(
            command_might_be_dangerous(&vec_str(&["sudo", "git", "reset", "--hard"])),
            true
        );
    }

    #[test]
    fn usr_bin_git_is_dangerous() {
        assert_eq!(
            command_might_be_dangerous(&vec_str(&["/usr/bin/git", "reset", "--hard"])),
            true
        );
    }

    #[test]
    fn git_branch_delete_is_dangerous() {
        assert_eq!(
            command_might_be_dangerous(&vec_str(&["git", "branch", "-d", "feature"])),
            true
        );
        assert_eq!(
            command_might_be_dangerous(&vec_str(&["git", "branch", "-D", "feature"])),
            true
        );
        assert_eq!(
            command_might_be_dangerous(&vec_str(&["bash", "-lc", "git branch --delete feature"])),
            true
        );
    }

    #[test]
    fn rm_rf_is_dangerous() {
        assert_eq!(
            command_might_be_dangerous(&vec_str(&["rm", "-rf", "/"])),
            true
        );
    }

    #[test]
    fn rm_f_is_dangerous() {
        assert_eq!(
            command_might_be_dangerous(&vec_str(&["rm", "-f", "/"])),
            true
        );
    }
}
