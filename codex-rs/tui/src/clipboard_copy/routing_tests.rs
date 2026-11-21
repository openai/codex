//! Clipboard routing, native ownership, and payload-boundary regressions.

use super::ClipboardCopyMode;
use super::ClipboardLease;
use super::CopyEnvironment;
use super::CopyFormat;
use super::CopyOutcome;
use super::CopyStatus;
use super::OSC52_MAX_RAW_BYTES;
use super::copy_to_clipboard_with;
use codex_terminal_detection::TerminalName;
use pretty_assertions::assert_eq;
use std::cell::RefCell;

#[test]
fn local_tmux_preserves_native_html_when_terminal_silently_rejects_copy() {
    let calls = RefCell::new(Vec::new());
    let result = copy_to_clipboard_with(
        "**hello**",
        CopyFormat::Markdown,
        CopyEnvironment {
            terminal_copy: ClipboardCopyMode::Tmux,
            forwarded_x11: false,
            wsl_session: false,
        },
        |_| {
            // Like default xterm: the command succeeds without delivery.
            calls.borrow_mut().push("tmux");
            Ok(())
        },
        |_| panic!("tmux send succeeded"),
        |text, html| {
            assert_eq!(
                (text, html),
                ("**hello**", Some("<p><strong>hello</strong></p>\n"))
            );
            calls.borrow_mut().push("native");
            Ok(Some(ClipboardLease::test()))
        },
        |_| panic!("native copy succeeded"),
    );
    assert!(matches!(result, Ok(CopyOutcome::Copied(Some(_)))));
    assert_eq!(calls.into_inner(), vec!["native", "tmux"]);
}

#[test]
fn copy_uses_osc52_when_tmux_fails_even_if_native_copy_succeeds() {
    let calls = RefCell::new(Vec::new());
    let result = copy_to_clipboard_with(
        "hello",
        CopyFormat::PlainText,
        CopyEnvironment {
            terminal_copy: ClipboardCopyMode::Tmux,
            forwarded_x11: false,
            wsl_session: false,
        },
        |_| {
            calls.borrow_mut().push("tmux");
            Err("tmux unavailable".into())
        },
        |_| {
            calls.borrow_mut().push("osc52");
            Ok(())
        },
        |text, html| {
            assert_eq!((text, html), ("hello", None));
            calls.borrow_mut().push("native");
            Ok(Some(ClipboardLease::test()))
        },
        |_| panic!("native copy succeeded"),
    );
    assert!(matches!(result, Ok(CopyOutcome::Copied(Some(_)))));
    assert_eq!(calls.into_inner(), vec!["native", "tmux", "osc52"]);
}

fn local_tmux_environment() -> CopyEnvironment {
    CopyEnvironment {
        terminal_copy: ClipboardCopyMode::Tmux,
        forwarded_x11: false,
        wsl_session: false,
    }
}

#[test]
fn rex_selects_osc52_without_ssh_or_tmux() {
    assert_eq!(
        ClipboardCopyMode::for_terminal(TerminalName::Rex),
        ClipboardCopyMode::Osc52,
    );
}

#[test]
fn osc52_forwarding_runs_even_when_native_copy_succeeds() {
    let calls = RefCell::new(Vec::new());
    let result = copy_to_clipboard_with(
        "hello",
        CopyFormat::PlainText,
        CopyEnvironment {
            terminal_copy: ClipboardCopyMode::Osc52,
            forwarded_x11: false,
            wsl_session: false,
        },
        |_| panic!("forwarding should use OSC 52 directly"),
        |_| {
            calls.borrow_mut().push("osc52");
            Ok(())
        },
        |text, html| {
            assert_eq!((text, html), ("hello", None));
            calls.borrow_mut().push("native");
            Ok(Some(ClipboardLease::test()))
        },
        |_| panic!("native copy succeeded"),
    );

    assert!(matches!(result, Ok(CopyOutcome::Copied(Some(_)))));
    assert_eq!(calls.into_inner(), vec!["native", "osc52"]);
}

#[test]
fn oversized_tmux_copy_preserves_native_copy() {
    for (size, expected_calls) in [
        (OSC52_MAX_RAW_BYTES, vec!["native", "tmux"]),
        (OSC52_MAX_RAW_BYTES + 1, vec!["native", "osc52"]),
    ] {
        let text = "x".repeat(size);
        let calls = RefCell::new(Vec::new());
        let result = copy_to_clipboard_with(
            &text,
            CopyFormat::PlainText,
            local_tmux_environment(),
            |actual| {
                assert_eq!(actual, text);
                calls.borrow_mut().push("tmux");
                Ok(())
            },
            |_| {
                calls.borrow_mut().push("osc52");
                Err("payload too large".into())
            },
            |actual, html| {
                assert_eq!((actual, html), (text.as_str(), None));
                calls.borrow_mut().push("native");
                Ok(Some(ClipboardLease::test()))
            },
            |_| panic!("PowerShell fallback should not be needed"),
        );
        assert!(matches!(result, Ok(CopyOutcome::Copied(Some(_)))));
        assert_eq!(calls.into_inner(), expected_calls);
    }
}

#[test]
fn empty_copy_reports_failure_without_touching_clipboards() {
    let result = copy_to_clipboard_with(
        "",
        CopyFormat::PlainText,
        local_tmux_environment(),
        |_| panic!("empty input must not reach tmux"),
        |_| panic!("empty input must not reach OSC 52"),
        |_, _| panic!("empty input must not reach the native clipboard"),
        |_| panic!("empty input must not reach PowerShell"),
    );
    insta::assert_snapshot!(result.err().expect("empty selection should fail"));
}

#[test]
fn confirmed_copy_without_new_lease_preserves_native_ownership() {
    let mut lease = Some(ClipboardLease::test());

    assert_eq!(
        CopyOutcome::Copied(None).store(&mut lease),
        CopyStatus::Confirmed
    );
    assert!(lease.is_some());
}

#[test]
fn screen_forwards_one_complete_osc52_across_bounded_passthrough_strings() {
    for text in ["hello", &"世界".repeat(500)] {
        let sequence = super::osc52_sequence(text, super::Osc52Target::Screen).unwrap();
        let chunks = sequence
            .strip_prefix("\x1bP")
            .unwrap()
            .strip_suffix("\x1b\\")
            .unwrap();
        let mut forwarded = String::new();
        for chunk in chunks.split("\x1b\\\x1bP") {
            assert!(chunk.len() < 256);
            forwarded.push_str(chunk);
        }
        assert_eq!(
            forwarded,
            super::osc52_sequence(text, super::Osc52Target::Direct).unwrap()
        );
    }
}
