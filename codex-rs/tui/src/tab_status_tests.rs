use std::path::PathBuf;

use codex_protocol::parse_command::ParsedCommand;
use crossterm::Command;
use pretty_assertions::assert_eq;

use super::ClearTabStatus;
use super::MAX_TAB_STATUS_DETAIL_CHARS;
use super::SetTabStatus;
use super::TabStatus;
use super::format_command_for_tab_status;
use super::format_parsed_command_for_tab_status;
use super::oneline_truncated;
use super::sanitize_detail;

#[test]
fn status_sequences_include_detail_and_clear_stale_fields() {
    let mut working = String::new();
    SetTabStatus(TabStatus::Working, Some("exec cargo build".to_string()))
        .write_ansi(&mut working)
        .expect("encode tab status");
    assert_eq!(
        working,
        "\x1b]21337;status=Working;indicator=#ff9500;status-color=#ff9500;detail=exec cargo build\x07"
    );

    let mut idle = String::new();
    SetTabStatus(TabStatus::Idle, /*detail*/ None)
        .write_ansi(&mut idle)
        .expect("encode tab status");
    assert_eq!(
        idle,
        "\x1b]21337;status=Idle;indicator=#00d75f;status-color=#888888;detail=\x07"
    );

    let mut clear = String::new();
    ClearTabStatus.write_ansi(&mut clear).expect("encode clear");
    assert_eq!(
        clear,
        "\x1b]21337;status=;indicator=;status-color=;detail=\x07"
    );
}

#[test]
fn detail_is_safe_bounded_osc_text() {
    assert_eq!(sanitize_detail(" a\\;b\t\u{202E}c\x1b "), "a\\\\\\;b c");

    let sanitized = sanitize_detail(&";".repeat(MAX_TAB_STATUS_DETAIL_CHARS + 1));
    assert_eq!(
        sanitized.chars().count(),
        MAX_TAB_STATUS_DETAIL_CHARS * 2 + 1
    );
    assert!(sanitized.ends_with('…'));

    let sanitized = sanitize_detail(&format!(
        "{}{}tail",
        "x".repeat(MAX_TAB_STATUS_DETAIL_CHARS),
        " \u{202E}".repeat(/*n*/ 10_000)
    ));
    assert_eq!(
        sanitized,
        format!("{}…", "x".repeat(MAX_TAB_STATUS_DETAIL_CHARS))
    );
}

#[test]
fn command_detail_strips_shell_wrapper() {
    assert_eq!(
        format_command_for_tab_status(&[
            "/opt/homebrew/bin/zsh".into(),
            "-lc".into(),
            "touch".into(),
            "/tmp/foo".into(),
        ]),
        "touch /tmp/foo"
    );
}

#[test]
fn parsed_detail_combines_reads_and_caps_the_list() {
    let parsed = ["a.rs", "b.rs", "c.rs", "d.rs"].map(|name| ParsedCommand::Read {
        cmd: format!("cat {name}"),
        name: name.to_string(),
        path: PathBuf::from(name),
    });
    assert_eq!(
        format_parsed_command_for_tab_status(&parsed),
        Some("Read a.rs, b.rs, c.rs, …".to_string())
    );
}

#[test]
fn parsed_detail_uses_first_mixed_command_and_quotes_search() {
    let parsed = [
        ParsedCommand::Search {
            cmd: "rg needle src".into(),
            query: Some("needle".into()),
            path: Some("src".into()),
        },
        ParsedCommand::Unknown {
            cmd: "ignored".into(),
        },
    ];
    assert_eq!(
        format_parsed_command_for_tab_status(&parsed),
        Some("Search \"needle\" in src".to_string())
    );
}

#[test]
fn unknown_command_detail_collapses_and_truncates() {
    let command = format!("  echo\n{}  ", "x".repeat(/*n*/ 100));
    let summary = format_parsed_command_for_tab_status(&[ParsedCommand::Unknown { cmd: command }])
        .expect("unknown command produces a summary");
    assert_eq!(summary.chars().count(), "Run ".chars().count() + 81);
    assert!(summary.starts_with("Run echo "));
    assert!(summary.ends_with('…'));
}

#[test]
fn oneline_detail_stops_scanning_long_trailing_whitespace() {
    let detail = format!("message{}unreachable", " ".repeat(/*n*/ 10_000));
    assert_eq!(oneline_truncated(&detail), "message…");
}
