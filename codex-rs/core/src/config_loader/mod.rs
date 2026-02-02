mod cloud_requirements;
mod config_requirements;
mod diagnostics;
mod fingerprint;
mod layer_io;
#[cfg(target_os = "macos")]
mod macos;
mod merge;
mod overrides;
mod requirements_exec_policy;
mod state;

#[cfg(test)]
mod tests;

use crate::config::CONFIG_TOML_FILE;
use crate::config::ConfigToml;
use crate::config::deserialize_config_toml_with_base;
use crate::config_loader::config_requirements::ConfigRequirementsWithSources;
use crate::config_loader::layer_io::LoadedConfigLayers;
use crate::git_info::resolve_root_git_project_for_trust;
use codex_app_server_protocol::ConfigLayerSource;
use codex_protocol::config_types::SandboxMode;
use codex_protocol::config_types::TrustLevel;
use codex_protocol::protocol::AskForApproval;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_absolute_path::AbsolutePathBufGuard;
use dunce::canonicalize as normalize_path;
use serde::Deserialize;
use std::io;
use std::path::Path;
use toml::Value as TomlValue;

pub use cloud_requirements::CloudRequirementsLoader;
pub use config_requirements::ConfigRequirements;
pub use config_requirements::ConfigRequirementsToml;
pub use config_requirements::McpServerIdentity;
pub use config_requirements::McpServerRequirement;
pub use config_requirements::RequirementSource;
pub use config_requirements::ResidencyRequirement;
pub use config_requirements::SandboxModeRequirement;
pub use config_requirements::Sourced;
pub use diagnostics::ConfigError;
pub use diagnostics::ConfigLoadError;
pub use diagnostics::TextPosition;
pub use diagnostics::TextRange;
pub(crate) use diagnostics::config_error_from_toml;
pub(crate) use diagnostics::first_layer_config_error;
pub(crate) use diagnostics::first_layer_config_error_from_entries;
pub use diagnostics::format_config_error;
pub use diagnostics::format_config_error_with_source;
pub(crate) use diagnostics::io_error_from_config_error;
pub use merge::merge_toml_values;
pub(crate) use overrides::build_cli_overrides_layer;
pub use state::ConfigLayerEntry;
pub use state::ConfigLayerStack;
pub use state::ConfigLayerStackOrdering;
pub use state::LoaderOverrides;

/// On Unix systems, load requirements from this file path, if present.
const DEFAULT_REQUIREMENTS_TOML_FILE_UNIX: &str = "/etc/codex/requirements.toml";

/// On Unix systems, load default settings from this file path, if present.
/// Note that /etc/codex/ is treated as a "config folder," so subfolders such
/// as skills/ and rules/ will also be honored.
pub const SYSTEM_CONFIG_TOML_FILE_UNIX: &str = "/etc/codex/config.toml";

const DEFAULT_PROJECT_ROOT_MARKERS: &[&str] = &[".git"];

