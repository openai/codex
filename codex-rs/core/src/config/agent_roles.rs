use super::AgentRoleConfig;
use crate::config_loader::ConfigLayerStack;
use crate::config_loader::ConfigLayerStackOrdering;
use codex_config::config_toml::AgentRoleToml;
use codex_config::config_toml::AgentsToml;
use codex_config::config_toml::ConfigToml;
use codex_exec_server::ExecutorFileSystem;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_absolute_path::AbsolutePathBufGuard;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use toml::Value as TomlValue;

pub(crate) async fn load_agent_roles(
    fs: &dyn ExecutorFileSystem,
    cfg: &ConfigToml,
    config_layer_stack: &ConfigLayerStack,
    startup_warnings: &mut Vec<String>,
) -> std::io::Result<BTreeMap<String, AgentRoleConfig>> {
    let layers = config_layer_stack.get_layers(
        ConfigLayerStackOrdering::LowestPrecedenceFirst,
        /*include_disabled*/ false,
    );
    if layers.is_empty() {
        return load_agent_roles_without_layers(fs, cfg).await;
    }

    let mut roles: BTreeMap<String, AgentRoleConfig> = BTreeMap::new();
    for layer in layers {
        let mut layer_roles: BTreeMap<String, AgentRoleConfig> = BTreeMap::new();
        let mut declared_role_files = BTreeSet::new();
        let agents_toml = match agents_toml_from_layer(&layer.config) {
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
    let (mut role, config_file) =
        agent_role_config_from_toml(fs, declared_role_name, role_toml).await?;
    let mut role_name = declared_role_name.to_string();
    if let Some(config_file) = config_file.as_ref() {
        let parsed_file =
            read_resolved_agent_role_file(fs, config_file, Some(declared_role_name)).await?;
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

fn agents_toml_from_layer(layer_toml: &TomlValue) -> std::io::Result<Option<AgentsToml>> {
    let Some(agents_toml) = layer_toml.get("agents") else {
        return Ok(None);
    };

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
) -> std::io::Result<(AgentRoleConfig, Option<AbsolutePathBuf>)> {
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

    Ok((
        AgentRoleConfig {
            description,
            config_file: config_file.as_ref().map(AbsolutePathBuf::to_path_buf),
            nickname_candidates,
        },
        config_file,
    ))
}

#[derive(Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(deny_unknown_fields)]
struct RawAgentRoleFileToml {
    name: Option<String>,
    description: Option<String>,
    nickname_candidates: Option<Vec<String>>,
    #[serde(flatten)]
    config: ConfigToml,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ResolvedAgentRoleFile {
    pub(crate) role_name: String,
    pub(crate) description: Option<String>,
    pub(crate) nickname_candidates: Option<Vec<String>>,
    pub(crate) config: TomlValue,
}

pub(crate) fn parse_agent_role_file_contents(
    contents: &str,
    role_file_label: &Path,
    config_base_dir: &Path,
    role_name_hint: Option<&str>,
) -> std::io::Result<ResolvedAgentRoleFile> {
    let role_file_toml: TomlValue = toml::from_str(contents).map_err(|err| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "failed to parse agent role file at {}: {err}",
                role_file_label.display()
            ),
        )
    })?;
    let _guard = AbsolutePathBufGuard::new(config_base_dir);
    let parsed: RawAgentRoleFileToml = role_file_toml.clone().try_into().map_err(|err| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "failed to deserialize agent role file at {}: {err}",
                role_file_label.display()
            ),
        )
    })?;
    let description = normalize_agent_role_description(
        &format!("agent role file {}.description", role_file_label.display()),
        parsed.description.as_deref(),
    )?;
    validate_agent_role_file_developer_instructions(
        role_file_label,
        parsed.config.developer_instructions.as_deref(),
        role_name_hint.is_none(),
    )?;

    let role_name = parsed
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| role_name_hint.map(ToOwned::to_owned))
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "agent role file at {} must define a non-empty `name`",
                    role_file_label.display()
                ),
            )
        })?;

    let nickname_candidates = normalize_agent_role_nickname_candidates(
        &format!(
            "agent role file {}.nickname_candidates",
            role_file_label.display()
        ),
        parsed.nickname_candidates.as_deref(),
    )?;

    let mut config = role_file_toml;
    let Some(config_table) = config.as_table_mut() else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "agent role file at {} must contain a TOML table",
                role_file_label.display()
            ),
        ));
    };
    config_table.remove("name");
    config_table.remove("description");
    config_table.remove("nickname_candidates");

    Ok(ResolvedAgentRoleFile {
        role_name,
        description,
        nickname_candidates,
        config,
    })
}

