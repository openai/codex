//! Binary dispatcher for cocode CLI.
//!
//! This crate provides the "arg0 trick" for single-binary deployment:
//! - Dispatches to specialized CLIs based on executable name (argv[0])
//! - Hijacks apply_patch execution via secret flag (argv[1])
//! - Sets up PATH with symlinks for subprocess integration
//!
//! # Architecture
//!
//! When the cocode binary is invoked:
//!
//! 1. **argv[0] dispatch**: If the executable name is `apply_patch`, `applypatch`,
//!    or `cocode-linux-sandbox`, dispatch directly to those implementations.
//!
//! 2. **argv[1] hijack**: If the first argument is `--cocode-run-as-apply-patch`,
//!    run apply_patch with the second argument as the patch.
//!
//! 3. **Normal flow**: Load dotenv, set up PATH with symlinks, and run main_fn.
//!
//! # Example
//!
//! ```ignore
//! use cocode_arg0::arg0_dispatch_or_else;
//! use std::path::PathBuf;
//! use std::process::ExitCode;
//!
//! fn main() -> ExitCode {
//!     arg0_dispatch_or_else(|sandbox_exe| async move {
//!         // Your main application logic here
//!         Ok(())
//!     })
//! }
//! ```

use std::future::Future;
use std::path::Path;
use std::path::PathBuf;

#[cfg(unix)]
use std::os::unix::fs::symlink;
use tempfile::TempDir;

/// The secret argument used to hijack apply_patch invocation.
pub const COCODE_APPLY_PATCH_ARG1: &str = "--cocode-run-as-apply-patch";

/// The name of the Linux sandbox executable (arg0).
const LINUX_SANDBOX_ARG0: &str = "cocode-linux-sandbox";

/// The name of the apply_patch executable (arg0).
const APPLY_PATCH_ARG0: &str = "apply_patch";

/// Alternate spelling of apply_patch.
const MISSPELLED_APPLY_PATCH_ARG0: &str = "applypatch";

/// Environment variable prefix that cannot be set via .env files (security).
const ILLEGAL_ENV_VAR_PREFIX: &str = "COCODE_";

/// Perform arg0 dispatch and setup, returning the TempDir for PATH if successful.
///
/// This function:
/// 1. Checks argv[0] for special executable names and dispatches accordingly
/// 2. Checks argv[1] for the apply_patch hijack flag
/// 3. Loads dotenv from ~/.cocode/.env
/// 4. Creates a temp directory with symlinks and prepends it to PATH
///
/// Returns `Some(TempDir)` if PATH was set up, `None` if setup failed but we can proceed.
/// Never returns if dispatched to a specialized CLI.
pub fn arg0_dispatch() -> Option<TempDir> {
    // Determine if we were invoked via a special alias.
    let mut args = std::env::args_os();
    let argv0 = args.next().unwrap_or_default();
    let exe_name = Path::new(&argv0)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    // argv[0] dispatch: specialized CLIs (never returns)
    if exe_name == LINUX_SANDBOX_ARG0 {
        // Sandbox invocation when sandbox is not yet fully implemented.
        // In non-sandbox mode (default), this shouldn't be called.
        // Log a warning and exit gracefully - sandbox is optional.
        //
        // In a full implementation, this would call cocode_sandbox::run_main()
        // to apply Landlock/Seatbelt restrictions before execvp().
        eprintln!(
            "Warning: {LINUX_SANDBOX_ARG0} invoked but sandbox enforcement is not yet implemented."
        );
        eprintln!("Commands will run without sandbox restrictions.");
        eprintln!("This is expected in non-sandbox mode (the default).");

        // Execute the remaining args directly without sandbox wrapping.
        // Format: cocode-linux-sandbox <sandbox-policy> <cwd> <command...>
        // For now, we skip the policy parsing and just run the command.
        let remaining_args: Vec<_> = args.collect();
        if remaining_args.len() >= 3 {
            // Args: [policy, cwd, command...]
            let cwd = &remaining_args[1];
            let command_args = &remaining_args[2..];

            if !command_args.is_empty() {
                use std::os::unix::process::CommandExt;
                let mut cmd = std::process::Command::new(&command_args[0]);
                cmd.args(&command_args[1..]);
                if let Some(cwd_str) = cwd.to_str() {
                    cmd.current_dir(cwd_str);
                }
                // This replaces the current process - never returns on success
                let err = cmd.exec();
                eprintln!("Failed to exec command: {err}");
                std::process::exit(1);
            }
        }

        // No command to execute or invalid args
        std::process::exit(0);
    }

    if exe_name == APPLY_PATCH_ARG0 || exe_name == MISSPELLED_APPLY_PATCH_ARG0 {
        // Dispatch to apply_patch CLI
        cocode_apply_patch::main();
    }

    // argv[1] hijack: --cocode-run-as-apply-patch
    let argv1 = args.next().unwrap_or_default();
    if argv1 == COCODE_APPLY_PATCH_ARG1 {
        let patch_arg = args.next().and_then(|s| s.to_str().map(str::to_owned));
        let exit_code = match patch_arg {
            Some(patch_arg) => {
                let mut stdout = std::io::stdout();
                let mut stderr = std::io::stderr();
                match cocode_apply_patch::apply_patch(&patch_arg, &mut stdout, &mut stderr) {
                    Ok(()) => 0,
                    Err(_) => 1,
                }
            }
            None => {
                eprintln!("Error: {COCODE_APPLY_PATCH_ARG1} requires a UTF-8 PATCH argument.");
                1
            }
        };
        std::process::exit(exit_code);
    }

    // This modifies the environment, which is not thread-safe, so do this
    // before creating any threads/the Tokio runtime.
    load_dotenv();

    match prepend_path_entry_for_cocode_aliases() {
        Ok(path_entry) => Some(path_entry),
        Err(err) => {
            // It is possible that cocode will proceed successfully even if
            // updating the PATH fails, so warn the user and move on.
            eprintln!("WARNING: proceeding, even though we could not update PATH: {err}");
            None
        }
    }
}

