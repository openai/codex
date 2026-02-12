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

fn is_git_global_option_with_value(arg: &str) -> bool {
    matches!(
        arg,
        "-C" | "-c"
            | "--config-env"
            | "--exec-path"
            | "--git-dir"
            | "--namespace"
            | "--super-prefix"
            | "--work-tree"
    )
}

fn is_git_global_option_with_inline_value(arg: &str) -> bool {
    matches!(
        arg,
        s if s.starts_with("--config-env=")
            || s.starts_with("--exec-path=")
            || s.starts_with("--git-dir=")
            || s.starts_with("--namespace=")
            || s.starts_with("--super-prefix=")
            || s.starts_with("--work-tree=")
    ) || ((arg.starts_with("-C") || arg.starts_with("-c")) && arg.len() > 2)
}

/// Find the first matching git subcommand, skipping known global options that
/// may appear before it (e.g., `-C`, `-c`, `--git-dir`).
///
/// Shared with `is_safe_command` to avoid git-global-option bypasses.
pub(crate) fn find_git_subcommand<'a>(
    command: &'a [String],
    subcommands: &[&str],
) -> Option<(usize, &'a str)> {
    let cmd0 = command.first().map(String::as_str)?;
    if !cmd0.ends_with("git") {
        return None;
    }

    let mut skip_next = false;
    for (idx, arg) in command.iter().enumerate().skip(1) {
        if skip_next {
            skip_next = false;
            continue;
        }

        let arg = arg.as_str();

        if is_git_global_option_with_inline_value(arg) {
            continue;
        }

        if is_git_global_option_with_value(arg) {
            skip_next = true;
            continue;
        }

        if arg == "--" || arg.starts_with('-') {
            continue;
        }

        if subcommands.contains(&arg) {
            return Some((idx, arg));
        }

        // In git, the first non-option token is the subcommand. If it isn't
        // one of the subcommands we're looking for, we must stop scanning to
        // avoid misclassifying later positional args (e.g., branch names).
        return None;
    }

    None
}

fn is_dangerous_to_call_with_exec(command: &[String]) -> bool {
    let cmd0 = command.first().map(String::as_str);

    match cmd0 {
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

    fn vec_str(items: &[&str]) -> Vec<String> {
        items.iter().map(std::string::ToString::to_string).collect()
    }

    #[test]
    fn git_reset_is_not_dangerous() {
        assert!(!command_might_be_dangerous(&vec_str(&["git", "reset"])));
    }

    #[test]
    fn bash_git_reset_is_not_dangerous() {
        assert!(!command_might_be_dangerous(&vec_str(&[
            "bash",
            "-lc",
            "git reset --hard",
        ])));
    }

    #[test]
    fn zsh_git_reset_is_not_dangerous() {
        assert!(!command_might_be_dangerous(&vec_str(&[
            "zsh",
            "-lc",
            "git reset --hard",
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
            "git status",
        ])));
    }

    #[test]
    fn sudo_git_reset_is_not_dangerous() {
        assert!(!command_might_be_dangerous(&vec_str(&[
            "sudo", "git", "reset", "--hard",
        ])));
    }

    #[test]
    fn usr_bin_git_is_not_dangerous() {
        assert!(!command_might_be_dangerous(&vec_str(&[
            "/usr/bin/git",
            "reset",
            "--hard",
        ])));
    }

    #[test]
    fn git_branch_delete_is_not_dangerous() {
        assert!(!command_might_be_dangerous(&vec_str(&[
            "git", "branch", "-d", "feature",
        ])));
        assert!(!command_might_be_dangerous(&vec_str(&[
            "git", "branch", "-D", "feature",
        ])));
        assert!(!command_might_be_dangerous(&vec_str(&[
            "bash",
            "-lc",
            "git branch --delete feature",
        ])));
    }

    #[test]
    fn git_branch_delete_with_stacked_short_flags_is_not_dangerous() {
        assert!(!command_might_be_dangerous(&vec_str(&[
            "git", "branch", "-dv", "feature",
        ])));
        assert!(!command_might_be_dangerous(&vec_str(&[
            "git", "branch", "-vd", "feature",
        ])));
        assert!(!command_might_be_dangerous(&vec_str(&[
            "git", "branch", "-vD", "feature",
        ])));
        assert!(!command_might_be_dangerous(&vec_str(&[
            "git", "branch", "-Dvv", "feature",
        ])));
    }

    #[test]
    fn git_branch_delete_with_global_options_is_not_dangerous() {
        assert!(!command_might_be_dangerous(&vec_str(&[
            "git", "-C", ".", "branch", "-d", "feature",
        ])));
        assert!(!command_might_be_dangerous(&vec_str(&[
            "git",
            "-c",
            "color.ui=false",
            "branch",
            "-D",
            "feature",
        ])));
        assert!(!command_might_be_dangerous(&vec_str(&[
            "bash",
            "-lc",
            "git -C . branch -d feature",
        ])));
    }

    #[test]
    fn git_checkout_reset_is_not_dangerous() {
        // The first non-option token is "checkout", so later positional args
        // like branch names must not be treated as subcommands.
        assert!(!command_might_be_dangerous(&vec_str(&[
            "git", "checkout", "reset",
        ])));
    }

    #[test]
    fn git_push_force_is_not_dangerous() {
        assert!(!command_might_be_dangerous(&vec_str(&[
            "git", "push", "--force", "origin", "main",
        ])));
        assert!(!command_might_be_dangerous(&vec_str(&[
            "git", "push", "-f", "origin", "main",
        ])));
        assert!(!command_might_be_dangerous(&vec_str(&[
            "git",
            "-C",
            ".",
            "push",
            "--force-with-lease",
            "origin",
            "main",
        ])));
    }

    #[test]
    fn git_push_plus_refspec_is_not_dangerous() {
        assert!(!command_might_be_dangerous(&vec_str(&[
            "git", "push", "origin", "+main",
        ])));
        assert!(!command_might_be_dangerous(&vec_str(&[
            "git",
            "push",
            "origin",
            "+refs/heads/main:refs/heads/main",
        ])));
    }

    #[test]
    fn git_push_delete_flag_is_not_dangerous() {
        assert!(!command_might_be_dangerous(&vec_str(&[
            "git", "push", "--delete", "origin", "feature",
        ])));
        assert!(!command_might_be_dangerous(&vec_str(&[
            "git", "push", "-d", "origin", "feature",
        ])));
    }

    #[test]
    fn git_push_delete_refspec_is_not_dangerous() {
        assert!(!command_might_be_dangerous(&vec_str(&[
            "git", "push", "origin", ":feature",
        ])));
        assert!(!command_might_be_dangerous(&vec_str(&[
            "bash",
            "-lc",
            "git push origin :feature",
        ])));
    }

    #[test]
    fn git_push_without_force_is_not_dangerous() {
        assert!(!command_might_be_dangerous(&vec_str(&[
            "git", "push", "origin", "main",
        ])));
    }

    #[test]
    fn git_clean_force_is_not_dangerous_even_when_f_is_not_first_flag() {
        assert!(!command_might_be_dangerous(&vec_str(&[
            "git", "clean", "-fdx",
        ])));
        assert!(!command_might_be_dangerous(&vec_str(&[
            "git", "clean", "-xdf",
        ])));
        assert!(!command_might_be_dangerous(&vec_str(&[
            "git", "clean", "--force",
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
}