/// To build up the set of admin-enforced constraints, we build up from multiple
/// configuration layers in the following order, but a constraint defined in an
/// earlier layer cannot be overridden by a later layer:
///
/// - admin:    managed preferences (*)
/// - cloud:    managed cloud requirements
/// - system    `/etc/codex/requirements.toml`
///
/// For backwards compatibility, we also load from
/// `/etc/codex/managed_config.toml` and map it to
/// `/etc/codex/requirements.toml`.
///
/// Configuration is built up from multiple layers in the following order:
///
/// - admin:    managed preferences (*)
/// - system    `/etc/codex/config.toml`
/// - user      `${CODEX_HOME}/config.toml`
/// - cwd       `${PWD}/config.toml` (loaded but disabled when the directory is untrusted)
/// - tree      parent directories up to root looking for `./.codex/config.toml` (loaded but disabled when untrusted)
/// - repo      `$(git rev-parse --show-toplevel)/.codex/config.toml` (loaded but disabled when untrusted)
/// - runtime   e.g., --config flags, model selector in UI
///
/// (*) Only available on macOS via managed device profiles.
///
/// See https://developers.openai.com/codex/security for details.
///
/// When loading the config stack for a thread, there should be a `cwd`
/// associated with it such that `cwd` should be `Some(...)`. Only for
/// thread-agnostic config loading (e.g., for the app server's `/config`
/// endpoint) should `cwd` be `None`.
pub async fn load_config_layers_state(
    codex_home: &Path,
    cwd: Option<AbsolutePathBuf>,
    cli_overrides: &[(String, TomlValue)],
    overrides: LoaderOverrides,
    cloud_requirements: CloudRequirementsLoader,
) -> io::Result<ConfigLayerStack> {
    let mut config_requirements_toml = ConfigRequirementsWithSources::default();

    #[cfg(target_os = "macos")]
    macos::load_managed_admin_requirements_toml(
        &mut config_requirements_toml,
        overrides
            .macos_managed_config_requirements_base64
            .as_deref(),
    )
    .await?;

    if let Some(requirements) = cloud_requirements.get().await {
        config_requirements_toml
            .merge_unset_fields(RequirementSource::CloudRequirements, requirements);
    }

    // Honor /etc/codex/requirements.toml.
    if cfg!(unix) {
        load_requirements_toml(
            &mut config_requirements_toml,
            DEFAULT_REQUIREMENTS_TOML_FILE_UNIX,
        )
        .await?;
    }

    // Make a best-effort to support the legacy `managed_config.toml` as a
    // requirements specification.
    let loaded_config_layers = layer_io::load_config_layers_internal(codex_home, overrides).await?;
    load_requirements_from_legacy_scheme(
        &mut config_requirements_toml,
        loaded_config_layers.clone(),
    )
    .await?;

    let mut layers = Vec::<ConfigLayerEntry>::new();

    let cli_overrides_layer = if cli_overrides.is_empty() {
        None
    } else {
        Some(overrides::build_cli_overrides_layer(cli_overrides))
    };

    // Include an entry for the "system" config folder, loading its config.toml,
    // if it exists.
    let system_config_toml_file = if cfg!(unix) {
        Some(AbsolutePathBuf::from_absolute_path(
            SYSTEM_CONFIG_TOML_FILE_UNIX,
        )?)
    } else {
        // TODO(gt): Determine the path to load on Windows.
        None
    };
    if let Some(system_config_toml_file) = system_config_toml_file {
        let system_layer =
            load_config_toml_for_required_layer(&system_config_toml_file, |config_toml| {
                ConfigLayerEntry::new(
                    ConfigLayerSource::System {
                        file: system_config_toml_file.clone(),
                    },
                    config_toml,
                )
            })
            .await?;
        layers.push(system_layer);
    }

    // Add a layer for $CODEX_HOME/config.toml if it exists. Note if the file
    // exists, but is malformed, then this error should be propagated to the
    // user.
    let user_file = AbsolutePathBuf::resolve_path_against_base(CONFIG_TOML_FILE, codex_home)?;
    let user_layer = load_config_toml_for_required_layer(&user_file, |config_toml| {
        ConfigLayerEntry::new(
            ConfigLayerSource::User {
                file: user_file.clone(),
            },
            config_toml,
        )
    })
    .await?;
    layers.push(user_layer);

    if let Some(cwd) = cwd {
        let mut merged_so_far = TomlValue::Table(toml::map::Map::new());
        for layer in &layers {
            merge_toml_values(&mut merged_so_far, &layer.config);
        }
        if let Some(cli_overrides_layer) = cli_overrides_layer.as_ref() {
            merge_toml_values(&mut merged_so_far, cli_overrides_layer);
        }

        let project_root_markers = match project_root_markers_from_config(&merged_so_far) {
            Ok(markers) => markers.unwrap_or_else(default_project_root_markers),
            Err(err) => {
                if let Some(config_error) = first_layer_config_error_from_entries(&layers).await {
                    return Err(io_error_from_config_error(
                        io::ErrorKind::InvalidData,
                        config_error,
                        None,
                    ));
                }
                return Err(err);
            }
        };
        let project_trust_context = match project_trust_context(
            &merged_so_far,
            &cwd,
            &project_root_markers,
            codex_home,
            &user_file,
        )
        .await
        {
            Ok(context) => context,
            Err(err) => {
                let source = err
                    .get_ref()
                    .and_then(|err| err.downcast_ref::<toml::de::Error>())
                    .cloned();
                if let Some(config_error) = first_layer_config_error_from_entries(&layers).await {
                    return Err(io_error_from_config_error(
                        io::ErrorKind::InvalidData,
                        config_error,
                        source,
                    ));
                }
                return Err(err);
            }
        };
        let project_layers = load_project_layers(
            &cwd,
            &project_trust_context.project_root,
            &project_trust_context,
            codex_home,
        )
        .await?;
        layers.extend(project_layers);
    }

    // Add a layer for runtime overrides from the CLI or UI, if any exist.
    if let Some(cli_overrides_layer) = cli_overrides_layer {
        layers.push(ConfigLayerEntry::new(
            ConfigLayerSource::SessionFlags,
            cli_overrides_layer,
        ));
    }

    // Make a best-effort to support the legacy `managed_config.toml` as a
    // config layer on top of everything else. For fields in
    // `managed_config.toml` that do not have an equivalent in
    // `ConfigRequirements`, note users can still override these values on a
    // per-turn basis in the TUI and VS Code.
    let LoadedConfigLayers {
        managed_config,
        managed_config_from_mdm,
    } = loaded_config_layers;
    if let Some(config) = managed_config {
        let managed_parent = config.file.as_path().parent().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Managed config file {} has no parent directory",
                    config.file.as_path().display()
                ),
            )
        })?;
        let managed_config =
            resolve_relative_paths_in_config_toml(config.managed_config, managed_parent)?;
        layers.push(ConfigLayerEntry::new(
            ConfigLayerSource::LegacyManagedConfigTomlFromFile { file: config.file },
            managed_config,
        ));
    }
    if let Some(config) = managed_config_from_mdm {
        layers.push(ConfigLayerEntry::new(
            ConfigLayerSource::LegacyManagedConfigTomlFromMdm,
            config,
        ));
    }

    ConfigLayerStack::new(
        layers,
        config_requirements_toml.clone().try_into()?,
        config_requirements_toml.into_toml(),
    )
}

