#![cfg(test)]

//! Unit tests for the crate and its related test suites.

use super::LogFormat;
use super::ShutdownAction;
use super::ShutdownSignal;
use super::ShutdownState;
#[cfg(debug_assertions)]
use super::loader_overrides_with_test_user_config_file;
use super::turn_admission::TurnAdmission;
#[cfg(debug_assertions)]
use codex_config::LoaderOverrides;
#[cfg(debug_assertions)]
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;

#[test]
fn shutdown_waits_for_admitted_requests_but_force_remains_available() {
    let admission = TurnAdmission::default();
    let active = admission.subscribe_active();
    let in_flight = admission.admit().expect("request admitted");
    let mut shutdown = ShutdownState::default();
    let connection_count = 1;
    let running_turn_count = 0;
    shutdown.on_signal(
        ShutdownSignal::Forceable,
        connection_count,
        running_turn_count,
        &admission,
    );
    assert!(matches!(
        shutdown.update(running_turn_count, *active.borrow(), connection_count),
        ShutdownAction::Noop
    ));
    shutdown.on_signal(
        ShutdownSignal::Forceable,
        connection_count,
        running_turn_count,
        &admission,
    );
    assert!(shutdown.forced());
    assert!(matches!(
        shutdown.update(running_turn_count, *active.borrow(), connection_count),
        ShutdownAction::Finish
    ));
    drop(in_flight);
}

#[test]
fn log_format_from_env_value_matches_json_values_case_insensitively() {
    assert_eq!(LogFormat::from_env_value(Some("json")), LogFormat::Json);
    assert_eq!(LogFormat::from_env_value(Some("JSON")), LogFormat::Json);
    assert_eq!(LogFormat::from_env_value(Some("  Json  ")), LogFormat::Json);
}

#[test]
fn log_format_from_env_value_defaults_for_non_json_values() {
    assert_eq!(
        LogFormat::from_env_value(/*value*/ None),
        LogFormat::Default
    );
    assert_eq!(LogFormat::from_env_value(Some("")), LogFormat::Default);
    assert_eq!(LogFormat::from_env_value(Some("text")), LogFormat::Default);
    assert_eq!(LogFormat::from_env_value(Some("jsonl")), LogFormat::Default);
}

#[cfg(debug_assertions)]
#[test]
fn debug_test_user_config_file_overrides_loader_path() {
    let path = std::env::temp_dir().join("codex-app-server-test-config.toml");
    let loader_overrides =
        loader_overrides_with_test_user_config_file(LoaderOverrides::default(), Some(path.clone()))
            .expect("test config path should be valid");

    assert_eq!(
        loader_overrides.user_config_path,
        Some(AbsolutePathBuf::from_absolute_path(path).expect("absolute test path"))
    );
}

#[path = "stderr_logging_tests.rs"]
mod stderr_logging_tests;
