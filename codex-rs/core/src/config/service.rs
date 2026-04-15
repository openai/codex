use super::ConfigToml;
use super::deserialize_config_toml_with_base;
use crate::config::edit::ConfigEdit;
use crate::config::edit::ConfigEditsBuilder;
use crate::config::managed_features::validate_explicit_feature_settings_in_config_toml;
use crate::config::managed_features::validate_feature_requirements_in_config_toml;
use crate::config_loader::CloudRequirementsLoader;
use crate::config_loader::ConfigLayerEntry;
use crate::config_loader::ConfigLayerStack;
use crate::config_loader::ConfigLayerStackOrdering;
use crate::config_loader::ConfigRequirementsToml;
use crate::config_loader::LoaderOverrides;
use crate::config_loader::load_config_layers_state;
use crate::config_loader::merge_toml_values;
use crate::path_utils;
use crate::path_utils::SymlinkWritePaths;
use crate::path_utils::resolve_symlink_write_paths;
use crate::path_utils::write_atomically;
use codex_app_server_protocol::Config as ApiConfig;
use codex_app_server_protocol::ConfigBatchWriteParams;
use codex_app_server_protocol::ConfigLayerMetadata;
use codex_app_server_protocol::ConfigLayerSource;
use codex_app_server_protocol::ConfigReadParams;
use codex_app_server_protocol::ConfigReadResponse;
use codex_app_server_protocol::ConfigValueWriteParams;
use codex_app_server_protocol::ConfigWriteErrorCode;
use codex_app_server_protocol::ConfigWriteResponse;
use codex_app_server_protocol::MergeStrategy;
use codex_app_server_protocol::OverriddenMetadata;
use codex_app_server_protocol::WriteStatus;
use codex_config::CONFIG_TOML_FILE;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde_json::Value as JsonValue;
use std::borrow::Cow;
use std::path::Path;
use std::path::PathBuf;
use thiserror::Error;
use tokio::task;
use toml::Value as TomlValue;
use toml_edit::Item as TomlItem;

#[derive(Debug, Error)]
pub enum ConfigServiceError {
    #[error("{message}")]
    Write {
        code: ConfigWriteErrorCode,
        message: String,
    },

    #[error("{context}: {source}")]
    Io {
        context: &'static str,
        #[source]
        source: std::io::Error,
    },

    #[error("{context}: {source}")]
    Json {
        context: &'static str,
        #[source]
        source: serde_json::Error,
    },

    #[error("{context}: {source}")]
    Toml {
        context: &'static str,
        #[source]
        source: toml::de::Error,
    },

    #[error("{context}: {source}")]
    Anyhow {
        context: &'static str,
        #[source]
        source: anyhow::Error,
    },
}

impl ConfigServiceError {
    fn write(code: ConfigWriteErrorCode, message: impl Into<String>) -> Self {
        Self::Write {
            code,
            message: message.into(),
        }
    }

    fn io(context: &'static str, source: std::io::Error) -> Self {
        Self::Io { context, source }
    }

    fn json(context: &'static str, source: serde_json::Error) -> Self {
        Self::Json { context, source }
    }

    fn toml(context: &'static str, source: toml::de::Error) -> Self {
        Self::Toml { context, source }
    }

    fn anyhow(context: &'static str, source: anyhow::Error) -> Self {
        Self::Anyhow { context, source }
    }

    pub fn write_error_code(&self) -> Option<ConfigWriteErrorCode> {
        match self {
            Self::Write { code, .. } => Some(code.clone()),
            _ => None,
        }
    }
}

#[derive(Clone)]
pub struct ConfigService {
    codex_home: PathBuf,
    cli_overrides: Vec<(String, TomlValue)>,
    loader_overrides: LoaderOverrides,
    cloud_requirements: CloudRequirementsLoader,
}