/// Attempts to load a config.toml file from `config_toml`.
/// - If the file exists and is valid TOML, passes the parsed `toml::Value` to
///   `create_entry` and returns the resulting layer entry.
/// - If the file does not exist, uses an empty `Table` with `create_entry` and
///   returns the resulting layer entry.
/// - If there is an error reading the file or parsing the TOML, returns an
///   error.
async fn load_config_toml_for_required_layer(
    config_toml: impl AsRef<Path>,
    create_entry: impl FnOnce(TomlValue) -> ConfigLayerEntry,
) -> io::Result<ConfigLayerEntry> {
    let toml_file = config_toml.as_ref();
    let toml_value = match tokio::fs::read_to_string(toml_file).await {
        Ok(contents) => {
            let config: TomlValue = toml::from_str(&contents).map_err(|err| {
                let config_error = config_error_from_toml(toml_file, &contents, err.clone());
                io_error_from_config_error(io::ErrorKind::InvalidData, config_error, Some(err))
            })?;
            let config_parent = toml_file.parent().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "Config file {} has no parent directory",
                        toml_file.display()
                    ),
                )
            })?;
            resolve_relative_paths_in_config_toml(config, config_parent)
        }
        Err(e) => {
            if e.kind() == io::ErrorKind::NotFound {
                Ok(TomlValue::Table(toml::map::Map::new()))
            } else {
                Err(io::Error::new(
                    e.kind(),
                    format!("Failed to read config file {}: {e}", toml_file.display()),
                ))
            }
        }
    }?;

    Ok(create_entry(toml_value))
}

/// If available, apply requirements from `/etc/codex/requirements.toml` to
/// `config_requirements_toml` by filling in any unset fields.
async fn load_requirements_toml(
    config_requirements_toml: &mut ConfigRequirementsWithSources,
    requirements_toml_file: impl AsRef<Path>,
) -> io::Result<()> {
    let requirements_toml_file =
        AbsolutePathBuf::from_absolute_path(requirements_toml_file.as_ref())?;
    match tokio::fs::read_to_string(&requirements_toml_file).await {
        Ok(contents) => {
            let requirements_config: ConfigRequirementsToml =
                toml::from_str(&contents).map_err(|e| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "Error parsing requirements file {}: {e}",
                            requirements_toml_file.as_ref().display(),
                        ),
                    )
                })?;
            config_requirements_toml.merge_unset_fields(
                RequirementSource::SystemRequirementsToml {
                    file: requirements_toml_file.clone(),
                },
                requirements_config,
            );
        }
        Err(e) => {
            if e.kind() != io::ErrorKind::NotFound {
                return Err(io::Error::new(
                    e.kind(),
                    format!(
                        "Failed to read requirements file {}: {e}",
                        requirements_toml_file.as_ref().display(),
                    ),
                ));
            }
        }
    }

    Ok(())
}

