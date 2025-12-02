use crate::error_code::INTERNAL_ERROR_CODE;
use crate::error_code::INVALID_REQUEST_ERROR_CODE;
use anyhow::anyhow;
use codex_app_server_protocol::ConfigBatchWriteParams;
use codex_app_server_protocol::ConfigLayer;
use codex_app_server_protocol::ConfigLayerMetadata;
use codex_app_server_protocol::ConfigLayerName;
use codex_app_server_protocol::ConfigReadParams;
use codex_app_server_protocol::ConfigReadResponse;
use codex_app_server_protocol::ConfigValueWriteParams;
use codex_app_server_protocol::ConfigWriteErrorCode;
use codex_app_server_protocol::ConfigWriteResponse;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::MergeStrategy;
use codex_app_server_protocol::OverriddenMetadata;
use codex_app_server_protocol::WriteStatus;
use codex_core::config::ConfigToml;
use codex_core::config_loader::LoadedConfigLayers;
use codex_core::config_loader::LoaderOverrides;
use codex_core::config_loader::load_config_layers_with_overrides;
use codex_core::config_loader::merge_toml_values;
use serde_json::Value as JsonValue;
use serde_json::json;
use sha2::Digest;
use sha2::Sha256;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use tempfile::NamedTempFile;
use tokio::task;
use toml::Value as TomlValue;

const SESSION_FLAGS_SOURCE: &str = "--config";
const MDM_SOURCE: &str = "com.openai.codex/config_toml_base64";
const CONFIG_FILE_NAME: &str = "config.toml";

#[derive(Clone)]
pub(crate) struct ConfigApi {
    codex_home: PathBuf,
    cli_overrides: Vec<(String, TomlValue)>,
    loader_overrides: LoaderOverrides,
}

impl ConfigApi {
    pub(crate) fn new(codex_home: PathBuf, cli_overrides: Vec<(String, TomlValue)>) -> Self {
        Self {
            codex_home,
            cli_overrides,
            loader_overrides: LoaderOverrides::default(),
        }
    }

    #[cfg(test)]
    fn with_overrides(
        codex_home: PathBuf,
        cli_overrides: Vec<(String, TomlValue)>,
        loader_overrides: LoaderOverrides,
    ) -> Self {
        Self {
            codex_home,
            cli_overrides,
            loader_overrides,
        }
    }

    pub(crate) async fn read(
        &self,
        params: ConfigReadParams,
    ) -> Result<ConfigReadResponse, JSONRPCErrorError> {
        let layers = self
            .load_layers_state()
            .await
            .map_err(|err| internal_error("failed to read configuration layers", err))?;

        let effective = layers.effective_config();
        validate_config(&effective).map_err(|err| internal_error("invalid configuration", err))?;

        let response = ConfigReadResponse {
            config: to_json_value(&effective),
            origins: layers.origins(),
            layers: params.include_layers.then(|| layers.layers_high_to_low()),
        };

        Ok(response)
    }

    pub(crate) async fn write_value(
        &self,
        params: ConfigValueWriteParams,
    ) -> Result<ConfigWriteResponse, JSONRPCErrorError> {
        let edits = vec![(params.key_path, params.value, params.merge_strategy)];
        self.apply_edits(params.file_path, params.expected_version, edits)
            .await
    }

    pub(crate) async fn batch_write(
        &self,
        params: ConfigBatchWriteParams,
    ) -> Result<ConfigWriteResponse, JSONRPCErrorError> {
        let edits = params
            .edits
            .into_iter()
            .map(|edit| (edit.key_path, edit.value, edit.merge_strategy))
            .collect();

        self.apply_edits(params.file_path, params.expected_version, edits)
            .await
    }

