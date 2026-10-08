use std::io;

use codex_terminal_detection::Multiplexer;
use codex_terminal_detection::TerminalInfo;
use codex_terminal_detection::TerminalName;
use crossterm::Command;
use pretty_assertions::assert_eq;

use super::ClearItermSessionStatus;
use super::ItermSessionStatus;
use super::NO_EMITTED_STATUS;
use super::SessionStatusState;
use super::SetItermSessionStatus;
use super::iterm_session_status_supported;
use super::sanitize_iterm_session_detail;

#[test]
fn encodes_session_status_fields() {
    let mut encoded = Vec::new();
    for (status, detail) in [
        (ItermSessionStatus::Idle, ""),
        (ItermSessionStatus::Working, "Reading source"),
        (ItermSessionStatus::Waiting, ""),
    ] {
        let mut value = String::new();
        SetItermSessionStatus(status, detail)
            .write_ansi(&mut value)
            .expect("encode iTerm2 session status");
        encoded.push(value);
    }

    let snapshot = encoded
        .iter()
        .map(|value| value.escape_debug().to_string())
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!("iterm_session_status_fields", snapshot);
}

#[test]
fn detail_is_safe_bounded_osc_text() {
    assert_eq!(
        sanitize_iterm_session_detail("  Read a\\b; c\n\u{202e}\x1b  "),
        "Read a\\\\b\\; c"
    );
    assert_eq!(
        sanitize_iterm_session_detail(&"x".repeat(/*n*/ 300)),
        "x".repeat(/*n*/ 240)
    );
}

#[test]
fn clear_empties_every_owned_field() {
    let mut encoded = String::new();
    ClearItermSessionStatus
        .write_ansi(&mut encoded)
        .expect("encode iTerm2 session status clear");

    assert_eq!(
        encoded,
        "\x1b]21337;status=;indicator=;status-color=;detail=\x07"
    );
}

#[test]
fn cache_changes_only_after_a_successful_write() {
    let state = SessionStatusState::new();
    let mut emitted = Vec::new();
    state
        .update(ItermSessionStatus::Working as u8, "Thinking", || {
            Err(io::Error::other("write failed"))
        })
        .expect_err("failed write should be returned");

    state
        .update(ItermSessionStatus::Working as u8, "Thinking", || {
            emitted.push(ItermSessionStatus::Working as u8);
            Ok(())
        })
        .expect("retry status write");
    state
        .update(ItermSessionStatus::Working as u8, "Thinking", || {
            emitted.push(ItermSessionStatus::Waiting as u8);
            Ok(())
        })
        .expect("deduplicate status write");
    state
        .update(ItermSessionStatus::Working as u8, "Reading", || {
            emitted.push(ItermSessionStatus::Working as u8);
            Ok(())
        })
        .expect("emit changed detail");
    state.invalidate();
    state
        .update(ItermSessionStatus::Working as u8, "Thinking", || {
            emitted.push(ItermSessionStatus::Working as u8);
            Ok(())
        })
        .expect("re-emit status after terminal handoff");
    state
        .update(NO_EMITTED_STATUS, "", || {
            emitted.push(NO_EMITTED_STATUS);
            Ok(())
        })
        .expect("clear status");

    assert_eq!(
        emitted,
        [
            ItermSessionStatus::Working as u8,
            ItermSessionStatus::Working as u8,
            ItermSessionStatus::Working as u8,
            NO_EMITTED_STATUS,
        ]
    );
}

#[test]
fn supports_only_direct_iterm_ttys() {
    let direct_iterm = TerminalInfo {
        name: TerminalName::Iterm2,
        term_program: Some("iTerm.app".to_string()),
        version: Some("3.7.0".to_string()),
        term: None,
        multiplexer: None,
    };
    assert!(iterm_session_status_supported(
        &direct_iterm,
        /*stdout_is_terminal*/ true,
        /*screen_active*/ false,
    ));
    assert!(!iterm_session_status_supported(
        &direct_iterm,
        /*stdout_is_terminal*/ false,
        /*screen_active*/ false,
    ));
    assert!(!iterm_session_status_supported(
        &direct_iterm,
        /*stdout_is_terminal*/ true,
        /*screen_active*/ true,
    ));

    assert!(!iterm_session_status_supported(
        &TerminalInfo {
            multiplexer: Some(Multiplexer::Tmux { version: None }),
            ..direct_iterm.clone()
        },
        /*stdout_is_terminal*/ true,
        /*screen_active*/ false,
    ));
    assert!(!iterm_session_status_supported(
        &TerminalInfo {
            name: TerminalName::Ghostty,
            ..direct_iterm
        },
        /*stdout_is_terminal*/ true,
        /*screen_active*/ false,
    ));
}
