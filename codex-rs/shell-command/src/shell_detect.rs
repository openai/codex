use std::ffi::OsStr;
use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum ShellType {
    Zsh,
    Bash,
    PowerShell,
    Sh,
    Cmd,
}

impl ShellType {
    pub fn name(self) -> &'static str {
        match self {
            Self::Zsh => "zsh",
            Self::Bash => "bash",
            Self::PowerShell => "powershell",
            Self::Sh => "sh",
            Self::Cmd => "cmd",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectedShell {
    pub shell_type: ShellType,
    pub shell_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelShellResolveError {
    Unsupported(PathBuf),
    MissingBareName(PathBuf),
    MissingPath(PathBuf),
    NotAFile(PathBuf),
    NotExecutable(PathBuf),
    UnsupportedWindowsLaunch(PathBuf),
    UnsupportedWindowsPathNamespace(PathBuf),
    UnresolvedWindowsRelativePath(PathBuf),
    NonUtf8ResolvedPath(PathBuf),
    RelativeWorkingDirectory(PathBuf),
}

impl std::fmt::Display for ModelShellResolveError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported(path) => {
                write!(
                    formatter,
                    "unsupported model-provided shell `{}`",
                    path.display()
                )
            }
            Self::MissingBareName(path) => write!(
                formatter,
                "model-provided shell `{}` was not found on the invocation PATH",
                path.display()
            ),
            Self::MissingPath(path) => write!(
                formatter,
                "model-provided shell `{}` does not exist",
                path.display()
            ),
            Self::NotAFile(path) => write!(
                formatter,
                "model-provided shell `{}` is not a regular file",
                path.display()
            ),
            Self::NotExecutable(path) => write!(
                formatter,
                "model-provided shell `{}` is not executable",
                path.display()
            ),
            Self::UnsupportedWindowsLaunch(path) => write!(
                formatter,
                "model-provided Windows shell `{}` must resolve to an .exe executable",
                path.display()
            ),
            Self::UnsupportedWindowsPathNamespace(path) => write!(
                formatter,
                "Windows shell resolution refuses remote, device, or verbatim path namespace `{}`",
                path.display()
            ),
            Self::UnresolvedWindowsRelativePath(path) => write!(
                formatter,
                "model-provided Windows shell path `{}` is drive-relative and cannot be resolved against the selected working directory",
                path.display()
            ),
            Self::NonUtf8ResolvedPath(path) => write!(
                formatter,
                "resolved model-provided shell path `{}` is not representable in command argv",
                path.display()
            ),
            Self::RelativeWorkingDirectory(path) => write!(
                formatter,
                "cannot resolve a model-provided shell against relative working directory `{}`",
                path.display()
            ),
        }
    }
}

impl std::error::Error for ModelShellResolveError {}

impl DetectedShell {
    pub fn name(&self) -> &'static str {
        self.shell_type.name()
    }
}

pub fn detect_shell_type(shell_path: impl AsRef<std::path::Path>) -> Option<ShellType> {
    let shell_path = shell_path.as_ref();
    #[cfg(windows)]
    {
        return shell_path
            .as_os_str()
            .to_str()
            .and_then(detect_shell_type_from_hint);
    }
    #[cfg(not(windows))]
    match shell_path.as_os_str().to_str() {
        Some("zsh") => Some(ShellType::Zsh),
        Some("sh") => Some(ShellType::Sh),
        Some("cmd") => Some(ShellType::Cmd),
        Some("bash") => Some(ShellType::Bash),
        Some("pwsh") => Some(ShellType::PowerShell),
        Some("powershell") => Some(ShellType::PowerShell),
        _ => {
            let shell_name = shell_path.file_stem();
            if let Some(shell_name) = shell_name {
                let shell_name_path = std::path::Path::new(shell_name);
                if shell_name_path != shell_path {
                    return detect_shell_type(shell_name_path);
                }
            }
            None
        }
    }
}

