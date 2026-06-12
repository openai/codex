use std::path::Path;
use std::path::PathBuf;

use super::collect_verified_or_first_error;
use super::first_available_or_first_error;
use super::first_verified_containing_path;

#[test]
fn package_verification_keeps_healthy_sibling_installations() {
    let result = collect_verified_or_first_error([
        Err("damaged older registration"),
        Ok("healthy current registration"),
    ]);

    assert_eq!(result, Ok(vec!["healthy current registration"]));
}

#[test]
fn package_verification_fails_when_no_registration_is_healthy() {
    let result = collect_verified_or_first_error::<(), _>([
        Err("first damaged registration"),
        Err("second damaged registration"),
    ]);

    assert_eq!(result, Err("first damaged registration"));
}

#[test]
fn discovery_continues_after_an_unhealthy_channel() {
    let result = first_available_or_first_error([
        Err("stable package is unhealthy"),
        Ok(Vec::new()),
        Ok(vec!["healthy alpha package"]),
    ]);

    assert_eq!(result, Ok(Some("healthy alpha package")));
}

#[test]
fn discovery_reports_verification_error_after_all_channels_fail() {
    let result = first_available_or_first_error::<(), _>([
        Err("stable package is unhealthy"),
        Ok(Vec::new()),
        Err("nightly package is unhealthy"),
    ]);

    assert_eq!(result, Err("stable package is unhealthy"));
}

#[derive(Clone)]
struct ContainmentCandidate {
    root: Result<PathBuf, &'static str>,
    verification: Result<&'static str, &'static str>,
}

#[test]
fn current_process_scan_reaches_a_later_healthy_channel() {
    let current_executable = Path::new(r"C:\CodexBeta\app\codex.exe");
    let result = first_verified_containing_path(
        current_executable,
        [
            Err("stale stable query"),
            Ok(vec![ContainmentCandidate {
                root: Err("missing stale installation"),
                verification: Err("must not verify an unresolved root"),
            }]),
            Ok(vec![ContainmentCandidate {
                root: Ok(PathBuf::from(r"C:\OtherCodex")),
                verification: Err("must not verify an unrelated package"),
            }]),
            Ok(vec![ContainmentCandidate {
                root: Ok(PathBuf::from(r"C:\CodexBeta")),
                verification: Ok("healthy beta package"),
            }]),
        ],
        |candidate| candidate.root.clone(),
        |candidate| candidate.verification,
    );

    assert_eq!(result, Ok(Some("healthy beta package")));
}

#[test]
fn current_process_scan_retains_containing_package_failure() {
    let current_executable = Path::new(r"C:\Codex\app\codex.exe");
    let result = first_verified_containing_path(
        current_executable,
        [
            Ok(vec![ContainmentCandidate {
                root: Ok(PathBuf::from(r"C:\Codex")),
                verification: Err("containing package failed integrity validation"),
            }]),
            Ok(vec![ContainmentCandidate {
                root: Ok(PathBuf::from(r"C:\CodexBeta")),
                verification: Ok("unrelated healthy package"),
            }]),
        ],
        |candidate| candidate.root.clone(),
        |candidate| candidate.verification,
    );

    assert_eq!(
        result,
        Err("containing package failed integrity validation")
    );
}
