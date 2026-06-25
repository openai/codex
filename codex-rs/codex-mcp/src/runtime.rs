//! Runtime support for Model Context Protocol (MCP) servers.
//!
//! This module contains data that describes the runtime environment in which MCP
//! servers execute, plus the sandbox state payload sent to capable servers and a
//! tiny shared metrics helper. Transport startup and orchestration live in
//! [`crate::rmcp_client`] and [`crate::connection_manager`].

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use codex_exec_server::Environment;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::EnvironmentRegistrySnapshot;
use codex_exec_server::HttpClient;
use codex_exec_server::ReqwestHttpClient;
use codex_protocol::models::PermissionProfile;
use codex_utils_path_uri::PathUri;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxState {
    #[serde(default = "default_sandbox_environment_id")]
    pub environment_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment_instance_id: Option<String>,
    pub permission_profile: PermissionProfile,
    pub codex_linux_sandbox_exe: Option<PathBuf>,
    pub sandbox_cwd: PathUri,
    #[serde(default)]
    pub use_legacy_landlock: bool,
}

fn default_sandbox_environment_id() -> String {
    codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string()
}

/// Runtime context used when resolving per-server MCP environments.
///
/// `McpConfig` describes what servers exist. This value carries the canonical
/// environment registry plus the local stdio fallback cwd used when a local
/// stdio server omits its own working directory.
#[derive(Clone)]
pub struct McpRuntimeContext {
    environment_manager: Arc<EnvironmentManager>,
    environment_snapshot: EnvironmentRegistrySnapshot,
    local_stdio_fallback_cwd: PathBuf,
}

impl McpRuntimeContext {
    pub fn new(
        environment_manager: Arc<EnvironmentManager>,
        local_stdio_fallback_cwd: PathBuf,
    ) -> Self {
        let environment_snapshot = environment_manager.registry_snapshot();
        Self {
            environment_manager,
            environment_snapshot,
            local_stdio_fallback_cwd,
        }
    }

    /// Builds a runtime using concrete environment generations pinned by the active turn.
    pub fn new_with_environment_overrides(
        environment_manager: Arc<EnvironmentManager>,
        local_stdio_fallback_cwd: PathBuf,
        overrides: impl IntoIterator<Item = (String, Arc<Environment>)>,
    ) -> Self {
        let environment_snapshot = environment_manager
            .registry_snapshot()
            .with_overrides(overrides);
        Self {
            environment_manager,
            environment_snapshot,
            local_stdio_fallback_cwd,
        }
    }

    pub(crate) fn local_stdio_fallback_cwd(&self) -> PathBuf {
        self.local_stdio_fallback_cwd.clone()
    }