async fn load_requirements_from_legacy_scheme(
    config_requirements_toml: &mut ConfigRequirementsWithSources,
    loaded_config_layers: LoadedConfigLayers,
) -> io::Result<()> {
    // In this implementation, earlier layers cannot be overwritten by later
    // layers, so list managed_config_from_mdm first because it has the highest
    // precedence.
    let LoadedConfigLayers {
        managed_config,
        managed_config_from_mdm,
    } = loaded_config_layers;

    for (source, config) in managed_config_from_mdm
        .map(|config| (RequirementSource::LegacyManagedConfigTomlFromMdm, config))
        .into_iter()
        .chain(managed_config.map(|c| {
            (
                RequirementSource::LegacyManagedConfigTomlFromFile { file: c.file },
                c.managed_config,
            )
        }))
    {
        let legacy_config: LegacyManagedConfigToml =
            config.try_into().map_err(|err: toml::de::Error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Failed to parse config requirements as TOML: {err}"),
                )
            })?;

        let new_requirements_toml = ConfigRequirementsToml::from(legacy_config);
        config_requirements_toml.merge_unset_fields(source, new_requirements_toml);
    }

    Ok(())
}

/// Reads `project_root_markers` from the [toml::Value] produced by merging
/// `config.toml` from the config layers in the stack preceding
/// [ConfigLayerSource::Project].
///
/// Invariants:
/// - If `project_root_markers` is not specified, returns `Ok(None)`.
/// - If `project_root_markers` is specified, returns `Ok(Some(markers))` where
///   `markers` is a `Vec<String>` (including `Ok(Some(Vec::new()))` for an
///   empty array, which indicates that root detection should be disabled).
/// - Returns an error if `project_root_markers` is specified but is not an
///   array of strings.
pub(crate) fn project_root_markers_from_config(
    config: &TomlValue,
) -> io::Result<Option<Vec<String>>> {
    let Some(table) = config.as_table() else {
        return Ok(None);
    };
    let Some(markers_value) = table.get("project_root_markers") else {
        return Ok(None);
    };
    let TomlValue::Array(entries) = markers_value else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "project_root_markers must be an array of strings",
        ));
    };
    if entries.is_empty() {
        return Ok(Some(Vec::new()));
    }
    let mut markers = Vec::new();
    for entry in entries {
        let Some(marker) = entry.as_str() else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "project_root_markers must be an array of strings",
            ));
        };
        markers.push(marker.to_string());
    }
    Ok(Some(markers))
}

pub(crate) fn default_project_root_markers() -> Vec<String> {
    DEFAULT_PROJECT_ROOT_MARKERS
        .iter()
        .map(ToString::to_string)
        .collect()
}

struct ProjectTrustContext {
    project_root: AbsolutePathBuf,
    project_root_key: String,
    repo_root_key: Option<String>,
    projects_trust: std::collections::HashMap<String, TrustLevel>,
    user_config_file: AbsolutePathBuf,
}

struct ProjectTrustDecision {
    trust_level: Option<TrustLevel>,
    trust_key: String,
}

impl ProjectTrustDecision {
    fn is_trusted(&self) -> bool {
        matches!(self.trust_level, Some(TrustLevel::Trusted))
    }
}

impl ProjectTrustContext {
    fn decision_for_dir(&self, dir: &AbsolutePathBuf) -> ProjectTrustDecision {
        let dir_key = dir.as_path().to_string_lossy().to_string();
        if let Some(trust_level) = self.projects_trust.get(&dir_key).copied() {
            return ProjectTrustDecision {
                trust_level: Some(trust_level),
                trust_key: dir_key,
            };
        }

        if let Some(trust_level) = self.projects_trust.get(&self.project_root_key).copied() {
            return ProjectTrustDecision {
                trust_level: Some(trust_level),
                trust_key: self.project_root_key.clone(),
            };
        }

        if let Some(repo_root_key) = self.repo_root_key.as_ref()
            && let Some(trust_level) = self.projects_trust.get(repo_root_key).copied()
        {
            return ProjectTrustDecision {
                trust_level: Some(trust_level),
                trust_key: repo_root_key.clone(),
            };
        }

        ProjectTrustDecision {
            trust_level: None,
            trust_key: self
                .repo_root_key
                .clone()
                .unwrap_or_else(|| self.project_root_key.clone()),
        }
    }