/// Detects a requested remote shell type without interpreting the spelling
/// using the controller's native path rules.
///
/// The returned type is only a compatibility hint. Callers must discard the
/// spelling and launch the environment-reported shell executable.
pub fn detect_shell_type_from_hint(shell_hint: &str) -> Option<ShellType> {
    let file_name = shell_hint.rsplit(['/', '\\']).next()?;
    if file_name.is_empty() {
        return None;
    }
    let stem = file_name
        .rsplit_once('.')
        .filter(|(_, extension)| extension.eq_ignore_ascii_case("exe"))
        .map_or(file_name, |(stem, _)| stem);

    if stem.eq_ignore_ascii_case("zsh") {
        Some(ShellType::Zsh)
    } else if stem.eq_ignore_ascii_case("sh") {
        Some(ShellType::Sh)
    } else if stem.eq_ignore_ascii_case("cmd") {
        Some(ShellType::Cmd)
    } else if stem.eq_ignore_ascii_case("bash") {
        Some(ShellType::Bash)
    } else if stem.eq_ignore_ascii_case("pwsh") || stem.eq_ignore_ascii_case("powershell") {
        Some(ShellType::PowerShell)
    } else {
        None
    }
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

fn file_exists(path: &std::path::Path) -> Option<PathBuf> {
    if std::fs::metadata(path).is_ok_and(|metadata| metadata.is_file()) {
        Some(PathBuf::from(path))
    } else {
        None
    }
}

fn get_shell_path(
    shell_type: ShellType,
    provided_path: Option<&PathBuf>,
    binary_name: &str,
    fallback_paths: &[&str],
) -> Option<PathBuf> {
    if let Some(path) = provided_path.and_then(|path| file_exists(path)) {
        return Some(path);
    }

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
        if let Some(path) = file_exists(std::path::Path::new(path)) {
            return Some(path);
        }
    }

    None
}

const ZSH_FALLBACK_PATHS: &[&str] = &["/bin/zsh"];

fn get_zsh_shell(path: Option<&PathBuf>) -> Option<DetectedShell> {
    let shell_path = get_shell_path(ShellType::Zsh, path, "zsh", ZSH_FALLBACK_PATHS);

    shell_path.map(|shell_path| DetectedShell {
        shell_type: ShellType::Zsh,
        shell_path,
    })
}

const BASH_FALLBACK_PATHS: &[&str] = &["/bin/bash", "/usr/bin/bash"];

fn get_bash_shell(path: Option<&PathBuf>) -> Option<DetectedShell> {
    let shell_path = get_shell_path(ShellType::Bash, path, "bash", BASH_FALLBACK_PATHS);

    shell_path.map(|shell_path| DetectedShell {
        shell_type: ShellType::Bash,
        shell_path,
    })
}

const SH_FALLBACK_PATHS: &[&str] = &["/bin/sh"];

