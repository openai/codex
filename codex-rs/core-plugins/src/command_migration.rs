mod plugin;
mod render;

use render::rewrite_terms;
use render::slugify_name;
use render::yaml_string;
use serde_yaml::Value as YamlValue;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

const COMMAND_SKILL_PREFIX: &str = "source-command";
const MAX_SKILL_NAME_LEN: usize = 64;

pub(crate) use plugin::migrate_plugin_commands;
pub(crate) use plugin::migrated_command_skills_root;

/// Describes source-specific terms that should be rewritten in migrated command skills.
#[derive(Clone, Copy)]
pub struct RewriteProfile {
    doc_file_name: &'static str,
    term_variants: &'static [&'static str],
    case_sensitive_term_variants: &'static [&'static str],
}

impl RewriteProfile {
    pub const fn new(doc_file_name: &'static str, term_variants: &'static [&'static str]) -> Self {
        Self {
            doc_file_name,
            term_variants,
            case_sensitive_term_variants: &[],
        }
    }

    pub const fn with_case_sensitive_term_variants(
        mut self,
        term_variants: &'static [&'static str],
    ) -> Self {
        self.case_sensitive_term_variants = term_variants;
        self
    }
}

/// Controls how migrated commands obtain the description required by a Codex skill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandDescriptionMode {
    /// Skip source commands that do not declare a non-empty frontmatter description.
    RequireFrontmatter,
    /// Derive a stable description from the source command name when frontmatter is absent.
    UseSourceNameFallback,
}

/// Describes source-specific command migration behavior.
#[derive(Clone, Copy)]
pub struct CommandMigrationProfile {
    rewrite_profile: RewriteProfile,
    description_mode: CommandDescriptionMode,
}

impl CommandMigrationProfile {
    pub const fn new(
        rewrite_profile: RewriteProfile,
        description_mode: CommandDescriptionMode,
    ) -> Self {
        Self {
            rewrite_profile,
            description_mode,
        }
    }
}

#[derive(Debug)]
struct ParsedCommand {
    description: Option<String>,
    body: String,
}

#[derive(Debug, PartialEq, Eq)]
struct CommandSource {
    source_file: PathBuf,
    name: String,
    source_name: String,
}

#[derive(Clone, Copy)]
enum CommandSkillSizeLimit {
    Unbounded,
    MaxBytes(usize),
}

pub fn count_missing_commands_with_profile(
    source_commands: &Path,
    target_skills: &Path,
    profile: CommandMigrationProfile,
) -> io::Result<usize> {
    Ok(missing_command_names_with_profile(source_commands, target_skills, profile)?.len())
}

pub fn missing_command_names_with_profile(
    source_commands: &Path,
    target_skills: &Path,
    profile: CommandMigrationProfile,
) -> io::Result<Vec<String>> {
    Ok(
        unique_supported_command_sources(source_commands, profile.description_mode)?
            .into_iter()
            .filter(|source| !target_skills.join(&source.name).exists())
            .map(|source| source.name)
            .collect(),
    )
}

pub fn import_commands_with_profile(
    source_commands: &Path,
    target_skills: &Path,
    profile: CommandMigrationProfile,
) -> io::Result<Vec<String>> {
    if !source_commands.is_dir() {
        return Ok(Vec::new());
    }
    fs::create_dir_all(target_skills)?;
    import_command_sources(
        unique_supported_command_sources(source_commands, profile.description_mode)?,
        target_skills,
        profile,
        CommandSkillSizeLimit::Unbounded,
    )
}

fn import_command_sources(
    command_sources: Vec<CommandSource>,
    target_skills: &Path,
    profile: CommandMigrationProfile,
    size_limit: CommandSkillSizeLimit,
) -> io::Result<Vec<String>> {
    if command_sources.is_empty() {
        return Ok(Vec::new());
    }

    let mut imported = Vec::new();
    for CommandSource {
        source_file,
        name,
        source_name,
    } in command_sources
    {
        let document = parse_command(&source_file)?;
        let target_dir = target_skills.join(&name);
        if target_dir.exists() {
            continue;
        }
        let Some(description) =
            command_skill_description(&document, &source_name, profile.description_mode)
        else {
            continue;
        };
        let rendered = render_command_skill(
            &document.body,
            &name,
            &description,
            &source_name,
            profile.rewrite_profile,
        );
        if let CommandSkillSizeLimit::MaxBytes(max_bytes) = size_limit
            && rendered.len() > max_bytes
        {
            continue;
        }
        fs::create_dir_all(&target_dir)?;
        fs::write(target_dir.join("SKILL.md"), rendered)?;
        imported.push(name);
    }

    Ok(imported)
}

fn unique_supported_command_sources(
    source_commands: &Path,
    description_mode: CommandDescriptionMode,
) -> io::Result<Vec<CommandSource>> {
    Ok(unique_command_sources(supported_command_sources(
        source_commands,
        description_mode,
    )?))
}

fn supported_command_sources(
    source_commands: &Path,
    description_mode: CommandDescriptionMode,
) -> io::Result<Vec<CommandSource>> {
    let mut sources = Vec::new();
    for source_file in command_source_files(source_commands)? {
        let document = parse_command(&source_file)?;
        let source_name = command_source_name(source_commands, &source_file);
        let Some(name) = command_skill_name_if_supported(
            &source_name,
            &source_file,
            &document,
            description_mode,
        ) else {
            continue;
        };
        sources.push(CommandSource {
            source_file,
            name,
            source_name,
        });
    }
    Ok(sources)
}

