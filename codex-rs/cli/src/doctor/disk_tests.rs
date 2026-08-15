use std::cell::RefCell;
use std::io;
use std::path::Path;

use pretty_assertions::assert_eq;
use serde_json::json;

use super::CheckStatus;
use super::FAIL_THRESHOLD;
use super::GIB;
use super::WARNING_THRESHOLD;
use super::check;
use super::check_with_paths;

#[test]
fn disk_capacity_is_reported_without_loaded_configuration() {
    let check = check(/*config*/ None, Path::new("."));

    for label in ["CODEX_HOME available: ", "worktree available: "] {
        assert!(
            check
                .details
                .iter()
                .any(|detail| detail.starts_with(label) && !detail.contains("unavailable"))
        );
    }
}

#[test]
fn disk_capacity_thresholds_produce_complete_diagnostics() {
    for (available, status, measured, summary) in [
        (0, "fail", "0.0 MiB", "critically low disk space (0.0 MiB)"),
        (
            FAIL_THRESHOLD - 1,
            "fail",
            "1024.0 MiB",
            "critically low disk space (1024.0 MiB)",
        ),
        (
            FAIL_THRESHOLD,
            "warning",
            "1.0 GiB",
            "low disk space (1.0 GiB)",
        ),
        (
            WARNING_THRESHOLD - 1,
            "warning",
            "5.0 GiB",
            "low disk space (5.0 GiB)",
        ),
        (
            WARNING_THRESHOLD,
            "ok",
            "5.0 GiB",
            "sufficient free disk space (5.0 GiB)",
        ),
        (
            WARNING_THRESHOLD + GIB,
            "ok",
            "6.0 GiB",
            "sufficient free disk space (6.0 GiB)",
        ),
    ] {
        let issues = ["CODEX_HOME", "worktree"]
            .into_iter()
            .filter(|_| status != "ok")
            .map(|label| {
                json!({
                    "severity": status,
                    "cause": format!("{label} has insufficient disk space"),
                    "measured": measured,
                    "expected": "at least 5.0 GiB",
                    "remedy": "Free disk space or move the worktree to a larger volume.",
                    "fields": [format!("{label} available")],
                })
            })
            .collect::<Vec<_>>();

        let check = check_with_paths(Some(Path::new(".")), Path::new("."), |_| Ok(available));

        assert_eq!(
            serde_json::to_value(check).unwrap(),
            json!({
                "id": "system.disk",
                "category": "disk",
                "status": status,
                "summary": summary,
                "details": [
                    "warning threshold: 5.0 GiB",
                    "failure threshold: 1.0 GiB",
                    format!("CODEX_HOME available: {measured}"),
                    format!("worktree available: {measured}"),
                ],
                "issues": issues,
                "remediation": null,
                "durationMs": 0,
            }),
            "available bytes: {available}"
        );
    }
}

#[test]
fn disk_measurement_errors_produce_complete_warning_diagnostics() {
    let check = check_with_paths(Some(Path::new(".")), Path::new("."), |_| {
        Err(io::ErrorKind::PermissionDenied.into())
    });

    let issues = ["CODEX_HOME", "worktree"]
        .into_iter()
        .map(|label| {
            json!({
                "severity": "warning",
                "cause": format!("disk space for {label} could not be checked"),
                "measured": "PermissionDenied",
                "expected": "readable filesystem capacity",
                "remedy": "Check filesystem access and available disk space.",
                "fields": [format!("{label} available")],
            })
        })
        .collect::<Vec<_>>();

    assert_eq!(
        serde_json::to_value(check).unwrap(),
        json!({
            "id": "system.disk",
            "category": "disk",
            "status": "warning",
            "summary": "disk capacity could not be fully verified",
            "details": [
                "warning threshold: 5.0 GiB",
                "failure threshold: 1.0 GiB",
                "CODEX_HOME available: unavailable (PermissionDenied)",
                "worktree available: unavailable (PermissionDenied)",
            ],
            "issues": issues,
            "remediation": null,
            "durationMs": 0,
        })
    );
}

#[test]
fn nonexistent_relative_codex_home_uses_current_directory_capacity() {
    let measured_paths = RefCell::new(Vec::new());
    let check = check_with_paths(
        Some(Path::new("doctor-nonexistent-relative-home/nested")),
        Path::new("."),
        |path| {
            measured_paths.borrow_mut().push(path.to_path_buf());
            Ok(WARNING_THRESHOLD)
        },
    );

    assert_eq!(check.status, CheckStatus::Ok);
    assert_eq!(
        measured_paths.into_inner(),
        [std::env::current_dir().unwrap(), Path::new(".").into()]
    );
}
