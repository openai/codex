use super::CONFIG_TOML_FILE;
use super::ConfigToml;
use crate::config::edit::ConfigEdit;
use crate::config::edit::ConfigEditsBuilder;
use crate::config_loader::ConfigLayerEntry;
use crate::config_loader::ConfigLayerStack;
use crate::config_loader::LoaderOverrides;
use crate::config_loader::load_config_layers_state;
use crate::config_loader::merge_toml_values;
use codex_app_server_protocol::Config as ApiConfig;
use codex_app_server_protocol::ConfigBatchWriteParams;
use codex_app_server_protocol::ConfigLayerMetadata;
use codex_app_server_protocol::ConfigLayerName;
use codex_app_server_protocol::ConfigReadParams;
use codex_app_server_protocol::ConfigReadResponse;
use codex_app_server_protocol::ConfigValueWriteParams;
use codex_app_server_protocol::ConfigWriteErrorCode;
use codex_app_server_protocol::ConfigWriteResponse;
use codex_app_server_protocol::MergeStrategy;
use codex_app_server_protocol::OverriddenMetadata;
use codex_app_server_protocol::WriteStatus;
use serde_json::Value as JsonValue;
use std::path::Path;
use std::path::PathBuf;
use thiserror::Error;
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
}

impl ConfigService {
    pub fn new(codex_home: PathBuf, cli_overrides: Vec<(String, TomlValue)>) -> Self {
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

    pub async fn read(
        &self,
        params: ConfigReadParams,
    ) -> Result<ConfigReadResponse, ConfigServiceError> {
        let layers = self
            .load_layers_state()
            .await
            .map_err(|err| ConfigServiceError::io("failed to read configuration layers", err))?;

        let effective = layers.effective_config();
        validate_config(&effective)
            .map_err(|err| ConfigServiceError::toml("invalid configuration", err))?;

        let json_value = serde_json::to_value(&effective)
            .map_err(|err| ConfigServiceError::json("failed to serialize configuration", err))?;
        let config: ApiConfig = serde_json::from_value(json_value)
            .map_err(|err| ConfigServiceError::json("failed to deserialize configuration", err))?;

        Ok(ConfigReadResponse {
            config,
            origins: layers.origins(),
            layers: params.include_layers.then(|| layers.layers_high_to_low()),
        })
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
            .load_layers_state()
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
        let allowed_path = self.codex_home.join(CONFIG_TOML_FILE);
        let provided_path = file_path
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| allowed_path.clone());

        if !paths_match(&allowed_path, &provided_path) {
            return Err(ConfigServiceError::write(
                ConfigWriteErrorCode::ConfigLayerReadonly,
                "Only writes to the user config are allowed",
            ));
        }

        let layers = self
            .load_layers_state()
            .await
            .map_err(|err| ConfigServiceError::io("failed to load configuration", err))?;

        if let Some(expected) = expected_version.as_deref()
            && expected != layers.user.version
        {
            return Err(ConfigServiceError::write(
                ConfigWriteErrorCode::ConfigVersionConflict,
                "Configuration was modified since last read. Fetch latest version and retry.",
            ));
        }

        let mut user_config = layers.user.config.clone();
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

        let updated_layers = layers.with_user_config(user_config.clone());
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

        let file_path = provided_path
            .canonicalize()
            .unwrap_or(provided_path.clone())
            .display()
            .to_string();

        Ok(ConfigWriteResponse {
            status,
            version: updated_layers.user.version.clone(),
            file_path,
            overridden_metadata: overridden,
        })
    }