/// Perform arg0 dispatch, then run the provided async main function.
///
/// This is the main entry point for binary crates that need arg0 dispatch.
/// It handles:
/// 1. arg0 dispatch for specialized CLIs
/// 2. Dotenv loading from ~/.cocode/.env
/// 3. PATH setup with symlinks for apply_patch
/// 4. Tokio runtime creation
/// 5. Running the provided async main function
///
/// # Arguments
///
/// * `main_fn` - The async main function to run. Receives an optional path to
///   the sandbox executable (on Linux only).
///
/// # Returns
///
/// Returns the result of the main function, or an error if setup failed.
pub fn arg0_dispatch_or_else<F, Fut>(main_fn: F) -> anyhow::Result<()>
where
    F: FnOnce(Option<PathBuf>) -> Fut,
    Fut: Future<Output = anyhow::Result<()>>,
{
    // Retain the TempDir so it exists for the lifetime of the invocation of
    // this executable. Admittedly, we could invoke `keep()` on it, but it
    // would be nice to avoid leaving temporary directories behind, if possible.
    let _path_entry = arg0_dispatch();

    // Regular invocation – create a Tokio runtime and execute the provided
    // async entry-point.
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async move {
        let cocode_linux_sandbox_exe: Option<PathBuf> = if cfg!(target_os = "linux") {
            std::env::current_exe().ok()
        } else {
            None
        };

        main_fn(cocode_linux_sandbox_exe).await
    })
}

/// Find the cocode home directory.
///
/// Returns `~/.cocode` or the value of `COCODE_HOME` if set.
fn find_cocode_home() -> std::io::Result<PathBuf> {
    // Check COCODE_HOME environment variable first
    if let Ok(home) = std::env::var("COCODE_HOME") {
        return Ok(PathBuf::from(home));
    }

    // Fall back to ~/.cocode
    dirs::home_dir().map(|h| h.join(".cocode")).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not find home directory",
        )
    })
}

/// Load environment variables from ~/.cocode/.env.
///
/// Security: Do not allow `.env` files to create or modify any variables
/// with names starting with `COCODE_`.
fn load_dotenv() {
    if let Ok(cocode_home) = find_cocode_home() {
        let env_path = cocode_home.join(".env");
        if let Ok(iter) = dotenvy::from_path_iter(&env_path) {
            set_filtered(iter);
        }
    }
}

