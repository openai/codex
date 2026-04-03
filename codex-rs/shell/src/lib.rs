mod snapshot;

pub use snapshot::SNAPSHOT_DIR;
pub use snapshot::SNAPSHOT_RETENTION;
pub use snapshot::ShellSnapshot;
use snapshot::ShellSnapshotState;
pub use snapshot::remove_snapshot_file;
pub use snapshot::snapshot_session_id_from_file_name;

use serde::Deserialize;
use serde::Serialize;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum ShellType {
    Zsh,
    Bash,
    PowerShell,
    Sh,
    Cmd,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shell {
    pub shell_type: ShellType,
    pub shell_path: PathBuf,
    #[serde(skip, default)]
    snapshot_state: ShellSnapshotState,
}

impl Shell {
    pub fn new(shell_type: ShellType, shell_path: PathBuf) -> Self {
        Self {
            shell_type,
            shell_path,
            snapshot_state: ShellSnapshotState::default(),
        }
    }

    pub fn name(&self) -> &'static str {
        match self.shell_type {
            ShellType::Zsh => "zsh",
            ShellType::Bash => "bash",
            ShellType::PowerShell => "powershell",
            ShellType::Sh => "sh",
            ShellType::Cmd => "cmd",
        }
    }

    /// Takes a string of shell and returns the full list of command args to
    /// use with `exec()` to run the shell command.
    pub fn derive_exec_args(&self, command: &str, use_login_shell: bool) -> Vec<String> {
        match self.shell_type {
            ShellType::Zsh | ShellType::Bash | ShellType::Sh => {
                let arg = if use_login_shell { "-lc" } else { "-c" };
                vec![
                    self.shell_path.to_string_lossy().to_string(),
                    arg.to_string(),
                    command.to_string(),
                ]
            }
            ShellType::PowerShell => {
                let mut args = vec![self.shell_path.to_string_lossy().to_string()];
                if !use_login_shell {
                    args.push("-NoProfile".to_string());
                }

                args.push("-Command".to_string());
                args.push(command.to_string());
                args
            }
            ShellType::Cmd => {
                let mut args = vec![self.shell_path.to_string_lossy().to_string()];
                args.push("/c".to_string());
                args.push(command.to_string());
                args
            }
        }
    }
}

impl PartialEq for Shell {
    fn eq(&self, other: &Self) -> bool {
        self.shell_type == other.shell_type && self.shell_path == other.shell_path
    }
}

impl Eq for Shell {}

pub fn detect_shell_type(shell_path: &Path) -> Option<ShellType> {
    match shell_path.as_os_str().to_str() {
        Some("zsh") => Some(ShellType::Zsh),
        Some("sh") => Some(ShellType::Sh),
        Some("cmd") => Some(ShellType::Cmd),
        Some("bash") => Some(ShellType::Bash),
        Some("pwsh") => Some(ShellType::PowerShell),
        Some("powershell") => Some(ShellType::PowerShell),
        _ => {
            let shell_name = shell_path.file_stem()?;
            let shell_name_path = Path::new(shell_name);
            if shell_name_path == shell_path {
                return None;
            }
            detect_shell_type(shell_name_path)
        }
    }
}

pub fn get_shell_by_model_provided_path(shell_path: &Path) -> Shell {
    detect_shell_type(shell_path)
        .and_then(|shell_type| get_shell(shell_type, Some(shell_path)))
        .unwrap_or_else(ultimate_fallback_shell)
}

pub fn get_shell(shell_type: ShellType, path: Option<&Path>) -> Option<Shell> {
    match shell_type {
        ShellType::Zsh => get_zsh_shell(path),
        ShellType::Bash => get_bash_shell(path),
        ShellType::PowerShell => get_powershell_shell(path),
        ShellType::Sh => get_sh_shell(path),
        ShellType::Cmd => get_cmd_shell(path),
    }
}

pub fn default_user_shell() -> Shell {
    default_user_shell_from_path(get_user_shell_path())
}

