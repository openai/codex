use super::*;
use pretty_assertions::assert_eq;

#[test]
fn windows_executable_lookup_key_matrix() {
    let cases = [
        (r"C:\workspace\Git.ExE.", "git", false),
        (r"C:\workspace\git.exe ", "git", false),
        (r"\\server\share\git.exe.", "git", false),
        (r"\\server\share\git.exe ", "git", false),
        (r"\\?\C:\workspace\git.exe", "git", true),
        (r"\\?\C:\workspace\git.exe.", "git.exe.", true),
        (r"\\?\C:\workspace\git.exe ", "git.exe ", true),
        (r"\\?\UNC\server\share\git.exe", "git", true),
        (r"\\?\UNC\server\share\git.exe.", "git.exe.", true),
        (r"\\?\UNC\server\share\git.exe ", "git.exe ", true),
        (r"\\.\C:\workspace\git.exe", "git", true),
        (r"\\.\C:\workspace\git.exe.", "git.exe.", true),
        (r"\\.\C:\workspace\git.exe ", "git.exe ", true),
        (r"\\.\UNC\server\share\git.exe", "git", true),
        (r"\\.\UNC\server\share\git.exe.", "git.exe.", true),
        (r"\\.\UNC\server\share\git.exe ", "git.exe ", true),
    ];

    for (raw_path, expected_key, namespace) in cases {
        let path = Path::new(raw_path);
        assert_eq!(
            (
                executable_path_lookup_key(path).as_deref(),
                has_windows_verbatim_or_device_prefix(path),
            ),
            (Some(expected_key), namespace),
            "{}",
            raw_path
        );
    }

    for alias in ["powershell.exe.", "powershell.exe ", "PowerShell.ExE. . "] {
        assert_eq!(executable_lookup_key(alias), "powershell", "{alias}");
    }
}
