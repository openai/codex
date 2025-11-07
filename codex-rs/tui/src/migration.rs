use chrono::Local;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

pub const MIGRATION_PROMPT_TEMPLATE: &str = include_str!("../prompt_for_migrate_command.md");
pub const CONTINUE_MIGRATION_PROMPT_TEMPLATE: &str =
    include_str!("../prompt_for_continue_migration_command.md");
const MIGRATION_PLAN_TEMPLATE: &str = include_str!("../migration_plan_template.md");
const MIGRATION_JOURNAL_TEMPLATE: &str = include_str!("../migration_journal_template.md");

#[derive(Debug, Clone)]
pub struct MigrationWorkspace {
    pub dir_path: PathBuf,
    pub dir_name: String,
    pub plan_path: PathBuf,
    pub journal_path: PathBuf,
}

pub fn create_migration_workspace(
    base_dir: &Path,
    summary: &str,
) -> Result<MigrationWorkspace, std::io::Error> {
    fs::create_dir_all(base_dir)?;
    let slug = sanitize_migration_slug(summary);
    let base_name = format!("migration_{slug}");
    let (dir_path, dir_name) = next_available_migration_dir(base_dir, &base_name);
    fs::create_dir_all(&dir_path)?;
    let created_at = Local::now().format("%Y-%m-%d %H:%M %Z").to_string();
    let plan_path = dir_path.join("plan.md");
    let journal_path = dir_path.join("journal.md");
    let replacements = [
        ("{{MIGRATION_SUMMARY}}", summary),
        ("{{WORKSPACE_NAME}}", dir_name.as_str()),
        ("{{CREATED_AT}}", created_at.as_str()),
    ];
    let plan_contents = fill_template(MIGRATION_PLAN_TEMPLATE, &replacements);
    let journal_contents = fill_template(MIGRATION_JOURNAL_TEMPLATE, &replacements);
    fs::write(&plan_path, plan_contents)?;
    fs::write(&journal_path, journal_contents)?;
    Ok(MigrationWorkspace {
        dir_path,
        dir_name,
        plan_path,
        journal_path,
    })
}

pub fn build_migration_prompt(summary: &str) -> String {
    fill_template(
        MIGRATION_PROMPT_TEMPLATE,
        &[("{{MIGRATION_SUMMARY}}", summary)],
    )
}

pub fn build_continue_migration_prompt() -> String {
    CONTINUE_MIGRATION_PROMPT_TEMPLATE.to_string()
}

pub fn sanitize_migration_slug(summary: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = true;
    for ch in summary.trim().to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            last_was_dash = false;
        } else if !last_was_dash {
            slug.push('-');
            last_was_dash = true;
        }
    }
    let mut trimmed = slug.trim_matches('-').to_string();
    if trimmed.len() > 48 {
        trimmed = trimmed
            .chars()
            .take(48)
            .collect::<String>()
            .trim_matches('-')
            .to_string();
    }
    if trimmed.is_empty() {
        return Local::now().format("plan-%Y%m%d-%H%M%S").to_string();
    }
    trimmed
}

fn next_available_migration_dir(base_dir: &Path, base_name: &str) -> (PathBuf, String) {
    let mut candidate_name = base_name.to_string();
    let mut candidate_path = base_dir.join(&candidate_name);
    let mut suffix = 2;
    while candidate_path.exists() {
        candidate_name = format!("{base_name}_{suffix:02}");
        candidate_path = base_dir.join(&candidate_name);
        suffix += 1;
    }
    (candidate_path, candidate_name)
}

fn fill_template(template: &str, replacements: &[(&str, &str)]) -> String {
    let mut filled = template.to_string();
    for (needle, value) in replacements {
        filled = filled.replace(needle, value);
    }
    filled
}

#[cfg(test)]
mod tests {
    use super::sanitize_migration_slug;

    #[test]
    fn slug_sanitizes_whitespace_and_length() {
        let slug = sanitize_migration_slug("  Launch 🚀 Phase #2 migration :: Big Refactor  ");
        assert_eq!(slug, "launch-phase-2-migration-big-refactor");
    }

    #[test]
    fn slug_falls_back_to_timestamp() {
        let slug = sanitize_migration_slug("     ");
        assert!(slug.starts_with("plan-"));
        assert!(slug.len() > 10);
    }
}
