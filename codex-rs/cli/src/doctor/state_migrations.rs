//! Reports runtime database migration compatibility without applying migrations.

use std::path::PathBuf;

use codex_core::config::Config;
use codex_state::RuntimeDbMigrationInspection;
use codex_state::RuntimeDbMigrationIssue;
use codex_state::RuntimeDbMigrationStatus;

use super::CheckStatus;
use super::DoctorCheck;
use super::DoctorIssue;

pub(super) async fn migration_check(config: &Config) -> DoctorCheck {
    migration_check_from_inspections(
        codex_state::inspect_runtime_db_migrations(&config.sqlite_home).await,
    )
}

pub(super) fn incompatible_database_paths(
    inspections: &[RuntimeDbMigrationInspection],
) -> Vec<PathBuf> {
    inspections
        .iter()
        .filter(|&inspection| {
            matches!(inspection.status, RuntimeDbMigrationStatus::Incompatible(_))
        })
        .map(|inspection| inspection.path.clone())
        .collect()
}

pub(super) fn migration_check_from_inspections(
    inspections: Vec<RuntimeDbMigrationInspection>,
) -> DoctorCheck {
    let mut details = Vec::new();
    let mut issues = Vec::new();
    let mut has_incompatible = false;
    let mut has_unreadable = false;

    for inspection in inspections {
        match inspection.status {
            RuntimeDbMigrationStatus::Missing => {
                details.push(format!(
                    "{} migrations: skipped (missing)",
                    inspection.label
                ));
            }
            RuntimeDbMigrationStatus::Compatible { applied, pending } => {
                details.push(format!(
                    "{} migrations: compatible ({applied} applied, {pending} pending)",
                    inspection.label
                ));
            }
            RuntimeDbMigrationStatus::Incompatible(db_issues) => {
                has_incompatible = true;
                details.push(format!(
                    "{} migration database: {}",
                    inspection.label,
                    inspection.path.display()
                ));
                for issue in db_issues {
                    let cause = migration_issue_cause(inspection.label, &issue);
                    details.push(format!("{} migrations: {cause}", inspection.label));
                    issues.push(
                        DoctorIssue::new(CheckStatus::Fail, cause)
                            .measured("applied migration metadata")
                            .expected("embedded migration metadata")
                            .remedy("run codex doctor --fix")
                            .field(format!("{} migrations", inspection.label)),
                    );
                }
            }
            RuntimeDbMigrationStatus::Unreadable(error) => {
                has_unreadable = true;
                details.push(format!(
                    "{} migrations: unreadable ({error})",
                    inspection.label
                ));
            }
        }
    }

    let (status, summary) = if has_incompatible {
        (
            CheckStatus::Fail,
            "state database migration history is incompatible",
        )
    } else if has_unreadable {
        (
            CheckStatus::Fail,
            "state database migration history could not be inspected",
        )
    } else {
        (
            CheckStatus::Ok,
            "state database migration history is compatible",
        )
    };
    let mut check =
        DoctorCheck::new("state.migrations", "migrations", status, summary).details(details);
    for issue in issues {
        check = check.issue(issue);
    }
    if has_incompatible {
        check = check.remediation(
            "Run `codex doctor --fix` to stop Codex app servers, back up the affected database, and rebuild it.",
        );
    } else if has_unreadable {
        check = check
            .remediation("Resolve the reported SQLite access error, then rerun `codex doctor`.");
    }
    check
}

fn migration_issue_cause(label: &str, issue: &RuntimeDbMigrationIssue) -> String {
    match issue {
        RuntimeDbMigrationIssue::Dirty { version } => {
            format!("{label} migration {version} is partially applied")
        }
        RuntimeDbMigrationIssue::ChecksumMismatch { version } => {
            format!("{label} migration {version} was previously applied but has been modified")
        }
    }
}

#[cfg(test)]
#[path = "state_migrations_tests.rs"]
mod tests;
