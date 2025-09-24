#![cfg(target_os = "windows")]

pub fn is_safe_command_windows(command: &[String]) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::is_safe_command_windows;

    fn vec_str(args: &[&str]) -> Vec<String> {
        args.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn common_commands_are_not_auto_approved() {
        for cmd in [
            vec_str(&["powershell.exe",  "-NoLogo",  "-Command", "echo hello"]),
            vec_str(&["copy", "foo", "bar"]),
            vec_str(&["del", "file.txt"]),
            vec_str(&["powershell.exe", "Get-ChildItem"]),
        ] {
            assert!(!is_safe_command_windows(&cmd));
        }
    }
}
