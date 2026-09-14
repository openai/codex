//! Clipboard routing, native ownership, and payload-boundary regressions.

use super::ClipboardLease;
use super::CopyEnvironment;
use super::CopyFormat;
use super::OSC52_MAX_RAW_BYTES;
use super::copy_to_clipboard_with;
use pretty_assertions::assert_eq;
use std::cell::RefCell;

#[test]
fn local_tmux_preserves_native_html_when_terminal_silently_rejects_copy() {
    let calls = RefCell::new(Vec::new());
    let result = copy_to_clipboard_with(
        "**hello**",
        CopyFormat::Markdown,
        CopyEnvironment {
            ssh_session: false,
            tmux_session: true,
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
    assert!(matches!(result, Ok(Some(_))));
    assert_eq!(calls.into_inner(), vec!["native", "tmux"]);
}

#[test]
fn copy_uses_osc52_when_tmux_fails_even_if_native_copy_succeeds() {
    let calls = RefCell::new(Vec::new());
    let result = copy_to_clipboard_with(
        "hello",
        CopyFormat::PlainText,
        CopyEnvironment {
            ssh_session: false,
            tmux_session: true,
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
    assert!(matches!(result, Ok(Some(_))));
    assert_eq!(calls.into_inner(), vec!["native", "tmux", "osc52"]);
}

fn local_tmux_environment() -> CopyEnvironment {
    CopyEnvironment {
        tmux_session: true,
        ssh_session: false,
        wsl_session: false,
    }
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
        assert!(result.is_ok());
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
