use std::path::PathBuf;

use codex_state::RuntimeDbMigrationInspection;
use codex_state::RuntimeDbMigrationIssue;
use codex_state::RuntimeDbMigrationStatus;
use pretty_assertions::assert_eq;

use super::super::DoctorReport;
use super::super::output::HumanOutputOptions;
use super::super::output::render_human_report;
use super::*;

#[test]
fn checksum_mismatch_is_fixable_failure() {
    let path = PathBuf::from("/tmp/state_5.sqlite");
    let inspections = vec![RuntimeDbMigrationInspection {
        label: "state DB",
        path: path.clone(),
        status: RuntimeDbMigrationStatus::Incompatible(vec![
            RuntimeDbMigrationIssue::ChecksumMismatch { version: 36 },
        ]),
    }];

    let check = migration_check_from_inspections(inspections.clone());

    assert_eq!(check.id, "state.migrations");
    assert_eq!(check.status, CheckStatus::Fail);
    assert_eq!(
        check.summary,
        "state database migration history is incompatible"
    );
    assert_eq!(incompatible_database_paths(&inspections), vec![path]);
    assert_eq!(check.issues.len(), 1);
    assert_eq!(
        check.remediation.as_deref(),
        Some(
            "Run `codex doctor --fix` to stop Codex app servers, back up the affected database, and rebuild it."
        )
    );
}

#[test]
fn compatible_and_missing_databases_stay_ok() {
    let inspections = vec![
        RuntimeDbMigrationInspection {
            label: "state DB",
            path: PathBuf::from("state_5.sqlite"),
            status: RuntimeDbMigrationStatus::Compatible {
                applied: 37,
                pending: 0,
            },
        },
        RuntimeDbMigrationInspection {
            label: "log DB",
            path: PathBuf::from("logs_2.sqlite"),
            status: RuntimeDbMigrationStatus::Missing,
        },
    ];

    let check = migration_check_from_inspections(inspections);

    assert_eq!(check.status, CheckStatus::Ok);
    assert_eq!(
        check.summary,
        "state database migration history is compatible"
    );
}

#[test]
fn migration_conflict_has_human_snapshot_coverage() {
    let check = migration_check_from_inspections(vec![RuntimeDbMigrationInspection {
        label: "state DB",
        path: PathBuf::from("/Users/example/.codex/state_5.sqlite"),
        status: RuntimeDbMigrationStatus::Incompatible(vec![
            RuntimeDbMigrationIssue::ChecksumMismatch { version: 36 },
        ]),
    }]);
    let report = DoctorReport {
        schema_version: 1,
        generated_at: "2026-06-15T12:00:00Z".to_string(),
        overall_status: CheckStatus::Fail,
        codex_version: "0.0.0-test".to_string(),
        checks: vec![check],
    };

    let rendered = render_human_report(
        &report,
        HumanOutputOptions {
            show_details: true,
            show_all: false,
            ascii: true,
            color_enabled: false,
        },
    );

    insta::assert_snapshot!("doctor_migration_conflict", rendered);
}
