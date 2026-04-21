use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

/// Shell executable families that Codex treats as known command wrappers.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ShellType {
    Zsh,
    Bash,
    PowerShell,
    Sh,
    Cmd,
}

pub fn detect_shell_type(shell_path: &Path) -> Option<ShellType> {
    let shell_text = shell_path.as_os_str().to_str()?;
    // Keep this exact: repo-local files named like shells must not inherit
    // shell-wrapper trust in approval or display decisions.
    match shell_text {
        "zsh" | "/bin/zsh" | "/usr/bin/zsh" | "/usr/local/bin/zsh" | "/opt/homebrew/bin/zsh" => {
            Some(ShellType::Zsh)
        }
        "sh" | "/bin/sh" | "/usr/bin/sh" => Some(ShellType::Sh),
        "bash"
        | "/bin/bash"
        | "/usr/bin/bash"
        | "/usr/local/bin/bash"
        | "/opt/homebrew/bin/bash" => Some(ShellType::Bash),
        "pwsh"
        | "powershell"
        | "pwsh.exe"
        | "powershell.exe"
        | "/usr/local/bin/pwsh"
        | "/usr/bin/pwsh"
        | "/bin/pwsh"
        | "/opt/homebrew/bin/pwsh" => Some(ShellType::PowerShell),
        "cmd" | "cmd.exe" => Some(ShellType::Cmd),
        _ => match shell_text.replace('\\', "/").to_ascii_lowercase().as_str() {
            "c:/windows/system32/cmd.exe" => Some(ShellType::Cmd),
            "c:/windows/system32/windowspowershell/v1.0/powershell.exe"
            | "c:/program files/powershell/7/pwsh.exe" => Some(ShellType::PowerShell),
            _ => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_exact_shell_names_and_system_paths() {
        assert_eq!(
            detect_shell_type(&PathBuf::from("bash")),
            Some(ShellType::Bash)
        );
        assert_eq!(
            detect_shell_type(&PathBuf::from("/bin/bash")),
            Some(ShellType::Bash)
        );
        assert_eq!(
            detect_shell_type(&PathBuf::from("powershell.exe")),
            Some(ShellType::PowerShell)
        );
        assert_eq!(
            detect_shell_type(&PathBuf::from(
                r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"
            )),
            Some(ShellType::PowerShell)
        );
    }

    #[test]
    fn rejects_shell_like_attacker_controlled_paths() {
        assert_eq!(detect_shell_type(&PathBuf::from(".poc/bash")), None);
        assert_eq!(detect_shell_type(&PathBuf::from("/tmp/bash")), None);
        assert_eq!(detect_shell_type(&PathBuf::from("/tmp/bash.evil")), None);
        assert_eq!(
            detect_shell_type(&PathBuf::from(r"C:\tmp\powershell.exe")),
            None
        );
    }
}