    fn disabled_reason_for_dir(&self, dir: &AbsolutePathBuf) -> Option<String> {
        let decision = self.decision_for_dir(dir);
        if decision.is_trusted() {
            return None;
        }

        let trust_key = decision.trust_key.as_str();
        let user_config_file = self.user_config_file.as_path().display();
        match decision.trust_level {
            Some(TrustLevel::Untrusted) => Some(format!(
                "{trust_key} is marked as untrusted in {user_config_file}. To load config.toml, mark it trusted."
            )),
            _ => Some(format!(
                "To load config.toml, add {trust_key} as a trusted project in {user_config_file}."
            )),
        }
    }
}

fn project_layer_entry(
    trust_context: &ProjectTrustContext,
    dot_codex_folder: &AbsolutePathBuf,
    layer_dir: &AbsolutePathBuf,
    config: TomlValue,
    config_toml_exists: bool,
) -> ConfigLayerEntry {
    let source = ConfigLayerSource::Project {
        dot_codex_folder: dot_codex_folder.clone(),
    };

    if config_toml_exists && let Some(reason) = trust_context.disabled_reason_for_dir(layer_dir) {
        ConfigLayerEntry::new_disabled(source, config, reason)
    } else {
        ConfigLayerEntry::new(source, config)
    }
}

async fn project_trust_context(
    merged_config: &TomlValue,
    cwd: &AbsolutePathBuf,
    project_root_markers: &[String],
    config_base_dir: &Path,
    user_config_file: &AbsolutePathBuf,
) -> io::Result<ProjectTrustContext> {
    let config_toml = deserialize_config_toml_with_base(merged_config.clone(), config_base_dir)?;

    let project_root = find_project_root(cwd, project_root_markers).await?;
    let projects = config_toml.projects.unwrap_or_default();

    let project_root_key = project_root.as_path().to_string_lossy().to_string();
    let repo_root = resolve_root_git_project_for_trust(cwd.as_path());
    let repo_root_key = repo_root
        .as_ref()
        .map(|root| root.to_string_lossy().to_string());

    let projects_trust = projects
        .into_iter()
        .filter_map(|(key, project)| project.trust_level.map(|trust_level| (key, trust_level)))
        .collect();

    Ok(ProjectTrustContext {
        project_root,
        project_root_key,
        repo_root_key,
        projects_trust,
        user_config_file: user_config_file.clone(),
    })
}

/// Takes a `toml::Value` parsed from a config.toml file and walks through it,
/// resolving any `AbsolutePathBuf` fields against `base_dir`, returning a new
/// `toml::Value` with the same shape but with paths resolved.
///
/// This ensures that multiple config layers can be merged together correctly
/// even if they were loaded from different directories.
fn resolve_relative_paths_in_config_toml(
    value_from_config_toml: TomlValue,
    base_dir: &Path,
) -> io::Result<TomlValue> {
    // Use the serialize/deserialize round-trip to convert the
    // `toml::Value` into a `ConfigToml` with `AbsolutePath
    let _guard = AbsolutePathBufGuard::new(base_dir);
    let Ok(resolved) = value_from_config_toml.clone().try_into::<ConfigToml>() else {
        return Ok(value_from_config_toml);
    };
    drop(_guard);

    let resolved_value = TomlValue::try_from(resolved).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Failed to serialize resolved config: {e}"),
        )
    })?;

    Ok(copy_shape_from_original(
        &value_from_config_toml,
        &resolved_value,
    ))
}

