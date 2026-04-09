use std::fs::File;
use std::future::Future;
use std::path::Path;
use std::path::PathBuf;

use codex_apply_patch::CODEX_CORE_APPLY_PATCH_ARG1;
#[cfg(target_os = "linux")]
use codex_sandboxing::landlock::CODEX_LINUX_SANDBOX_ARG0;
use codex_utils_home_dir::find_codex_home;
#[cfg(unix)]
use std::os::unix::fs::symlink;
use tempfile::TempDir;

const APPLY_PATCH_ARG0: &str = "apply_patch";
const MISSPELLED_APPLY_PATCH_ARG0: &str = "applypatch";
#[cfg(unix)]
const EXECVE_WRAPPER_ARG0: &str = "codex-execve-wrapper";
const LOCK_FILENAME: &str = ".lock";
const TOKIO_WORKER_STACK_SIZE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Arg0DispatchPaths {
    /// Stable path to the current Codex executable for child re-execs.
    ///
    /// Prefer this over [`std::env::current_exe()`] in code that may run under
    /// a test harness, where `current_exe()` can point at the harness binary
    /// instead of the real Codex CLI.
    pub codex_self_exe: Option<PathBuf>,
    pub codex_linux_sandbox_exe: Option<PathBuf>,
    pub main_execve_wrapper_exe: Option<PathBuf>,
}

/// Keeps the per-session PATH entry alive and locked for the process lifetime.
pub struct Arg0PathEntryGuard {
    _temp_dir: TempDir,
    _lock_file: File,
    paths: Arg0DispatchPaths,
}

impl Arg0PathEntryGuard {
    fn new(temp_dir: TempDir, lock_file: File, paths: Arg0DispatchPaths) -> Self {
        Self {
            _temp_dir: temp_dir,
            _lock_file: lock_file,
            paths,
        }
    }

    pub fn paths(&self) -> &Arg0DispatchPaths {
        &self.paths
    }
}

pub fn arg0_dispatch() -> Option<Arg0PathEntryGuard> {
    // Determine if we were invoked via the special alias.
    let mut args = std::env::args_os();
    let argv0 = args.next().unwrap_or_default();
    let exe_name = Path::new(&argv0)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    #[cfg(unix)]
    if exe_name == EXECVE_WRAPPER_ARG0 {
        let mut args = std::env::args();
        let _ = args.next();
        let file = match args.next() {
            Some(file) => file,
            None => std::process::exit(1),
        };
        let argv = args.collect::<Vec<_>>();

        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(_) => std::process::exit(1),
        };
        let exit_code = runtime.block_on(
            codex_shell_escalation::run_shell_escalation_execve_wrapper(file, argv),
        );
        match exit_code {
            Ok(exit_code) => std::process::exit(exit_code),
            Err(_) => std::process::exit(1),
        }
    }

    codex_linux_sandbox::dispatch_if_requested();

    if exe_name == APPLY_PATCH_ARG0 || exe_name == MISSPELLED_APPLY_PATCH_ARG0 {
        codex_apply_patch::main();
    }

    let argv1 = args.next().unwrap_or_default();
    if argv1 == CODEX_CORE_APPLY_PATCH_ARG1 {
        let patch_arg = args.next().and_then(|s| s.to_str().map(str::to_owned));
        let exit_code = match patch_arg {
            Some(patch_arg) => {
                let mut stdout = std::io::stdout();
                let mut stderr = std::io::stderr();
                let cwd = match codex_utils_absolute_path::AbsolutePathBuf::current_dir() {
                    Ok(cwd) => cwd,
                    Err(_) => std::process::exit(1),
                };
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(_) => std::process::exit(1),
                };
                match runtime.block_on(codex_apply_patch::apply_patch(
                    &patch_arg,
                    &cwd,
                    &mut stdout,
                    &mut stderr,
                    codex_exec_server::LOCAL_FS.as_ref(),
                )) {
                    Ok(()) => 0,
                    Err(_) => 1,
                }
            }
            None => {
                eprintln!("Error: {CODEX_CORE_APPLY_PATCH_ARG1} requires a UTF-8 PATCH argument.");
                1
            }
        };
        std::process::exit(exit_code);
    }

    // This modifies the environment, which is not thread-safe, so do this
    // before creating any threads/the Tokio runtime.
    load_dotenv();

    match prepend_path_entry_for_codex_aliases() {
        Ok(path_entry) => Some(path_entry),
        Err(err) => {
            // It is possible that Codex will proceed successfully even if
            // updating the PATH fails, so warn the user and move on.
            eprintln!("WARNING: proceeding, even though we could not update PATH: {err}");
            None
        }
    }
}

