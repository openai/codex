//! Helpers for mapping config parse/validation failures to file locations and
//! rendering them in a user-friendly way.

use crate::ConfigLayerEntry;
use crate::ConfigLayerStack;
use crate::ConfigLayerStackOrdering;
use codex_app_server_protocol::ConfigLayerSource;
use codex_utils_absolute_path::AbsolutePathBufGuard;
use serde::de::DeserializeOwned;
use serde_path_to_error::Path as SerdePath;
use serde_path_to_error::Segment as SerdeSegment;
use std::fmt;
use std::fmt::Write;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use toml_edit::Document;
use toml_edit::Item;
use toml_edit::Table;
use toml_edit::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextPosition {
    pub line: usize,
    pub column: usize,
}

/// Text range in 1-based line/column coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextRange {
    pub start: TextPosition,
    pub end: TextPosition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    pub path: PathBuf,
    pub range: TextRange,
    pub message: String,
}

impl ConfigError {
    pub fn new(path: PathBuf, range: TextRange, message: impl Into<String>) -> Self {
        Self {
            path,
            range,
            message: message.into(),
        }
    }
}

#[derive(Debug)]
pub struct ConfigLoadError {
    error: ConfigError,
    source: Option<toml::de::Error>,
}

impl ConfigLoadError {
    pub fn new(error: ConfigError, source: Option<toml::de::Error>) -> Self {
        Self { error, source }
    }

    pub fn config_error(&self) -> &ConfigError {
        &self.error
    }
}

impl fmt::Display for ConfigLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}: {}",
            self.error.path.display(),
            self.error.range.start.line,
            self.error.range.start.column,
            self.error.message
        )
    }
}

impl std::error::Error for ConfigLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_ref()
            .map(|err| err as &dyn std::error::Error)
    }
}

pub fn io_error_from_config_error(
    kind: io::ErrorKind,
    error: ConfigError,
    source: Option<toml::de::Error>,
) -> io::Error {
    io::Error::new(kind, ConfigLoadError::new(error, source))
}

pub fn config_error_from_toml(
    path: impl AsRef<Path>,
    contents: &str,
    err: toml::de::Error,
) -> ConfigError {
    let range = err
        .span()
        .map(|span| text_range_from_span(contents, span))
        .unwrap_or_else(default_range);
    ConfigError::new(path.as_ref().to_path_buf(), range, err.message())
}

pub fn config_error_from_typed_toml<T: DeserializeOwned>(
    path: impl AsRef<Path>,
    contents: &str,
) -> Option<ConfigError> {
    let deserializer = match toml::de::Deserializer::parse(contents) {
        Ok(deserializer) => deserializer,
        Err(err) => return Some(config_error_from_toml(path, contents, err)),
    };

    let result: Result<T, _> = serde_path_to_error::deserialize(deserializer);
    match result {
        Ok(_) => None,
        Err(err) => {
            let path_hint = err.path().clone();
            let toml_err: toml::de::Error = err.into_inner();
            let range = span_for_config_path(contents, &path_hint)
                .or_else(|| toml_err.span())
                .map(|span| text_range_from_span(contents, span))
                .unwrap_or_else(default_range);
            Some(ConfigError::new(
                path.as_ref().to_path_buf(),
                range,
                toml_err.message(),
            ))
        }
    }
}

pub async fn first_layer_config_error<T: DeserializeOwned>(
    layers: &ConfigLayerStack,
    config_toml_file: &str,
) -> Option<ConfigError> {
    // When the merged config fails schema validation, we surface the first concrete
    // per-file error to point users at a specific file and range rather than an
    // opaque merged-layer failure.
    first_layer_config_error_for_entries::<T, _>(
        layers.get_layers(ConfigLayerStackOrdering::LowestPrecedenceFirst, false),
        config_toml_file,
    )
    .await
}