#[cfg(unix)]
fn get_user_shell_path() -> Option<PathBuf> {
    let uid = unsafe { libc::getuid() };
    use std::ffi::CStr;
    use std::mem::MaybeUninit;
    use std::ptr;

    let mut passwd = MaybeUninit::<libc::passwd>::uninit();

    // We cannot use getpwuid here: it returns pointers into libc-managed
    // storage, which is not safe to read concurrently on all targets (the musl
    // static build used by the CLI can segfault when parallel callers race on
    // that buffer). getpwuid_r keeps the passwd data in caller-owned memory.
    let suggested_buffer_len = unsafe { libc::sysconf(libc::_SC_GETPW_R_SIZE_MAX) };
    let buffer_len = usize::try_from(suggested_buffer_len)
        .ok()
        .filter(|len| *len > 0)
        .unwrap_or(1024);
    let mut buffer = vec![0; buffer_len];

    loop {
        let mut result = ptr::null_mut();
        let status = unsafe {
            libc::getpwuid_r(
                uid,
                passwd.as_mut_ptr(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                &mut result,
            )
        };

        if status == 0 {
            if result.is_null() {
                return None;
            }

            let passwd = unsafe { passwd.assume_init_ref() };
            if passwd.pw_shell.is_null() {
                return None;
            }

            let shell_path = unsafe { CStr::from_ptr(passwd.pw_shell) }
                .to_string_lossy()
                .into_owned();
            return Some(PathBuf::from(shell_path));
        }

        if status != libc::ERANGE {
            return None;
        }

        // Retry with a larger buffer until libc can materialize the passwd entry.
        let new_len = buffer.len().checked_mul(2)?;
        if new_len > 1024 * 1024 {
            return None;
        }
        buffer.resize(new_len, 0);
    }
}

#[cfg(not(unix))]
fn get_user_shell_path() -> Option<PathBuf> {
    None
}

fn default_user_shell_from_path(user_shell_path: Option<PathBuf>) -> Shell {
    if cfg!(windows) {
        get_shell(ShellType::PowerShell, /*path*/ None).unwrap_or_else(ultimate_fallback_shell)
    } else {
        let user_default_shell = user_shell_path
            .and_then(|shell| detect_shell_type(&shell))
            .and_then(|shell_type| get_shell(shell_type, /*path*/ None));

        let shell_with_fallback = if cfg!(target_os = "macos") {
            user_default_shell
                .or_else(|| get_shell(ShellType::Zsh, /*path*/ None))
                .or_else(|| get_shell(ShellType::Bash, /*path*/ None))
        } else {
            user_default_shell
                .or_else(|| get_shell(ShellType::Bash, /*path*/ None))
                .or_else(|| get_shell(ShellType::Zsh, /*path*/ None))
        };

        shell_with_fallback.unwrap_or_else(ultimate_fallback_shell)
    }
}

fn file_exists(path: &Path) -> Option<PathBuf> {
    if std::fs::metadata(path).is_ok_and(|metadata| metadata.is_file()) {
        Some(path.to_path_buf())
    } else {
        None
    }
}

fn get_shell_path(
    shell_type: ShellType,
    provided_path: Option<&Path>,
    binary_name: &str,
    fallback_paths: &[&str],
) -> Option<PathBuf> {
    // If exact provided path exists, use it.
    if provided_path.and_then(file_exists).is_some() {
        return provided_path.map(Path::to_path_buf);
    }

    // Check whether the shell we are trying to load is the user's default
    // shell and prefer that exact path when available.
    let default_shell_path = get_user_shell_path();
    if let Some(default_shell_path) = default_shell_path
        && detect_shell_type(&default_shell_path) == Some(shell_type)
        && file_exists(&default_shell_path).is_some()
    {
        return Some(default_shell_path);
    }

    if let Ok(path) = which::which(binary_name) {
        return Some(path);
    }

    for path in fallback_paths {
        if let Some(path) = file_exists(Path::new(path)) {
            return Some(path);
        }
    }

    None
}

const ZSH_FALLBACK_PATHS: &[&str] = &["/bin/zsh"];

fn get_zsh_shell(path: Option<&Path>) -> Option<Shell> {
    let shell_path = get_shell_path(ShellType::Zsh, path, "zsh", ZSH_FALLBACK_PATHS);

    shell_path.map(|shell_path| Shell::new(ShellType::Zsh, shell_path))
}

const BASH_FALLBACK_PATHS: &[&str] = &["/bin/bash"];

fn get_bash_shell(path: Option<&Path>) -> Option<Shell> {
    let shell_path = get_shell_path(ShellType::Bash, path, "bash", BASH_FALLBACK_PATHS);

    shell_path.map(|shell_path| Shell::new(ShellType::Bash, shell_path))
}

const SH_FALLBACK_PATHS: &[&str] = &["/bin/sh"];

fn get_sh_shell(path: Option<&Path>) -> Option<Shell> {
    let shell_path = get_shell_path(ShellType::Sh, path, "sh", SH_FALLBACK_PATHS);

    shell_path.map(|shell_path| Shell::new(ShellType::Sh, shell_path))
}

// Note the `pwsh` and `powershell` fallback paths are where the respective
// shells are commonly installed on GitHub Actions Windows runners, but may not
// be present on all Windows machines:
// https://docs.github.com/en/actions/tutorials/build-and-test-code/powershell
#[cfg(windows)]
const PWSH_FALLBACK_PATHS: &[&str] = &[r#"C:\Program Files\PowerShell\7\pwsh.exe"#];
#[cfg(not(windows))]
const PWSH_FALLBACK_PATHS: &[&str] = &["/usr/local/bin/pwsh"];

#[cfg(windows)]
const POWERSHELL_FALLBACK_PATHS: &[&str] =
    &[r#"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"#];
#[cfg(not(windows))]
const POWERSHELL_FALLBACK_PATHS: &[&str] = &[];

fn get_powershell_shell(path: Option<&Path>) -> Option<Shell> {
    let shell_path = get_shell_path(ShellType::PowerShell, path, "pwsh", PWSH_FALLBACK_PATHS)
        .or_else(|| {
            get_shell_path(
                ShellType::PowerShell,
                path,
                "powershell",
                POWERSHELL_FALLBACK_PATHS,
            )
        });

    shell_path.map(|shell_path| Shell::new(ShellType::PowerShell, shell_path))
}

fn get_cmd_shell(path: Option<&Path>) -> Option<Shell> {
    let shell_path = get_shell_path(ShellType::Cmd, path, "cmd", &[]);

    shell_path.map(|shell_path| Shell::new(ShellType::Cmd, shell_path))
}

fn ultimate_fallback_shell() -> Shell {
    if cfg!(windows) {
        Shell::new(ShellType::Cmd, PathBuf::from("cmd.exe"))
    } else {
        Shell::new(ShellType::Sh, PathBuf::from("/bin/sh"))
    }
}

#[cfg(test)]
mod detect_shell_type_tests {
    use super::*;

    #[test]
    fn test_detect_shell_type() {
        assert_eq!(detect_shell_type(Path::new("zsh")), Some(ShellType::Zsh));
        assert_eq!(detect_shell_type(Path::new("bash")), Some(ShellType::Bash));
        assert_eq!(
            detect_shell_type(Path::new("pwsh")),
            Some(ShellType::PowerShell)
        );
        assert_eq!(
            detect_shell_type(Path::new("powershell")),
            Some(ShellType::PowerShell)
        );
        assert_eq!(detect_shell_type(Path::new("fish")), None);
        assert_eq!(detect_shell_type(Path::new("other")), None);
        assert_eq!(
            detect_shell_type(Path::new("/bin/zsh")),
            Some(ShellType::Zsh)
        );
        assert_eq!(
            detect_shell_type(Path::new("/bin/bash")),
            Some(ShellType::Bash)
        );
        assert_eq!(
            detect_shell_type(Path::new("powershell.exe")),
            Some(ShellType::PowerShell)
        );
        assert_eq!(
            detect_shell_type(Path::new(if cfg!(windows) {
                "C:\\windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe"
            } else {
                "/usr/local/bin/pwsh"
            })),
            Some(ShellType::PowerShell)
        );
        assert_eq!(
            detect_shell_type(Path::new("pwsh.exe")),
            Some(ShellType::PowerShell)
        );
        assert_eq!(
            detect_shell_type(Path::new("/usr/local/bin/pwsh")),
            Some(ShellType::PowerShell)
        );
        assert_eq!(detect_shell_type(Path::new("/bin/sh")), Some(ShellType::Sh));
        assert_eq!(detect_shell_type(Path::new("sh")), Some(ShellType::Sh));
        assert_eq!(detect_shell_type(Path::new("cmd")), Some(ShellType::Cmd));
        assert_eq!(
            detect_shell_type(Path::new("cmd.exe")),
            Some(ShellType::Cmd)
        );
    }
}

#[cfg(test)]
#[path = "shell_tests.rs"]
mod tests;