    /// Returns whether both values describe the same process-local launch inputs.
    pub fn has_same_launch_context(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.environment_manager, &other.environment_manager)
            && self
                .environment_snapshot
                .retains_same_instances(&other.environment_snapshot)
            && self.local_stdio_fallback_cwd == other.local_stdio_fallback_cwd
    }

    pub(crate) fn has_same_launch_environment_for(
        &self,
        other: &Self,
        config: &codex_config::McpServerConfig,
    ) -> bool {
        if !Arc::ptr_eq(&self.environment_manager, &other.environment_manager) {
            return false;
        }
        let same_environment = match (
            self.environment_snapshot
                .get_environment(&config.environment_id),
            other
                .environment_snapshot
                .get_environment(&config.environment_id),
        ) {
            (Some(current), Some(previous)) => Arc::ptr_eq(&current, &previous),
            (None, None) => true,
            _ => false,
        };
        if !same_environment {
            return false;
        }
        !config.is_local_environment()
            || !matches!(
                &config.transport,
                codex_config::McpServerTransportConfig::Stdio { cwd: None, .. }
            )
            || self.local_stdio_fallback_cwd == other.local_stdio_fallback_cwd
    }

    pub(crate) fn resolve_server_environment(
        &self,
        server_name: &str,
        config: &codex_config::McpServerConfig,
    ) -> Result<Option<Arc<Environment>>, String> {
        // Resolve `"local"` through the shared registry when available. Local
        // HTTP is the one current exception: it can use the ambient HTTP client
        // even when no local Environment is configured.
        if let Some(environment) = self
            .environment_snapshot
            .get_environment(&config.environment_id)
        {
            return Ok(Some(environment));
        }

        if config.is_local_environment() {
            return match config.transport {
                codex_config::McpServerTransportConfig::Stdio { .. } => Err(format!(
                    "local stdio MCP server `{server_name}` requires a local environment"
                )),
                codex_config::McpServerTransportConfig::StreamableHttp { .. } => Ok(None),
            };
        }

        Err(format!(
            "MCP server `{server_name}` references unknown environment id `{}`",
            config.environment_id
        ))
    }

    /// Resolves the HTTP capability owned by the server's configured environment.
    pub fn resolve_http_client(
        &self,
        server_name: &str,
        config: &codex_config::McpServerConfig,
    ) -> Result<Arc<dyn HttpClient>, String> {
        Ok(self
            .resolve_server_environment(server_name, config)?
            .map_or_else(
                || Arc::new(ReqwestHttpClient) as Arc<dyn HttpClient>,
                |environment| environment.get_http_client(),
            ))
    }
}