pub async fn first_layer_config_error_from_entries<T: DeserializeOwned>(
    layers: &[ConfigLayerEntry],
    config_toml_file: &str,
) -> Option<ConfigError> {
    first_layer_config_error_for_entries::<T, _>(layers.iter(), config_toml_file).await
}

async fn first_layer_config_error_for_entries<'a, T: DeserializeOwned, I>(
    layers: I,
    config_toml_file: &str,
) -> Option<ConfigError>
where
    I: IntoIterator<Item = &'a ConfigLayerEntry>,
{
    for layer in layers {
        let Some(path) = config_path_for_layer(layer, config_toml_file) else {
            continue;
        };
        let contents = match tokio::fs::read_to_string(&path).await {
            Ok(contents) => contents,
            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
            Err(err) => {
                tracing::debug!("Failed to read config file {}: {err}", path.display());
                continue;
            }
        };

        let Some(parent) = path.parent() else {
            tracing::debug!("Config file {} has no parent directory", path.display());
            continue;
        };
        let _guard = AbsolutePathBufGuard::new(parent);
        if let Some(error) = config_error_from_typed_toml::<T>(&path, &contents) {
            return Some(error);
        }
    }

    None
}

fn config_path_for_layer(layer: &ConfigLayerEntry, config_toml_file: &str) -> Option<PathBuf> {
    match &layer.name {
        ConfigLayerSource::System { file } => Some(file.to_path_buf()),
        ConfigLayerSource::User { file } => Some(file.to_path_buf()),
        ConfigLayerSource::Project { dot_codex_folder } => {
            Some(dot_codex_folder.as_path().join(config_toml_file))
        }
        ConfigLayerSource::LegacyManagedConfigTomlFromFile { file } => Some(file.to_path_buf()),
        ConfigLayerSource::Mdm { .. }
        | ConfigLayerSource::SessionFlags
        | ConfigLayerSource::LegacyManagedConfigTomlFromMdm => None,
    }
}

fn text_range_from_span(contents: &str, span: std::ops::Range<usize>) -> TextRange {
    let start = position_for_offset(contents, span.start);
    let end_index = if span.end > span.start {
        span.end - 1
    } else {
        span.end
    };
    let end = position_for_offset(contents, end_index);
    TextRange { start, end }
}

pub fn format_config_error(error: &ConfigError, contents: &str) -> String {
    let mut output = String::new();
    let start = error.range.start;
    let _ = writeln!(
        output,
        "{}:{}:{}: {}",
        error.path.display(),
        start.line,
        start.column,
        error.message
    );

    let line_index = start.line.saturating_sub(1);
    let line = match contents.lines().nth(line_index) {
        Some(line) => line.trim_end_matches('\r'),
        None => return output.trim_end().to_string(),
    };

    let line_number = start.line;
    let gutter = line_number.to_string().len();
    let _ = writeln!(output, "{:width$} |", "", width = gutter);
    let _ = writeln!(output, "{line_number:>gutter$} | {line}");

    let highlight_len = if error.range.end.line == error.range.start.line
        && error.range.end.column >= error.range.start.column
    {
        error.range.end.column - error.range.start.column + 1
    } else {
        1
    };
    let spaces = " ".repeat(start.column.saturating_sub(1));
    let carets = "^".repeat(highlight_len.max(1));
    let _ = writeln!(output, "{:width$} | {spaces}{carets}", "", width = gutter);
    output.trim_end().to_string()
}

pub fn format_config_error_with_source(error: &ConfigError) -> String {
    match std::fs::read_to_string(&error.path) {
        Ok(contents) => format_config_error(error, &contents),
        Err(_) => format_config_error(error, ""),
    }
}