impl ConfigService {
    pub fn new(
        codex_home: PathBuf,
        cli_overrides: Vec<(String, TomlValue)>,
        loader_overrides: LoaderOverrides,
        cloud_requirements: CloudRequirementsLoader,
    ) -> Self {
        Self {
            codex_home,
            cli_overrides,
            loader_overrides,
            cloud_requirements,
        }
    }

    pub fn new_with_defaults(codex_home: PathBuf) -> Self {
        Self {
            codex_home,
            cli_overrides: Vec::new(),
            loader_overrides: LoaderOverrides::default(),
            cloud_requirements: CloudRequirementsLoader::default(),
        }
    }

    pub async fn read(
        &self,
        params: ConfigReadParams,
    ) -> Result<ConfigReadResponse, ConfigServiceError> {
        let layers = match params.cwd.as_deref() {
            Some(cwd) => {
                let cwd = AbsolutePathBuf::try_from(PathBuf::from(cwd)).map_err(|err| {
                    ConfigServiceError::io("failed to resolve config cwd to an absolute path", err)
                })?;
                crate::config::ConfigBuilder::default()
                    .codex_home(self.codex_home.clone())
                    .cli_overrides(self.cli_overrides.clone())
                    .loader_overrides(self.loader_overrides.clone())
                    .fallback_cwd(Some(cwd.to_path_buf()))
                    .cloud_requirements(self.cloud_requirements.clone())
                    .build()
                    .await
                    .map_err(|err| {
                        ConfigServiceError::io("failed to read configuration layers", err)
                    })?
                    .config_layer_stack
            }
            None => self.load_thread_agnostic_config().await.map_err(|err| {
                ConfigServiceError::io("failed to read configuration layers", err)
            })?,
        };

        let effective = layers.effective_config();

        let effective_config_toml: ConfigToml = effective
            .try_into()
            .map_err(|err| ConfigServiceError::toml("invalid configuration", err))?;

        let json_value = serde_json::to_value(&effective_config_toml)
            .map_err(|err| ConfigServiceError::json("failed to serialize configuration", err))?;
        let config: ApiConfig = serde_json::from_value(json_value)
            .map_err(|err| ConfigServiceError::json("failed to deserialize configuration", err))?;

        Ok(ConfigReadResponse {
            config,
            origins: layers.origins(),
            layers: params.include_layers.then(|| {
                layers
                    .get_layers(
                        ConfigLayerStackOrdering::HighestPrecedenceFirst,
                        /*include_disabled*/ true,
                    )
                    .iter()
                    .map(|layer| layer.as_layer())
                    .collect()
            }),
        })
    }

    pub async fn read_requirements(
        &self,
    ) -> Result<Option<ConfigRequirementsToml>, ConfigServiceError> {
        let layers = self
            .load_thread_agnostic_config()
            .await
            .map_err(|err| ConfigServiceError::io("failed to read configuration layers", err))?;

        let requirements = layers.requirements_toml().clone();
        if requirements.is_empty() {
            Ok(None)
        } else {
            Ok(Some(requirements))
        }
    }

    pub async fn write_value(
        &self,
        params: ConfigValueWriteParams,
    ) -> Result<ConfigWriteResponse, ConfigServiceError> {
        let edits = vec![(params.key_path, params.value, params.merge_strategy)];
        self.apply_edits(params.file_path, params.expected_version, edits)
            .await
    }

    pub async fn batch_write(
        &self,
        params: ConfigBatchWriteParams,
    ) -> Result<ConfigWriteResponse, ConfigServiceError> {
        let edits = params
            .edits
            .into_iter()
            .map(|edit| (edit.key_path, edit.value, edit.merge_strategy))
            .collect();

        self.apply_edits(params.file_path, params.expected_version, edits)
            .await
    }

    pub async fn load_user_saved_config(
        &self,
    ) -> Result<codex_app_server_protocol::UserSavedConfig, ConfigServiceError> {
        let layers = self
            .load_thread_agnostic_config()
            .await
            .map_err(|err| ConfigServiceError::io("failed to load configuration", err))?;

        let toml_value = layers.effective_config();
        let cfg: ConfigToml = toml_value
            .try_into()
            .map_err(|err| ConfigServiceError::toml("failed to parse config.toml", err))?;
        Ok(cfg.into())
    }