    async fn apply_edits(
        &self,
        file_path: String,
        expected_version: Option<String>,
        edits: Vec<(String, JsonValue, MergeStrategy)>,
    ) -> Result<ConfigWriteResponse, JSONRPCErrorError> {
        let allowed_path = self.codex_home.join(CONFIG_FILE_NAME);
        if !paths_match(&allowed_path, &file_path) {
            return Err(config_write_error(
                ConfigWriteErrorCode::ConfigLayerReadonly,
                "Only writes to the user config are allowed",
            ));
        }

        let layers = self
            .load_layers_state()
            .await
            .map_err(|err| internal_error("failed to load configuration", err))?;

        if let Some(expected) = expected_version.as_deref()
            && expected != layers.user.version
        {
            return Err(config_write_error(
                ConfigWriteErrorCode::ConfigVersionConflict,
                "Configuration was modified since last read. Fetch latest version and retry.",
            ));
        }

        let mut user_config = layers.user.config.clone();
        let mut mutated = false;
        let mut parsed_segments = Vec::new();

        for (key_path, value, strategy) in edits.into_iter() {
            let segments = parse_key_path(&key_path).map_err(|message| {
                config_write_error(ConfigWriteErrorCode::ConfigValidationError, message)
            })?;
            let parsed_value = parse_value(value).map_err(|message| {
                config_write_error(ConfigWriteErrorCode::ConfigValidationError, message)
            })?;

            let changed = apply_merge(&mut user_config, &segments, parsed_value.as_ref(), strategy)
                .map_err(|err| match err {
                    MergeError::PathNotFound => config_write_error(
                        ConfigWriteErrorCode::ConfigPathNotFound,
                        "Path not found",
                    ),
                    MergeError::Validation(message) => {
                        config_write_error(ConfigWriteErrorCode::ConfigValidationError, message)
                    }
                })?;

            mutated |= changed;
            parsed_segments.push(segments);
        }

        validate_config(&user_config).map_err(|err| {
            config_write_error(
                ConfigWriteErrorCode::ConfigValidationError,
                format!("Invalid configuration: {err}"),
            )
        })?;

        let updated_layers = layers.with_user_config(user_config.clone());
        let effective = updated_layers.effective_config();
        validate_config(&effective).map_err(|err| {
            config_write_error(
                ConfigWriteErrorCode::ConfigValidationError,
                format!("Invalid configuration: {err}"),
            )
        })?;

        if mutated {
            self.persist_user_config(&user_config)
                .await
                .map_err(|err| internal_error("failed to persist config.toml", err))?;
        }

        let overridden = first_overridden_edit(&updated_layers, &effective, &parsed_segments);
        let status = overridden
            .as_ref()
            .map(|_| WriteStatus::OkOverridden)
            .unwrap_or(WriteStatus::Ok);

        Ok(ConfigWriteResponse {
            status,
            version: updated_layers.user.version.clone(),
            overridden_metadata: overridden,
        })
    }

    async fn load_layers_state(&self) -> std::io::Result<LayersState> {
        let LoadedConfigLayers {
            base,
            managed_config,
            managed_preferences,
        } = load_config_layers_with_overrides(&self.codex_home, self.loader_overrides.clone())
            .await?;

        let user = LayerState::new(
            ConfigLayerName::User,
            self.codex_home.join(CONFIG_FILE_NAME),
            base,
        );

        let session_flags = LayerState::new(
            ConfigLayerName::SessionFlags,
            PathBuf::from(SESSION_FLAGS_SOURCE),
            {
                let mut root = TomlValue::Table(toml::map::Map::new());
                for (path, value) in self.cli_overrides.iter() {
                    apply_override(&mut root, path, value.clone());
                }
                root
            },
        );

        let system = managed_config.map(|cfg| {
            LayerState::new(
                ConfigLayerName::System,
                system_config_path(&self.codex_home),
                cfg,
            )
        });

        let mdm = managed_preferences
            .map(|cfg| LayerState::new(ConfigLayerName::Mdm, PathBuf::from(MDM_SOURCE), cfg));

        Ok(LayersState {
            user,
            session_flags,
            system,
            mdm,
        })
    }

