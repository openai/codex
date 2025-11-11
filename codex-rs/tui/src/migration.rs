use chrono::Local;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub(crate) struct MigrationWorkspace {
    pub dir_path: PathBuf,
    pub plan_path: PathBuf,
    pub progress_log_path: PathBuf,
}

pub(crate) fn prepare_workspace(root: &Path, label: &str) -> io::Result<MigrationWorkspace> {
    let trimmed = label.trim();
    let now = Local::now();
    let slug = slugify_label(trimmed);
    let base_dir_name = if slug.is_empty() {
        format!("migration_{}", now.format("%Y%m%d-%H%M%S"))
    } else {
        format!("migration_{slug}")
    };
    let (dir_name, dir_path) = next_available_dir(root, &base_dir_name);
    fs::create_dir_all(&dir_path)?;
    let created_label = now.format("%Y-%m-%d %H:%M:%S %Z").to_string();

    let plan_path = dir_path.join("plan.md");
    if !plan_path.exists() {
        fs::write(
            &plan_path,
            initial_plan_template(trimmed, &dir_name, &created_label),
        )?;
    }

    let progress_log_path = dir_path.join("progress_log.md");
    if !progress_log_path.exists() {
        fs::write(
            &progress_log_path,
            progress_log_template(trimmed, &created_label),
        )?;
    }

    Ok(MigrationWorkspace {
        dir_path,
        plan_path,
        progress_log_path,
    })
}

pub(crate) fn slugify_label(label: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;
    for ch in label.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            last_was_dash = false;
        } else if matches!(ch, ' ' | '\t' | '-' | '_' | '/' | '.' | ':' | '+' | '&') {
            if !slug.is_empty() && !last_was_dash {
                slug.push('-');
                last_was_dash = true;
            }
        } else if !slug.is_empty() && !last_was_dash {
            slug.push('-');
            last_was_dash = true;
        }
    }
    slug.trim_matches('-').to_string()
}

fn next_available_dir(root: &Path, base_name: &str) -> (String, PathBuf) {
    let mut counter = 1usize;
    loop {
        let candidate_name = if counter == 1 {
            base_name.to_string()
        } else {
            format!("{base_name}-{counter}")
        };
        let candidate_path = root.join(&candidate_name);
        if !candidate_path.exists() {
            return (candidate_name, candidate_path);
        }
        counter += 1;
    }
}

fn initial_plan_template(label: &str, dir_name: &str, created_ts: &str) -> String {
    format!(
        "# Migration Plan: {label}\n\n_Seeded {created_ts} via `/migrate` (workspace `{dir_name}`)._\n\nUse this document as the canonical playbook. Capture:\n- the current vs. target architecture, data contracts, and release gating.\n- readiness checks before each phase starts.\n- numbered tasks with owners, dependencies, validation, and rollback notes.\n- workstream handoffs plus links to artifacts produced in `progress_log.md`.\n\n## Context\n- Current state:\n- Target state:\n- Non-goals:\n\n## Readiness Gates\n1. _Document prerequisites here._\n\n## Phased Execution Plan\n<!-- Expand each phase with entry criteria, tasks, validation, and exit signals. -->\n\n## Parallel Workstreams\n| Workstream | Objective | Dependencies | Sync Artifacts |\n| --- | --- | --- | --- |\n\n## Rollout & Rollback\n- Rollout steps:\n- Observability & SLOs:\n- Abort conditions + rollback path:\n\n## Post-migration Hardening\n- Follow-up tasks:\n- Success metrics:\n"
    )
}

fn progress_log_template(label: &str, created_ts: &str) -> String {
    format!(
        "# Progress Log: {label}\n\nUse this log so agents can publish async updates other workstreams can learn from. Each row should be timestamped and link to the artifacts or PRs created.\n\n| Timestamp | Owner | Workstream | Update | Next Step |\n| --- | --- | --- | --- | --- |\n| {created_ts} | system | kickoff | Workspace initialized via `/migrate`. | Draft initial migration plan. |\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn slugify_handles_symbols() {
        assert_eq!(
            slugify_label("Payments & Billing 2.0 / EU"),
            "payments-billing-2-0-eu"
        );
    }

    #[test]
    fn slugify_trims_redundant_dashes() {
        assert_eq!(slugify_label("   --alpha--beta--  "), "alpha-beta");
    }

    #[test]
    fn prepare_workspace_creates_structure() {
        let temp = tempdir().unwrap();
        let workspace = prepare_workspace(temp.path(), "Replatform Search").unwrap();
        let dir_name = workspace
            .dir_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert!(dir_name.starts_with("migration_replatform-search"));
        assert!(workspace.dir_path.exists());
        assert!(workspace.plan_path.exists());
        assert!(workspace.progress_log_path.exists());
        let plan = fs::read_to_string(workspace.plan_path).unwrap();
        assert!(plan.contains("Migration Plan: Replatform Search"));
    }

    #[test]
    fn prepare_workspace_appends_suffix_when_needed() {
        let temp = tempdir().unwrap();
        let first = prepare_workspace(temp.path(), "Observability").unwrap();
        let second = prepare_workspace(temp.path(), "Observability").unwrap();
        let first_name = first
            .dir_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let second_name = second
            .dir_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert_ne!(first_name, second_name);
        assert!(second_name.ends_with("-2"));
    }

    #[test]
    fn prepare_workspace_handles_symbol_only_label() {
        let temp = tempdir().unwrap();
        let workspace = prepare_workspace(temp.path(), "***").unwrap();
        let dir_name = workspace
            .dir_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert!(dir_name.starts_with("migration_"));
    }
}