/// While we want to deploy the Codex CLI as a single executable for simplicity,
/// we also want to expose some of its functionality as distinct CLIs, so we use
/// the "arg0 trick" to determine which CLI to dispatch. This effectively allows
/// us to simulate deploying multiple executables as a single binary on Mac and
/// Linux (but not Windows).
///
/// When the current executable is invoked through the hard-link or alias named
/// `codex-linux-sandbox` we *directly* execute
/// [`codex_linux_sandbox::run_main`] (which never returns). Otherwise we:
///
/// 1.  Load `.env` values from `~/.codex/.env` before creating any threads.
/// 2.  Construct a Tokio multi-thread runtime.
/// 3.  Capture the current executable path and derive the
///     `codex-linux-sandbox` helper path (falling back to the current
///     executable if needed) so children can re-invoke the sandbox when running
///     on Linux.
/// 4.  Execute the provided async `main_fn` inside that runtime, forwarding any
///     error. Note that `main_fn` receives [`Arg0DispatchPaths`], which
///     contains the helper executable paths needed to construct
///     [`codex_core::config::Config`].
///
/// This function should be used to wrap any `main()` function in binary crates
/// in this workspace that depends on these helper CLIs.
pub fn arg0_dispatch_or_else<F, Fut>(main_fn: F) -> anyhow::Result<()>
where
    F: FnOnce(Arg0DispatchPaths) -> Fut,
    Fut: Future<Output = anyhow::Result<()>>,
{
    // Retain the TempDir so it exists for the lifetime of the invocation of
    // this executable. Admittedly, we could invoke `keep()` on it, but it
    // would be nice to avoid leaving temporary directories behind, if possible.
    let path_entry_guard = arg0_dispatch();

    // Regular invocation – create a Tokio runtime and execute the provided
    // async entry-point.
    let runtime = build_runtime()?;
    runtime.block_on(async move {
        let current_exe = std::env::current_exe().ok();
        let paths = Arg0DispatchPaths {
            codex_self_exe: current_exe.clone(),
            codex_linux_sandbox_exe: if cfg!(target_os = "linux") {
                linux_sandbox_exe_path(path_entry_guard.as_ref(), current_exe)
            } else {
                None
            },
            main_execve_wrapper_exe: path_entry_guard
                .as_ref()
                .and_then(|path_entry| path_entry.paths().main_execve_wrapper_exe.clone()),
        };

        main_fn(paths).await
    })
}

fn linux_sandbox_exe_path(
    path_entry_guard: Option<&Arg0PathEntryGuard>,
    current_exe: Option<PathBuf>,
) -> Option<PathBuf> {
    // Prefer the `codex-linux-sandbox` alias when available so callers can
    // re-exec through a path whose basename still triggers arg0 dispatch on
    // bubblewrap builds that do not support `--argv0`.
    path_entry_guard
        .and_then(|path_entry| path_entry.paths().codex_linux_sandbox_exe.clone())
        .or(current_exe)
}

fn build_runtime() -> anyhow::Result<tokio::runtime::Runtime> {
    let mut builder = tokio::runtime::Builder::new_multi_thread();
    builder.enable_all();
    builder.thread_stack_size(TOKIO_WORKER_STACK_SIZE_BYTES);
    Ok(builder.build()?)
}

const ILLEGAL_ENV_VAR_PREFIX: &str = "CODEX_";

/// Load env vars from ~/.codex/.env.
///
/// Security: Do not allow `.env` files to create or modify any variables
/// with names starting with `CODEX_`.
fn load_dotenv() {
    if let Ok(codex_home) = find_codex_home()
        && let Ok(iter) = dotenvy::from_path_iter(codex_home.join(".env"))
    {
        set_filtered(iter);
    }
}