async fn read_resolved_agent_role_file(
    fs: &dyn ExecutorFileSystem,
    path: &AbsolutePathBuf,
    role_name_hint: Option<&str>,
) -> std::io::Result<ResolvedAgentRoleFile> {
    let contents = fs.read_file_text(path, /*sandbox*/ None).await?;
    let config_base_dir = path.parent().unwrap_or_else(|| path.clone());
    parse_agent_role_file_contents(
        &contents,
        path.as_path(),
        config_base_dir.as_path(),
        role_name_hint,
    )
}

fn normalize_agent_role_description(
    field_label: &str,
    description: Option<&str>,
) -> std::io::Result<Option<String>> {
    match description.map(str::trim) {
        Some("") => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("{field_label} cannot be blank"),
        )),
        Some(description) => Ok(Some(description.to_string())),
        None => Ok(None),
    }
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

fn validate_agent_role_file_developer_instructions(
    role_file_label: &Path,
    developer_instructions: Option<&str>,
    require_present: bool,
) -> std::io::Result<()> {
    match developer_instructions.map(str::trim) {
        Some("") => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "agent role file at {}.developer_instructions cannot be blank",
                role_file_label.display()
            ),
        )),
        Some(_) => Ok(()),
        None if require_present => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "agent role file at {} must define `developer_instructions`",
                role_file_label.display()
            ),
        )),
        None => Ok(()),
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

    let metadata = fs
        .get_metadata(config_file, /*sandbox*/ None)
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

fn normalize_agent_role_nickname_candidates(
    field_label: &str,
    nickname_candidates: Option<&[String]>,
) -> std::io::Result<Option<Vec<String>>> {
    let Some(nickname_candidates) = nickname_candidates else {
        return Ok(None);
    };

    if nickname_candidates.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("{field_label} must contain at least one name"),
        ));
    }

    let mut normalized_candidates = Vec::with_capacity(nickname_candidates.len());
    let mut seen_candidates = BTreeSet::new();

    for nickname in nickname_candidates {
        let normalized_nickname = nickname.trim();
        if normalized_nickname.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("{field_label} cannot contain blank names"),
            ));
        }

        if !seen_candidates.insert(normalized_nickname.to_owned()) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("{field_label} cannot contain duplicates"),
            ));
        }

        if !normalized_nickname
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '-' | '_'))
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "{field_label} may only contain ASCII letters, digits, spaces, hyphens, and underscores"
                ),
            ));
        }

        normalized_candidates.push(normalized_nickname.to_owned());
    }

    Ok(Some(normalized_candidates))
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

async fn collect_agent_role_files(
    fs: &dyn ExecutorFileSystem,
    dir: &AbsolutePathBuf,
) -> std::io::Result<Vec<AbsolutePathBuf>> {
    let mut files = Vec::new();
    let mut dirs = vec![dir.clone()];
    while let Some(dir) = dirs.pop() {
        let entries = match fs.read_directory(&dir, /*sandbox*/ None).await {
            Ok(entries) => entries,
            Err(err) if err.kind() == ErrorKind::NotFound => continue,
            Err(err) => return Err(err),
        };

        for entry in entries {
            let path = dir.join(entry.file_name);
            if entry.is_directory {
                dirs.push(path);
                continue;
            }
            if entry.is_file
                && path
                    .as_path()
                    .extension()
                    .is_some_and(|extension| extension == "toml")
            {
                files.push(path);
            }
        }
    }

    files.sort();
    Ok(files)
}
