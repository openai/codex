use codex_external_agent_migration::CommandDescriptionMode;
use codex_external_agent_migration::CommandMigrationProfile;
use codex_external_agent_migration::RewriteProfile;
use codex_external_agent_migration::build_mcp_config_from_json_file;
use codex_external_agent_migration::count_missing_commands_with_profile;
use codex_external_agent_migration::import_commands_with_profile;
use codex_external_agent_migration::import_subagents_with_rewrite_profile;
use codex_external_agent_migration::missing_command_names_with_profile;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use toml::Value as TomlValue;

use super::is_non_empty_text_file;
use super::source::InstructionSourceGroup;

pub(super) const CONFIG_DIR: &str = ".cursor";
pub(super) const LEGACY_RULES_FILE: &str = ".cursorrules";
pub(super) const REWRITE_PROFILE: RewriteProfile = RewriteProfile::new(
    LEGACY_RULES_FILE,
    &[
        "cursor agent",
        "cursor-agent",
        "cursor_agent",
        "cursoragent",
    ],
)
.with_case_sensitive_term_variants(&["Cursor"]);
const COMMAND_MIGRATION_PROFILE: CommandMigrationProfile = CommandMigrationProfile::new(
    REWRITE_PROFILE,
    CommandDescriptionMode::UseSourceNameFallback,
);

pub(super) fn build_mcp_config(source_dir: &Path) -> io::Result<TomlValue> {
    build_mcp_config_from_json_file(&source_dir.join("mcp.json"))
}

pub(super) fn repo_instruction_source_groups(
    repo_root: &Path,
) -> io::Result<Vec<InstructionSourceGroup>> {
    let mut scopes = rule_scope_directories(repo_root)?;
    if !scopes.iter().any(|scope| scope == repo_root) {
        scopes.push(repo_root.to_path_buf());
    }
    scopes.sort();
    scopes.dedup();

    let mut groups = Vec::new();
    for scope in scopes {
        let legacy_rules = (scope == repo_root).then(|| repo_root.join(LEGACY_RULES_FILE));
        let sources = instruction_sources(legacy_rules, &scope.join(CONFIG_DIR).join("rules"))?;
        if !sources.is_empty() {
            groups.push(InstructionSourceGroup { scope, sources });
        }
    }
    Ok(groups)
}

pub(super) fn home_instruction_sources(external_agent_home: &Path) -> io::Result<Vec<PathBuf>> {
    instruction_sources(
        /*legacy_rules*/ None,
        &external_agent_home.join("rules"),
    )
}

pub(super) fn read_instruction_source(path: &Path) -> io::Result<String> {
    let contents = fs::read_to_string(path)?;
    if path.extension().and_then(|extension| extension.to_str()) == Some("mdc") {
        Ok(strip_markdown_frontmatter(&contents).to_string())
    } else {
        Ok(contents)
    }
}

pub(super) fn import_source_commands(
    source_commands: &Path,
    target_skills: &Path,
) -> io::Result<Vec<String>> {
    import_commands_with_profile(source_commands, target_skills, COMMAND_MIGRATION_PROFILE)
}

pub(super) fn count_missing_source_commands(
    source_commands: &Path,
    target_skills: &Path,
) -> io::Result<usize> {
    count_missing_commands_with_profile(source_commands, target_skills, COMMAND_MIGRATION_PROFILE)
}

pub(super) fn missing_source_command_names(
    source_commands: &Path,
    target_skills: &Path,
) -> io::Result<Vec<String>> {
    missing_command_names_with_profile(source_commands, target_skills, COMMAND_MIGRATION_PROFILE)
}

pub(super) fn import_source_subagents(
    source_agents: &Path,
    target_agents: &Path,
) -> io::Result<Vec<String>> {
    import_subagents_with_rewrite_profile(source_agents, target_agents, REWRITE_PROFILE)
}

fn instruction_sources(
    legacy_rules: Option<PathBuf>,
    rules_dir: &Path,
) -> io::Result<Vec<PathBuf>> {
    let mut sources = Vec::new();
    if let Some(legacy_rules) = legacy_rules
        && is_non_empty_text_file(&legacy_rules)?
    {
        sources.push(legacy_rules);
    }

    collect_rule_sources(rules_dir, &mut sources)?;
    sources.sort();
    Ok(sources)
}

fn rule_scope_directories(repo_root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut scopes = Vec::new();
    let mut pending = vec![repo_root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(err) if directory == repo_root => return Err(err),
            Err(_) => continue,
        };
        for entry in entries {
            let Ok(entry) = entry else {
                continue;
            };
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if !file_type.is_dir() {
                continue;
            }
            let path = entry.path();
            if entry.file_name().to_str() == Some(CONFIG_DIR) {
                if path.join("rules").is_dir() {
                    scopes.push(directory.clone());
                }
                continue;
            }
            if !should_skip_rule_scope_directory(&path) {
                pending.push(path);
            }
        }
    }
    scopes.sort();
    scopes.dedup();
    Ok(scopes)
}

fn should_skip_rule_scope_directory(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| matches!(name, ".git" | ".hg" | ".svn" | "node_modules" | "target"))
}

fn collect_rule_sources(rules_dir: &Path, sources: &mut Vec<PathBuf>) -> io::Result<()> {
    if !rules_dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(rules_dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_rule_sources(&path, sources)?;
            continue;
        }
        if !file_type.is_file()
            || !matches!(
                path.extension().and_then(|extension| extension.to_str()),
                Some("md" | "mdc")
            )
        {
            continue;
        }
        let contents = fs::read_to_string(&path)?;
        if rule_is_always_applied(&contents)
            && !strip_markdown_frontmatter(&contents).trim().is_empty()
        {
            sources.push(path);
        }
    }
    Ok(())
}

fn rule_is_always_applied(contents: &str) -> bool {
    let Some((frontmatter, _body)) = split_markdown_frontmatter(contents) else {
        return false;
    };
    frontmatter.lines().any(|line| {
        line.split_once(':').is_some_and(|(key, value)| {
            key.trim() == "alwaysApply"
                && yaml_scalar_without_comment(value).eq_ignore_ascii_case("true")
        })
    })
}

fn split_markdown_frontmatter(contents: &str) -> Option<(&str, &str)> {
    let rest = contents.strip_prefix("---")?;
    let rest = rest
        .strip_prefix("\r\n")
        .or_else(|| rest.strip_prefix('\n'))?;
    let mut frontmatter_end = 0usize;
    for line in rest.split_inclusive('\n') {
        let line_contents = line.trim_end_matches(['\r', '\n']);
        if line_contents == "---" {
            let body = &rest[frontmatter_end + line.len()..];
            return Some((&rest[..frontmatter_end], body));
        }
        frontmatter_end += line.len();
    }
    None
}

fn strip_markdown_frontmatter(contents: &str) -> &str {
    split_markdown_frontmatter(contents).map_or(contents, |(_frontmatter, body)| body)
}

fn yaml_scalar_without_comment(value: &str) -> &str {
    let value = value.trim();
    let comment_start = value.char_indices().find_map(|(index, character)| {
        (character == '#'
            && value[..index]
                .chars()
                .next_back()
                .is_some_and(char::is_whitespace))
        .then_some(index)
    });
    comment_start.map_or(value, |comment_start| value[..comment_start].trim_end())
}