    async fn apply_edits(
        &self,
        file_path: Option<String>,
        expected_version: Option<String>,
        edits: Vec<(String, JsonValue, MergeStrategy)>,
    ) -> Result<ConfigWriteResponse, ConfigServiceError> {
        let allowed_path =
            AbsolutePathBuf::resolve_path_against_base(CONFIG_TOML_FILE, &self.codex_home)
                .map_err(|err| ConfigServiceError::io("failed to resolve user config path", err))?;
        let provided_path = match file_path {
            Some(path) => AbsolutePathBuf::from_absolute_path(PathBuf::from(path))
                .map_err(|err| ConfigServiceError::io("failed to resolve user config path", err))?,
            None => allowed_path.clone(),
        };

        if !paths_match(&allowed_path, &provided_path) {
            return Err(ConfigServiceError::write(
                ConfigWriteErrorCode::ConfigLayerReadonly,
                "Only writes to the user config are allowed",
            ));
        }

        let layers = self
            .load_thread_agnostic_config()
            .await
            .map_err(|err| ConfigServiceError::io("failed to load configuration", err))?;
        let user_layer = match layers.get_user_layer() {
            Some(layer) => Cow::Borrowed(layer),
            None => Cow::Owned(create_empty_user_layer(&allowed_path).await?),
        };

        if let Some(expected) = expected_version.as_deref()
            && expected != user_layer.version
        {
            return Err(ConfigServiceError::write(
                ConfigWriteErrorCode::ConfigVersionConflict,
                "Configuration was modified since last read. Fetch latest version and retry.",
            ));
        }

        let mut user_config = user_layer.config.clone();
        let mut parsed_segments = Vec::new();
        let mut config_edits = Vec::new();

        for (key_path, value, strategy) in edits.into_iter() {
            let segments = parse_key_path(&key_path).map_err(|message| {
                ConfigServiceError::write(ConfigWriteErrorCode::ConfigValidationError, message)
            })?;
            let original_value = value_at_path(&user_config, &segments).cloned();
            let parsed_value = parse_value(value).map_err(|message| {
                ConfigServiceError::write(ConfigWriteErrorCode::ConfigValidationError, message)
            })?;

            apply_merge(&mut user_config, &segments, parsed_value.as_ref(), strategy).map_err(
                |err| match err {
                    MergeError::PathNotFound => ConfigServiceError::write(
                        ConfigWriteErrorCode::ConfigPathNotFound,
                        "Path not found",
                    ),
                    MergeError::Validation(message) => ConfigServiceError::write(
                        ConfigWriteErrorCode::ConfigValidationError,
                        message,
                    ),
                },
            )?;

            let updated_value = value_at_path(&user_config, &segments).cloned();
            if original_value != updated_value {
                let edit = match updated_value {
                    Some(value) => ConfigEdit::SetPath {
                        segments: segments.clone(),
                        value: toml_value_to_item(&value).map_err(|err| {
                            ConfigServiceError::anyhow("failed to build config edits", err)
                        })?,
                    },
                    None => ConfigEdit::ClearPath {
                        segments: segments.clone(),
                    },
                };
                config_edits.push(edit);
            }

            parsed_segments.push(segments);
        }

        validate_config(&user_config).map_err(|err| {
            ConfigServiceError::write(
                ConfigWriteErrorCode::ConfigValidationError,
                format!("Invalid configuration: {err}"),
            )
        })?;
        let user_config_toml =
            deserialize_config_toml_with_base(user_config.clone(), &self.codex_home).map_err(
                |err| {
                    ConfigServiceError::write(
                        ConfigWriteErrorCode::ConfigValidationError,
                        format!("Invalid configuration: {err}"),
                    )
                },
            )?;
        validate_explicit_feature_settings_in_config_toml(
            &user_config_toml,
            layers.requirements().feature_requirements.as_ref(),
        )
        .map_err(|err| {
            ConfigServiceError::write(
                ConfigWriteErrorCode::ConfigValidationError,
                format!("Invalid configuration: {err}"),
            )
        })?;
        validate_feature_requirements_in_config_toml(
            &user_config_toml,
            layers.requirements().feature_requirements.as_ref(),
        )
        .map_err(|err| {
            ConfigServiceError::write(
                ConfigWriteErrorCode::ConfigValidationError,
                format!("Invalid configuration: {err}"),
            )
        })?;

        let updated_layers = layers.with_user_config(&provided_path, user_config.clone());
        let effective = updated_layers.effective_config();
        validate_config(&effective).map_err(|err| {
            ConfigServiceError::write(
                ConfigWriteErrorCode::ConfigValidationError,
                format!("Invalid configuration: {err}"),
            )
        })?;

        if !config_edits.is_empty() {
            ConfigEditsBuilder::new(&self.codex_home)
                .with_edits(config_edits)
                .apply()
                .await
                .map_err(|err| ConfigServiceError::anyhow("failed to persist config.toml", err))?;
        }

        let overridden = first_overridden_edit(&updated_layers, &effective, &parsed_segments);
        let status = overridden
            .as_ref()
            .map(|_| WriteStatus::OkOverridden)
            .unwrap_or(WriteStatus::Ok);

        Ok(ConfigWriteResponse {
            status,
            version: updated_layers
                .get_user_layer()
                .ok_or_else(|| {
                    ConfigServiceError::write(
                        ConfigWriteErrorCode::UserLayerNotFound,
                        "user layer not found in updated layers",
                    )
                })?
                .version
                .clone(),
            file_path: provided_path,
            overridden_metadata: overridden,
        })
    }

