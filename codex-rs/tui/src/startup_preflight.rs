//! Conservative authentication checks that run before the provisional composer appears.
//!
//! Existing configuration, daemon, or authentication state keeps the composer visible.

use std::ffi::OsString;
use std::io;
use std::path::Path;

use codex_protocol::shell_environment::OPENAI_FEDERATION_RULE_ID_ENV_VAR;
use codex_protocol::shell_environment::OPENAI_IDENTITY_TOKEN_FILE_ENV_VAR;
use codex_utils_absolute_path::AbsolutePathBuf;

/// Recognize the single configuration override synthesized by the legacy `--search` flag.
pub(super) fn has_only_search_config_override(cli_kv_overrides: &[(String, toml::Value)]) -> bool {
    matches!(
        cli_kv_overrides,
        [(key, toml::Value::String(value))] if key == "web_search" && value == "live"
    )
}

/// Hide the composer when the default file-backed account cannot already be authenticated.
pub(super) fn should_delay_startup_composer_for_first_login(
    codex_home: &Path,
    system_config_path: io::Result<AbsolutePathBuf>,
    managed_configuration: impl FnOnce() -> io::Result<bool>,
    environment_variable: impl Fn(&str) -> Option<OsString>,
) -> bool {
    if environment_variable(codex_login::CODEX_ACCESS_TOKEN_ENV_VAR).is_some_and(|credential| {
        credential
            .to_str()
            .is_some_and(|value| !value.trim().is_empty())
    }) || environment_variable(OPENAI_FEDERATION_RULE_ID_ENV_VAR).is_some()
        || environment_variable(OPENAI_IDENTITY_TOKEN_FILE_ENV_VAR).is_some()
    {
        return false;
    }

    let Ok(system_config_path) = system_config_path else {
        return false;
    };
    if !matches!(system_config_path.as_path().try_exists(), Ok(false)) {
        return false;
    }

    match std::fs::metadata(codex_home) {
        Ok(metadata) if !metadata.is_dir() => return false,
        Ok(_) => {
            for state_file in ["auth.json", "config.toml", "environments.toml"] {
                if !matches!(codex_home.join(state_file).try_exists(), Ok(false)) {
                    return false;
                }
            }

            let Ok(daemon_socket) =
                codex_app_server_client::app_server_control_socket_path(codex_home)
            else {
                return false;
            };
            if !matches!(daemon_socket.as_path().try_exists(), Ok(false)) {
                return false;
            }
        }
        Err(error)
            if error.kind() == io::ErrorKind::NotFound
                && matches!(codex_home.try_exists(), Ok(false)) => {}
        Err(_) => return false,
    }

    matches!(managed_configuration(), Ok(false))
}

#[cfg(test)]
#[path = "startup_preflight_tests.rs"]
mod tests;
