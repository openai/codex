use super::*;
use pretty_assertions::assert_eq;
use std::io;
use std::path::PathBuf;

const DEFAULT_SYSTEM_ROOT: &str = r"C:\Windows\System32";
const DEFAULT_PROGRAM_FILES_ROOT: &str = r"C:\Program Files";
const DEFAULT_WINDOWS_POWERSHELL_EXE: &str =
    r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe";
const DEFAULT_WINDOWS_PWSH_EXE: &str = r"C:\Program Files\PowerShell\7\pwsh.exe";

fn default_root(root: TrustedPowerShellRoot) -> io::Result<PathBuf> {
    Ok(match root {
        TrustedPowerShellRoot::System => PathBuf::from(DEFAULT_SYSTEM_ROOT),
        TrustedPowerShellRoot::ProgramFiles => PathBuf::from(DEFAULT_PROGRAM_FILES_ROOT),
    })
}

fn resolve_existing(executable: &str) -> Option<PathBuf> {
    resolve_trusted_powershell_parser_executable_with(
        executable,
        default_root,
        |path| Ok(path.to_path_buf()),
        |_| true,
    )
}

#[test]
fn accepts_only_authoritative_powershell_paths() {
    assert_eq!(
        resolve_existing(DEFAULT_WINDOWS_POWERSHELL_EXE),
        Some(PathBuf::from(DEFAULT_WINDOWS_POWERSHELL_EXE)),
    );
    assert_eq!(
        resolve_existing(r"c:/WINDOWS/System32/WindowsPowerShell/v1.0/POWERSHELL.EXE"),
        Some(PathBuf::from(DEFAULT_WINDOWS_POWERSHELL_EXE)),
    );
    assert_eq!(
        resolve_existing(r"\\?\C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"),
        Some(PathBuf::from(DEFAULT_WINDOWS_POWERSHELL_EXE)),
    );
    assert_eq!(
        resolve_existing(DEFAULT_WINDOWS_PWSH_EXE),
        Some(PathBuf::from(DEFAULT_WINDOWS_PWSH_EXE)),
    );
    assert_eq!(
        resolve_existing(r"c:/program files/powershell/7/PWSH.EXE"),
        Some(PathBuf::from(DEFAULT_WINDOWS_PWSH_EXE)),
    );
}

#[test]
fn derives_trust_from_non_c_known_folders() {
    let resolve_root = |root| {
        Ok(match root {
            TrustedPowerShellRoot::System => PathBuf::from(r"D:\Windows\System32"),
            TrustedPowerShellRoot::ProgramFiles => PathBuf::from(r"E:\Program Files"),
        })
    };
    let resolve = |executable| {
        resolve_trusted_powershell_parser_executable_with(
            executable,
            resolve_root,
            |path| Ok(path.to_path_buf()),
            |_| true,
        )
    };

    assert_eq!(
        resolve(r"d:/WINDOWS/System32/WindowsPowerShell/v1.0/POWERSHELL.EXE"),
        Some(PathBuf::from(
            r"D:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"
        )),
    );
    assert_eq!(
        resolve(r"E:\Program Files\PowerShell\7\pwsh.exe"),
        Some(PathBuf::from(r"E:\Program Files\PowerShell\7\pwsh.exe")),
    );
    assert_eq!(resolve(DEFAULT_WINDOWS_POWERSHELL_EXE), None);
    assert_eq!(resolve(DEFAULT_WINDOWS_PWSH_EXE), None);
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
fn fails_closed_when_a_known_folder_or_install_is_missing() {
    let resolve_root = |root| match root {
        TrustedPowerShellRoot::System => Err(io::Error::new(
            io::ErrorKind::NotFound,
            "system root unavailable",
        )),
        TrustedPowerShellRoot::ProgramFiles => Ok(PathBuf::from(DEFAULT_PROGRAM_FILES_ROOT)),
    };

    assert_eq!(
        resolve_trusted_powershell_parser_executable_with(
            DEFAULT_WINDOWS_POWERSHELL_EXE,
            resolve_root,
            |path| Ok(path.to_path_buf()),
            |_| true,
        ),
        None,
    );
    assert_eq!(
        resolve_trusted_powershell_parser_executable_with(
            DEFAULT_WINDOWS_PWSH_EXE,
            resolve_root,
            |path| Ok(path.to_path_buf()),
            |_| true,
        ),
        Some(PathBuf::from(DEFAULT_WINDOWS_PWSH_EXE)),
    );
    assert_eq!(
        resolve_trusted_powershell_parser_executable_with(
            DEFAULT_WINDOWS_PWSH_EXE,
            resolve_root,
            |path| Ok(path.to_path_buf()),
            |_| false,
        ),
        None,
    );
}

#[test]
fn rejects_canonicalization_failures_and_reparse_escape() {
    assert_eq!(
        resolve_trusted_powershell_parser_executable_with(
            DEFAULT_WINDOWS_POWERSHELL_EXE,
            default_root,
            |_| Err(io::Error::other("canonicalization failed")),
            |_| true,
        ),
        None,
    );

    let escaped = resolve_trusted_powershell_parser_executable_with(
        DEFAULT_WINDOWS_POWERSHELL_EXE,
        default_root,
        |path| {
            if path
                .to_string_lossy()
                .ends_with(r"WindowsPowerShell\v1.0\powershell.exe")
            {
                Ok(PathBuf::from(r"C:\Users\attacker\powershell.exe"))
            } else {
                Ok(path.to_path_buf())
            }
        },
        |_| true,
    );
    assert_eq!(escaped, None);
}