    /// Loads a "thread-agnostic" config, which means the config layers do not
    /// include any in-repo .codex/ folders because there is no cwd/project root
    /// associated with this query.
    async fn load_thread_agnostic_config(&self) -> std::io::Result<ConfigLayerStack> {
        let cwd: Option<AbsolutePathBuf> = None;
        load_config_layers_state(
            &self.codex_home,
            cwd,
            &self.cli_overrides,
            self.loader_overrides.clone(),
            self.cloud_requirements.clone(),
        )
        .await
    }
}

async fn create_empty_user_layer(
    config_toml: &AbsolutePathBuf,
) -> Result<ConfigLayerEntry, ConfigServiceError> {
    let SymlinkWritePaths {
        read_path,
        write_path,
    } = resolve_symlink_write_paths(config_toml.as_path())
        .map_err(|err| ConfigServiceError::io("failed to resolve user config path", err))?;
    let toml_value = match read_path {
        Some(path) => match crate::async_fs::read_to_string(&path).await {
            Ok(contents) => toml::from_str(&contents).map_err(|e| {
                ConfigServiceError::toml("failed to parse existing user config.toml", e)
            })?,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                write_empty_user_config(write_path.clone()).await?;
                TomlValue::Table(toml::map::Map::new())
            }
            Err(err) => {
                return Err(ConfigServiceError::io(
                    "failed to read user config.toml",
                    err,
                ));
            }
        },
        None => {
            write_empty_user_config(write_path).await?;
            TomlValue::Table(toml::map::Map::new())
        }
    };
    Ok(ConfigLayerEntry::new(
        ConfigLayerSource::User {
            file: config_toml.clone(),
        },
        toml_value,
    ))
}

async fn write_empty_user_config(write_path: PathBuf) -> Result<(), ConfigServiceError> {
    task::spawn_blocking(move || write_atomically(&write_path, ""))
        .await
        .map_err(|err| ConfigServiceError::anyhow("config persistence task panicked", err.into()))?
        .map_err(|err| ConfigServiceError::io("failed to create empty user config.toml", err))
}