    async fn load_layers_state(&self) -> std::io::Result<ConfigLayerStack> {
        load_config_layers_state(
            &self.codex_home,
            &self.cli_overrides,
            self.loader_overrides.clone(),
        )
        .await
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

fn paths_match(expected: &Path, provided: &Path) -> bool {
    if let (Ok(expanded_expected), Ok(expanded_provided)) =
        (expected.canonicalize(), provided.canonicalize())
    {
        return expanded_expected == expanded_provided;
    }

    expected == provided
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

fn override_message(layer: &ConfigLayerName) -> String {
    match layer {
        ConfigLayerName::Mdm => "Overridden by managed policy (mdm)".to_string(),
        ConfigLayerName::System => "Overridden by managed config (system)".to_string(),
        ConfigLayerName::SessionFlags => "Overridden by session flags".to_string(),
        ConfigLayerName::User => "Overridden by user config".to_string(),
    }
}

fn compute_override_metadata(
    layers: &ConfigLayerStack,
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
    let check =
        |state: &ConfigLayerEntry| value_at_path(&state.config, segments).map(|_| state.metadata());

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

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use codex_app_server_protocol::AskForApproval;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    #[test]
    fn toml_value_to_item_handles_nested_config_tables() {
        let config = r#"
[mcp_servers.docs]
command = "docs-server"

[mcp_servers.docs.http_headers]
X-Doc = "42"
"#;

        let value: TomlValue = toml::from_str(config).expect("parse config example");
        let item = toml_value_to_item(&value).expect("convert to toml_edit item");

        let root = item.as_table().expect("root table");
        assert!(!root.is_implicit(), "root table should be explicit");

        let mcp_servers = root
            .get("mcp_servers")
            .and_then(TomlItem::as_table)
            .expect("mcp_servers table");
        assert!(
            !mcp_servers.is_implicit(),
            "mcp_servers table should be explicit"
        );

        let docs = mcp_servers
            .get("docs")
            .and_then(TomlItem::as_table)
            .expect("docs table");
        assert_eq!(
            docs.get("command")
                .and_then(TomlItem::as_value)
                .and_then(toml_edit::Value::as_str),
            Some("docs-server")
        );

        let http_headers = docs
            .get("http_headers")
            .and_then(TomlItem::as_table)
            .expect("http_headers table");
        assert_eq!(
            http_headers
                .get("X-Doc")
                .and_then(TomlItem::as_value)
                .and_then(toml_edit::Value::as_str),
            Some("42")
        );
    }

    #[tokio::test]
    async fn write_value_preserves_comments_and_order() -> Result<()> {
        let tmp = tempdir().expect("tempdir");
        let original = r#"# Codex user configuration
model = "gpt-5"
approval_policy = "on-request"

[notice]
# Preserve this comment
hide_full_access_warning = true

[features]
unified_exec = true
"#;
        std::fs::write(tmp.path().join(CONFIG_TOML_FILE), original)?;

        let service = ConfigService::new(tmp.path().to_path_buf(), vec![]);
        service
            .write_value(ConfigValueWriteParams {
                file_path: Some(tmp.path().join(CONFIG_TOML_FILE).display().to_string()),
                key_path: "features.remote_compaction".to_string(),
                value: serde_json::json!(true),
                merge_strategy: MergeStrategy::Replace,
                expected_version: None,
            })
            .await
            .expect("write succeeds");

        let updated =
            std::fs::read_to_string(tmp.path().join(CONFIG_TOML_FILE)).expect("read config");
        let expected = r#"# Codex user configuration
model = "gpt-5"
approval_policy = "on-request"

[notice]
# Preserve this comment
hide_full_access_warning = true

[features]
unified_exec = true
remote_compaction = true
"#;
        assert_eq!(updated, expected);
        Ok(())
    }

    #[tokio::test]
    async fn read_includes_origins_and_layers() {
        let tmp = tempdir().expect("tempdir");
        std::fs::write(tmp.path().join(CONFIG_TOML_FILE), "model = \"user\"").unwrap();

        let managed_path = tmp.path().join("managed_config.toml");
        std::fs::write(&managed_path, "approval_policy = \"never\"").unwrap();

        let service = ConfigService::with_overrides(
            tmp.path().to_path_buf(),
            vec![],
            LoaderOverrides {
                managed_config_path: Some(managed_path),
                #[cfg(target_os = "macos")]
                managed_preferences_base64: None,
            },
        );

        let response = service
            .read(ConfigReadParams {
                include_layers: true,
            })
            .await
            .expect("response");

        assert_eq!(response.config.approval_policy, Some(AskForApproval::Never));

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
            tmp.path().join(CONFIG_TOML_FILE),
            "approval_policy = \"on-request\"",
        )
        .unwrap();

        let managed_path = tmp.path().join("managed_config.toml");
        std::fs::write(&managed_path, "approval_policy = \"never\"").unwrap();

        let service = ConfigService::with_overrides(
            tmp.path().to_path_buf(),
            vec![],
            LoaderOverrides {
                managed_config_path: Some(managed_path),
                #[cfg(target_os = "macos")]
                managed_preferences_base64: None,
            },
        );

        let result = service
            .write_value(ConfigValueWriteParams {
                file_path: Some(tmp.path().join(CONFIG_TOML_FILE).display().to_string()),
                key_path: "approval_policy".to_string(),
                value: serde_json::json!("never"),
                merge_strategy: MergeStrategy::Replace,
                expected_version: None,
            })
            .await
            .expect("result");

        let read_after = service
            .read(ConfigReadParams {
                include_layers: true,
            })
            .await
            .expect("read");
        assert_eq!(
            read_after.config.approval_policy,
            Some(AskForApproval::Never)
        );
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
        std::fs::write(tmp.path().join(CONFIG_TOML_FILE), "model = \"user\"").unwrap();

        let service = ConfigService::new(tmp.path().to_path_buf(), vec![]);
        let error = service
            .write_value(ConfigValueWriteParams {
                file_path: Some(tmp.path().join(CONFIG_TOML_FILE).display().to_string()),
                key_path: "model".to_string(),
                value: serde_json::json!("gpt-5"),
                merge_strategy: MergeStrategy::Replace,
                expected_version: Some("sha256:bogus".to_string()),
            })
            .await
            .expect_err("should fail");

        assert_eq!(
            error.write_error_code(),
            Some(ConfigWriteErrorCode::ConfigVersionConflict)
        );
    }

    #[tokio::test]
    async fn write_value_defaults_to_user_config_path() {
        let tmp = tempdir().expect("tempdir");
        std::fs::write(tmp.path().join(CONFIG_TOML_FILE), "").unwrap();

        let service = ConfigService::new(tmp.path().to_path_buf(), vec![]);
        service
            .write_value(ConfigValueWriteParams {
                file_path: None,
                key_path: "model".to_string(),
                value: serde_json::json!("gpt-new"),
                merge_strategy: MergeStrategy::Replace,
                expected_version: None,
            })
            .await
            .expect("write succeeds");

        let contents =
            std::fs::read_to_string(tmp.path().join(CONFIG_TOML_FILE)).expect("read config");
        assert!(
            contents.contains("model = \"gpt-new\""),
            "config.toml should be updated even when file_path is omitted"
        );
    }

    #[tokio::test]
    async fn invalid_user_value_rejected_even_if_overridden_by_managed() {
        let tmp = tempdir().expect("tempdir");
        std::fs::write(tmp.path().join(CONFIG_TOML_FILE), "model = \"user\"").unwrap();

        let managed_path = tmp.path().join("managed_config.toml");
        std::fs::write(&managed_path, "approval_policy = \"never\"").unwrap();

        let service = ConfigService::with_overrides(
            tmp.path().to_path_buf(),
            vec![],
            LoaderOverrides {
                managed_config_path: Some(managed_path),
                #[cfg(target_os = "macos")]
                managed_preferences_base64: None,
            },
        );

        let error = service
            .write_value(ConfigValueWriteParams {
                file_path: Some(tmp.path().join(CONFIG_TOML_FILE).display().to_string()),
                key_path: "approval_policy".to_string(),
                value: serde_json::json!("bogus"),
                merge_strategy: MergeStrategy::Replace,
                expected_version: None,
            })
            .await
            .expect_err("should fail validation");

        assert_eq!(
            error.write_error_code(),
            Some(ConfigWriteErrorCode::ConfigValidationError)
        );

        let contents =
            std::fs::read_to_string(tmp.path().join(CONFIG_TOML_FILE)).expect("read config");
        assert_eq!(contents.trim(), "model = \"user\"");
    }

    #[tokio::test]
    async fn read_reports_managed_overrides_user_and_session_flags() {
        let tmp = tempdir().expect("tempdir");
        std::fs::write(tmp.path().join(CONFIG_TOML_FILE), "model = \"user\"").unwrap();

        let managed_path = tmp.path().join("managed_config.toml");
        std::fs::write(&managed_path, "model = \"system\"").unwrap();

        let cli_overrides = vec![(
            "model".to_string(),
            TomlValue::String("session".to_string()),
        )];

        let service = ConfigService::with_overrides(
            tmp.path().to_path_buf(),
            cli_overrides,
            LoaderOverrides {
                managed_config_path: Some(managed_path),
                #[cfg(target_os = "macos")]
                managed_preferences_base64: None,
            },
        );

        let response = service
            .read(ConfigReadParams {
                include_layers: true,
            })
            .await
            .expect("response");

        assert_eq!(response.config.model.as_deref(), Some("system"));
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
        std::fs::write(tmp.path().join(CONFIG_TOML_FILE), "").unwrap();

        let managed_path = tmp.path().join("managed_config.toml");
        std::fs::write(&managed_path, "approval_policy = \"never\"").unwrap();

        let service = ConfigService::with_overrides(
            tmp.path().to_path_buf(),
            vec![],
            LoaderOverrides {
                managed_config_path: Some(managed_path),
                #[cfg(target_os = "macos")]
                managed_preferences_base64: None,
            },
        );

        let result = service
            .write_value(ConfigValueWriteParams {
                file_path: Some(tmp.path().join(CONFIG_TOML_FILE).display().to_string()),
                key_path: "approval_policy".to_string(),
                value: serde_json::json!("on-request"),
                merge_strategy: MergeStrategy::Replace,
                expected_version: None,
            })
            .await
            .expect("result");

        assert_eq!(result.status, WriteStatus::OkOverridden);
        let overridden = result.overridden_metadata.expect("overridden metadata");
        assert_eq!(overridden.overriding_layer.name, ConfigLayerName::System);
        assert_eq!(overridden.effective_value, serde_json::json!("never"));
    }
}
