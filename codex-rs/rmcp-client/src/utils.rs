use anyhow::Result;
use reqwest::ClientBuilder;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderName;
use reqwest::header::HeaderValue;
use std::collections::HashMap;
use std::env;
use std::ffi::OsString;

pub(crate) fn create_env_for_mcp_server(
    extra_env: Option<HashMap<OsString, OsString>>,
    env_vars: &[String],
) -> HashMap<OsString, OsString> {
    DEFAULT_ENV_VARS
        .iter()
        .copied()
        .chain(env_vars.iter().map(String::as_str))
        .filter_map(|var| env::var_os(var).map(|value| (OsString::from(var), value)))
        .chain(extra_env.unwrap_or_default())
        .collect()
}

pub(crate) fn create_env_overlay_for_remote_mcp_server(
    extra_env: Option<HashMap<OsString, OsString>>,
    env_vars: &[String],
) -> HashMap<OsString, OsString> {
    // Remote stdio should inherit PATH/HOME/etc. from the executor side, not
    // from the orchestrator process. Only forward variables explicitly named
    // by the MCP config plus literal env overrides from that config.
    env_vars
        .iter()
        .filter_map(|var| env::var_os(var).map(|value| (OsString::from(var), value)))
        .chain(extra_env.unwrap_or_default())
        .collect()
}

pub(crate) fn build_default_headers(
    http_headers: Option<HashMap<String, String>>,
    env_http_headers: Option<HashMap<String, String>>,
) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();

    if let Some(static_headers) = http_headers {
        for (name, value) in static_headers {
            let header_name = match HeaderName::from_bytes(name.as_bytes()) {
                Ok(name) => name,
                Err(err) => {
                    tracing::warn!("invalid HTTP header name `{name}`: {err}");
                    continue;
                }
            };
            let header_value = match HeaderValue::from_str(value.as_str()) {
                Ok(value) => value,
                Err(err) => {
                    tracing::warn!("invalid HTTP header value for `{name}`: {err}");
                    continue;
                }
            };
            headers.insert(header_name, header_value);
        }
    }

    if let Some(env_headers) = env_http_headers {
        for (name, env_var) in env_headers {
            if let Ok(value) = env::var(&env_var) {
                if value.trim().is_empty() {
                    continue;
                }

                let header_name = match HeaderName::from_bytes(name.as_bytes()) {
                    Ok(name) => name,
                    Err(err) => {
                        tracing::warn!("invalid HTTP header name `{name}`: {err}");
                        continue;
                    }
                };

                let header_value = match HeaderValue::from_str(value.as_str()) {
                    Ok(value) => value,
                    Err(err) => {
                        tracing::warn!(
                            "invalid HTTP header value read from {env_var} for `{name}`: {err}"
                        );
                        continue;
                    }
                };
                headers.insert(header_name, header_value);
            }
        }
    }

    Ok(headers)
}

pub(crate) fn apply_default_headers(
    builder: ClientBuilder,
    default_headers: &HeaderMap,
) -> ClientBuilder {
    if default_headers.is_empty() {
        builder
    } else {
        builder.default_headers(default_headers.clone())
    }
}

#[cfg(unix)]
pub(crate) const DEFAULT_ENV_VARS: &[&str] = &[
    "HOME",
    "LOGNAME",
    "PATH",
    "SHELL",
    "USER",
    "__CF_USER_TEXT_ENCODING",
    "LANG",
    "LC_ALL",
    "TERM",
    "TMPDIR",
    "TZ",
];

#[cfg(windows)]
pub(crate) const DEFAULT_ENV_VARS: &[&str] = &[
    // Core path resolution
    "PATH",
    "PATHEXT",
    // Shell and system roots
    "COMSPEC",
    "SYSTEMROOT",
    "SYSTEMDRIVE",
    // User context and profiles
    "USERNAME",
    "USERDOMAIN",
    "USERPROFILE",
    "HOMEDRIVE",
    "HOMEPATH",
    // Program locations
    "PROGRAMFILES",
    "PROGRAMFILES(X86)",
    "PROGRAMW6432",
    "PROGRAMDATA",
    // App data and caches
    "LOCALAPPDATA",
    "APPDATA",
    // Temp locations
    "TEMP",
    "TMP",
    // Common shells/pwsh hints
    "POWERSHELL",
    "PWSH",
];

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    use serial_test::serial;
    use std::ffi::OsStr;

    struct EnvVarGuard {
        key: String,
        original: Option<OsString>,
    }

    impl EnvVarGuard {
        fn set(key: &str, value: impl AsRef<OsStr>) -> Self {
            let original = std::env::var_os(key);
            unsafe {
                std::env::set_var(key, value.as_ref());
            }
            Self {
                key: key.to_string(),
                original,
            }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(value) = &self.original {
                unsafe {
                    std::env::set_var(&self.key, value);
                }
            } else {
                unsafe {
                    std::env::remove_var(&self.key);
                }
            }
        }
    }

    #[tokio::test]
    async fn create_env_honors_overrides() {
        let value = "custom".to_string();
        let expected = OsString::from(&value);
        let env = create_env_for_mcp_server(
            Some(HashMap::from([(OsString::from("TZ"), expected.clone())])),
            &[],
        );
        assert_eq!(env.get(OsStr::new("TZ")), Some(&expected));
    }

    #[test]
    #[serial(extra_rmcp_env)]
    fn create_env_includes_additional_whitelisted_variables() {
        let custom_var = "EXTRA_RMCP_ENV";
        let value = "from-env";
        let expected = OsString::from(value);
        let _guard = EnvVarGuard::set(custom_var, value);
        let env = create_env_for_mcp_server(/*extra_env*/ None, &[custom_var.to_string()]);
        assert_eq!(env.get(OsStr::new(custom_var)), Some(&expected));
    }

    #[test]
    #[serial(extra_rmcp_env)]
    fn create_remote_env_overlay_only_forwards_explicit_variables() {
        let default_var = DEFAULT_ENV_VARS[0];
        let custom_var = "EXTRA_REMOTE_RMCP_ENV";
        let custom_value = OsString::from("from-env");
        let _default_guard = EnvVarGuard::set(default_var, "from-default");
        let _custom_guard = EnvVarGuard::set(custom_var, &custom_value);

        let env = create_env_overlay_for_remote_mcp_server(
            /*extra_env*/ None,
            &[custom_var.to_string()],
        );

        assert_eq!(
            env,
            HashMap::from([(OsString::from(custom_var), custom_value)])
        );
    }

    #[cfg(unix)]
    #[test]
    #[serial(extra_rmcp_env)]
    fn create_env_preserves_path_when_it_is_not_utf8() {
        use std::os::unix::ffi::OsStrExt;

        let raw_path = std::ffi::OsStr::from_bytes(b"/tmp/codex-\xFF/bin");
        let expected = raw_path.to_os_string();
        let _guard = EnvVarGuard::set("PATH", raw_path);

        let env = create_env_for_mcp_server(/*extra_env*/ None, &[]);

        assert_eq!(env.get(OsStr::new("PATH")), Some(&expected));
    }
}
