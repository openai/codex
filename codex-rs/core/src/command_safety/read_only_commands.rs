use crate::bash::parse_shell_lc_plain_commands;

pub fn is_read_only_subagent_command(command: &[String]) -> bool {
    if let Some(all_commands) = parse_shell_lc_plain_commands(command)
        && !all_commands.is_empty()
        && all_commands.iter().all(|cmd| is_read_only_exec(cmd))
    {
        return true;
    }

    is_read_only_exec(command)
}

fn is_read_only_exec(command: &[String]) -> bool {
    let Some(cmd0) = command.first().map(String::as_str) else {
        return false;
    };

    match std::path::Path::new(cmd0)
        .file_name()
        .and_then(|osstr| osstr.to_str())
    {
        Some("ls" | "tree" | "head" | "tail" | "wc" | "file" | "stat" | "cat") => true,
        Some("find") => {
            #[rustfmt::skip]
            const UNSAFE_FIND_OPTIONS: &[&str] = &[
                "-exec", "-execdir", "-ok", "-okdir",
                "-delete",
                "-fls", "-fprint", "-fprint0", "-fprintf",
            ];

            !command
                .iter()
                .any(|arg| UNSAFE_FIND_OPTIONS.contains(&arg.as_str()))
        }
        Some("git") => matches!(
            command.get(1).map(String::as_str),
            Some("status" | "log" | "diff" | "blame" | "show")
        ),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::string::ToString;

    fn vec_str(args: &[&str]) -> Vec<String> {
        args.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn allows_read_only_commands() {
        assert_eq!(is_read_only_subagent_command(&vec_str(&["ls"])), true);
        assert_eq!(
            is_read_only_subagent_command(&vec_str(&["git", "status"])),
            true
        );
        assert_eq!(
            is_read_only_subagent_command(&vec_str(&["git", "diff", "--stat"])),
            true
        );
        assert_eq!(
            is_read_only_subagent_command(&vec_str(&["find", ".", "-name", "*.rs"])),
            true
        );
        assert_eq!(is_read_only_subagent_command(&vec_str(&["tree"])), true);
        assert_eq!(
            is_read_only_subagent_command(&vec_str(&["head", "-n", "10"])),
            true
        );
        assert_eq!(
            is_read_only_subagent_command(&vec_str(&["tail", "-n", "10"])),
            true
        );
        assert_eq!(is_read_only_subagent_command(&vec_str(&["wc", "-l"])), true);
        assert_eq!(
            is_read_only_subagent_command(&vec_str(&["cat", "Cargo.toml"])),
            true
        );
        assert_eq!(
            is_read_only_subagent_command(&vec_str(&["file", "Cargo.toml"])),
            true
        );
        assert_eq!(
            is_read_only_subagent_command(&vec_str(&["stat", "Cargo.toml"])),
            true
        );
    }

    #[test]
    fn rejects_mutating_or_unknown_commands() {
        assert_eq!(
            is_read_only_subagent_command(&vec_str(&["rm", "-rf"])),
            false
        );
        assert_eq!(
            is_read_only_subagent_command(&vec_str(&["git", "checkout", "main"])),
            false
        );
        assert_eq!(
            is_read_only_subagent_command(&vec_str(&["find", ".", "-delete"])),
            false
        );
        assert_eq!(
            is_read_only_subagent_command(&vec_str(&["touch", "file"])),
            false
        );
    }

    #[test]
    fn allows_bash_lc_with_safe_commands() {
        assert_eq!(
            is_read_only_subagent_command(&vec_str(&["bash", "-lc", "ls && git status"])),
            true
        );
    }

    #[test]
    fn rejects_bash_lc_with_mutating_commands() {
        assert_eq!(
            is_read_only_subagent_command(&vec_str(&["bash", "-lc", "ls && touch file"])),
            false
        );
    }
}
