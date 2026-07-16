use crate::RewriteProfile;
use crate::invalid_data_error;
use serde_yaml::Value as YamlValue;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use toml::Value as TomlValue;

#[derive(Debug)]
pub(crate) struct ParsedDocument {
    pub(crate) frontmatter: BTreeMap<String, FrontmatterValue>,
    pub(crate) body: String,
    frontmatter_error: Option<String>,
}

#[derive(Debug)]
pub(crate) enum FrontmatterValue {
    Scalar(String),
    Other,
}

#[derive(Debug)]
pub(crate) struct AgentMetadata {
    name: String,
    description: String,
    permission_mode: Option<String>,
    effort: Option<String>,
}

pub fn count_missing_subagents(source_agents: &Path, target_agents: &Path) -> io::Result<usize> {
    Ok(missing_subagent_names(source_agents, target_agents)?.len())
}

pub fn missing_subagent_names(
    source_agents: &Path,
    target_agents: &Path,
) -> io::Result<Vec<String>> {
    let mut names = Vec::new();
    for source_file in agent_source_files(source_agents)? {
        let document = parse_document(&source_file)?;
        let Some(metadata) = agent_metadata(&document) else {
            continue;
        };
        let Some(target) = subagent_target_file(&source_file, target_agents) else {
            continue;
        };
        if !target.exists() {
            names.push(metadata.name);
        }
    }
    Ok(names)
}

pub fn import_subagents_with_rewrite_profile(
    source_agents: &Path,
    target_agents: &Path,
    rewrite_profile: RewriteProfile,
) -> io::Result<Vec<String>> {
    if !source_agents.is_dir() {
        return Ok(Vec::new());
    }

    fs::create_dir_all(target_agents)?;
    let mut imported = Vec::new();
    for source_file in agent_source_files(source_agents)? {
        let Some(target) = subagent_target_file(&source_file, target_agents) else {
            continue;
        };
        if target.exists() {
            continue;
        }
        let document = parse_document(&source_file)?;
        let Some(metadata) = agent_metadata(&document) else {
            continue;
        };
        fs::write(
            &target,
            render_agent_toml(&document.body, &metadata, rewrite_profile)?,
        )?;
        imported.push(metadata.name);
    }

    Ok(imported)
}

fn agent_source_files(source_agents: &Path) -> io::Result<Vec<PathBuf>> {
    if !source_agents.is_dir() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    for entry in fs::read_dir(source_agents)? {
        let entry = entry?;
        let path = entry.path();
        if !entry.file_type()?.is_file()
            || path.extension().and_then(|ext| ext.to_str()) != Some("md")
        {
            continue;
        }
        if path.file_stem().and_then(|stem| stem.to_str()) == Some("README") {
            continue;
        }
        files.push(path);
    }
    files.sort();
    Ok(files)
}

pub(crate) fn subagent_target_file(source_file: &Path, target_agents: &Path) -> Option<PathBuf> {
    Some(target_agents.join(format!("{}.toml", source_file.file_stem()?.to_str()?)))
}

fn parse_document(source_file: &Path) -> io::Result<ParsedDocument> {
    let content = fs::read_to_string(source_file)?;
    Ok(parse_document_content(&content))
}

pub(crate) fn parse_document_content(content: &str) -> ParsedDocument {
    let Some(rest) = content
        .strip_prefix("---\n")
        .or_else(|| content.strip_prefix("---\r\n"))
    else {
        return ParsedDocument {
            frontmatter: BTreeMap::new(),
            body: content.to_string(),
            frontmatter_error: None,
        };
    };
    let Some((end, body_start)) = frontmatter_end(rest) else {
        return ParsedDocument {
            frontmatter: BTreeMap::new(),
            body: content.to_string(),
            frontmatter_error: None,
        };
    };

    let raw_frontmatter = &rest[..end];
    let body = &rest[body_start..];
    let (frontmatter, frontmatter_error) = parse_frontmatter(raw_frontmatter);
    ParsedDocument {
        frontmatter,
        body: body.to_string(),
        frontmatter_error,
    }
}

fn frontmatter_end(rest: &str) -> Option<(usize, usize)> {
    [
        "\r\n---\r\n",
        "\r\n---\n",
        "\n---\r\n",
        "\n---\n",
        "\r\n---",
        "\n---",
    ]
    .into_iter()
    .filter_map(|delimiter| rest.find(delimiter).map(|end| (end, end + delimiter.len())))
    .min_by_key(|(end, _body_start)| *end)
}