/// Helper to set vars from a dotenvy iterator while filtering out `CODEX_` keys.
fn set_filtered<I>(iter: I)
where
    I: IntoIterator<Item = Result<(String, String), dotenvy::Error>>,
{
    for (key, value) in iter.into_iter().flatten() {
        if !key.to_ascii_uppercase().starts_with(ILLEGAL_ENV_VAR_PREFIX) {
            // It is safe to call set_var() because our process is
            // single-threaded at this point in its execution.
            unsafe { std::env::set_var(&key, &value) };
        }
    }
}

/// Creates a temporary directory with either:
///
/// - UNIX: `apply_patch` symlink to the current executable
/// - WINDOWS: `apply_patch.bat` batch script to invoke the current executable
///   with the "secret" --codex-run-as-apply-patch flag.
///
/// This temporary directory is prepended to the PATH environment variable so
/// that `apply_patch` can be on the PATH without requiring the user to
/// install a separate `apply_patch` executable, simplifying the deployment of
/// Codex CLI.
/// Note: In debug builds the temp-dir guard is disabled to ease local testing.
///
/// IMPORTANT: This function modifies the PATH environment variable, so it MUST
/// be called before multiple threads are spawned.
pub fn prepend_path_entry_for_codex_aliases() -> std::io::Result<Arg0PathEntryGuard> {
    let codex_home = find_codex_home()?;
    #[cfg(not(debug_assertions))]
    {
        // Guard against placing helpers in system temp directories outside debug builds.
        let temp_root = std::env::temp_dir();
        if codex_home.starts_with(&temp_root) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "Refusing to create helper binaries under temporary dir {temp_root:?} (codex_home: {codex_home:?})"
                ),
            ));
        }
    }

    std::fs::create_dir_all(&codex_home)?;
    // Use a CODEX_HOME-scoped temp root to avoid cluttering the top-level directory.
    let temp_root = codex_home.join("tmp").join("arg0");
    std::fs::create_dir_all(&temp_root)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        // Ensure only the current user can access the temp directory.
        std::fs::set_permissions(&temp_root, std::fs::Permissions::from_mode(0o700))?;
    }

    // Best-effort cleanup of stale per-session dirs. Ignore failures so startup proceeds.
    if let Err(err) = janitor_cleanup(&temp_root) {
        eprintln!("WARNING: failed to clean up stale arg0 temp dirs: {err}");
    }

    let temp_dir = tempfile::Builder::new()
        .prefix("codex-arg0")
        .tempdir_in(&temp_root)?;
    let path = temp_dir.path();

    let lock_path = path.join(LOCK_FILENAME);
    let lock_file = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)?;
    lock_file.try_lock()?;

    for filename in &[
        APPLY_PATCH_ARG0,
        MISSPELLED_APPLY_PATCH_ARG0,
        #[cfg(target_os = "linux")]
        CODEX_LINUX_SANDBOX_ARG0,
        #[cfg(unix)]
        EXECVE_WRAPPER_ARG0,
    ] {
        let exe = std::env::current_exe()?;

        #[cfg(unix)]
        {
            let link = path.join(filename);
            symlink(&exe, &link)?;
        }

        #[cfg(windows)]
        {
            let batch_script = path.join(format!("{filename}.bat"));
            std::fs::write(
                &batch_script,
                format!(
                    r#"@echo off
"{}" {CODEX_CORE_APPLY_PATCH_ARG1} %*
"#,
                    exe.display()
                ),
            )?;
        }
    }

    #[cfg(unix)]
    const PATH_SEPARATOR: &str = ":";

    #[cfg(windows)]
    const PATH_SEPARATOR: &str = ";";

    let updated_path_env_var = match std::env::var_os("PATH") {
        Some(existing_path) => {
            let mut path_env_var =
                std::ffi::OsString::with_capacity(path.as_os_str().len() + 1 + existing_path.len());
            path_env_var.push(path);
            path_env_var.push(PATH_SEPARATOR);
            path_env_var.push(existing_path);
            path_env_var
        }
        None => path.as_os_str().to_owned(),
    };

    unsafe {
        std::env::set_var("PATH", updated_path_env_var);
    }

    let paths = Arg0DispatchPaths {
        codex_self_exe: std::env::current_exe().ok(),
        codex_linux_sandbox_exe: {
            #[cfg(target_os = "linux")]
            {
                Some(path.join(CODEX_LINUX_SANDBOX_ARG0))
            }
            #[cfg(not(target_os = "linux"))]
            {
                None
            }
        },
        main_execve_wrapper_exe: {
            #[cfg(unix)]
            {
                Some(path.join(EXECVE_WRAPPER_ARG0))
            }
            #[cfg(not(unix))]
            {
                None
            }
        },
    };

    Ok(Arg0PathEntryGuard::new(temp_dir, lock_file, paths))
}

