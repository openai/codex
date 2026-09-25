//! Credential resolution must retain executor provenance when host fallback is considered.

use super::resolve_bearer_token;
use crate::server::McpCredentialPolicy;
use pretty_assertions::assert_eq;

#[test]
fn executor_credentials_reject_host_fallback_for_arbitrary_environment_names() {
    for env_var in ["PATH", "MCP_UNPROTECTED_TEST_TOKEN"] {
        let error = resolve_bearer_token("docs", Some(env_var), McpCredentialPolicy::ExecutorOnly)
            .expect_err("executor credential names must never resolve on the host");

        assert_eq!(
            error.to_string(),
            "MCP server 'docs' requires executor-side environment credential resolution; update the executor to a version that supports it (host fallback is disabled)",
        );
    }
}

#[test]
fn servers_without_environment_credentials_do_not_require_host_fallback() {
    for policy in [
        McpCredentialPolicy::ExecutorOnly,
        McpCredentialPolicy::HostFallbackAllowed,
    ] {
        assert_eq!(
            resolve_bearer_token("docs", /*bearer_token_env_var*/ None, policy)
                .expect("a server without an environment credential needs no resolution"),
            None,
        );
    }
}

#[test]
fn configured_credentials_retain_host_environment_resolution() {
    // PATH is nonsecret and read without changing the process environment.
    let expected = match std::env::var("PATH") {
        Ok(value) if value.is_empty() => {
            Err("Environment variable PATH for MCP server 'docs' is empty".to_string())
        }
        Ok(value) => Ok(Some(value)),
        Err(std::env::VarError::NotPresent) => {
            Err("Environment variable PATH for MCP server 'docs' is not set".to_string())
        }
        Err(std::env::VarError::NotUnicode(_)) => Err(
            "Environment variable PATH for MCP server 'docs' contains invalid Unicode".to_string(),
        ),
    };

    assert_eq!(
        resolve_bearer_token(
            "docs",
            Some("PATH"),
            McpCredentialPolicy::HostFallbackAllowed
        )
        .map_err(|error| error.to_string()),
        expected,
    );
}
