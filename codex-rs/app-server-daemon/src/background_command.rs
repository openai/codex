//! Recovers background startup after the launching directory is removed.
//! Preserve process-scoped paths; retain usable Unix cwd defaults for clients.

use std::path::Path;
use std::process::Command;

use anyhow::Result;

pub(crate) fn set_working_directory(command: &mut Command, state_dir: &Path) -> Result<()> {
    #[cfg(unix)]
    let cwd_missing = match std::env::current_dir() {
        Ok(_) => false,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => true,
        Err(err) => return Err(err.into()),
    };
    if let Ok(value) = std::env::var("CODEX_HOME")
        && !value.is_empty()
    {
        let home = codex_utils_home_dir::find_codex_home()?;
        // Remove parent components without discarding a valid configured symlink alias.
        let absolute = std::path::absolute(value)?;
        let alias = home.join(&absolute);
        let preserved = if alias.canonicalize().ok().as_ref() == Some(&home) {
            alias.as_path()
        } else if home.as_path().to_str().is_some() {
            home.as_path()
        } else {
            // The home resolver ignores non-Unicode env values; keep the valid original alias.
            absolute.as_path()
        };
        command.env("CODEX_HOME", preserved);
    }
    // Preserve process-scoped paths before changing cwd; CA names match CUSTOM_CA_ENV_KEYS.
    for name in [
        #[cfg(windows)]
        "CODEX_SQLITE_HOME",
        "CODEX_CA_CERTIFICATE",
        "SSL_CERT_FILE",
        "REQUESTS_CA_BUNDLE",
        "CURL_CA_BUNDLE",
        "NODE_EXTRA_CA_CERTS",
        "GIT_SSL_CAINFO",
        "CARGO_HTTP_CAINFO",
        "PIP_CERT",
        "BUNDLE_SSL_CA_CERT",
        "npm_config_cafile",
        "NPM_CONFIG_CAFILE",
        "AWS_CONFIG_FILE",
        "AWS_SHARED_CREDENTIALS_FILE",
        "AWS_WEB_IDENTITY_TOKEN_FILE",
        "AWS_CONTAINER_AUTHORIZATION_TOKEN_FILE",
    ] {
        let Some(mut value) = std::env::var_os(name) else {
            continue;
        };
        if matches!(
            name,
            "CODEX_SQLITE_HOME" | "npm_config_cafile" | "NPM_CONFIG_CAFILE"
        ) {
            value = value.to_str().unwrap_or_default().trim().into();
        }
        // These consumers expand `~` independently of the working directory.
        let expands_home = if matches!(name, "npm_config_cafile" | "NPM_CONFIG_CAFILE") {
            value
                .to_str()
                .is_some_and(|path| path.starts_with("~/") || path.starts_with("~\\"))
        } else {
            matches!(
                name,
                "CODEX_SQLITE_HOME" | "AWS_CONFIG_FILE" | "AWS_SHARED_CREDENTIALS_FILE"
            ) && std::path::Path::new(&value).starts_with("~")
        };
        if value.is_empty() || expands_home {
            continue;
        }
        command.env(name, std::path::absolute(value)?);
    }
    if let Some(value) = std::env::var_os("SSL_CERT_DIR") {
        let paths = std::env::split_paths(&value)
            .filter(|path| !path.as_os_str().is_empty())
            .map(std::path::absolute)
            .collect::<std::io::Result<Vec<_>>>()?;
        command.env("SSL_CERT_DIR", std::env::join_paths(paths)?);
    }
    #[cfg(unix)]
    if cwd_missing {
        command.current_dir(state_dir);
    }
    #[cfg(windows)]
    command.current_dir(state_dir);
    Ok(())
}
