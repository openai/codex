use codex_utils_absolute_path::AbsolutePathBuf;
use serde_yaml::Value as YamlValue;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

const SOURCE_EXTERNAL_AGENT_NAME: &str = "claude";
const COMMAND_SKILL_PREFIX: &str = "source-command";
const MAX_SKILL_NAME_LEN: usize = 64;
const PLUGIN_COMMANDS_DIR: &str = "commands";
const PLUGIN_METADATA_DIR: &str = ".codex-plugin";
const MIGRATED_COMMAND_SKILLS_DIR: &str = "migrated-command-skills";

#[derive(Debug)]
struct ParsedCommand {
    description: Option<String>,
    body: String,
}

#[derive(Clone, Copy)]
enum CommandMigrationSource {
    ExternalAgent,
    Plugin,
}

impl CommandMigrationSource {
    fn rewrite_content(self, content: &str) -> String {
        match self {
            Self::ExternalAgent => rewrite_external_agent_terms(content),
            Self::Plugin => content.to_string(),
        }
    }
}

pub fn count_missing_commands(source_commands: &Path, target_skills: &Path) -> io::Result<usize> {
    Ok(missing_command_names(source_commands, target_skills)?.len())
}

pub fn missing_command_names(
    source_commands: &Path,
    target_skills: &Path,
) -> io::Result<Vec<String>> {
    Ok(unique_supported_command_sources(source_commands)?
        .into_iter()
        .filter(|(_source_file, name)| !target_skills.join(name).exists())
        .map(|(_source_file, name)| name)
        .collect())
}

pub fn import_commands(source_commands: &Path, target_skills: &Path) -> io::Result<Vec<String>> {
    import_commands_from_source(
        source_commands,
        target_skills,
        CommandMigrationSource::ExternalAgent,
    )
}

fn import_commands_from_source(
    source_commands: &Path,
    target_skills: &Path,
    migration_source: CommandMigrationSource,
) -> io::Result<Vec<String>> {
    if !source_commands.is_dir() {
        return Ok(Vec::new());
    }

    fs::create_dir_all(target_skills)?;
    let mut imported = Vec::new();
    for (source_file, name) in unique_supported_command_sources(source_commands)? {
        let document = parse_command(&source_file)?;
        let target_dir = target_skills.join(&name);
        if target_dir.exists() {
            continue;
        }
        fs::create_dir_all(&target_dir)?;
        let source_name = command_source_name(source_commands, &source_file);
        let Some(description) = document.description.as_deref() else {
            continue;
        };
        fs::write(
            target_dir.join("SKILL.md"),
            render_command_skill(
                &document.body,
                &name,
                description,
                &source_name,
                migration_source,
            ),
        )?;
        imported.push(name);
    }

    Ok(imported)
}

pub(crate) fn migrate_plugin_commands(plugin_root: &Path) -> io::Result<()> {
    let target_skills = plugin_root
        .join(PLUGIN_METADATA_DIR)
        .join(MIGRATED_COMMAND_SKILLS_DIR);
    if target_skills.is_dir() {
        fs::remove_dir_all(&target_skills)?;
    } else if target_skills.exists() {
        fs::remove_file(&target_skills)?;
    }
    import_commands_from_source(
        &plugin_root.join(PLUGIN_COMMANDS_DIR),
        &target_skills,
        CommandMigrationSource::Plugin,
    )?;
    Ok(())
}

pub(crate) fn migrated_command_skills_root(plugin_root: &AbsolutePathBuf) -> AbsolutePathBuf {
    plugin_root
        .join(PLUGIN_METADATA_DIR)
        .join(MIGRATED_COMMAND_SKILLS_DIR)
}

fn unique_supported_command_sources(source_commands: &Path) -> io::Result<Vec<(PathBuf, String)>> {
    let mut by_name = BTreeMap::<String, Vec<PathBuf>>::new();
    for source_file in command_source_files(source_commands)? {
        let document = parse_command(&source_file)?;
        let Some(name) = command_skill_name_if_supported(source_commands, &source_file, &document)
        else {
            continue;
        };
        by_name.entry(name).or_default().push(source_file);
    }

    Ok(by_name
        .into_iter()
        .filter_map(|(name, source_files)| {
            let [source_file] = source_files.as_slice() else {
                return None;
            };
            Some((source_file.clone(), name))
        })
        .collect())
}