    async fn persist_user_config(&self, user_config: &TomlValue) -> anyhow::Result<()> {
        let codex_home = self.codex_home.clone();
        let serialized = toml::to_string_pretty(user_config)?;

        task::spawn_blocking(move || -> anyhow::Result<()> {
            std::fs::create_dir_all(&codex_home)?;

            let target = codex_home.join(CONFIG_FILE_NAME);
            let tmp = NamedTempFile::new_in(&codex_home)?;
            std::fs::write(tmp.path(), serialized.as_bytes())?;
            tmp.persist(&target)?;
            Ok(())
        })
        .await
        .map_err(|err| anyhow!("config persistence task panicked: {err}"))??;

        Ok(())
    }
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

fn apply_override(target: &mut TomlValue, path: &str, value: TomlValue) {
    use toml::value::Table;

    let segments: Vec<&str> = path.split('.').collect();
    let mut current = target;

    for (idx, segment) in segments.iter().enumerate() {
        let is_last = idx == segments.len() - 1;

        if is_last {
            match current {
                TomlValue::Table(table) => {
                    table.insert(segment.to_string(), value);
                }
                _ => {
                    let mut table = Table::new();
                    table.insert(segment.to_string(), value);
                    *current = TomlValue::Table(table);
                }
            }
            return;
        }

        match current {
            TomlValue::Table(table) => {
                current = table
                    .entry((*segment).to_string())
                    .or_insert_with(|| TomlValue::Table(Table::new()));
            }
            _ => {
                *current = TomlValue::Table(Table::new());
                if let TomlValue::Table(tbl) = current {
                    current = tbl
                        .entry((*segment).to_string())
                        .or_insert_with(|| TomlValue::Table(Table::new()));
                }
            }
        }
    }
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

#[derive(Clone)]
struct LayerState {
    name: ConfigLayerName,
    source: PathBuf,
    config: TomlValue,
    version: String,
}

impl LayerState {
    fn new(name: ConfigLayerName, source: PathBuf, config: TomlValue) -> Self {
        let version = version_for_toml(&config);
        Self {
            name,
            source,
            config,
            version,
        }
    }

    fn metadata(&self) -> ConfigLayerMetadata {
        ConfigLayerMetadata {
            name: self.name.clone(),
            source: self.source.display().to_string(),
            version: self.version.clone(),
        }
    }

    fn as_layer(&self) -> ConfigLayer {
        ConfigLayer {
            name: self.name.clone(),
            source: self.source.display().to_string(),
            version: self.version.clone(),
            config: to_json_value(&self.config),
        }
    }
}

#[derive(Clone)]
struct LayersState {
    user: LayerState,
    session_flags: LayerState,
    system: Option<LayerState>,
    mdm: Option<LayerState>,
}

impl LayersState {
    fn with_user_config(self, user_config: TomlValue) -> Self {
        Self {
            user: LayerState::new(self.user.name, self.user.source, user_config),
            session_flags: self.session_flags,
            system: self.system,
            mdm: self.mdm,
        }
    }

    fn effective_config(&self) -> TomlValue {
        let mut merged = self.user.config.clone();
        merge_toml_values(&mut merged, &self.session_flags.config);
        if let Some(system) = &self.system {
            merge_toml_values(&mut merged, &system.config);
        }
        if let Some(mdm) = &self.mdm {
            merge_toml_values(&mut merged, &mdm.config);
        }
        merged
    }

    fn origins(&self) -> HashMap<String, ConfigLayerMetadata> {
        let mut origins = HashMap::new();
        let mut path = Vec::new();

        record_origins(
            &self.user.config,
            &self.user.metadata(),
            &mut path,
            &mut origins,
        );
        record_origins(
            &self.session_flags.config,
            &self.session_flags.metadata(),
            &mut path,
            &mut origins,
        );
        if let Some(system) = &self.system {
            record_origins(&system.config, &system.metadata(), &mut path, &mut origins);
        }
        if let Some(mdm) = &self.mdm {
            record_origins(&mdm.config, &mdm.metadata(), &mut path, &mut origins);
        }

        origins
    }

    fn layers_high_to_low(&self) -> Vec<ConfigLayer> {
        let mut layers = Vec::new();
        if let Some(mdm) = &self.mdm {
            layers.push(mdm.as_layer());
        }
        if let Some(system) = &self.system {
            layers.push(system.as_layer());
        }
        layers.push(self.session_flags.as_layer());
        layers.push(self.user.as_layer());
        layers
    }
}

fn record_origins(
    value: &TomlValue,
    meta: &ConfigLayerMetadata,
    path: &mut Vec<String>,
    origins: &mut HashMap<String, ConfigLayerMetadata>,
) {
    match value {
        TomlValue::Table(table) => {
            for (key, val) in table {
                path.push(key.clone());
                record_origins(val, meta, path, origins);
                path.pop();
            }
        }
        TomlValue::Array(items) => {
            for (idx, item) in items.iter().enumerate() {
                path.push(idx.to_string());
                record_origins(item, meta, path, origins);
                path.pop();
            }
        }
        _ => {
            if !path.is_empty() {
                origins.insert(path.join("."), meta.clone());
            }
        }
    }
}

fn to_json_value(value: &TomlValue) -> JsonValue {
    serde_json::to_value(value).unwrap_or(JsonValue::Null)
}

fn validate_config(value: &TomlValue) -> Result<(), toml::de::Error> {
    let _: ConfigToml = value.clone().try_into()?;
    Ok(())
}

fn version_for_toml(value: &TomlValue) -> String {
    let json = to_json_value(value);
    let canonical = canonical_json(&json);
    let serialized = serde_json::to_vec(&canonical).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(serialized);
    let hash = hasher.finalize();
    let hex = hash
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sha256:{hex}")
}

fn canonical_json(value: &JsonValue) -> JsonValue {
    match value {
        JsonValue::Object(map) => {
            let mut sorted = serde_json::Map::new();
            let mut keys = map.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            for key in keys {
                if let Some(val) = map.get(&key) {
                    sorted.insert(key, canonical_json(val));
                }
            }
            JsonValue::Object(sorted)
        }
        JsonValue::Array(items) => JsonValue::Array(items.iter().map(canonical_json).collect()),
        other => other.clone(),
    }
}

fn paths_match(expected: &Path, provided: &str) -> bool {
    let provided_path = PathBuf::from(provided);
    if let (Ok(expanded_expected), Ok(expanded_provided)) =
        (expected.canonicalize(), provided_path.canonicalize())
    {
        return expanded_expected == expanded_provided;
    }

    expected == provided_path
}

fn value_at_path<'a>(root: &'a TomlValue, segments: &[String]) -> Option<&'a TomlValue> {
    let mut current = root;
    for segment in segments {
        match current {
            TomlValue::Table(table) => {
                current = table.get(segment)?;
            }
            TomlValue::Array(items) => {
                let idx: usize = segment.parse().ok()?;
                current = items.get(idx)?;
            }
            _ => return None,
        }
    }
    Some(current)
}