fn get_sh_shell(path: Option<&PathBuf>) -> Option<DetectedShell> {
    let shell_path = get_shell_path(ShellType::Sh, path, "sh", SH_FALLBACK_PATHS);

    shell_path.map(|shell_path| DetectedShell {
        shell_type: ShellType::Sh,
        shell_path,
    })
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

fn get_powershell_shell(path: Option<&PathBuf>) -> Option<DetectedShell> {
    let shell_path = get_shell_path(ShellType::PowerShell, path, "pwsh", PWSH_FALLBACK_PATHS)
        .or_else(|| {
            get_shell_path(
                ShellType::PowerShell,
                path,
                "powershell",
                POWERSHELL_FALLBACK_PATHS,
            )
        });

    shell_path.map(|shell_path| DetectedShell {
        shell_type: ShellType::PowerShell,
        shell_path,
    })
}

fn get_cmd_shell(path: Option<&PathBuf>) -> Option<DetectedShell> {
    let shell_path = get_shell_path(ShellType::Cmd, path, "cmd", &[]);

    shell_path.map(|shell_path| DetectedShell {
        shell_type: ShellType::Cmd,
        shell_path,
    })
}

pub fn ultimate_fallback_shell() -> DetectedShell {
    if cfg!(windows) {
        DetectedShell {
            shell_type: ShellType::Cmd,
            shell_path: PathBuf::from("cmd.exe"),
        }
    } else {
        DetectedShell {
            shell_type: ShellType::Sh,
            shell_path: PathBuf::from("/bin/sh"),
        }
    }
}

/// Legacy configured-shell compatibility helper.
///
/// This may fall back to the user's configured/default shell and therefore
/// must not be used for model-selected executable input. Model-selected shells
/// must go through [`resolve_model_provided_shell_in`].
pub fn get_shell_by_model_provided_path(shell_path: &PathBuf) -> DetectedShell {
    detect_shell_type(shell_path)
        .and_then(|shell_type| get_shell(shell_type, Some(shell_path)))
        .unwrap_or_else(ultimate_fallback_shell)
}

/// Resolves a model-selected shell exactly once against the invocation's PATH
/// and selected execution cwd.
///
/// Unlike configured-shell detection, this never falls back to another
/// executable. The returned path is the one policy and runtime must both use.
pub fn resolve_model_provided_shell_in(
    shell_path: &Path,
    search_path: &OsStr,
    path_ext: Option<&OsStr>,
    cwd: &Path,
) -> Result<DetectedShell, ModelShellResolveError> {
    let shell_type = detect_shell_type(shell_path)
        .ok_or_else(|| ModelShellResolveError::Unsupported(shell_path.to_path_buf()))?;
    if !cwd.is_absolute() {
        return Err(ModelShellResolveError::RelativeWorkingDirectory(
            cwd.to_path_buf(),
        ));
    }

    #[cfg(not(windows))]
    let is_bare_name = shell_path.components().count() == 1;
    #[cfg(windows)]
    let resolved_path = resolve_model_shell_path_windows(
        shell_path,
        search_path,
        path_ext.unwrap_or_else(|| OsStr::new("")),
        cwd,
    )?;
    #[cfg(not(windows))]
    let resolved_path = {
        let _ = path_ext;
        if is_bare_name {
            std::env::split_paths(search_path)
                .map(|directory| resolve_relative_path(&directory, cwd).join(shell_path))
                .find(|candidate| validate_model_shell_path(candidate).is_ok())
                .ok_or_else(|| ModelShellResolveError::MissingBareName(shell_path.to_path_buf()))?
        } else {
            resolve_relative_path(shell_path, cwd)
        }
    };

    if resolved_path.to_str().is_none() {
        return Err(ModelShellResolveError::NonUtf8ResolvedPath(resolved_path));
    }
    validate_model_shell_path(&resolved_path)?;
    Ok(DetectedShell {
        shell_type,
        shell_path: resolved_path,
    })
}

fn resolve_relative_path(path: &Path, cwd: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    };
    absolute
        .components()
        .filter(|component| !matches!(component, std::path::Component::CurDir))
        .collect()
}

#[cfg(any(windows, test))]
#[cfg_attr(not(windows), allow(dead_code))]
fn resolve_model_shell_path_windows(
    shell_path: &Path,
    search_path: &OsStr,
    path_ext: &OsStr,
    cwd: &Path,
) -> Result<PathBuf, ModelShellResolveError> {
    validate_windows_local_path_namespace(shell_path)?;
    let path_extensions = path_ext
        .to_string_lossy()
        .split(';')
        .filter(|extension| extension.starts_with('.') && extension.len() > 1)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let candidate_names = windows_executable_candidates(shell_path, &path_extensions);

    let has_separator = shell_path.components().count() > 1;
    if has_separator {
        let exact_path = resolve_windows_path_against_cwd(shell_path, cwd)?;
        if !exact_path.is_absolute() {
            return Err(ModelShellResolveError::UnresolvedWindowsRelativePath(
                shell_path.to_path_buf(),
            ));
        }
        let mut first_specific_error = None;
        for candidate in candidate_names
            .into_iter()
            .map(|candidate| resolve_relative_path(&candidate, cwd))
        {
            match validate_model_shell_path(&candidate) {
                Ok(()) => return Ok(candidate),
                Err(ModelShellResolveError::MissingPath(_)) => {}
                Err(err) if first_specific_error.is_none() => first_specific_error = Some(err),
                Err(_) => {}
            }
        }
        if first_specific_error.is_none()
            && shell_path.extension().is_none()
            && std::fs::metadata(&exact_path).is_ok_and(|metadata| metadata.is_file())
        {
            return Err(ModelShellResolveError::UnsupportedWindowsLaunch(exact_path));
        }
        return Err(first_specific_error.unwrap_or(ModelShellResolveError::MissingPath(exact_path)));
    }

    for directory in std::env::split_paths(search_path) {
        // Preserve PATH ordering: validate an entry immediately before it
        // would be searched, without inspecting later unused entries.
        let resolved_directory = resolve_windows_path_against_cwd(&directory, cwd)?;
        if !resolved_directory.is_absolute() {
            return Err(ModelShellResolveError::UnresolvedWindowsRelativePath(
                directory,
            ));
        }
        for candidate_name in &candidate_names {
            let candidate = resolved_directory.join(candidate_name);
            if validate_model_shell_path(&candidate).is_ok() {
                return Ok(candidate);
            }
        }
    }

    Err(ModelShellResolveError::MissingBareName(
        shell_path.to_path_buf(),
    ))
}