/// Ensure that every field in `original` is present in the returned
/// `toml::Value`, taking the value from `resolved` where possible. This ensures
/// the fields that we "removed" during the serialize/deserialize round-trip in
/// `resolve_config_paths` are preserved, out of an abundance of caution.
fn copy_shape_from_original(original: &TomlValue, resolved: &TomlValue) -> TomlValue {
    match (original, resolved) {
        (TomlValue::Table(original_table), TomlValue::Table(resolved_table)) => {
            let mut table = toml::map::Map::new();
            for (key, original_value) in original_table {
                let resolved_value = resolved_table.get(key).unwrap_or(original_value);
                table.insert(
                    key.clone(),
                    copy_shape_from_original(original_value, resolved_value),
                );
            }
            TomlValue::Table(table)
        }
        (TomlValue::Array(original_array), TomlValue::Array(resolved_array)) => {
            let mut items = Vec::new();
            for (index, original_value) in original_array.iter().enumerate() {
                let resolved_value = resolved_array.get(index).unwrap_or(original_value);
                items.push(copy_shape_from_original(original_value, resolved_value));
            }
            TomlValue::Array(items)
        }
        (_, resolved_value) => resolved_value.clone(),
    }
}

async fn find_project_root(
    cwd: &AbsolutePathBuf,
    project_root_markers: &[String],
) -> io::Result<AbsolutePathBuf> {
    if project_root_markers.is_empty() {
        return Ok(cwd.clone());
    }

    for ancestor in cwd.as_path().ancestors() {
        for marker in project_root_markers {
            let marker_path = ancestor.join(marker);
            if tokio::fs::metadata(&marker_path).await.is_ok() {
                return AbsolutePathBuf::from_absolute_path(ancestor);
            }
        }
    }
    Ok(cwd.clone())
}

/// Return the appropriate list of layers (each with
/// [ConfigLayerSource::Project] as the source) between `cwd` and
/// `project_root`, inclusive. The list is ordered in _increasing_ precdence,
/// starting from folders closest to `project_root` (which is the lowest
/// precedence) to those closest to `cwd` (which is the highest precedence).
async fn load_project_layers(
    cwd: &AbsolutePathBuf,
    project_root: &AbsolutePathBuf,
    trust_context: &ProjectTrustContext,
    codex_home: &Path,
) -> io::Result<Vec<ConfigLayerEntry>> {
    let codex_home_abs = AbsolutePathBuf::from_absolute_path(codex_home)?;
    let codex_home_normalized =
        normalize_path(codex_home_abs.as_path()).unwrap_or_else(|_| codex_home_abs.to_path_buf());
    let mut dirs = cwd
        .as_path()
        .ancestors()
        .scan(false, |done, a| {
            if *done {
                None
            } else {
                if a == project_root.as_path() {
                    *done = true;
                }
                Some(a)
            }
        })
        .collect::<Vec<_>>();
    dirs.reverse();

    let mut layers = Vec::new();
    for dir in dirs {
        let dot_codex = dir.join(".codex");
        if !tokio::fs::metadata(&dot_codex)
            .await
            .map(|meta| meta.is_dir())
            .unwrap_or(false)
        {
            continue;
        }

        let layer_dir = AbsolutePathBuf::from_absolute_path(dir)?;
        let decision = trust_context.decision_for_dir(&layer_dir);
        let dot_codex_abs = AbsolutePathBuf::from_absolute_path(&dot_codex)?;
        let dot_codex_normalized =
            normalize_path(dot_codex_abs.as_path()).unwrap_or_else(|_| dot_codex_abs.to_path_buf());
        if dot_codex_abs == codex_home_abs || dot_codex_normalized == codex_home_normalized {
            continue;
        }
        let config_file = dot_codex_abs.join(CONFIG_TOML_FILE)?;
        match tokio::fs::read_to_string(&config_file).await {
            Ok(contents) => {
                let config: TomlValue = match toml::from_str(&contents) {
                    Ok(config) => config,
                    Err(e) => {
                        if decision.is_trusted() {
                            let config_file_display = config_file.as_path().display();
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                format!(
                                    "Error parsing project config file {config_file_display}: {e}"
                                ),
                            ));
                        }
                        layers.push(project_layer_entry(
                            trust_context,
                            &dot_codex_abs,
                            &layer_dir,
                            TomlValue::Table(toml::map::Map::new()),
                            true,
                        ));
                        continue;
                    }
                };
                let config =
                    resolve_relative_paths_in_config_toml(config, dot_codex_abs.as_path())?;
                let entry =
                    project_layer_entry(trust_context, &dot_codex_abs, &layer_dir, config, true);
                layers.push(entry);
            }
            Err(err) => {
                if err.kind() == io::ErrorKind::NotFound {
                    // If there is no config.toml file, record an empty entry
                    // for this project layer, as this may still have subfolders
                    // that are significant in the overall ConfigLayerStack.
                    layers.push(project_layer_entry(
                        trust_context,
                        &dot_codex_abs,
                        &layer_dir,
                        TomlValue::Table(toml::map::Map::new()),
                        false,
                    ));
                } else {
                    let config_file_display = config_file.as_path().display();
                    return Err(io::Error::new(
                        err.kind(),
                        format!("Failed to read project config file {config_file_display}: {err}"),
                    ));
                }
            }
        }
    }

    Ok(layers)
}

