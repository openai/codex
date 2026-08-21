use std::collections::HashMap;

use codex_config::CONFIG_TOML_FILE;
use codex_config::McpServerConfig;
use codex_config::McpServerDisabledReason;
use codex_config::RequirementSource;
use codex_config::RequirementsLayerEntry;
use codex_config::compose_requirements_for_hostname;
use codex_config::format_config_layer_source;
use codex_config::host_name;
use codex_config::loader::LocalTomlLayerStack;
use codex_config::loader::load_local_config_layers;
use codex_exec_server_protocol::EnvironmentConfigLayer;
use codex_exec_server_protocol::EnvironmentConfigLayerStack;
use codex_exec_server_protocol::EnvironmentConfigReadParams;
use codex_exec_server_protocol::EnvironmentConfigReadResponse;
use codex_file_system::ExecutorFileSystem;
use codex_utils_home_dir::find_codex_home;
use codex_utils_path_uri::PathUri;

use crate::Environment;
use crate::ExecServerError;

#[derive(Debug, thiserror::Error)]
pub(crate) enum ReadEnvironmentConfigError {
    #[error("{0}")]
    InvalidParams(String),
    #[error("{0}")]
    Internal(String),
}

pub(crate) async fn read_environment_config(
    file_system: &dyn ExecutorFileSystem,
    params: EnvironmentConfigReadParams,
) -> Result<EnvironmentConfigReadResponse, ReadEnvironmentConfigError> {
    validate_paths(&params)?;
    let cwd = params
        .cwd
        .to_abs_path()
        .map_err(|error| ReadEnvironmentConfigError::InvalidParams(error.to_string()))?;
    let codex_home = find_codex_home().map_err(|error| {
        ReadEnvironmentConfigError::Internal(format!("failed to find Codex home: {error}"))
    })?;
    let layers = load_local_config_layers(file_system, codex_home.as_path(), &cwd)
        .await
        .map_err(|error| {
            ReadEnvironmentConfigError::Internal(format!(
                "failed to load executor-local config: {error}"
            ))
        })?
        .project(&params.config_paths, &params.requirements_paths);

    Ok(EnvironmentConfigReadResponse {
        user_home_dir: dirs::home_dir()
            .and_then(|home_dir| PathUri::from_host_native_path(home_dir).ok()),
        codex_home_dir: PathUri::from_abs_path(&codex_home),
        hostname: host_name(),
        config: serialize_layer_stack(layers.config, |source| {
            format_config_layer_source(source, CONFIG_TOML_FILE)
        })?,
        requirements: serialize_layer_stack(layers.requirements, ToString::to_string)?,
    })
}

impl Environment {
    /// Reads executor-owned HTTP MCP servers from the selected project's current config.
    pub async fn discover_http_mcp_servers(
        &self,
        cwd: PathUri,
    ) -> Result<Vec<(String, McpServerConfig)>, ExecServerError> {
        let (response, http_header_env_vars) =
            tokio::time::timeout(std::time::Duration::from_secs(10), async {
                let capabilities = self.info().await?.capabilities;
                if !capabilities.environment_config_read {
                    return Ok((None, false));
                }
                self.read_environment_config(EnvironmentConfigReadParams {
                    cwd,
                    config_paths: vec![vec!["mcp_servers".to_string()]],
                    requirements_paths: vec![vec!["mcp_servers".to_string()]],
                })
                .await
                .map(|response| (Some(response), capabilities.http_header_env_vars))
            })
            .await
            .map_err(|_| {
                ExecServerError::Protocol("executor MCP discovery timed out".to_string())
            })??;
        let Some(response) = response else {
            return Ok(Vec::new());
        };
        let requirements = compose_requirements_for_hostname(
            response.requirements.layers.into_iter().map(|layer| {
                RequirementsLayerEntry::from_toml(RequirementSource::Unknown, layer.toml)
            }),
            response.hostname.as_deref(),
        )
        .map_err(|error| {
            ExecServerError::Protocol(format!("invalid executor-local MCP requirements: {error}"))
        })?
        .and_then(|requirements| requirements.mcp_servers);
        let mut merged = toml::Value::Table(toml::map::Map::new());
        for layer in response.config.layers {
            let config = toml::from_str::<toml::Value>(&layer.toml).map_err(|error| {
                ExecServerError::Protocol(format!("invalid executor-local MCP config: {error}"))
            })?;
            codex_config::merge_toml_values(&mut merged, &config);
        }
        let mut servers = merged
            .get("mcp_servers")
            .cloned()
            .unwrap_or_else(|| toml::Value::Table(toml::map::Map::new()));
        if let Some(servers) = servers.as_table_mut() {
            servers.retain(|_, server| {
                server.get("url").is_some()
                    && (http_header_env_vars || server.get("bearer_token_env_var").is_none())
            });
        }
        let servers = servers
            .try_into::<HashMap<String, McpServerConfig>>()
            .map_err(|error| {
                ExecServerError::Protocol(format!("invalid executor-local MCP servers: {error}"))
            })?;

        Ok(servers
            .into_iter()
            .map(|(name, mut server)| {
                if let Some(requirements) = requirements.as_ref()
                    && !requirements
                        .value
                        .get(&name)
                        .is_some_and(|requirement| server.matches_requirement(requirement))
                {
                    server.enabled = false;
                    server.disabled_reason = Some(McpServerDisabledReason::Requirements {
                        source: requirements.source.clone(),
                    });
                }
                (name, server)
            })
            .collect())
    }
}

fn validate_paths(params: &EnvironmentConfigReadParams) -> Result<(), ReadEnvironmentConfigError> {
    if params.config_paths.is_empty() && params.requirements_paths.is_empty() {
        return Err(ReadEnvironmentConfigError::InvalidParams(
            "at least one config or requirements path is required".to_string(),
        ));
    }
    if params
        .config_paths
        .iter()
        .chain(&params.requirements_paths)
        .any(Vec::is_empty)
    {
        return Err(ReadEnvironmentConfigError::InvalidParams(
            "TOML paths must contain at least one key segment".to_string(),
        ));
    }
    Ok(())
}

fn serialize_layer_stack<S>(
    stack: LocalTomlLayerStack<S>,
    source_name: impl Fn(&S) -> String,
) -> Result<EnvironmentConfigLayerStack, ReadEnvironmentConfigError> {
    let layers = stack
        .layers
        .into_iter()
        .map(|layer| {
            let toml = toml::to_string(&layer.toml).map_err(|error| {
                ReadEnvironmentConfigError::Internal(format!(
                    "failed to serialize executor-local config: {error}"
                ))
            })?;
            Ok(EnvironmentConfigLayer {
                source: source_name(&layer.source),
                base_dir: PathUri::from_abs_path(&layer.base_dir),
                toml,
            })
        })
        .collect::<Result<Vec<_>, ReadEnvironmentConfigError>>()?;
    Ok(EnvironmentConfigLayerStack {
        layers,
        cloud_insertion_index: stack.cloud_insertion_index,
    })
}
