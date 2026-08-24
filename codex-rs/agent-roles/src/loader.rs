use crate::AgentRoleConfig;
use crate::ResolvedAgentRoleFile;
use crate::agent_role_config::normalize_agent_role_description;
use crate::agent_role_config::normalize_agent_role_nickname_candidates;
use crate::discovery::collect_agent_role_files;
use crate::parse_agent_role_file_contents;
use codex_config::ConfigLayerStack;
use codex_config::config_toml::AgentRoleToml;
use codex_config::config_toml::AgentsToml;
use codex_config::config_toml::ConfigToml;
use codex_file_system::ExecutorFileSystem;
use codex_file_system::GetMetadataOptions;
use codex_file_system::ReadFileOptions;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_absolute_path::AbsolutePathBufGuard;
use codex_utils_path_uri::PathUri;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::Path;
use std::path::PathBuf;
use toml::Value as TomlValue;

pub async fn load_agent_roles(
    fs: &dyn ExecutorFileSystem,
    cfg: &ConfigToml,
    config_layer_stack: &ConfigLayerStack,
    startup_warnings: &mut Vec<String>,
) -> std::io::Result<BTreeMap<String, AgentRoleConfig>> {
    let mut layers = config_layer_stack.layers_low_to_high().peekable();
    if layers.peek().is_none() {
        return load_agent_roles_without_layers(fs, cfg).await;
    }

    let mut roles: BTreeMap<String, AgentRoleConfig> = BTreeMap::new();
    for layer in layers {
        let mut layer_roles: BTreeMap<String, AgentRoleConfig> = BTreeMap::new();
        let mut declared_role_files = BTreeSet::new();
        let config_folder = layer.config_folder();
        let agents_toml = match agents_toml_from_layer(&layer.config, config_folder.as_deref()) {
            Ok(agents_toml) => agents_toml,
            Err(err) => {
                push_agent_role_warning(startup_warnings, err);
                None
            }
        };
        if let Some(agents_toml) = agents_toml {
            for (declared_role_name, role_toml) in &agents_toml.roles {
                let (role_name, role) =
                    match read_declared_role(fs, declared_role_name, role_toml).await {
                        Ok(role) => role,
                        Err(err) => {
                            push_agent_role_warning(startup_warnings, err);
                            continue;
                        }
                    };
                if let Some(config_file) = role.config_file.clone() {
                    declared_role_files.insert(config_file);
                }
                if layer_roles.contains_key(&role_name) {
                    push_agent_role_warning(
                        startup_warnings,
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            format!(
                                "duplicate agent role name `{role_name}` declared in the same config layer"
                            ),
                        ),
                    );
                    continue;
                }
                layer_roles.insert(role_name, role);
            }
        }

        if let Some(config_folder) = layer.config_folder() {
            for (role_name, role) in discover_agent_roles_in_dir(
                fs,
                &config_folder.join("agents"),
                &declared_role_files,
                startup_warnings,
            )
            .await?
            {
                if layer_roles.contains_key(&role_name) {
                    push_agent_role_warning(
                        startup_warnings,
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            format!(
                                "duplicate agent role name `{role_name}` declared in the same config layer"
                            ),
                        ),
                    );
                    continue;
                }
                layer_roles.insert(role_name, role);
            }
        }

        for (role_name, role) in layer_roles {
            let mut merged_role = role;
            if let Some(existing_role) = roles.get(&role_name) {
                merge_missing_role_fields(&mut merged_role, existing_role);
            }
            if let Err(err) = validate_required_agent_role_description(
                &role_name,
                merged_role.description.as_deref(),
            ) {
                push_agent_role_warning(startup_warnings, err);
                continue;
            }
            roles.insert(role_name, merged_role);
        }
    }

    Ok(roles)
}

fn push_agent_role_warning(startup_warnings: &mut Vec<String>, err: std::io::Error) {
    let message = format!("Ignoring malformed agent role definition: {err}");
    tracing::warn!("{message}");
    startup_warnings.push(message);
}

async fn load_agent_roles_without_layers(
    fs: &dyn ExecutorFileSystem,
    cfg: &ConfigToml,
) -> std::io::Result<BTreeMap<String, AgentRoleConfig>> {
    let mut roles = BTreeMap::new();
    if let Some(agents_toml) = cfg.agents.as_ref() {
        for (declared_role_name, role_toml) in &agents_toml.roles {
            let (role_name, role) = read_declared_role(fs, declared_role_name, role_toml).await?;
            validate_required_agent_role_description(&role_name, role.description.as_deref())?;

            if roles.insert(role_name.clone(), role).is_some() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("duplicate agent role name `{role_name}` declared in config"),
                ));
            }
        }
    }

    Ok(roles)
}

async fn read_declared_role(
    fs: &dyn ExecutorFileSystem,
    declared_role_name: &str,
    role_toml: &AgentRoleToml,
) -> std::io::Result<(String, AgentRoleConfig)> {
    let mut role = agent_role_config_from_toml(fs, declared_role_name, role_toml).await?;
    let mut role_name = declared_role_name.to_string();
    if let Some(config_file) = role.config_file.as_deref() {
        let config_file = AbsolutePathBuf::from_absolute_path(config_file)?;
        let parsed_file =
            read_resolved_agent_role_file(fs, &config_file, Some(declared_role_name)).await?;
        role_name = parsed_file.role_name;
        role.description = parsed_file.description.or(role.description);
        role.nickname_candidates = parsed_file.nickname_candidates.or(role.nickname_candidates);
    }

    Ok((role_name, role))
}