/// The legacy mechanism for specifying admin-enforced configuration is to read
/// from a file like `/etc/codex/managed_config.toml` that has the same
/// structure as `config.toml` where fields like `approval_policy` can specify
/// exactly one value rather than a list of allowed values.
///
/// If present, re-interpret `managed_config.toml` as a `requirements.toml`
/// where each specified field is treated as a constraint allowing only that
/// value.
#[derive(Deserialize, Debug, Clone, Default, PartialEq)]
struct LegacyManagedConfigToml {
    approval_policy: Option<AskForApproval>,
    sandbox_mode: Option<SandboxMode>,
}

impl From<LegacyManagedConfigToml> for ConfigRequirementsToml {
    fn from(legacy: LegacyManagedConfigToml) -> Self {
        let mut config_requirements_toml = ConfigRequirementsToml::default();

        let LegacyManagedConfigToml {
            approval_policy,
            sandbox_mode,
        } = legacy;
        if let Some(approval_policy) = approval_policy {
            config_requirements_toml.allowed_approval_policies = Some(vec![approval_policy]);
        }
        if let Some(sandbox_mode) = sandbox_mode {
            let required_mode: SandboxModeRequirement = sandbox_mode.into();
            // Allowing read-only is a requirement for Codex to function correctly.
            // So in this backfill path, we append read-only if it's not already specified.
            let mut allowed_modes = vec![SandboxModeRequirement::ReadOnly];
            if required_mode != SandboxModeRequirement::ReadOnly {
                allowed_modes.push(required_mode);
            }
            config_requirements_toml.allowed_sandbox_modes = Some(allowed_modes);
        }
        config_requirements_toml
    }
}

// Cannot name this `mod tests` because of tests.rs in this folder.
#[cfg(test)]
mod unit_tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn ensure_resolve_relative_paths_in_config_toml_preserves_all_fields() -> anyhow::Result<()> {
        let tmp = tempdir()?;
        let base_dir = tmp.path();
        let contents = r#"
# This is a field recognized by config.toml that is an AbsolutePathBuf in
# the ConfigToml struct.
model_instructions_file = "./some_file.md"

# This is a field recognized by config.toml.
model = "gpt-1000"

# This is a field not recognized by config.toml.
foo = "xyzzy"
"#;
        let user_config: TomlValue = toml::from_str(contents)?;

        let normalized_toml_value = resolve_relative_paths_in_config_toml(user_config, base_dir)?;
        let mut expected_toml_value = toml::map::Map::new();
        expected_toml_value.insert(
            "model_instructions_file".to_string(),
            TomlValue::String(
                AbsolutePathBuf::resolve_path_against_base("./some_file.md", base_dir)?
                    .as_path()
                    .to_string_lossy()
                    .to_string(),
            ),
        );
        expected_toml_value.insert(
            "model".to_string(),
            TomlValue::String("gpt-1000".to_string()),
        );
        expected_toml_value.insert("foo".to_string(), TomlValue::String("xyzzy".to_string()));
        assert_eq!(normalized_toml_value, TomlValue::Table(expected_toml_value));
        Ok(())
    }

    #[test]
    fn legacy_managed_config_backfill_includes_read_only_sandbox_mode() {
        let legacy = LegacyManagedConfigToml {
            approval_policy: None,
            sandbox_mode: Some(SandboxMode::WorkspaceWrite),
        };

        let requirements = ConfigRequirementsToml::from(legacy);

        assert_eq!(
            requirements.allowed_sandbox_modes,
            Some(vec![
                SandboxModeRequirement::ReadOnly,
                SandboxModeRequirement::WorkspaceWrite
            ])
        );
    }
}