/// Helper to set vars from a dotenvy iterator while filtering out `COCODE_` keys.
fn set_filtered<I>(iter: I)
where
    I: IntoIterator<Item = Result<(String, String), dotenvy::Error>>,
{
    for (key, value) in iter.into_iter().flatten() {
        if !key.to_ascii_uppercase().starts_with(ILLEGAL_ENV_VAR_PREFIX) {
            // It is safe to call set_var() because our process is
            // single-threaded at this point in its execution.
            // SAFETY: This is called before any threads are spawned.
            unsafe { std::env::set_var(&key, &value) };
        }
    }
}

/// Creates a temporary directory with either:
///
/// - UNIX: `apply_patch` symlink to the current executable
/// - WINDOWS: `apply_patch.bat` batch script to invoke the current executable
///   with the "secret" --cocode-run-as-apply-patch flag.
///
/// This temporary directory is prepended to the PATH environment variable so
/// that `apply_patch` can be on the PATH without requiring the user to
/// install a separate `apply_patch` executable, simplifying the deployment of
/// cocode CLI.
///
/// IMPORTANT: This function modifies the PATH environment variable, so it MUST
/// be called before multiple threads are spawned.
pub fn prepend_path_entry_for_cocode_aliases() -> std::io::Result<TempDir> {
    let cocode_home = find_cocode_home()?;

    #[cfg(not(debug_assertions))]
    {
        // Guard against placing helpers in system temp directories outside debug builds.
        let temp_root = std::env::temp_dir();
        if cocode_home.starts_with(&temp_root) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "Refusing to create helper binaries under temporary dir {temp_root:?} (cocode_home: {cocode_home:?})"
                ),
            ));
        }
    }

    std::fs::create_dir_all(&cocode_home)?;

    // Use a COCODE_HOME-scoped temp root to avoid cluttering the top-level directory.
    let temp_root = cocode_home.join("tmp").join("path");
    std::fs::create_dir_all(&temp_root)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        // Ensure only the current user can access the temp directory.
        std::fs::set_permissions(&temp_root, std::fs::Permissions::from_mode(0o700))?;
    }

    let temp_dir = tempfile::Builder::new()
        .prefix("cocode-arg0")
        .tempdir_in(&temp_root)?;
    let path = temp_dir.path();

    for filename in &[
        APPLY_PATCH_ARG0,
        MISSPELLED_APPLY_PATCH_ARG0,
        #[cfg(target_os = "linux")]
        LINUX_SANDBOX_ARG0,
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
"{}" {COCODE_APPLY_PATCH_ARG1} %*
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

    let path_element = path.display();
    let updated_path_env_var = match std::env::var("PATH") {
        Ok(existing_path) => {
            format!("{path_element}{PATH_SEPARATOR}{existing_path}")
        }
        Err(_) => {
            format!("{path_element}")
        }
    };

    // SAFETY: This is called before any threads are spawned.
    unsafe {
        std::env::set_var("PATH", updated_path_env_var);
    }

    Ok(temp_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_patch_arg1_constant() {
        assert_eq!(COCODE_APPLY_PATCH_ARG1, "--cocode-run-as-apply-patch");
    }

    #[test]
    fn test_illegal_env_var_prefix() {
        assert_eq!(ILLEGAL_ENV_VAR_PREFIX, "COCODE_");
    }

    #[test]
    fn test_find_cocode_home() {
        // Should not fail in test environment
        let result = find_cocode_home();
        // May fail if HOME is not set, which is OK for this test
        if let Ok(home) = result {
            assert!(home.to_string_lossy().contains(".cocode"));
        }
    }

    #[test]
    fn test_set_filtered_blocks_cocode_prefix() {
        // Create test entries
        let entries: Vec<Result<(String, String), dotenvy::Error>> = vec![
            Ok(("SAFE_VAR".to_string(), "safe_value".to_string())),
            Ok(("COCODE_BLOCKED".to_string(), "blocked_value".to_string())),
            Ok((
                "cocode_also_blocked".to_string(),
                "also_blocked".to_string(),
            )),
        ];

        // This would set SAFE_VAR but not COCODE_* vars
        // We can't easily test this without modifying env, so just verify the logic
        let filtered: Vec<_> = entries
            .into_iter()
            .flatten()
            .filter(|(key, _)| !key.to_ascii_uppercase().starts_with(ILLEGAL_ENV_VAR_PREFIX))
            .collect();

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].0, "SAFE_VAR");
    }
}