fn position_for_offset(contents: &str, index: usize) -> TextPosition {
    let bytes = contents.as_bytes();
    if bytes.is_empty() {
        return TextPosition { line: 1, column: 1 };
    }

    let safe_index = index.min(bytes.len().saturating_sub(1));
    let column_offset = index.saturating_sub(safe_index);
    let index = safe_index;

    let line_start = bytes[..index]
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map(|pos| pos + 1)
        .unwrap_or(0);
    let line = bytes[..line_start]
        .iter()
        .filter(|byte| **byte == b'\n')
        .count();

    let column = std::str::from_utf8(&bytes[line_start..=index])
        .map(|slice| slice.chars().count().saturating_sub(1))
        .unwrap_or_else(|_| index - line_start);
    let column = column + column_offset;

    TextPosition {
        line: line + 1,
        column: column + 1,
    }
}

fn default_range() -> TextRange {
    let position = TextPosition { line: 1, column: 1 };
    TextRange {
        start: position,
        end: position,
    }
}

enum TomlNode<'a> {
    Item(&'a Item),
    Table(&'a Table),
    Value(&'a Value),
}

fn span_for_path(contents: &str, path: &SerdePath) -> Option<std::ops::Range<usize>> {
    let doc = contents.parse::<Document<String>>().ok()?;
    let node = node_for_path(doc.as_item(), path)?;
    match node {
        TomlNode::Item(item) => item.span(),
        TomlNode::Table(table) => table.span(),
        TomlNode::Value(value) => value.span(),
    }
}

fn span_for_config_path(contents: &str, path: &SerdePath) -> Option<std::ops::Range<usize>> {
    if is_features_table_path(path)
        && let Some(span) = span_for_features_value(contents)
    {
        return Some(span);
    }
    span_for_path(contents, path)
}

fn is_features_table_path(path: &SerdePath) -> bool {
    let mut segments = path.iter();
    matches!(segments.next(), Some(SerdeSegment::Map { key }) if key == "features")
        && segments.next().is_none()
}

fn span_for_features_value(contents: &str) -> Option<std::ops::Range<usize>> {
    let doc = contents.parse::<Document<String>>().ok()?;
    let root = doc.as_item().as_table_like()?;
    let features_item = root.get("features")?;
    let features_table = features_item.as_table_like()?;
    for (_, item) in features_table.iter() {
        match item {
            Item::Value(Value::Boolean(_)) => continue,
            Item::Value(value) => return value.span(),
            Item::Table(table) => return table.span(),
            Item::ArrayOfTables(array) => return array.span(),
            Item::None => continue,
        }
    }
    None
}

fn node_for_path<'a>(item: &'a Item, path: &SerdePath) -> Option<TomlNode<'a>> {
    let segments: Vec<_> = path.iter().cloned().collect();
    let mut node = TomlNode::Item(item);
    let mut index = 0;
    while index < segments.len() {
        match &segments[index] {
            SerdeSegment::Map { key } | SerdeSegment::Enum { variant: key } => {
                if let Some(next) = map_child(&node, key) {
                    node = next;
                    index += 1;
                    continue;
                }

                if index + 1 < segments.len() {
                    index += 1;
                    continue;
                }
                return None;
            }
            SerdeSegment::Seq { index: seq_index } => {
                node = seq_child(&node, *seq_index)?;
                index += 1;
            }
            SerdeSegment::Unknown => return None,
        }
    }
    Some(node)
}

fn map_child<'a>(node: &TomlNode<'a>, key: &str) -> Option<TomlNode<'a>> {
    match node {
        TomlNode::Item(item) => {
            let table = item.as_table_like()?;
            table.get(key).map(TomlNode::Item)
        }
        TomlNode::Table(table) => table.get(key).map(TomlNode::Item),
        TomlNode::Value(Value::InlineTable(table)) => table.get(key).map(TomlNode::Value),
        _ => None,
    }
}

fn seq_child<'a>(node: &TomlNode<'a>, index: usize) -> Option<TomlNode<'a>> {
    match node {
        TomlNode::Item(Item::Value(Value::Array(array))) => array.get(index).map(TomlNode::Value),
        TomlNode::Item(Item::ArrayOfTables(array)) => array.get(index).map(TomlNode::Table),
        TomlNode::Value(Value::Array(array)) => array.get(index).map(TomlNode::Value),
        _ => None,
    }
}