#[cfg(any(windows, test))]
#[cfg_attr(not(windows), allow(dead_code))]
fn validate_windows_local_path_namespace(path: &Path) -> Result<(), ModelShellResolveError> {
    let Some(std::path::Component::Prefix(prefix)) = path.components().next() else {
        return Ok(());
    };

    // Only ordinary drive-letter paths are accepted as prefixed syntax because
    // they do not directly encode a remote host or device namespace. UNC,
    // device, and every verbatim namespace can cause remote or device I/O
    // during metadata lookup and must not be probed.
    //
    // This is not proof that the storage is local: a mapped drive or a reparse
    // point can still redirect a normal drive path before approval. Closing
    // that residual requires a two-phase or handle-based resolution design.
    match prefix.kind() {
        std::path::Prefix::Disk(_) => Ok(()),
        _ => Err(ModelShellResolveError::UnsupportedWindowsPathNamespace(
            path.to_path_buf(),
        )),
    }
}

#[cfg(any(windows, test))]
#[cfg_attr(not(windows), allow(dead_code))]
fn resolve_windows_path_against_cwd(
    path: &Path,
    cwd: &Path,
) -> Result<PathBuf, ModelShellResolveError> {
    validate_windows_local_path_namespace(path)?;

    let has_prefix = matches!(
        path.components().next(),
        Some(std::path::Component::Prefix(_))
    );
    if !path.is_absolute() && !has_prefix {
        // Relative and root-relative paths inherit directory or drive context
        // from cwd, so validate that namespace before joining or probing.
        validate_windows_local_path_namespace(cwd)?;
    }

    Ok(resolve_relative_path(path, cwd))
}

#[cfg(any(windows, test))]
#[cfg_attr(not(windows), allow(dead_code))]
fn windows_executable_candidates(shell_path: &Path, path_extensions: &[String]) -> Vec<PathBuf> {
    match shell_path.extension() {
        Some(extension) if extension.eq_ignore_ascii_case("exe") => {
            vec![shell_path.to_path_buf()]
        }
        Some(_) => Vec::new(),
        None => path_extensions
            .iter()
            .filter(|extension| extension.eq_ignore_ascii_case(".exe"))
            .map(|extension| {
                let mut candidate = shell_path.as_os_str().to_os_string();
                candidate.push(extension);
                PathBuf::from(candidate)
            })
            .collect(),
    }
}

fn validate_model_shell_path(path: &Path) -> Result<(), ModelShellResolveError> {
    let metadata = std::fs::metadata(path)
        .map_err(|_| ModelShellResolveError::MissingPath(path.to_path_buf()))?;
    if !metadata.is_file() {
        return Err(ModelShellResolveError::NotAFile(path.to_path_buf()));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(ModelShellResolveError::NotExecutable(path.to_path_buf()));
        }
    }

    Ok(())
}