fn command_source_files(source_commands: &Path) -> io::Result<Vec<PathBuf>> {
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

fn command_skill_name(source_commands: &Path, source_file: &Path) -> String {
    slugify_name(&format!(
        "{COMMAND_SKILL_PREFIX}-{}",
        command_source_name(source_commands, source_file)
    ))
}

fn command_skill_name_if_supported(
    source_commands: &Path,
    source_file: &Path,
    document: &ParsedCommand,
) -> Option<String> {
    if source_file.file_stem().and_then(|stem| stem.to_str()) == Some("README") {
        return None;
    }
    document
        .description
        .as_deref()
        .filter(|description| !description.trim().is_empty())?;
    let name = command_skill_name(source_commands, source_file);
    if name.chars().count() > MAX_SKILL_NAME_LEN
        || has_unsupported_command_template_features(&document.body)
    {
        return None;
    }
    Some(name)
}

fn command_source_name(source_commands: &Path, source_file: &Path) -> String {
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
    migration_source: CommandMigrationSource,
) -> String {
    let body = migration_source.rewrite_content(body.trim());
    let template_body = if body.is_empty() {
        "No command template body was found.".to_string()
    } else {
        body
    };
    format!(
        "---\nname: {}\ndescription: {}\n---\n\n# {name}\n\nUse this skill when the user asks to run the migrated source command `{source_name}`.\n\n## Command Template\n\n{template_body}\n",
        yaml_string(name),
        yaml_string(&migration_source.rewrite_content(description)),
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
    template
        .as_bytes()
        .windows(2)
        .any(|window| window[0] == b'$' && window[1].is_ascii_digit())
}

fn yaml_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn slugify_name(value: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash {
            slug.push('-');
            last_was_dash = true;
        }
    }

    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "migrated".to_string()
    } else {
        slug
    }
}

fn rewrite_external_agent_terms(content: &str) -> String {
    let mut rewritten = replace_case_insensitive_with_boundaries(
        content,
        &format!("{SOURCE_EXTERNAL_AGENT_NAME}.md"),
        "AGENTS.md",
    );
    for from in [
        format!("{SOURCE_EXTERNAL_AGENT_NAME} code"),
        format!("{SOURCE_EXTERNAL_AGENT_NAME}-code"),
        format!("{SOURCE_EXTERNAL_AGENT_NAME}_code"),
        format!("{SOURCE_EXTERNAL_AGENT_NAME}code"),
        SOURCE_EXTERNAL_AGENT_NAME.to_string(),
    ] {
        rewritten = replace_case_insensitive_with_boundaries(&rewritten, &from, "Codex");
    }
    rewritten
}

fn replace_case_insensitive_with_boundaries(
    input: &str,
    needle: &str,
    replacement: &str,
) -> String {
    let needle_lower = needle.to_ascii_lowercase();
    if needle_lower.is_empty() {
        return input.to_string();
    }

    let haystack_lower = input.to_ascii_lowercase();
    let bytes = input.as_bytes();
    let mut output = String::with_capacity(input.len());
    let mut last_emitted = 0usize;
    let mut search_start = 0usize;

    while let Some(relative_pos) = haystack_lower[search_start..].find(&needle_lower) {
        let start = search_start + relative_pos;
        let end = start + needle_lower.len();
        let boundary_before = start == 0 || !is_word_byte(bytes[start - 1]);
        let boundary_after = end == bytes.len() || !is_word_byte(bytes[end]);

        if boundary_before && boundary_after {
            output.push_str(&input[last_emitted..start]);
            output.push_str(replacement);
            last_emitted = end;
        }

        search_start = start + 1;
    }

    if last_emitted == 0 {
        return input.to_string();
    }

    output.push_str(&input[last_emitted..]);
    output
}

fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}