fn merge_missing_role_fields(role: &mut AgentRoleConfig, fallback: &AgentRoleConfig) {
    role.description = role.description.clone().or(fallback.description.clone());
    role.config_file = role.config_file.clone().or(fallback.config_file.clone());
    role.nickname_candidates = role
        .nickname_candidates
        .clone()
        .or(fallback.nickname_candidates.clone());
}

fn agents_toml_from_layer(
    layer_toml: &TomlValue,
    config_base_dir: Option<&Path>,
) -> std::io::Result<Option<AgentsToml>> {
    let Some(agents_toml) = layer_toml.get("agents") else {
        return Ok(None);
    };

    // AbsolutePathBufGuard resolves relative paths while it remains in scope.
    let _guard = config_base_dir.map(AbsolutePathBufGuard::new);
    agents_toml
        .clone()
        .try_into()
        .map(Some)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))
}

async fn agent_role_config_from_toml(
    fs: &dyn ExecutorFileSystem,
    role_name: &str,
    role: &AgentRoleToml,
) -> std::io::Result<AgentRoleConfig> {
    let config_file = role
        .config_file
        .as_ref()
        .map(AbsolutePathBuf::from_absolute_path)
        .transpose()?;
    validate_agent_role_config_file(fs, role_name, config_file.as_ref()).await?;
    let description = normalize_agent_role_description(
        &format!("agents.{role_name}.description"),
        role.description.as_deref(),
    )?;
    let nickname_candidates = normalize_agent_role_nickname_candidates(
        &format!("agents.{role_name}.nickname_candidates"),
        role.nickname_candidates.as_deref(),
    )?;

    Ok(AgentRoleConfig {
        description,
        config_file: config_file.map(AbsolutePathBuf::into_path_buf),
        nickname_candidates,
    })
}

async fn read_resolved_agent_role_file(
    fs: &dyn ExecutorFileSystem,
    path: &AbsolutePathBuf,
    role_name_hint: Option<&str>,
) -> std::io::Result<ResolvedAgentRoleFile> {
    let path_uri = PathUri::from_abs_path(path);
    let contents = fs
        .read_file_text(&path_uri, ReadFileOptions::default(), /*sandbox*/ None)
        .await?;
    let config_base_dir = path.parent().unwrap_or_else(|| path.clone());
    parse_agent_role_file_contents(
        &contents,
        path.as_path(),
        config_base_dir.as_path(),
        role_name_hint,
    )
}

fn validate_required_agent_role_description(
    role_name: &str,
    description: Option<&str>,
) -> std::io::Result<()> {
    if description.is_some() {
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("agent role `{role_name}` must define a description"),
        ))
    }
}

async fn validate_agent_role_config_file(
    fs: &dyn ExecutorFileSystem,
    role_name: &str,
    config_file: Option<&AbsolutePathBuf>,
) -> std::io::Result<()> {
    let Some(config_file) = config_file else {
        return Ok(());
    };

    let config_file_uri = PathUri::from_abs_path(config_file);
    let metadata = fs
        .get_metadata(
            &config_file_uri,
            GetMetadataOptions::default(),
            /*sandbox*/ None,
        )
        .await
        .map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "agents.{role_name}.config_file must point to an existing file at {}: {e}",
                    config_file.as_path().display()
                ),
            )
        })?;
    if metadata.is_file {
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "agents.{role_name}.config_file must point to a file: {}",
                config_file.as_path().display()
            ),
        ))
    }
}

async fn discover_agent_roles_in_dir(
    fs: &dyn ExecutorFileSystem,
    agents_dir: &AbsolutePathBuf,
    declared_role_files: &BTreeSet<PathBuf>,
    startup_warnings: &mut Vec<String>,
) -> std::io::Result<BTreeMap<String, AgentRoleConfig>> {
    let mut roles = BTreeMap::new();

    for agent_file in collect_agent_role_files(fs, agents_dir).await? {
        if declared_role_files.contains(agent_file.as_path()) {
            continue;
        }
        let parsed_file =
            match read_resolved_agent_role_file(fs, &agent_file, /*role_name_hint*/ None).await {
                Ok(parsed_file) => parsed_file,
                Err(err) => {
                    push_agent_role_warning(startup_warnings, err);
                    continue;
                }
            };
        let role_name = parsed_file.role_name;
        if roles.contains_key(&role_name) {
            push_agent_role_warning(
                startup_warnings,
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!(
                        "duplicate agent role name `{role_name}` discovered in {}",
                        agents_dir.as_path().display()
                    ),
                ),
            );
            continue;
        }
        roles.insert(
            role_name,
            AgentRoleConfig {
                description: parsed_file.description,
                config_file: Some(agent_file.to_path_buf()),
                nickname_candidates: parsed_file.nickname_candidates,
            },
        );
    }

    Ok(roles)
}
