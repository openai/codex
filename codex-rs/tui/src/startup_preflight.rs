//! Conservative first-install checks that run before the provisional composer appears.
//!
//! Any existing user, system, daemon, or authentication state keeps the composer visible.

use std::io;
use std::path::Path;

use codex_utils_absolute_path::AbsolutePathBuf;

/// Hide the composer only on a first installation without user or machine-wide configuration.
pub(super) fn should_delay_startup_composer_for_first_login(
    codex_home: &Path,
    system_config_path: io::Result<AbsolutePathBuf>,
    managed_configuration: impl FnOnce() -> io::Result<bool>,
    environment_variable: impl Fn(&str) -> Option<String>,
) -> bool {
    if environment_variable("CODEX_HOME").is_some_and(|value| !value.is_empty())
        || environment_variable(codex_login::CODEX_ACCESS_TOKEN_ENV_VAR)
            .is_some_and(|credential| !credential.trim().is_empty())
    {
        return false;
    }

    let Ok(system_config_path) = system_config_path else {
        return false;
    };
    if !matches!(system_config_path.as_path().try_exists(), Ok(false)) {
        return false;
    }

    let pristine_home = match codex_home.try_exists() {
        Ok(false) => true,
        Err(_) => false,
        Ok(true) => {
            let Ok(mut entries) = std::fs::read_dir(codex_home) else {
                return false;
            };
            let Some(Ok(temporary_root)) = entries.next() else {
                return false;
            };
            if temporary_root.file_name() != "tmp"
                || !temporary_root
                    .file_type()
                    .is_ok_and(|file_type| file_type.is_dir())
                || entries.next().is_some()
            {
                return false;
            }

            let Ok(mut entries) = std::fs::read_dir(temporary_root.path()) else {
                return false;
            };
            let Some(Ok(arg0_root)) = entries.next() else {
                return false;
            };
            arg0_root.file_name() == "arg0"
                && arg0_root
                    .file_type()
                    .is_ok_and(|file_type| file_type.is_dir())
                && entries.next().is_none()
        }
    };

    pristine_home && matches!(managed_configuration(), Ok(false))
}

#[cfg(test)]
#[path = "startup_preflight_tests.rs"]
mod tests;
