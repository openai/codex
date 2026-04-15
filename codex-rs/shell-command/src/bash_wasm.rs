use std::path::PathBuf;

use crate::shell_detect::ShellType;
use crate::shell_detect::detect_shell_type;

pub fn try_parse_shell(_shell_lc_arg: &str) -> Option<()> {
    None
}

pub fn try_parse_word_only_commands_sequence(_tree: &(), _src: &str) -> Option<Vec<Vec<String>>> {
    None
}

pub fn extract_bash_command(command: &[String]) -> Option<(&str, &str)> {
    let [shell, flag, script] = command else {
        return None;
    };
    if !matches!(flag.as_str(), "-lc" | "-c")
        || !matches!(
            detect_shell_type(&PathBuf::from(shell)),
            Some(ShellType::Zsh) | Some(ShellType::Bash) | Some(ShellType::Sh)
        )
    {
        return None;
    }
    Some((shell, script))
}

pub fn parse_shell_lc_plain_commands(_command: &[String]) -> Option<Vec<Vec<String>>> {
    None
}

pub fn parse_shell_lc_single_command_prefix(_command: &[String]) -> Option<Vec<String>> {
    None
}
