use super::*;
use pretty_assertions::assert_eq;

fn resolve_existing(executable: &str) -> Option<&'static str> {
    resolve_trusted_powershell_parser_executable(executable, |_| true)
}

#[test]
fn accepts_only_fixed_trusted_powershell_paths() {
    assert_eq!(
        resolve_existing(WINDOWS_POWERSHELL_EXE),
        Some(WINDOWS_POWERSHELL_EXE),
    );
    assert_eq!(
        resolve_existing(r"c:/WINDOWS/System32/WindowsPowerShell/v1.0/POWERSHELL.EXE"),
        Some(WINDOWS_POWERSHELL_EXE),
    );
    assert_eq!(
        resolve_existing(r"\\?\C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"),
        Some(WINDOWS_POWERSHELL_EXE),
    );
    assert_eq!(resolve_existing(WINDOWS_PWSH_EXE), Some(WINDOWS_PWSH_EXE));
    assert_eq!(
        resolve_existing(r"c:/program files/powershell/7/PWSH.EXE"),
        Some(WINDOWS_PWSH_EXE),
    );
}

#[test]
fn rejects_search_path_and_workspace_controlled_variants() {
    for executable in [
        "powershell",
        "powershell.exe",
        "pwsh",
        "pwsh.exe",
        r".\powershell.exe",
        r".\pwsh.exe",
        "./powershell.exe",
        "./pwsh.exe",
        r"tools\powershell.exe",
        r"tools\pwsh.exe",
        r"C:\workspace\powershell.exe",
        r"C:\workspace\pwsh.exe",
        r"\\server\share\pwsh.exe",
    ] {
        assert_eq!(
            resolve_existing(executable),
            None,
            "{executable:?} must not be launched as a parser",
        );
    }
}

#[test]
fn rejects_command_wrappers_and_path_aliases() {
    for executable in [
        r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.cmd",
        r"C:\Program Files\PowerShell\7\pwsh.cmd",
        r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell",
        r"C:\Program Files\PowerShell\7\pwsh",
        r"C:\Windows\System32\WindowsPowerShell\v1.0\..\powershell.exe",
        r"\\.\C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe",
        r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe.cmd",
        r"C:\Program Files\PowerShell\7\pwsh.exe.cmd",
    ] {
        assert_eq!(
            resolve_existing(executable),
            None,
            "{executable:?} must fail closed",
        );
    }
}

#[test]
fn fails_closed_when_the_trusted_install_is_missing() {
    assert_eq!(
        resolve_trusted_powershell_parser_executable(WINDOWS_POWERSHELL_EXE, |_| false),
        None,
    );
    assert_eq!(
        resolve_trusted_powershell_parser_executable(WINDOWS_PWSH_EXE, |_| false),
        None,
    );
}
