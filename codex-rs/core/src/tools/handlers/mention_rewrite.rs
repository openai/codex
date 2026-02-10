use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use codex_protocol::parse_command::ParsedCommand;

use crate::connectors;
use crate::features::Feature;
use crate::mentions::build_connector_slug_counts;
use crate::mentions::build_skill_name_counts;
use crate::parse_command::parse_command;
use crate::skills::SkillMetadata;
use crate::skills::injection::MentionRewriteContext;
use crate::skills::injection::build_mention_rewrite_context;
use crate::skills::injection::rewrite_text_mentions;

pub(crate) async fn mention_rewrite_context_for_read_paths(
    session: &crate::codex::Session,
    turn: &crate::codex::TurnContext,
    read_paths: Vec<PathBuf>,
) -> Option<MentionRewriteContext> {
    if read_paths.is_empty() {
        return None;
    }

    let skills_outcome = session
        .services
        .skills_manager
        .skills_for_cwd(&turn.cwd, false)
        .await;
    if skills_outcome.skills.is_empty() {
        return None;
    }

    let canonical_paths = read_paths
        .into_iter()
        .map(|path| dunce::canonicalize(&path).unwrap_or(path))
        .collect::<Vec<_>>();
    if !canonical_paths
        .iter()
        .any(|path| should_rewrite_mentions_for_path(path, &skills_outcome.skills))
    {
        return None;
    }

    let (skill_name_counts, skill_name_counts_lower) =
        build_skill_name_counts(&skills_outcome.skills, &skills_outcome.disabled_paths);
    let connectors = if turn.config.features.enabled(Feature::Apps) {
        let mcp_tools = session
            .services
            .mcp_connection_manager
            .read()
            .await
            .list_all_tools()
            .await;
        connectors::accessible_connectors_from_mcp_tools(&mcp_tools)
    } else {
        Vec::new()
    };
    let connector_slug_counts = build_connector_slug_counts(&connectors);

    Some(build_mention_rewrite_context(
        &skills_outcome.skills,
        &skills_outcome.disabled_paths,
        &skill_name_counts,
        &skill_name_counts_lower,
        &connector_slug_counts,
        &connectors,
    ))
}

pub(crate) async fn mention_rewrite_context_for_command_reads(
    session: &crate::codex::Session,
    turn: &crate::codex::TurnContext,
    command: &[String],
    cwd: &Path,
) -> Option<MentionRewriteContext> {
    let read_paths = command_read_paths(command, cwd);
    mention_rewrite_context_for_read_paths(session, turn, read_paths).await
}

pub(crate) fn command_read_paths(command: &[String], cwd: &Path) -> Vec<PathBuf> {
    parse_command(command)
        .into_iter()
        .filter_map(|parsed| match parsed {
            ParsedCommand::Read { path, .. } => {
                if path.is_absolute() {
                    Some(path)
                } else {
                    Some(cwd.join(path))
                }
            }
            _ => None,
        })
        .collect()
}

pub(crate) fn should_rewrite_mentions_for_path(path: &Path, skills: &[SkillMetadata]) -> bool {
    if path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("SKILL.md"))
    {
        return true;
    }

    skills.iter().any(|skill| {
        skill
            .path
            .parent()
            .is_some_and(|skill_dir| path.starts_with(skill_dir))
    })
}

pub(crate) fn rewrite_text_with_mentions(text: &str, context: &MentionRewriteContext) -> String {
    let mut explicit_app_paths = HashSet::new();
    rewrite_text_mentions(text, context, &mut explicit_app_paths)
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::path::PathBuf;

    use codex_protocol::protocol::SkillScope;
    use pretty_assertions::assert_eq;

    use crate::skills::SkillMetadata;

    use super::command_read_paths;
    use super::should_rewrite_mentions_for_path;

    fn make_skill(name: &str, path: &str) -> SkillMetadata {
        SkillMetadata {
            name: name.to_string(),
            description: format!("{name} skill"),
            short_description: None,
            interface: None,
            dependencies: None,
            path: PathBuf::from(path),
            scope: SkillScope::User,
        }
    }

    #[test]
    fn should_rewrite_mentions_for_skill_paths() {
        let skills = vec![make_skill("alpha-skill", "/tmp/skills/alpha/SKILL.md")];

        assert_eq!(
            true,
            should_rewrite_mentions_for_path(Path::new("/tmp/skills/alpha/SKILL.md"), &skills)
        );
        assert_eq!(
            true,
            should_rewrite_mentions_for_path(
                Path::new("/tmp/skills/alpha/references/secondary_context.md"),
                &skills,
            )
        );
        assert_eq!(
            true,
            should_rewrite_mentions_for_path(Path::new("/tmp/random/SKILL.md"), &skills)
        );
        assert_eq!(
            false,
            should_rewrite_mentions_for_path(Path::new("/tmp/random/README.md"), &skills)
        );
    }

    #[test]
    fn command_read_paths_extracts_reads_from_shell_command() {
        let command = vec![
            "bash".to_string(),
            "-lc".to_string(),
            "cat SKILL.md".to_string(),
        ];
        let paths = command_read_paths(&command, Path::new("/tmp/skills/alpha"));

        assert_eq!(paths, vec![PathBuf::from("/tmp/skills/alpha/SKILL.md")]);
    }
}
