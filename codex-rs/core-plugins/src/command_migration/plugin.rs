use super::CommandDescriptionMode;
use super::CommandMigrationProfile;
use super::CommandSkillSizeLimit;
use super::CommandSource;
use super::RewriteProfile;
use super::import_command_sources;
use super::supported_command_sources;
use super::unique_command_sources;
use crate::manifest::load_plugin_command_paths;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::fs;
use std::io;
use std::path::Path;

const PLUGIN_COMMANDS_DIR: &str = "commands";
const PLUGIN_METADATA_DIR: &str = ".codex-plugin";
const MIGRATED_COMMAND_SKILLS_DIR: &str = "migrated-command-skills";
const MAX_MIGRATED_COMMAND_SKILL_BYTES: usize = 4_000;

const PLUGIN_REWRITE_PROFILE: RewriteProfile = RewriteProfile::new("AGENTS.md", &[]);
const PLUGIN_MIGRATION_PROFILE: CommandMigrationProfile = CommandMigrationProfile::new(
    PLUGIN_REWRITE_PROFILE,
    CommandDescriptionMode::RequireFrontmatter,
);

pub(crate) fn migrate_plugin_commands(plugin_root: &Path) -> io::Result<()> {
    let target_skills = plugin_root
        .join(PLUGIN_METADATA_DIR)
        .join(MIGRATED_COMMAND_SKILLS_DIR);
    if target_skills.is_dir() {
        fs::remove_dir_all(&target_skills)?;
    } else if target_skills.exists() {
        fs::remove_file(&target_skills)?;
    }
    import_command_sources(
        plugin_command_sources(plugin_root)?,
        &target_skills,
        PLUGIN_MIGRATION_PROFILE,
        CommandSkillSizeLimit::MaxBytes(MAX_MIGRATED_COMMAND_SKILL_BYTES),
    )?;
    Ok(())
}

pub(crate) fn migrated_command_skills_root(plugin_root: &AbsolutePathBuf) -> AbsolutePathBuf {
    plugin_root
        .join(PLUGIN_METADATA_DIR)
        .join(MIGRATED_COMMAND_SKILLS_DIR)
}

fn plugin_command_sources(plugin_root: &Path) -> io::Result<Vec<CommandSource>> {
    let command_paths = load_plugin_command_paths(plugin_root)?
        .unwrap_or_else(|| vec![plugin_root.join(PLUGIN_COMMANDS_DIR)]);
    let mut sources = Vec::new();
    for command_path in command_paths {
        sources.extend(supported_command_sources(
            &command_path,
            CommandDescriptionMode::RequireFrontmatter,
        )?);
    }
    Ok(unique_command_sources(sources))
}