fn janitor_cleanup(temp_root: &Path) -> std::io::Result<()> {
    let entries = match std::fs::read_dir(temp_root) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err),
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        // Skip the directory if locking fails or the lock is currently held.
        let Some(_lock_file) = try_lock_dir(&path)? else {
            continue;
        };

        match std::fs::remove_dir_all(&path) {
            Ok(()) => {}
            // Expected TOCTOU race: directory can disappear after read_dir/lock checks.
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
            Err(err) => return Err(err),
        }
    }

    Ok(())
}

fn try_lock_dir(dir: &Path) -> std::io::Result<Option<File>> {
    let lock_path = dir.join(LOCK_FILENAME);
    let lock_file = match File::options().read(true).write(true).open(&lock_path) {
        Ok(file) => file,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };

    match lock_file.try_lock() {
        Ok(()) => Ok(Some(lock_file)),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(err) => Err(err.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::Arg0DispatchPaths;
    use super::Arg0PathEntryGuard;
    use super::LOCK_FILENAME;
    use super::janitor_cleanup;
    use super::linux_sandbox_exe_path;
    use std::fs;
    use std::fs::File;
    use std::path::Path;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn create_lock(dir: &Path) -> std::io::Result<File> {
        let lock_path = dir.join(LOCK_FILENAME);
        File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(lock_path)
    }

    #[test]
    fn linux_sandbox_exe_path_prefers_codex_linux_sandbox_alias() -> std::io::Result<()> {
        let temp_dir = TempDir::new()?;
        let lock_file = create_lock(temp_dir.path())?;
        let alias_path = temp_dir.path().join("codex-linux-sandbox");
        let path_entry = Arg0PathEntryGuard::new(
            temp_dir,
            lock_file,
            Arg0DispatchPaths {
                codex_self_exe: Some(PathBuf::from("/usr/bin/codex")),
                codex_linux_sandbox_exe: Some(alias_path.clone()),
                main_execve_wrapper_exe: None,
            },
        );

        assert_eq!(
            linux_sandbox_exe_path(Some(&path_entry), Some(PathBuf::from("/usr/bin/codex"))),
            Some(alias_path),
        );
        Ok(())
    }

    #[test]
    fn janitor_skips_dirs_without_lock_file() -> std::io::Result<()> {
        let root = tempfile::tempdir()?;
        let dir = root.path().join("no-lock");
        fs::create_dir(&dir)?;

        janitor_cleanup(root.path())?;

        assert!(dir.exists());
        Ok(())
    }

    #[test]
    fn janitor_skips_dirs_with_held_lock() -> std::io::Result<()> {
        let root = tempfile::tempdir()?;
        let dir = root.path().join("locked");
        fs::create_dir(&dir)?;
        let lock_file = create_lock(&dir)?;
        lock_file.try_lock()?;

        janitor_cleanup(root.path())?;

        assert!(dir.exists());
        Ok(())
    }

    #[test]
    fn janitor_removes_dirs_with_unlocked_lock() -> std::io::Result<()> {
        let root = tempfile::tempdir()?;
        let dir = root.path().join("stale");
        fs::create_dir(&dir)?;
        create_lock(&dir)?;

        janitor_cleanup(root.path())?;

        assert!(!dir.exists());
        Ok(())
    }
}