fn override_message(layer: &ConfigLayerName) -> String {
    match layer {
        ConfigLayerName::Mdm => "Overridden by managed policy (mdm)".to_string(),
        ConfigLayerName::System => "Overridden by managed config (system)".to_string(),
        ConfigLayerName::SessionFlags => "Overridden by session flags".to_string(),
        ConfigLayerName::User => "Overridden by user config".to_string(),
    }
}

fn compute_override_metadata(
    layers: &LayersState,
    effective: &TomlValue,
    segments: &[String],
) -> Option<OverriddenMetadata> {
    let user_value = value_at_path(&layers.user.config, segments);
    let effective_value = value_at_path(effective, segments);

    if user_value.is_some() && user_value == effective_value {
        return None;
    }

    if user_value.is_none() && effective_value.is_none() {
        return None;
    }

    let effective_layer = find_effective_layer(layers, segments);
    let overriding_layer = effective_layer.unwrap_or_else(|| layers.user.metadata());
    let message = override_message(&overriding_layer.name);

    Some(OverriddenMetadata {
        message,
        overriding_layer,
        effective_value: effective_value
            .map(to_json_value)
            .unwrap_or(JsonValue::Null),
    })
}

fn first_overridden_edit(
    layers: &LayersState,
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

fn find_effective_layer(layers: &LayersState, segments: &[String]) -> Option<ConfigLayerMetadata> {
    let check =
        |state: &LayerState| value_at_path(&state.config, segments).map(|_| state.metadata());

    if let Some(mdm) = &layers.mdm
        && let Some(meta) = check(mdm)
    {
        return Some(meta);
    }
    if let Some(system) = &layers.system
        && let Some(meta) = check(system)
    {
        return Some(meta);
    }
    if let Some(meta) = check(&layers.session_flags) {
        return Some(meta);
    }
    check(&layers.user)
}

fn system_config_path(codex_home: &Path) -> PathBuf {
    if let Ok(path) = std::env::var("CODEX_MANAGED_CONFIG_PATH") {
        return PathBuf::from(path);
    }

    #[cfg(unix)]
    {
        let _ = codex_home;
        PathBuf::from("/etc/codex/managed_config.toml")
    }

    #[cfg(not(unix))]
    {
        codex_home.join("managed_config.toml")
    }
}

fn internal_error<E: std::fmt::Display>(context: &str, err: E) -> JSONRPCErrorError {
    JSONRPCErrorError {
        code: INTERNAL_ERROR_CODE,
        message: format!("{context}: {err}"),
        data: None,
    }
}

fn config_write_error(code: ConfigWriteErrorCode, message: impl Into<String>) -> JSONRPCErrorError {
    JSONRPCErrorError {
        code: INVALID_REQUEST_ERROR_CODE,
        message: message.into(),
        data: Some(json!({
            "config_write_error_code": code,
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    #[tokio::test]
    async fn read_includes_origins_and_layers() {
        let tmp = tempdir().expect("tempdir");
        std::fs::write(tmp.path().join(CONFIG_FILE_NAME), "model = \"user\"").unwrap();

        let managed_path = tmp.path().join("managed_config.toml");
        std::fs::write(&managed_path, "approval_policy = \"never\"").unwrap();

        let api = ConfigApi::with_overrides(
            tmp.path().to_path_buf(),
            vec![],
            LoaderOverrides {
                managed_config_path: Some(managed_path),
                #[cfg(target_os = "macos")]
                managed_preferences_base64: None,
            },
        );

        let response = api
            .read(ConfigReadParams {
                include_layers: true,
            })
            .await
            .expect("response");

        assert_eq!(
            response.config.get("approval_policy"),
            Some(&json!("never"))
        );

        assert_eq!(
            response
                .origins
                .get("approval_policy")
                .expect("origin")
                .name,
            ConfigLayerName::System
        );
        let layers = response.layers.expect("layers present");
        assert_eq!(layers.first().unwrap().name, ConfigLayerName::System);
        assert_eq!(layers.get(1).unwrap().name, ConfigLayerName::SessionFlags);
        assert_eq!(layers.last().unwrap().name, ConfigLayerName::User);
    }

    #[tokio::test]
    async fn write_value_reports_override() {
        let tmp = tempdir().expect("tempdir");
        std::fs::write(
            tmp.path().join(CONFIG_FILE_NAME),
            "approval_policy = \"on-request\"",
        )
        .unwrap();

        let managed_path = tmp.path().join("managed_config.toml");
        std::fs::write(&managed_path, "approval_policy = \"never\"").unwrap();

        let api = ConfigApi::with_overrides(
            tmp.path().to_path_buf(),
            vec![],
            LoaderOverrides {
                managed_config_path: Some(managed_path),
                #[cfg(target_os = "macos")]
                managed_preferences_base64: None,
            },
        );

        let result = api
            .write_value(ConfigValueWriteParams {
                file_path: tmp.path().join(CONFIG_FILE_NAME).display().to_string(),
                key_path: "approval_policy".to_string(),
                value: json!("never"),
                merge_strategy: MergeStrategy::Replace,
                expected_version: None,
            })
            .await
            .expect("result");

        let read_after = api
            .read(ConfigReadParams {
                include_layers: true,
            })
            .await
            .expect("read");
        let config_object = read_after.config.as_object().expect("object");
        assert_eq!(config_object.get("approval_policy"), Some(&json!("never")));
        assert_eq!(
            read_after
                .origins
                .get("approval_policy")
                .expect("origin")
                .name,
            ConfigLayerName::System
        );
        assert_eq!(result.status, WriteStatus::Ok);
        assert!(result.overridden_metadata.is_none());
    }

    #[tokio::test]
    async fn version_conflict_rejected() {
        let tmp = tempdir().expect("tempdir");
        std::fs::write(tmp.path().join(CONFIG_FILE_NAME), "model = \"user\"").unwrap();

        let api = ConfigApi::new(tmp.path().to_path_buf(), vec![]);
        let error = api
            .write_value(ConfigValueWriteParams {
                file_path: tmp.path().join(CONFIG_FILE_NAME).display().to_string(),
                key_path: "model".to_string(),
                value: json!("gpt-5"),
                merge_strategy: MergeStrategy::Replace,
                expected_version: Some("sha256:bogus".to_string()),
            })
            .await
            .expect_err("should fail");

        assert_eq!(error.code, INVALID_REQUEST_ERROR_CODE);
        assert_eq!(
            error
                .data
                .as_ref()
                .and_then(|d| d.get("config_write_error_code"))
                .and_then(serde_json::Value::as_str),
            Some("configVersionConflict")
        );
    }

    #[tokio::test]
    async fn invalid_user_value_rejected_even_if_overridden_by_managed() {
        let tmp = tempdir().expect("tempdir");
        std::fs::write(tmp.path().join(CONFIG_FILE_NAME), "model = \"user\"").unwrap();

        let managed_path = tmp.path().join("managed_config.toml");
        std::fs::write(&managed_path, "approval_policy = \"never\"").unwrap();

        let api = ConfigApi::with_overrides(
            tmp.path().to_path_buf(),
            vec![],
            LoaderOverrides {
                managed_config_path: Some(managed_path),
                #[cfg(target_os = "macos")]
                managed_preferences_base64: None,
            },
        );

        let error = api
            .write_value(ConfigValueWriteParams {
                file_path: tmp.path().join(CONFIG_FILE_NAME).display().to_string(),
                key_path: "approval_policy".to_string(),
                value: json!("bogus"),
                merge_strategy: MergeStrategy::Replace,
                expected_version: None,
            })
            .await
            .expect_err("should fail validation");

        assert_eq!(error.code, INVALID_REQUEST_ERROR_CODE);
        assert_eq!(
            error
                .data
                .as_ref()
                .and_then(|d| d.get("config_write_error_code"))
                .and_then(serde_json::Value::as_str),
            Some("configValidationError")
        );

        let contents =
            std::fs::read_to_string(tmp.path().join(CONFIG_FILE_NAME)).expect("read config");
        assert_eq!(contents.trim(), "model = \"user\"");
    }

    #[tokio::test]
    async fn read_reports_managed_overrides_user_and_session_flags() {
        let tmp = tempdir().expect("tempdir");
        std::fs::write(tmp.path().join(CONFIG_FILE_NAME), "model = \"user\"").unwrap();

        let managed_path = tmp.path().join("managed_config.toml");
        std::fs::write(&managed_path, "model = \"system\"").unwrap();

        let cli_overrides = vec![(
            "model".to_string(),
            TomlValue::String("session".to_string()),
        )];

        let api = ConfigApi::with_overrides(
            tmp.path().to_path_buf(),
            cli_overrides,
            LoaderOverrides {
                managed_config_path: Some(managed_path),
                #[cfg(target_os = "macos")]
                managed_preferences_base64: None,
            },
        );

        let response = api
            .read(ConfigReadParams {
                include_layers: true,
            })
            .await
            .expect("response");

        assert_eq!(response.config.get("model"), Some(&json!("system")));
        assert_eq!(
            response.origins.get("model").expect("origin").name,
            ConfigLayerName::System
        );
        let layers = response.layers.expect("layers");
        assert_eq!(layers.first().unwrap().name, ConfigLayerName::System);
        assert_eq!(layers.get(1).unwrap().name, ConfigLayerName::SessionFlags);
        assert_eq!(layers.get(2).unwrap().name, ConfigLayerName::User);
    }

    #[tokio::test]
    async fn write_value_reports_managed_override() {
        let tmp = tempdir().expect("tempdir");
        std::fs::write(tmp.path().join(CONFIG_FILE_NAME), "").unwrap();

        let managed_path = tmp.path().join("managed_config.toml");
        std::fs::write(&managed_path, "approval_policy = \"never\"").unwrap();

        let api = ConfigApi::with_overrides(
            tmp.path().to_path_buf(),
            vec![],
            LoaderOverrides {
                managed_config_path: Some(managed_path),
                #[cfg(target_os = "macos")]
                managed_preferences_base64: None,
            },
        );

        let result = api
            .write_value(ConfigValueWriteParams {
                file_path: tmp.path().join(CONFIG_FILE_NAME).display().to_string(),
                key_path: "approval_policy".to_string(),
                value: json!("on-request"),
                merge_strategy: MergeStrategy::Replace,
                expected_version: None,
            })
            .await
            .expect("result");

        assert_eq!(result.status, WriteStatus::OkOverridden);
        let overridden = result.overridden_metadata.expect("overridden metadata");
        assert_eq!(overridden.overriding_layer.name, ConfigLayerName::System);
        assert_eq!(overridden.effective_value, json!("never"));
    }
}