fn parse_value(value: JsonValue) -> Result<Option<TomlValue>, String> {
    if value.is_null() {
        return Ok(None);
    }

    serde_json::from_value::<TomlValue>(value)
        .map(Some)
        .map_err(|err| format!("invalid value: {err}"))
}

fn parse_key_path(path: &str) -> Result<Vec<String>, String> {
    if path.trim().is_empty() {
        return Err("keyPath must not be empty".to_string());
    }
    Ok(path
        .split('.')
        .map(std::string::ToString::to_string)
        .collect())
}

#[derive(Debug)]
enum MergeError {
    PathNotFound,
    Validation(String),
}

fn apply_merge(
    root: &mut TomlValue,
    segments: &[String],
    value: Option<&TomlValue>,
    strategy: MergeStrategy,
) -> Result<bool, MergeError> {
    let Some(value) = value else {
        return clear_path(root, segments);
    };

    let Some((last, parents)) = segments.split_last() else {
        return Err(MergeError::Validation(
            "keyPath must not be empty".to_string(),
        ));
    };

    let mut current = root;

    for segment in parents {
        match current {
            TomlValue::Table(table) => {
                current = table
                    .entry(segment.clone())
                    .or_insert_with(|| TomlValue::Table(toml::map::Map::new()));
            }
            _ => {
                *current = TomlValue::Table(toml::map::Map::new());
                if let TomlValue::Table(table) = current {
                    current = table
                        .entry(segment.clone())
                        .or_insert_with(|| TomlValue::Table(toml::map::Map::new()));
                }
            }
        }
    }

    let table = current.as_table_mut().ok_or_else(|| {
        MergeError::Validation("cannot set value on non-table parent".to_string())
    })?;

    if matches!(strategy, MergeStrategy::Upsert)
        && let Some(existing) = table.get_mut(last)
        && matches!(existing, TomlValue::Table(_))
        && matches!(value, TomlValue::Table(_))
    {
        merge_toml_values(existing, value);
        return Ok(true);
    }

    let changed = table
        .get(last)
        .map(|existing| Some(existing) != Some(value))
        .unwrap_or(true);
    table.insert(last.clone(), value.clone());
    Ok(changed)
}

fn clear_path(root: &mut TomlValue, segments: &[String]) -> Result<bool, MergeError> {
    let Some((last, parents)) = segments.split_last() else {
        return Err(MergeError::Validation(
            "keyPath must not be empty".to_string(),
        ));
    };

    let mut current = root;
    for segment in parents {
        match current {
            TomlValue::Table(table) => {
                current = table.get_mut(segment).ok_or(MergeError::PathNotFound)?;
            }
            _ => return Err(MergeError::PathNotFound),
        }
    }

    let Some(parent) = current.as_table_mut() else {
        return Err(MergeError::PathNotFound);
    };

    Ok(parent.remove(last).is_some())
}

fn toml_value_to_item(value: &TomlValue) -> anyhow::Result<TomlItem> {
    match value {
        TomlValue::Table(table) => {
            let mut table_item = toml_edit::Table::new();
            table_item.set_implicit(false);
            for (key, val) in table {
                table_item.insert(key, toml_value_to_item(val)?);
            }
            Ok(TomlItem::Table(table_item))
        }
        other => Ok(TomlItem::Value(toml_value_to_value(other)?)),
    }
}

fn toml_value_to_value(value: &TomlValue) -> anyhow::Result<toml_edit::Value> {
    match value {
        TomlValue::String(val) => Ok(toml_edit::Value::from(val.clone())),
        TomlValue::Integer(val) => Ok(toml_edit::Value::from(*val)),
        TomlValue::Float(val) => Ok(toml_edit::Value::from(*val)),
        TomlValue::Boolean(val) => Ok(toml_edit::Value::from(*val)),
        TomlValue::Datetime(val) => Ok(toml_edit::Value::from(*val)),
        TomlValue::Array(items) => {
            let mut array = toml_edit::Array::new();
            for item in items {
                array.push(toml_value_to_value(item)?);
            }
            Ok(toml_edit::Value::Array(array))
        }
        TomlValue::Table(table) => {
            let mut inline = toml_edit::InlineTable::new();
            for (key, val) in table {
                inline.insert(key, toml_value_to_value(val)?);
            }
            Ok(toml_edit::Value::InlineTable(inline))
        }
    }
}