fn unique_command_sources(command_sources: Vec<CommandSource>) -> Vec<CommandSource> {
    let mut by_name = BTreeMap::<String, BTreeMap<PathBuf, String>>::new();
    for source in command_sources {
        by_name
            .entry(source.name)
            .or_default()
            .insert(source.source_file, source.source_name);
    }

    by_name
        .into_iter()
        .filter_map(|(name, source_files)| {
            let mut source_files = source_files.into_iter();
            let (source_file, source_name) = source_files.next()?;
            if source_files.next().is_some() {
                return None;
            }
            Some(CommandSource {
                source_file,
                name,
                source_name,
            })
        })
        .collect()
}

fn command_source_files(source_commands: &Path) -> io::Result<Vec<PathBuf>> {
    if source_commands.is_file() {
        return Ok(
            if source_commands.extension().and_then(|ext| ext.to_str()) == Some("md") {
                vec![source_commands.to_path_buf()]
            } else {
                Vec::new()
            },
        );
    }

    let mut files = Vec::new();
    collect_markdown_files(source_commands, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_markdown_files(dir: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_markdown_files(&path, files)?;
        } else if file_type.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("md")
        {
            files.push(path);
        }
    }
    Ok(())
}

fn parse_command(source_file: &Path) -> io::Result<ParsedCommand> {
    Ok(parse_command_content(&fs::read_to_string(source_file)?))
}

fn parse_command_content(content: &str) -> ParsedCommand {
    let Some(rest) = content
        .strip_prefix("---\n")
        .or_else(|| content.strip_prefix("---\r\n"))
    else {
        return ParsedCommand {
            description: None,
            body: content.to_string(),
        };
    };
    let Some((end, body_start)) = frontmatter_end(rest) else {
        return ParsedCommand {
            description: None,
            body: content.to_string(),
        };
    };

    ParsedCommand {
        description: parse_command_description(&rest[..end]),
        body: rest[body_start..].to_string(),
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

fn parse_command_description(raw_frontmatter: &str) -> Option<String> {
    let parsed: YamlValue = serde_yaml::from_str(raw_frontmatter).ok()?;
    let mapping = parsed.as_mapping()?;
    mapping.iter().find_map(|(key, value)| {
        if key.as_str()?.trim() == "description" {
            yaml_scalar(value)
        } else {
            None
        }
    })
}

fn yaml_scalar(value: &YamlValue) -> Option<String> {
    match value {
        YamlValue::String(value) => Some(value.trim().to_string()),
        YamlValue::Bool(value) => Some(value.to_string()),
        YamlValue::Number(value) => Some(value.to_string()),
        YamlValue::Null | YamlValue::Sequence(_) | YamlValue::Mapping(_) | YamlValue::Tagged(_) => {
            None
        }
    }
}

fn command_skill_name(source_name: &str) -> String {
    slugify_name(&format!("{COMMAND_SKILL_PREFIX}-{source_name}"))
}

fn command_skill_name_if_supported(
    source_name: &str,
    source_file: &Path,
    document: &ParsedCommand,
    description_mode: CommandDescriptionMode,
) -> Option<String> {
    if source_file.file_stem().and_then(|stem| stem.to_str()) == Some("README") {
        return None;
    }
    command_skill_description(document, source_name, description_mode)?;
    let name = command_skill_name(source_name);
    if name.chars().count() > MAX_SKILL_NAME_LEN
        || has_unsupported_command_template_features(&document.body)
    {
        return None;
    }
    Some(name)
}

fn command_skill_description(
    document: &ParsedCommand,
    source_name: &str,
    description_mode: CommandDescriptionMode,
) -> Option<String> {
    document
        .description
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| match description_mode {
            CommandDescriptionMode::RequireFrontmatter => None,
            CommandDescriptionMode::UseSourceNameFallback => {
                Some(format!("Migrated source command `{source_name}`"))
            }
        })
}

fn command_source_name(source_commands: &Path, source_file: &Path) -> String {
    if source_commands.is_file() {
        return source_file
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default()
            .to_string();
    }
    source_file
        .strip_prefix(source_commands)
        .unwrap_or(source_file)
        .with_extension("")
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect::<Vec<_>>()
        .join("-")
}

fn render_command_skill(
    body: &str,
    name: &str,
    description: &str,
    source_name: &str,
    rewrite_profile: RewriteProfile,
) -> String {
    let body = rewrite_terms(body.trim(), rewrite_profile);
    let template_body = if body.is_empty() {
        "No command template body was found.".to_string()
    } else {
        body
    };
    format!(
        "---\nname: {}\ndescription: {}\n---\n\n# {name}\n\nUse this skill when the user asks to run the migrated source command `{source_name}`.\n\n## Command Template\n\n{template_body}\n",
        yaml_string(name),
        yaml_string(&rewrite_terms(description, rewrite_profile)),
    )
}

fn has_unsupported_command_template_features(template: &str) -> bool {
    template.contains("$ARGUMENTS")
        || contains_numbered_argument_placeholder(template)
        || (template.contains("{{") && template.contains("}}"))
        || template.contains("!`")
        || template.contains("! `")
        || template
            .split_whitespace()
            .any(|token| token.strip_prefix('@').is_some_and(|rest| !rest.is_empty()))
}

fn contains_numbered_argument_placeholder(template: &str) -> bool {
    let bytes = template.as_bytes();
    bytes
        .windows(2)
        .any(|window| window[0] == b'$' && window[1].is_ascii_digit())
}

#[cfg(test)]
#[path = "command_migration_tests.rs"]
mod tests;
