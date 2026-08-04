use std::io;
use std::path::PathBuf;

use codex_exec_server::ExecutorFileSystem;
use codex_protocol::protocol::Product;
use codex_skills::SkillDependencies;
use codex_skills::SkillParseError;
use codex_skills::SkillPolicy;
use codex_skills::SkillToolDependency;
use serde::Deserialize;

use super::MAX_DEPENDENCY_COMMAND_LEN;
use super::MAX_DEPENDENCY_DESCRIPTION_LEN;
use super::MAX_DEPENDENCY_TRANSPORT_LEN;
use super::MAX_DEPENDENCY_TYPE_LEN;
use super::MAX_DEPENDENCY_URL_LEN;
use super::MAX_DEPENDENCY_VALUE_LEN;
use super::SKILLS_METADATA_FILENAME;
use super::discovery::SkillMetadataDiscovery;

#[derive(Debug, Default, Deserialize)]
pub(super) struct SkillMetadataFile {
    // Keep parsing the host-only interface fields so malformed interface metadata
    // invalidates the file exactly as it does in the legacy loader.
    #[serde(default, rename = "interface")]
    _interface: Option<Interface>,
    #[serde(default)]
    pub(super) dependencies: Option<Dependencies>,
    #[serde(default)]
    pub(super) policy: Option<Policy>,
}

#[derive(Debug, Default, Deserialize)]
struct Interface {
    #[serde(rename = "display_name")]
    _display_name: Option<String>,
    #[serde(rename = "short_description")]
    _short_description: Option<String>,
    #[serde(rename = "icon_small")]
    _icon_small: Option<PathBuf>,
    #[serde(rename = "icon_large")]
    _icon_large: Option<PathBuf>,
    #[serde(rename = "brand_color")]
    _brand_color: Option<String>,
    #[serde(rename = "default_prompt")]
    _default_prompt: Option<String>,
}

#[derive(Default)]
pub(super) struct LoadedSkillMetadata {
    pub(super) dependencies: Option<SkillDependencies>,
    pub(super) policy: Option<SkillPolicy>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct Dependencies {
    #[serde(default)]
    tools: Vec<DependencyTool>,
}

#[derive(Debug, Deserialize)]
pub(super) struct Policy {
    #[serde(default)]
    allow_implicit_invocation: Option<bool>,
    #[serde(default)]
    products: Vec<Product>,
}

#[derive(Debug, Default, Deserialize)]
struct DependencyTool {
    #[serde(rename = "type")]
    kind: Option<String>,
    value: Option<String>,
    description: Option<String>,
    transport: Option<String>,
    command: Option<String>,
    url: Option<String>,
}

pub(super) async fn load_host_skill_metadata(
    file_system: &dyn ExecutorFileSystem,
    metadata: &SkillMetadataDiscovery,
) -> LoadedSkillMetadata {
    // Fail open: optional metadata should not block loading SKILL.md.
    let metadata_path = match metadata {
        SkillMetadataDiscovery::Present(path) => path,
        SkillMetadataDiscovery::Absent => return LoadedSkillMetadata::default(),
        SkillMetadataDiscovery::Probe(path) => {
            match file_system.get_metadata(path, /*sandbox*/ None).await {
                Ok(metadata) if metadata.is_file => {}
                Ok(_) => return LoadedSkillMetadata::default(),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    return LoadedSkillMetadata::default();
                }
                Err(error) => {
                    tracing::warn!(
                        "ignoring {path}: failed to stat {label}: {error}",
                        label = SKILLS_METADATA_FILENAME
                    );
                    return LoadedSkillMetadata::default();
                }
            }
            path
        }
    };

    let contents = match file_system
        .read_file_text(metadata_path, /*sandbox*/ None)
        .await
    {
        Ok(contents) => contents,
        Err(error) => {
            tracing::warn!(
                "ignoring {metadata_path}: failed to read {label}: {error}",
                label = SKILLS_METADATA_FILENAME
            );
            return LoadedSkillMetadata::default();
        }
    };

    let parsed: SkillMetadataFile = match serde_yaml::from_str(&contents) {
        Ok(parsed) => parsed,
        Err(error) => {
            tracing::warn!(
                "ignoring {metadata_path}: invalid {label}: {error}",
                label = SKILLS_METADATA_FILENAME
            );
            return LoadedSkillMetadata::default();
        }
    };

    let SkillMetadataFile {
        _interface: _,
        dependencies,
        policy,
    } = parsed;
    LoadedSkillMetadata {
        dependencies: resolve_dependencies(dependencies),
        policy: resolve_policy(policy),
    }
}
pub(super) fn resolve_dependencies(
    dependencies: Option<Dependencies>,
) -> Option<SkillDependencies> {
    let dependencies = dependencies?;
    let tools = dependencies
        .tools
        .into_iter()
        .filter_map(resolve_dependency_tool)
        .collect::<Vec<_>>();
    if tools.is_empty() {
        None
    } else {
        Some(SkillDependencies { tools })
    }
}

pub(super) fn resolve_policy(policy: Option<Policy>) -> Option<SkillPolicy> {
    policy.map(|policy| SkillPolicy {
        allow_implicit_invocation: policy.allow_implicit_invocation,
        products: policy.products,
    })
}

fn resolve_dependency_tool(tool: DependencyTool) -> Option<SkillToolDependency> {
    let r#type = resolve_required_str(
        tool.kind,
        MAX_DEPENDENCY_TYPE_LEN,
        "dependencies.tools.type",
    )?;
    let value = resolve_required_str(
        tool.value,
        MAX_DEPENDENCY_VALUE_LEN,
        "dependencies.tools.value",
    )?;
    let description = resolve_str(
        tool.description,
        MAX_DEPENDENCY_DESCRIPTION_LEN,
        "dependencies.tools.description",
    );
    let transport = resolve_str(
        tool.transport,
        MAX_DEPENDENCY_TRANSPORT_LEN,
        "dependencies.tools.transport",
    );
    let command = resolve_str(
        tool.command,
        MAX_DEPENDENCY_COMMAND_LEN,
        "dependencies.tools.command",
    );
    let url = resolve_str(tool.url, MAX_DEPENDENCY_URL_LEN, "dependencies.tools.url");

    Some(SkillToolDependency {
        r#type,
        value,
        description,
        transport,
        command,
        url,
    })
}

pub(super) fn sanitize_single_line(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(super) fn validate_len(
    value: &str,
    max_len: usize,
    field_name: &'static str,
) -> Result<(), SkillParseError> {
    if value.is_empty() {
        return Err(SkillParseError::MissingField(field_name));
    }
    if value.chars().count() > max_len {
        return Err(SkillParseError::InvalidField {
            field: field_name,
            reason: format!("exceeds maximum length of {max_len} characters"),
        });
    }
    Ok(())
}

fn resolve_str(value: Option<String>, max_len: usize, field: &'static str) -> Option<String> {
    let value = value?;
    let value = sanitize_single_line(&value);
    if value.is_empty() {
        tracing::warn!("ignoring {field}: value is empty");
        return None;
    }
    if value.chars().count() > max_len {
        tracing::warn!("ignoring {field}: exceeds maximum length of {max_len} characters");
        return None;
    }
    Some(value)
}

fn resolve_required_str(
    value: Option<String>,
    max_len: usize,
    field: &'static str,
) -> Option<String> {
    let Some(value) = value else {
        tracing::warn!("ignoring {field}: value is missing");
        return None;
    };
    resolve_str(Some(value), max_len, field)
}