fn parse_frontmatter(
    raw_frontmatter: &str,
) -> (BTreeMap<String, FrontmatterValue>, Option<String>) {
    let parsed: YamlValue = match serde_yaml::from_str(raw_frontmatter) {
        Ok(parsed) => parsed,
        Err(err) => return (BTreeMap::new(), Some(err.to_string())),
    };
    let Some(mapping) = parsed.as_mapping() else {
        return (
            BTreeMap::new(),
            Some("frontmatter is not a YAML mapping".to_string()),
        );
    };

    let mut frontmatter = BTreeMap::new();
    for (key, value) in mapping {
        let Some(key) = key.as_str().map(str::trim).filter(|key| !key.is_empty()) else {
            continue;
        };
        frontmatter.insert(key.to_string(), frontmatter_value_from_yaml(value));
    }

    (frontmatter, None)
}

fn frontmatter_value_from_yaml(value: &YamlValue) -> FrontmatterValue {
    match value {
        YamlValue::String(value) => FrontmatterValue::Scalar(value.trim().to_string()),
        YamlValue::Bool(value) => FrontmatterValue::Scalar(value.to_string()),
        YamlValue::Number(value) => FrontmatterValue::Scalar(value.to_string()),
        YamlValue::Null | YamlValue::Sequence(_) | YamlValue::Mapping(_) | YamlValue::Tagged(_) => {
            FrontmatterValue::Other
        }
    }
}

pub(crate) fn agent_metadata(document: &ParsedDocument) -> Option<AgentMetadata> {
    if document.frontmatter_error.is_some() || document.body.trim().is_empty() {
        return None;
    }
    let name = document
        .frontmatter
        .get("name")
        .and_then(FrontmatterValue::as_scalar)
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)?;

    let description = document
        .frontmatter
        .get("description")
        .and_then(FrontmatterValue::as_scalar)
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)?;

    Some(AgentMetadata {
        name,
        description,
        permission_mode: frontmatter_string(&document.frontmatter, "permissionMode"),
        effort: frontmatter_string(&document.frontmatter, "effort"),
    })
}

pub(crate) fn render_agent_toml(
    body: &str,
    metadata: &AgentMetadata,
    rewrite_profile: RewriteProfile,
) -> io::Result<String> {
    let mut document = toml::map::Map::new();
    document.insert("name".to_string(), TomlValue::String(metadata.name.clone()));
    document.insert(
        "description".to_string(),
        TomlValue::String(rewrite_profile.rewrite(&metadata.description)),
    );
    if let Some(effort) = metadata.effort.as_ref()
        && let Some(effort) = map_agent_reasoning_effort(effort)
    {
        document.insert(
            "model_reasoning_effort".to_string(),
            TomlValue::String(effort),
        );
    }
    if let Some(sandbox_mode) = metadata
        .permission_mode
        .as_deref()
        .and_then(map_agent_permission_mode)
    {
        document.insert(
            "sandbox_mode".to_string(),
            TomlValue::String(sandbox_mode.to_string()),
        );
    }
    document.insert(
        "developer_instructions".to_string(),
        TomlValue::String(render_agent_body(body, rewrite_profile)),
    );

    let serialized = toml::to_string_pretty(&TomlValue::Table(document))
        .map_err(|err| invalid_data_error(format!("failed to serialize agent TOML: {err}")))?;
    Ok(format!("{}\n", serialized.trim_end()))
}

fn render_agent_body(body: &str, rewrite_profile: RewriteProfile) -> String {
    let body = rewrite_profile.rewrite(body.trim());
    if body.is_empty() {
        "No subagent instructions were found.".to_string()
    } else {
        body
    }
}

fn frontmatter_string(
    frontmatter: &BTreeMap<String, FrontmatterValue>,
    key: &str,
) -> Option<String> {
    frontmatter
        .get(key)
        .and_then(FrontmatterValue::as_scalar)
        .map(ToOwned::to_owned)
}

fn map_agent_reasoning_effort(effort: &str) -> Option<String> {
    let mapped = match effort {
        "max" => "xhigh".to_string(),
        _ => effort.to_string(),
    };
    matches!(
        mapped.as_str(),
        "none" | "minimal" | "low" | "medium" | "high" | "xhigh"
    )
    .then_some(mapped)
}

fn map_agent_permission_mode(permission_mode: &str) -> Option<&'static str> {
    match permission_mode {
        "acceptEdits" => Some("workspace-write"),
        "readOnly" => Some("read-only"),
        _ => None,
    }
}

impl FrontmatterValue {
    pub(crate) fn as_scalar(&self) -> Option<&str> {
        match self {
            Self::Scalar(value) => Some(value),
            Self::Other => None,
        }
    }
}
