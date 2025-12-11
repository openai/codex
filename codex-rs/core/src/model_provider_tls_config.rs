//! TLS configuration for model providers.
//!
//! This module contains types for configuring TLS/mTLS connections to model providers.

use crate::default_client_config::TlsConfig;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use serde::Serialize;

/// TLS configuration for mutual TLS (mTLS) authentication with model providers.
///
/// Each certificate/key path can be specified either directly or via an environment variable:
/// - `ca-certificate` / `ca-certificate-env`: Path to a custom CA certificate (PEM format)
/// - `client-certificate` / `client-certificate-env`: Path to the client certificate (PEM format)
/// - `client-private-key` / `client-private-key-env`: Path to the client private key (PEM format)
///
/// If both the direct path and env var are specified, the env var takes precedence.
/// Paths from environment variables must be absolute.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ModelProviderTlsConfig {
    /// Path to a custom CA certificate (PEM format) to trust when connecting to this provider.
    /// Relative paths are resolved against the directory containing the config file.
    pub ca_certificate: Option<AbsolutePathBuf>,

    /// Environment variable containing the absolute path to a custom CA certificate.
    /// Takes precedence over `ca_certificate` if set.
    pub ca_certificate_env: Option<String>,

    /// Path to the client certificate (PEM format) for mutual TLS authentication.
    /// Must be provided together with `client_private_key`.
    /// Relative paths are resolved against the directory containing the config file.
    pub client_certificate: Option<AbsolutePathBuf>,

    /// Environment variable containing the absolute path to the client certificate.
    /// Takes precedence over `client_certificate` if set.
    pub client_certificate_env: Option<String>,

    /// Path to the client private key (PEM format) for mutual TLS authentication.
    /// Must be provided together with `client_certificate`.
    /// Relative paths are resolved against the directory containing the config file.
    pub client_private_key: Option<AbsolutePathBuf>,

    /// Environment variable containing the absolute path to the client private key.
    /// Takes precedence over `client_private_key` if set.
    pub client_private_key_env: Option<String>,
}

/// Parse a string value into an AbsolutePathBuf, validating that it's non-empty and absolute.
/// Returns None if the value is empty/whitespace, relative, or cannot be converted.
/// The `source` parameter is used for warning messages (e.g., "env var MY_VAR").
fn parse_absolute_path(value: &str, source: &str) -> Option<AbsolutePathBuf> {
    use std::path::Path;

    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let path = Path::new(trimmed);
    if !path.is_absolute() {
        tracing::warn!(
            "Ignoring relative path '{}' from {}; paths must be absolute",
            trimmed,
            source
        );
        return None;
    }

    match AbsolutePathBuf::from_absolute_path(trimmed) {
        Ok(abs_path) => Some(abs_path),
        Err(e) => {
            tracing::warn!("Failed to parse path from {}: {}", source, e);
            None
        }
    }
}

/// Resolve a path from either an environment variable or a direct config value.
/// The env var takes precedence if set and contains a valid absolute path.
fn resolve_path_from_env_or_config(
    env_var_name: Option<&str>,
    direct: Option<&AbsolutePathBuf>,
) -> Option<AbsolutePathBuf> {
    // Try env var first if name is provided
    if let Some(name) = env_var_name
        && let Ok(value) = std::env::var(name)
        && let Some(path) = parse_absolute_path(&value, &format!("env var {name}"))
    {
        return Some(path);
    }

    // Fall back to direct path (already absolute from deserialization)
    direct.cloned()
}

impl ModelProviderTlsConfig {
    /// Convert to the TlsConfig type used by HTTP clients.
    /// Environment variable values take precedence over direct path values.
    pub fn to_tls_config(&self) -> TlsConfig {
        TlsConfig {
            ca_certificate: resolve_path_from_env_or_config(
                self.ca_certificate_env.as_deref(),
                self.ca_certificate.as_ref(),
            ),
            client_certificate: resolve_path_from_env_or_config(
                self.client_certificate_env.as_deref(),
                self.client_certificate.as_ref(),
            ),
            client_private_key: resolve_path_from_env_or_config(
                self.client_private_key_env.as_deref(),
                self.client_private_key.as_ref(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;

    #[test]
    fn test_parse_absolute_path_valid() {
        let result = parse_absolute_path("/some/absolute/path.pem", "test source");
        assert_eq!(
            result.map(|p| p.to_path_buf()),
            Some(PathBuf::from("/some/absolute/path.pem"))
        );
    }

    #[test]
    fn test_parse_absolute_path_relative_rejected() {
        let result = parse_absolute_path("relative/path.pem", "test source");
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_absolute_path_empty() {
        let result = parse_absolute_path("", "test source");
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_absolute_path_whitespace_only() {
        let result = parse_absolute_path("   ", "test source");
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_absolute_path_trims_whitespace() {
        let result = parse_absolute_path("  /some/path.pem  ", "test source");
        assert_eq!(
            result.map(|p| p.to_path_buf()),
            Some(PathBuf::from("/some/path.pem"))
        );
    }

    #[test]
    fn test_resolve_path_fallback_to_direct() {
        let direct = AbsolutePathBuf::from_absolute_path("/fallback/ca.pem").unwrap();
        let result =
            resolve_path_from_env_or_config(Some("NONEXISTENT_ENV_VAR_12345"), Some(&direct));
        assert_eq!(
            result.map(|p| p.to_path_buf()),
            Some(PathBuf::from("/fallback/ca.pem"))
        );
    }

    #[test]
    fn test_resolve_path_no_env_var_name() {
        let direct = AbsolutePathBuf::from_absolute_path("/direct/ca.pem").unwrap();
        let result = resolve_path_from_env_or_config(None, Some(&direct));
        assert_eq!(
            result.map(|p| p.to_path_buf()),
            Some(PathBuf::from("/direct/ca.pem"))
        );
    }

    #[test]
    fn test_resolve_path_both_none() {
        let result = resolve_path_from_env_or_config(None, None);
        assert_eq!(result, None);
    }

    #[test]
    fn test_deserialize_tls_config_with_env_vars() {
        use codex_utils_absolute_path::AbsolutePathBufGuard;
        use tempfile::tempdir;

        let temp_dir = tempdir().expect("temp dir");
        let _guard = AbsolutePathBufGuard::new(temp_dir.path());

        let toml_str = r#"
ca-certificate = "/direct/ca.pem"
ca-certificate-env = "MY_CA_CERT"
client-certificate-env = "MY_CLIENT_CERT"
client-private-key-env = "MY_CLIENT_KEY"
        "#;

        let config: ModelProviderTlsConfig = toml::from_str(toml_str).unwrap();

        assert_eq!(
            config.ca_certificate.map(|p| p.to_path_buf()),
            Some(PathBuf::from("/direct/ca.pem"))
        );
        assert_eq!(config.ca_certificate_env, Some("MY_CA_CERT".into()));
        assert_eq!(config.client_certificate, None);
        assert_eq!(config.client_certificate_env, Some("MY_CLIENT_CERT".into()));
        assert_eq!(config.client_private_key, None);
        assert_eq!(config.client_private_key_env, Some("MY_CLIENT_KEY".into()));
    }
}