pub(crate) fn emit_duration(metric: &str, duration: Duration, tags: &[(&str, &str)]) {
    if let Some(metrics) = codex_otel::global() {
        let _ = metrics.record_duration(metric, duration, tags);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID;
    use codex_config::McpServerConfig;
    use codex_config::McpServerTransportConfig;
    use codex_exec_server::EnvironmentManager;
    use codex_utils_path_uri::LegacyAppPathString;
    use pretty_assertions::assert_eq;

    use super::*;

    fn stdio_server(environment_id: &str) -> McpServerConfig {
        McpServerConfig {
            auth: Default::default(),
            transport: McpServerTransportConfig::Stdio {
                command: "echo".to_string(),
                args: Vec::new(),
                env: None,
                env_vars: Vec::new(),
                cwd: None,
            },
            environment_id: environment_id.to_string(),
            enabled: true,
            required: false,
            supports_parallel_tool_calls: false,
            disabled_reason: None,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
            default_tools_approval_mode: None,
            enabled_tools: None,
            disabled_tools: None,
            scopes: None,
            oauth: None,
            oauth_resource: None,
            tools: HashMap::new(),
        }
    }

    fn http_server(environment_id: &str) -> McpServerConfig {
        McpServerConfig {
            auth: Default::default(),
            transport: McpServerTransportConfig::StreamableHttp {
                url: "http://127.0.0.1:1".to_string(),
                bearer_token_env_var: None,
                http_headers: None,
                env_http_headers: None,
            },
            environment_id: environment_id.to_string(),
            ..stdio_server(environment_id)
        }
    }

    #[test]
    fn sandbox_state_accepts_a_missing_environment_instance_id() {
        let sandbox_state = SandboxState {
            environment_id: DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string(),
            environment_instance_id: None,
            permission_profile: PermissionProfile::Disabled,
            codex_linux_sandbox_exe: None,
            sandbox_cwd: PathUri::parse("file:///tmp").expect("sandbox cwd"),
            use_legacy_landlock: false,
        };

        let serialized = serde_json::to_value(&sandbox_state).expect("serialize sandbox state");
        assert!(serialized.get("environmentInstanceId").is_none());
        assert_eq!(
            serde_json::from_value::<SandboxState>(serialized).expect("deserialize sandbox state"),
            sandbox_state
        );
    }

    #[test]
    fn local_stdio_requires_local_stdio_availability() {
        let runtime_context = McpRuntimeContext::new(
            Arc::new(EnvironmentManager::without_environments()),
            PathBuf::from("/tmp"),
        );

        let error = match runtime_context
            .resolve_server_environment("stdio", &stdio_server(DEFAULT_MCP_SERVER_ENVIRONMENT_ID))
        {
            Ok(_) => panic!("local stdio MCP should require a local environment"),
            Err(error) => error,
        };
        assert_eq!(
            error,
            "local stdio MCP server `stdio` requires a local environment"
        );
    }

    #[test]
    fn local_http_does_not_require_local_stdio_availability() {
        let runtime_context = McpRuntimeContext::new(
            Arc::new(EnvironmentManager::without_environments()),
            PathBuf::from("/tmp"),
        );

        let resolved_runtime = match runtime_context
            .resolve_server_environment("http", &http_server(DEFAULT_MCP_SERVER_ENVIRONMENT_ID))
        {
            Ok(resolved_runtime) => resolved_runtime,
            Err(error) => panic!("local HTTP MCP should resolve: {error}"),
        };
        assert!(resolved_runtime.is_none());
    }

    #[tokio::test]
    async fn fallback_cwd_only_invalidates_local_stdio_without_an_explicit_cwd() {
        let environment_manager = Arc::new(EnvironmentManager::default_for_tests());
        let before = McpRuntimeContext::new(
            Arc::clone(&environment_manager),
            PathBuf::from("/workspace/one"),
        );
        let after = McpRuntimeContext::new(environment_manager, PathBuf::from("/workspace/two"));
        let implicit_cwd = stdio_server(DEFAULT_MCP_SERVER_ENVIRONMENT_ID);
        let mut explicit_cwd = implicit_cwd.clone();
        let McpServerTransportConfig::Stdio { cwd, .. } = &mut explicit_cwd.transport else {
            unreachable!("stdio helper should build stdio transport");
        };
        *cwd = Some(LegacyAppPathString::from_path(std::path::Path::new(
            "/workspace/explicit",
        )));

        assert!(!before.has_same_launch_environment_for(&after, &implicit_cwd));
        assert!(before.has_same_launch_environment_for(&after, &explicit_cwd));
        assert!(before.has_same_launch_environment_for(
            &after,
            &http_server(DEFAULT_MCP_SERVER_ENVIRONMENT_ID),
        ));
    }

    #[test]
    fn unknown_explicit_environment_is_rejected() {
        let runtime_context = McpRuntimeContext::new(
            Arc::new(EnvironmentManager::without_environments()),
            PathBuf::from("/tmp"),
        );

        let error =
            match runtime_context.resolve_server_environment("stdio", &stdio_server("remote")) {
                Ok(_) => panic!("unknown MCP environment should fail"),
                Err(error) => error,
            };
        assert_eq!(
            error,
            "MCP server `stdio` references unknown environment id `remote`"
        );
    }

    #[tokio::test]
    async fn explicit_remote_stdio_and_http_accept_named_environment() {
        let runtime_context = McpRuntimeContext::new(
            Arc::new(
                EnvironmentManager::create_for_tests(
                    Some("ws://127.0.0.1:8765".to_string()),
                    /*local_runtime_paths*/ None,
                )
                .await,
            ),
            PathBuf::from("/tmp"),
        );

        let mut remote_stdio = stdio_server("remote");
        let McpServerTransportConfig::Stdio { cwd, .. } = &mut remote_stdio.transport else {
            unreachable!("stdio helper should build stdio transport");
        };
        *cwd = Some(LegacyAppPathString::from_path(&std::env::temp_dir()));
        for resolved_runtime in [
            runtime_context.resolve_server_environment("stdio", &remote_stdio),
            runtime_context.resolve_server_environment("http", &http_server("remote")),
        ] {
            let resolved_runtime = match resolved_runtime {
                Ok(resolved_runtime) => resolved_runtime,
                Err(error) => panic!("remote MCP should resolve: {error}"),
            };
            assert!(resolved_runtime.is_some());
        }
    }

    #[tokio::test]
    async fn runtime_context_pins_environment_registry_snapshot_across_upsert() {
        let environment_manager = Arc::new(
            EnvironmentManager::create_for_tests(
                Some("ws://127.0.0.1:8765".to_string()),
                /*local_runtime_paths*/ None,
            )
            .await,
        );
        let before =
            McpRuntimeContext::new(Arc::clone(&environment_manager), PathBuf::from("/tmp"));
        let before_environment = before
            .resolve_server_environment("http", &http_server("remote"))
            .expect("initial remote environment should resolve")
            .expect("initial remote environment should exist");

        environment_manager
            .upsert_environment(
                "remote".to_string(),
                "ws://127.0.0.1:8766".to_string(),
                /*connect_timeout*/ None,
            )
            .expect("replace remote environment");

        let pinned_environment = before
            .resolve_server_environment("http", &http_server("remote"))
            .expect("pinned remote environment should resolve")
            .expect("pinned remote environment should exist");
        let after = McpRuntimeContext::new(Arc::clone(&environment_manager), PathBuf::from("/tmp"));
        let replacement_environment = after
            .resolve_server_environment("http", &http_server("remote"))
            .expect("replacement remote environment should resolve")
            .expect("replacement remote environment should exist");

        assert!(Arc::ptr_eq(&before_environment, &pinned_environment));
        assert!(!Arc::ptr_eq(&before_environment, &replacement_environment));
        assert!(!before.has_same_launch_context(&after));
        assert!(!before.has_same_launch_environment_for(&after, &http_server("remote")));
        assert!(before.has_same_launch_environment_for(&after, &http_server("local")));

        let inherited = McpRuntimeContext::new_with_environment_overrides(
            environment_manager,
            PathBuf::from("/tmp"),
            [("remote".to_string(), Arc::clone(&before_environment))],
        );
        let inherited_environment = inherited
            .resolve_server_environment("http", &http_server("remote"))
            .expect("inherited remote environment should resolve")
            .expect("inherited remote environment should exist");
        assert!(Arc::ptr_eq(&before_environment, &inherited_environment));
        assert!(before.has_same_launch_context(&inherited));
        assert!(before.has_same_launch_environment_for(&inherited, &http_server("remote")));
    }

    #[tokio::test]
    async fn remote_stdio_accepts_foreign_absolute_cwd() {
        let runtime_context = McpRuntimeContext::new(
            Arc::new(
                EnvironmentManager::create_for_tests(
                    Some("ws://127.0.0.1:8765".to_string()),
                    /*local_runtime_paths*/ None,
                )
                .await,
            ),
            PathBuf::from("/tmp"),
        );
        let mut remote_stdio = stdio_server("remote");
        let McpServerTransportConfig::Stdio { cwd, .. } = &mut remote_stdio.transport else {
            unreachable!("stdio helper should build stdio transport");
        };
        *cwd = Some(
            PathUri::parse("file:///C:/plugins/demo")
                .expect("foreign cwd URI")
                .into(),
        );

        let resolved_runtime =
            match runtime_context.resolve_server_environment("stdio", &remote_stdio) {
                Ok(resolved_runtime) => resolved_runtime,
                Err(error) => panic!("foreign cwd should resolve: {error}"),
            };
        assert!(resolved_runtime.is_some());
    }

    #[tokio::test]
    async fn local_stdio_accepts_local_environment_when_available() {
        let runtime_context = McpRuntimeContext::new(
            Arc::new(EnvironmentManager::default_for_tests()),
            PathBuf::from("/tmp"),
        );

        let resolved_runtime = match runtime_context
            .resolve_server_environment("stdio", &stdio_server(DEFAULT_MCP_SERVER_ENVIRONMENT_ID))
        {
            Ok(resolved_runtime) => resolved_runtime,
            Err(error) => panic!("local stdio MCP should resolve: {error}"),
        };
        assert!(resolved_runtime.is_some());
    }
}
