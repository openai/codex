use std::io;

use codex_terminal_detection::Multiplexer;
use codex_terminal_detection::TerminalBackend;
use codex_terminal_detection::TerminalInfo;
use codex_terminal_detection::TerminalName;
use crossterm::Command;
use pretty_assertions::assert_eq;

use super::CachedStatus;
use super::ClearTerminalProgramStatus;
use super::ProgramStatus;
use super::ProgramStatusKind;
use super::SessionStatusState;
use super::SetTerminalProgramStatus;
use super::TerminalProgramStatusSupport;
use super::sanitize_iterm_session_detail;
use super::terminal_program_status_support;

#[test]
fn encodes_iterm_session_status_fields() {
    let mut encoded = Vec::new();
    for (status, detail) in [
        (ProgramStatus::Idle, ""),
        (ProgramStatus::Working, "Reading source"),
        (ProgramStatus::Blocked(ProgramStatusKind::Permission), ""),
        (ProgramStatus::Blocked(ProgramStatusKind::Question), ""),
        (ProgramStatus::Blocked(ProgramStatusKind::Auth), ""),
        (ProgramStatus::Done, ""),
        (ProgramStatus::Error, ""),
    ] {
        let mut value = String::new();
        SetTerminalProgramStatus {
            status,
            iterm_detail: detail,
            emit_iterm_status: true,
        }
        .write_ansi(&mut value)
        .expect("encode combined terminal program status");
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
fn encodes_generic_program_status() {
    let mut encoded = String::new();
    SetTerminalProgramStatus {
        status: ProgramStatus::Working,
        iterm_detail: "ignored",
        emit_iterm_status: false,
    }
    .write_ansi(&mut encoded)
    .expect("encode generic terminal program status");

    assert_eq!(encoded, "\x1b]7501;state=working:app=codex\x1b\\");
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
    let mut combined = String::new();
    ClearTerminalProgramStatus {
        emit_iterm_status: true,
    }
    .write_ansi(&mut combined)
    .expect("encode combined terminal program status clear");
    let mut generic = String::new();
    ClearTerminalProgramStatus {
        emit_iterm_status: false,
    }
    .write_ansi(&mut generic)
    .expect("encode generic terminal program status clear");

    assert_eq!(
        combined,
        "\x1b]21337;status=;indicator=;status-color=;detail=\x07\x1b]7501;state=clear\x1b\\"
    );
    assert_eq!(generic, "\x1b]7501;state=clear\x1b\\");
}

#[test]
fn cache_changes_only_after_a_successful_write() {
    let state = SessionStatusState::new();
    let mut emitted = Vec::new();
    let working = CachedStatus::Reported(ProgramStatus::Working);
    state
        .update(working, "Thinking", || {
            Err(io::Error::other("write failed"))
        })
        .expect_err("failed write should be returned");

    state
        .update(working, "Thinking", || {
            emitted.push(working);
            Ok(())
        })
        .expect("retry status write");
    state
        .update(working, "Thinking", || {
            emitted.push(CachedStatus::Reported(ProgramStatus::Blocked(
                ProgramStatusKind::Question,
            )));
            Ok(())
        })
        .expect("deduplicate status write");
    state
        .update(working, "Reading", || {
            emitted.push(working);
            Ok(())
        })
        .expect("emit changed detail");
    state.invalidate();
    state
        .update(working, "Thinking", || {
            emitted.push(working);
            Ok(())
        })
        .expect("re-emit status after terminal handoff");
    state
        .update(CachedStatus::Cleared, "", || {
            emitted.push(CachedStatus::Cleared);
            Ok(())
        })
        .expect("clear status");

    assert_eq!(emitted, [working, working, working, CachedStatus::Cleared,]);
}

#[test]
fn selects_protocols_for_terminal_environment() {
    let direct_iterm = TerminalInfo {
        backend: None,
        name: TerminalName::Iterm2,
        term_program: Some("iTerm.app".to_string()),
        version: Some("3.7.0".to_string()),
        term: None,
        multiplexer: None,
    };
    let both_protocols = TerminalProgramStatusSupport {
        osc_7501: true,
        osc_21337: true,
    };
    let no_protocols = TerminalProgramStatusSupport {
        osc_7501: false,
        osc_21337: false,
    };
    let generic_protocol = TerminalProgramStatusSupport {
        osc_7501: true,
        osc_21337: false,
    };
    assert_eq!(
        terminal_program_status_support(
            &direct_iterm,
            /*stdout_is_terminal*/ true,
            /*screen_active*/ false,
        ),
        both_protocols
    );
    assert_eq!(
        terminal_program_status_support(
            &direct_iterm,
            /*stdout_is_terminal*/ false,
            /*screen_active*/ false,
        ),
        no_protocols
    );

    for terminal in [
        TerminalInfo {
            multiplexer: Some(Multiplexer::Tmux { version: None }),
            ..direct_iterm.clone()
        },
        TerminalInfo {
            name: TerminalName::Ghostty,
            ..direct_iterm.clone()
        },
    ] {
        assert_eq!(
            terminal_program_status_support(
                &terminal, /*stdout_is_terminal*/ true, /*screen_active*/ false,
            ),
            generic_protocol
        );
    }
    assert_eq!(
        terminal_program_status_support(
            &direct_iterm,
            /*stdout_is_terminal*/ true,
            /*screen_active*/ true,
        ),
        generic_protocol
    );
    assert_eq!(
        terminal_program_status_support(
            &TerminalInfo {
                backend: Some(TerminalBackend::Vte {
                    version: Some(5204)
                }),
                ..direct_iterm
            },
            /*stdout_is_terminal*/ true,
            /*screen_active*/ false,
        ),
        no_protocols
    );
}

#[test]
fn selects_protocols_for_vte_backends() {
    for name in [TerminalName::GnomeTerminal, TerminalName::Vte] {
        for (version, osc_7501) in [
            (Some(5204), false),
            (Some(5300), false),
            (Some(5399), false),
            (Some(5400), true),
            (Some(7600), true),
            (None, true),
        ] {
            let terminal = TerminalInfo {
                name,
                term_program: None,
                version: None,
                term: None,
                multiplexer: None,
                backend: Some(TerminalBackend::Vte { version }),
            };
            assert_eq!(
                terminal_program_status_support(
                    &terminal, /*stdout_is_terminal*/ true, /*screen_active*/ false,
                ),
                TerminalProgramStatusSupport {
                    osc_7501,
                    osc_21337: false,
                },
                "terminal {name:?}, VTE backend version={version:?}"
            );
        }
    }
}