pub fn get_shell(shell_type: ShellType, path: Option<&PathBuf>) -> Option<DetectedShell> {
    match shell_type {
        ShellType::Zsh => get_zsh_shell(path),
        ShellType::Bash => get_bash_shell(path),
        ShellType::PowerShell => get_powershell_shell(path),
        ShellType::Sh => get_sh_shell(path),
        ShellType::Cmd => get_cmd_shell(path),
    }
}

pub fn default_user_shell() -> DetectedShell {
    default_user_shell_from_path(get_user_shell_path())
}

pub fn default_user_shell_from_path(user_shell_path: Option<PathBuf>) -> DetectedShell {
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

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_detect_shell_type() {
        assert_eq!(
            detect_shell_type(PathBuf::from("zsh")),
            Some(ShellType::Zsh)
        );
        assert_eq!(
            detect_shell_type(PathBuf::from("bash")),
            Some(ShellType::Bash)
        );
        assert_eq!(
            detect_shell_type(PathBuf::from("pwsh")),
            Some(ShellType::PowerShell)
        );
        assert_eq!(
            detect_shell_type(PathBuf::from("powershell")),
            Some(ShellType::PowerShell)
        );
        assert_eq!(detect_shell_type(PathBuf::from("fish")), None);
        assert_eq!(detect_shell_type(PathBuf::from("other")), None);
        assert_eq!(
            detect_shell_type(PathBuf::from("/bin/zsh")),
            Some(ShellType::Zsh)
        );
        assert_eq!(
            detect_shell_type(PathBuf::from("/bin/bash")),
            Some(ShellType::Bash)
        );
        assert_eq!(
            detect_shell_type(PathBuf::from("/usr/bin/bash")),
            Some(ShellType::Bash)
        );
        assert_eq!(
            detect_shell_type(PathBuf::from("powershell.exe")),
            Some(ShellType::PowerShell)
        );
        assert_eq!(
            detect_shell_type(PathBuf::from(if cfg!(windows) {
                "C:\\windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe"
            } else {
                "/usr/local/bin/pwsh"
            })),
            Some(ShellType::PowerShell)
        );
        assert_eq!(
            detect_shell_type(PathBuf::from("pwsh.exe")),
            Some(ShellType::PowerShell)
        );
        assert_eq!(
            detect_shell_type(PathBuf::from("/usr/local/bin/pwsh")),
            Some(ShellType::PowerShell)
        );
        assert_eq!(
            detect_shell_type(PathBuf::from("/bin/sh")),
            Some(ShellType::Sh)
        );
        assert_eq!(detect_shell_type(PathBuf::from("sh")), Some(ShellType::Sh));
        assert_eq!(
            detect_shell_type(PathBuf::from("cmd")),
            Some(ShellType::Cmd)
        );
        assert_eq!(
            detect_shell_type(PathBuf::from("cmd.exe")),
            Some(ShellType::Cmd)
        );
    }

    #[test]
    fn detects_remote_shell_hints_independently_of_controller_path_syntax() {
        for (hint, expected) in [
            ("bash", Some(ShellType::Bash)),
            ("/attacker/bash", Some(ShellType::Bash)),
            (r"C:\attacker\BASH.ExE", Some(ShellType::Bash)),
            (r"\\server\share\PwSh.EXE", Some(ShellType::PowerShell)),
            ("/opt/PowerShell", Some(ShellType::PowerShell)),
            (r"C:\Windows\System32\cmd.exe", Some(ShellType::Cmd)),
            ("/tmp/fish", None),
            (r"C:\attacker\powershell.exe:payload", None),
            ("/tmp/bash/", None),
            (r"C:\tmp\bash\", None),
        ] {
            assert_eq!(
                detect_shell_type_from_hint(hint),
                expected,
                "unexpected type for {hint:?}"
            );
        }
    }

    #[cfg(unix)]
    fn write_executable(path: &Path) {
        use std::os::unix::fs::PermissionsExt;

        std::fs::write(path, "#!/bin/sh\nexit 0\n").expect("write fake shell");
        let mut permissions = std::fs::metadata(path)
            .expect("fake shell metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).expect("make fake shell executable");
    }

    #[cfg(unix)]
    #[test]
    fn model_shell_resolver_uses_only_supplied_path() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let bin = temp_dir.path().join("bin");
        std::fs::create_dir(&bin).expect("create bin");
        let shell = bin.join("sh");
        write_executable(&shell);

        assert_eq!(
            resolve_model_provided_shell_in(
                Path::new("sh"),
                bin.as_os_str(),
                /*path_ext*/ None,
                temp_dir.path(),
            ),
            Ok(DetectedShell {
                shell_type: ShellType::Sh,
                shell_path: shell,
            })
        );
        assert_eq!(
            resolve_model_provided_shell_in(
                Path::new("sh"),
                OsStr::new(""),
                /*path_ext*/ None,
                temp_dir.path(),
            ),
            Err(ModelShellResolveError::MissingBareName(PathBuf::from("sh")))
        );
    }

    #[cfg(unix)]
    #[test]
    fn model_shell_resolver_resolves_relative_input_and_path_against_selected_cwd() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let tools = temp_dir.path().join("tools");
        std::fs::create_dir(&tools).expect("create tools");
        let shell = tools.join("bash");
        write_executable(&shell);

        for (requested, search_path) in [
            (Path::new("./tools/bash"), OsStr::new("")),
            (Path::new("bash"), OsStr::new("tools")),
        ] {
            assert_eq!(
                resolve_model_provided_shell_in(
                    requested,
                    search_path,
                    /*path_ext*/ None,
                    temp_dir.path(),
                ),
                Ok(DetectedShell {
                    shell_type: ShellType::Bash,
                    shell_path: shell.clone(),
                })
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn model_shell_resolver_rejects_unsupported_missing_and_non_launchable_inputs() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let non_executable = temp_dir.path().join("bash");
        std::fs::write(&non_executable, "#!/bin/sh\n").expect("write non-executable shell");
        let directory = temp_dir.path().join("sh");
        std::fs::create_dir(&directory).expect("create shell-named directory");
        let missing = temp_dir.path().join("zsh");

        assert_eq!(
            resolve_model_provided_shell_in(
                &non_executable,
                OsStr::new(""),
                /*path_ext*/ None,
                temp_dir.path(),
            ),
            Err(ModelShellResolveError::NotExecutable(non_executable))
        );
        assert_eq!(
            resolve_model_provided_shell_in(
                &directory,
                OsStr::new(""),
                /*path_ext*/ None,
                temp_dir.path(),
            ),
            Err(ModelShellResolveError::NotAFile(directory))
        );
        assert_eq!(
            resolve_model_provided_shell_in(
                &missing,
                OsStr::new(""),
                /*path_ext*/ None,
                temp_dir.path(),
            ),
            Err(ModelShellResolveError::MissingPath(missing))
        );
        assert_eq!(
            resolve_model_provided_shell_in(
                Path::new("fish"),
                OsStr::new(""),
                /*path_ext*/ None,
                temp_dir.path(),
            ),
            Err(ModelShellResolveError::Unsupported(PathBuf::from("fish")))
        );
        assert_eq!(
            resolve_model_provided_shell_in(
                Path::new("sh"),
                OsStr::new(""),
                /*path_ext*/ None,
                Path::new("."),
            ),
            Err(ModelShellResolveError::RelativeWorkingDirectory(
                PathBuf::from(".")
            ))
        );
    }

    #[cfg(unix)]
    #[test]
    fn model_shell_resolver_rejects_non_utf8_resolved_path() {
        use std::os::unix::ffi::OsStringExt;

        let temp_dir = tempfile::tempdir().expect("temp dir");
        let non_utf8_cwd = temp_dir
            .path()
            .join(std::ffi::OsString::from_vec(b"cwd-\xff".to_vec()));
        let shell = non_utf8_cwd.join("bash");

        assert_eq!(
            resolve_model_provided_shell_in(
                Path::new("./bash"),
                OsStr::new(""),
                /*path_ext*/ None,
                &non_utf8_cwd,
            ),
            Err(ModelShellResolveError::NonUtf8ResolvedPath(shell))
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_model_shell_resolver_uses_supplied_pathext() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let bin = temp_dir.path().join("bin");
        std::fs::create_dir(&bin).expect("create bin");
        for (requested, file_name) in [("PowerShell.ExE", "PowerShell.ExE"), ("PWSH", "PWSH.EXE")] {
            let shell = bin.join(file_name);
            std::fs::write(&shell, "fixture").expect("write fake executable");
            assert_eq!(
                resolve_model_provided_shell_in(
                    Path::new(requested),
                    bin.as_os_str(),
                    Some(OsStr::new(".EXE")),
                    temp_dir.path(),
                ),
                Ok(DetectedShell {
                    shell_type: ShellType::PowerShell,
                    shell_path: shell,
                })
            );
        }
        assert_eq!(
            resolve_model_provided_shell_in(
                Path::new("powershell"),
                bin.as_os_str(),
                Some(OsStr::new(".CMD")),
                temp_dir.path(),
            ),
            Err(ModelShellResolveError::MissingBareName(PathBuf::from(
                "powershell"
            )))
        );

        let extensionless = temp_dir.path().join("bash");
        std::fs::write(&extensionless, "fixture").expect("write extensionless executable");
        assert_eq!(
            resolve_model_provided_shell_in(
                &extensionless,
                OsStr::new(""),
                Some(OsStr::new("")),
                temp_dir.path(),
            ),
            Err(ModelShellResolveError::UnsupportedWindowsLaunch(
                extensionless.clone()
            ))
        );
        let sibling_exe = temp_dir.path().join("bash.EXE");
        std::fs::write(&sibling_exe, "fixture").expect("write sibling exe");
        assert_eq!(
            resolve_model_provided_shell_in(
                &extensionless,
                OsStr::new(""),
                Some(OsStr::new(".EXE")),
                temp_dir.path(),
            ),
            Ok(DetectedShell {
                shell_type: ShellType::Bash,
                shell_path: sibling_exe,
            })
        );

        for (requested, file_name, path_ext) in [
            ("cmd", "cmd.CMD", ".CMD"),
            ("powershell", "powershell.BAT", ".BAT"),
        ] {
            std::fs::write(bin.join(file_name), "fixture").expect("write script candidate");
            assert_eq!(
                resolve_model_provided_shell_in(
                    Path::new(requested),
                    bin.as_os_str(),
                    Some(OsStr::new(path_ext)),
                    temp_dir.path(),
                ),
                Err(ModelShellResolveError::MissingBareName(PathBuf::from(
                    requested
                )))
            );
        }

        let directory = temp_dir.path().join("sh.exe");
        std::fs::create_dir(&directory).expect("create shell-named directory");
        assert_eq!(
            resolve_model_provided_shell_in(
                &directory,
                OsStr::new(""),
                Some(OsStr::new(".EXE")),
                temp_dir.path(),
            ),
            Err(ModelShellResolveError::NotAFile(directory))
        );

        let drive_relative = PathBuf::from(r"C:relative\powershell.exe");
        assert_eq!(
            resolve_model_provided_shell_in(
                &drive_relative,
                OsStr::new(""),
                Some(OsStr::new(".EXE")),
                temp_dir.path(),
            ),
            Err(ModelShellResolveError::UnresolvedWindowsRelativePath(
                drive_relative
            ))
        );
        let later_shell = bin.join("bash.EXE");
        std::fs::write(&later_shell, "fixture").expect("write later PATH shell");
        let drive_relative_path = PathBuf::from(r"C:relative-bin");
        let search_path = std::env::join_paths([drive_relative_path.as_path(), bin.as_path()])
            .expect("join PATH entries");
        assert_eq!(
            resolve_model_provided_shell_in(
                Path::new("bash"),
                &search_path,
                Some(OsStr::new(".EXE")),
                temp_dir.path(),
            ),
            Err(ModelShellResolveError::UnresolvedWindowsRelativePath(
                drive_relative_path
            ))
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_model_shell_resolver_rejects_remote_device_and_verbatim_namespaces() {
        let temp_dir = tempfile::tempdir().expect("temp dir");

        for requested in [
            PathBuf::from(r"\\server\share\powershell.exe"),
            PathBuf::from(r"//server/share/powershell.exe"),
            PathBuf::from(r"\/server\share/powershell.exe"),
            PathBuf::from(r"\\.\GLOBALROOT\Device\HarddiskVolume1\powershell.exe"),
            PathBuf::from(r"\\?\C:\tools\powershell.exe"),
            PathBuf::from(r"\\?\C:/tools/powershell.exe"),
            PathBuf::from(r"\\?\UNC\server\share\powershell.exe"),
            PathBuf::from(r"\\?\UNC\server/share/powershell.exe"),
            PathBuf::from(r"\\?\Volume{12345678-1234-1234-1234-123456789abc}\powershell.exe"),
            PathBuf::from(r"\\?\Volume{12345678-1234-1234-1234-123456789abc}/powershell.exe"),
        ] {
            assert_eq!(
                resolve_model_provided_shell_in(
                    &requested,
                    OsStr::new(""),
                    Some(OsStr::new(".EXE")),
                    temp_dir.path(),
                ),
                Err(ModelShellResolveError::UnsupportedWindowsPathNamespace(
                    requested.clone()
                )),
                "unsafe namespace should fail before any executable probe: {requested:?}"
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn windows_model_shell_resolver_validates_path_namespaces_in_lookup_order() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let bin = temp_dir.path().join("bin");
        std::fs::create_dir(&bin).expect("create bin");
        let shell = bin.join("powershell.EXE");
        std::fs::write(&shell, "fixture").expect("write fake executable");
        let unsafe_directory = PathBuf::from(r"\\server\share\bin");

        let unsafe_first =
            std::env::join_paths([unsafe_directory.as_path(), bin.as_path()]).expect("join PATH");
        assert_eq!(
            resolve_model_provided_shell_in(
                Path::new("powershell"),
                &unsafe_first,
                Some(OsStr::new(".EXE")),
                temp_dir.path(),
            ),
            Err(ModelShellResolveError::UnsupportedWindowsPathNamespace(
                unsafe_directory.clone()
            ))
        );

        let local_first =
            std::env::join_paths([bin.as_path(), unsafe_directory.as_path()]).expect("join PATH");
        assert_eq!(
            resolve_model_provided_shell_in(
                Path::new("powershell"),
                &local_first,
                Some(OsStr::new(".EXE")),
                temp_dir.path(),
            ),
            Ok(DetectedShell {
                shell_type: ShellType::PowerShell,
                shell_path: shell,
            }),
            "a valid earlier PATH candidate must return before an unused unsafe entry"
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_model_shell_resolver_checks_unsafe_cwd_only_when_resolution_uses_it() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let relative_dir = temp_dir.path().join("tools");
        std::fs::create_dir(&relative_dir).expect("create tools");
        let shell = relative_dir.join("pwsh.exe");
        std::fs::write(&shell, "fixture").expect("write fake executable");

        assert_eq!(
            resolve_model_provided_shell_in(
                Path::new(r"tools\pwsh.exe"),
                OsStr::new(""),
                Some(OsStr::new(".EXE")),
                temp_dir.path(),
            ),
            Ok(DetectedShell {
                shell_type: ShellType::PowerShell,
                shell_path: shell.clone(),
            })
        );

        let unsafe_cwd = PathBuf::from(r"\\server\share\cwd");
        assert_eq!(
            resolve_model_provided_shell_in(
                Path::new(r".\pwsh.exe"),
                OsStr::new(""),
                Some(OsStr::new(".EXE")),
                &unsafe_cwd,
            ),
            Err(ModelShellResolveError::UnsupportedWindowsPathNamespace(
                unsafe_cwd.clone()
            ))
        );

        assert_eq!(
            resolve_model_provided_shell_in(
                &shell,
                OsStr::new(""),
                Some(OsStr::new(".EXE")),
                &unsafe_cwd,
            ),
            Ok(DetectedShell {
                shell_type: ShellType::PowerShell,
                shell_path: shell,
            }),
            "an absolute local shell does not use cwd during resolution"
        );
    }
}