fn validate_config(value: &TomlValue) -> Result<(), toml::de::Error> {
    let _: ConfigToml = value.clone().try_into()?;
    Ok(())
}

fn paths_match(expected: impl AsRef<Path>, provided: impl AsRef<Path>) -> bool {
    if let (Ok(expanded_expected), Ok(expanded_provided)) = (
        path_utils::normalize_for_path_comparison(&expected),
        path_utils::normalize_for_path_comparison(&provided),
    ) {
        expanded_expected == expanded_provided
    } else {
        expected.as_ref() == provided.as_ref()
    }
}

fn value_at_path<'a>(root: &'a TomlValue, segments: &[String]) -> Option<&'a TomlValue> {
    let mut current = root;
    for segment in segments {
        match current {
            TomlValue::Table(table) => {
                current = table.get(segment)?;
            }
            TomlValue::Array(items) => {
                let idx = segment.parse::<i64>().ok()?;
                let idx = usize::try_from(idx).ok()?;
                current = items.get(idx)?;
            }
            _ => return None,
        }
    }
    Some(current)
}

fn override_message(layer: &ConfigLayerSource) -> String {
    match layer {
        ConfigLayerSource::Mdm { domain, key: _ } => {
            format!("Overridden by managed policy (MDM): {domain}")
        }
        ConfigLayerSource::System { file } => {
            format!("Overridden by managed config (system): {}", file.display())
        }
        ConfigLayerSource::Project { dot_codex_folder } => format!(
            "Overridden by project config: {}/{CONFIG_TOML_FILE}",
            dot_codex_folder.display(),
        ),
        ConfigLayerSource::SessionFlags => "Overridden by session flags".to_string(),
        ConfigLayerSource::User { file } => {
            format!("Overridden by user config: {}", file.display())
        }
        ConfigLayerSource::LegacyManagedConfigTomlFromFile { file } => {
            format!(
                "Overridden by legacy managed_config.toml: {}",
                file.display()
            )
        }
        ConfigLayerSource::LegacyManagedConfigTomlFromMdm => {
            "Overridden by legacy managed configuration from MDM".to_string()
        }
    }
}

fn compute_override_metadata(
    layers: &ConfigLayerStack,
    effective: &TomlValue,
    segments: &[String],
) -> Option<OverriddenMetadata> {
    let user_value = match layers.get_user_layer() {
        Some(user_layer) => value_at_path(&user_layer.config, segments),
        None => return None,
    };
    let effective_value = value_at_path(effective, segments);

    if user_value.is_some() && user_value == effective_value {
        return None;
    }

    if user_value.is_none() && effective_value.is_none() {
        return None;
    }

    let overriding_layer = find_effective_layer(layers, segments)?;
    let message = override_message(&overriding_layer.name);

    Some(OverriddenMetadata {
        message,
        overriding_layer,
        effective_value: effective_value
            .and_then(|value| serde_json::to_value(value).ok())
            .unwrap_or(JsonValue::Null),
    })
}

fn first_overridden_edit(
    layers: &ConfigLayerStack,
    effective: &TomlValue,
    edits: &[Vec<String>],
) -> Option<OverriddenMetadata> {
    for segments in edits {
        if let Some(meta) = compute_override_metadata(layers, effective, segments) {
            return Some(meta);
        }
    }
    None
}

fn find_effective_layer(
    layers: &ConfigLayerStack,
    segments: &[String],
) -> Option<ConfigLayerMetadata> {
    for layer in layers.layers_high_to_low() {
        if let Some(meta) = value_at_path(&layer.config, segments).map(|_| layer.metadata()) {
            return Some(meta);
        }
    }

    None
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
