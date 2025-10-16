use crate::bash::parse_bash_lc_plain_commands;

pub fn command_might_be_dangerous(command: &[String]) -> bool {
    if is_dangerous_to_call_with_exec(command) {
        return true;
    }

    // Support `bash -lc "<script>"` where the any part of the script might contain a dangerous command.
    if let Some(all_commands) = parse_bash_lc_plain_commands(command)
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
        Some(cmd) if cmd.ends_with("git") => {
            let subcommand = command.get(1).map(String::as_str);

            if matches!(
                subcommand,
                Some(
                    "reset"
                        | "rm"
                        | "checkout"
                        | "clean"
                        | "stash"
                        | "rebase"
                        | "cherry-pick"
                        | "merge"
                        | "log"
                )
            ) {
                if matches!(subcommand, Some("log"))
                    && !command
                        .iter()
                        .skip(2)
                        .any(|arg| arg.starts_with("--output"))
                {
                    return false;
                }

                return true;
            }

            if matches!(subcommand, Some("bisect"))
                && matches!(command.get(2).map(String::as_str), Some("reset"))
                && command.iter().any(|arg| arg == "--hard")
            {
                return true;
            }

            if matches!(subcommand, Some("pull"))
                && command.iter().skip(2).any(|arg| arg == "--rebase")
            {
                return true;
            }

            false
        }

        Some(cmd) if cmd == "find" || cmd.ends_with("/find") => command
            .iter()
            .skip(1)
            .map(|arg| arg.replace('\\', ""))
            .any(|arg| arg == "-exec" || arg == "-execdir"),

        Some("rm") => {
            matches!(command.get(1).map(String::as_str), Some("-f" | "-rf"))
                || command.iter().skip(1).any(|arg| {
                    matches!(arg.as_str(), "--recursive" | "--no-preserve-root")
                        || (arg.starts_with('-') && (arg.contains('r') || arg.contains('R')))
                })
        }

        Some("dd") => command.iter().skip(1).any(|arg| arg.starts_with("of=")),

        Some("mkfs" | "fdisk" | "parted" | "wipefs") => true,

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
    fn git_checkout_is_dangerous() {
        assert!(command_might_be_dangerous(&vec_str(&[
            "git", "checkout", "main"
        ])));
    }

    #[test]
    fn git_clean_is_dangerous() {
        assert!(command_might_be_dangerous(&vec_str(&[
            "git", "clean", "-fd"
        ])));
    }

    #[test]
    fn git_stash_is_dangerous() {
        assert!(command_might_be_dangerous(&vec_str(&[
            "git", "stash", "--all"
        ])));
    }

    #[test]
    fn git_rebase_is_dangerous() {
        assert!(command_might_be_dangerous(&vec_str(&[
            "git", "rebase", "main"
        ])));
    }

    #[test]
    fn git_cherry_pick_is_dangerous() {
        assert!(command_might_be_dangerous(&vec_str(&[
            "git",
            "cherry-pick",
            "abc123"
        ])));
    }

    #[test]
    fn git_merge_is_dangerous() {
        assert!(command_might_be_dangerous(&vec_str(&[
            "git", "merge", "feature"
        ])));
    }

    #[test]
    fn git_bisect_reset_hard_is_dangerous() {
        assert!(command_might_be_dangerous(&vec_str(&[
            "git", "bisect", "reset", "--hard"
        ])));
    }

    #[test]
    fn git_pull_rebase_is_dangerous() {
        assert!(command_might_be_dangerous(&vec_str(&[
            "git", "pull", "--rebase"
        ])));
    }

    #[test]
    fn git_pull_without_rebase_is_not_dangerous() {
        assert!(!command_might_be_dangerous(&vec_str(&["git", "pull"])));
    }

    #[test]
    fn git_log_with_output_is_dangerous() {
        assert!(command_might_be_dangerous(&vec_str(&[
            "git",
            "log",
            "--pretty=%h",
            "--output=~/.bashrc"
        ])));
    }

    #[test]
    fn git_log_without_output_is_not_dangerous() {
        assert!(!command_might_be_dangerous(&vec_str(&[
            "git",
            "log",
            "--oneline"
        ])));
    }

    #[test]
    fn rm_recursive_variants_are_dangerous() {
        assert!(command_might_be_dangerous(&vec_str(&["rm", "-r", "tmp"])));
        assert!(command_might_be_dangerous(&vec_str(&["rm", "-Rf", "tmp"])));
        assert!(command_might_be_dangerous(&vec_str(&[
            "rm",
            "--recursive",
            "tmp"
        ])));
        assert!(command_might_be_dangerous(&vec_str(&[
            "rm",
            "--no-preserve-root",
            "/"
        ])));
    }

    #[test]
    fn dd_with_of_is_dangerous() {
        assert!(command_might_be_dangerous(&vec_str(&[
            "dd",
            "if=/dev/zero",
            "of=/dev/sda"
        ])));
    }

    #[test]
    fn dd_without_of_is_not_dangerous() {
        assert!(!command_might_be_dangerous(&vec_str(&[
            "dd",
            "if=/tmp/file"
        ])));
    }

    #[test]
    fn disk_tools_are_dangerous() {
        assert!(command_might_be_dangerous(&vec_str(&["mkfs", "/dev/sda1"])));
        assert!(command_might_be_dangerous(&vec_str(&["fdisk", "/dev/sda"])));
        assert!(command_might_be_dangerous(&vec_str(&[
            "parted", "/dev/sda"
        ])));
        assert!(command_might_be_dangerous(&vec_str(&[
            "wipefs", "/dev/sda"
        ])));
    }

    #[test]
    fn find_with_exec_is_dangerous() {
        assert!(command_might_be_dangerous(&vec_str(&[
            "find", ".", "-exec", "python3", "{}", ";"
        ])));
    }

    #[test]
    fn find_with_obfuscated_exec_is_dangerous() {
        assert!(command_might_be_dangerous(&vec_str(&[
            "find",
            ".",
            "-e\\x\\e\\c",
            "nohup",
            "python3"
        ])));
    }
}
