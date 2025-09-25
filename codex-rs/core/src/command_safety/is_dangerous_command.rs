use crate::bash::try_parse_bash;
use crate::bash::try_parse_word_only_commands_sequence;

const DANGEROUS_COMMANDS: &[&str] = &["git reset"];

pub fn command_might_be_dangerous(command: &[String]) -> bool {
    if contains_dangerous_command(command) {
        return true;
    }

    // Support `bash -lc "<script>"` where the any part of the script might contain a dangerous command.
    if let [bash, flag, script] = command
        && bash == "bash"
        && flag == "-lc"
        && let Some(tree) = try_parse_bash(script)
        && let Some(all_commands) = try_parse_word_only_commands_sequence(&tree, script)
        && all_commands
            .iter()
            .any(|cmd| contains_dangerous_command(cmd))
    {
        return true;
    }

    false
}

fn contains_dangerous_command(command: &[String]) -> bool {
    if command.is_empty() {
        return false;
    }

    let command_string = command.join(" ");

    DANGEROUS_COMMANDS
        .iter()
        .any(|dangerous| command_string.trim().contains(dangerous))
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
    fn git_reset_with_leading_space_is_dangerous() {
        assert!(command_might_be_dangerous(&vec_str(&["  git", "reset"])));
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
    fn bash_git_reset_with_leading_space_is_dangerous() {
        assert!(command_might_be_dangerous(&vec_str(&[
            "bash",
            "-lc",
            "   git reset --hard"
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
}
