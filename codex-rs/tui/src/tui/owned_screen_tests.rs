//! Exercise session ownership in an isolated process so global terminal modes cannot race tests.

#![cfg(unix)]

use super::OverlayInput;
use super::TuiEvent;
use crate::pager_overlay::Overlay;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;
use ratatui::layout::Rect;
use ratatui::layout::Size;
use std::process::Command;

const CHILD_ENV: &str = "CODEX_TUI_OWNED_SCREEN_CHILD";

#[test]
fn owned_screen_preserves_inline_viewport_across_overlay_handoff_and_resume() {
    let output = Command::new(std::env::current_exe().expect("test binary"))
        .args([
            "--exact",
            "tui::owned_screen_tests::owned_screen_lifecycle_child",
            "--ignored",
            "--nocapture",
        ])
        .env(CHILD_ENV, "1")
        .output()
        .expect("run isolated owned-screen child");
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let mut terminal = vt100::Parser::new(
        /*rows*/ 24, /*cols*/ 80, /*scrollback_len*/ 0,
    );
    terminal.process(&output.stdout);
    assert_eq!(
        (
            terminal.screen().alternate_screen(),
            terminal.screen().mouse_protocol_mode()
        ),
        (false, vt100::MouseProtocolMode::None),
    );
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "spawned by the owned-screen lifecycle test"]
async fn owned_screen_lifecycle_child() {
    if std::env::var_os(CHILD_ENV).is_none() {
        return;
    }
    let mut tui = super::test_support::make_test_tui().expect("test tui");
    let inline = Rect::new(
        /*x*/ 0, /*y*/ 4, /*width*/ 80, /*height*/ 5,
    );
    tui.terminal.set_viewport_area(inline);
    tui.set_alt_screen_enabled(/*enabled*/ false);
    tui.set_owned_screen(/*owned*/ true).expect("honor no-alt");
    tui.set_overlay_input(OverlayInput::Transcript)
        .expect("no-alt overlay");
    tui.set_overlay_input(OverlayInput::Default)
        .expect("close no-alt overlay");
    assert_eq!(
        (
            tui.is_owned_screen(),
            tui.is_alt_screen_active(),
            tui.terminal.viewport_area
        ),
        (false, false, inline),
    );

    let mut pager = Overlay::new_static_with_lines(
        vec!["static pager".into()],
        "DETAILS".to_string(),
        crate::keymap::RuntimeKeymap::defaults().pager,
    );
    pager
        .handle_event(&mut tui, TuiEvent::FocusLost)
        .expect("request no-alt static input");
    assert!(tui.overlay_input == OverlayInput::StaticPager);
    assert!(!tui.is_alt_screen_active());
    pager
        .handle_event(
            &mut tui,
            TuiEvent::Key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)),
        )
        .expect("close no-alt static pager");
    assert!(pager.is_done());
    assert!(tui.overlay_input == OverlayInput::Default);
    assert_eq!(tui.terminal.viewport_area, inline);

    tui.set_alt_screen_enabled(/*enabled*/ true);
    // A startup overlay may already hold the alternate screen when App selects ownership.
    tui.enter_alt_screen().expect("startup overlay");
    tui.set_overlay_input(OverlayInput::Transcript)
        .expect("nested transcript input");
    tui.leave_alt_screen_for_handoff()
        .expect("yield transcript to editor");
    assert!(tui.overlay_input == OverlayInput::Transcript);
    tui.enter_alt_screen()
        .expect("restore transcript after editor");
    tui.set_overlay_input(OverlayInput::Default)
        .expect("return to picker input");
    assert!(tui.is_alt_screen_active());
    let mut pager = Overlay::new_static_with_lines(
        vec!["static pager".into()],
        "DETAILS".to_string(),
        crate::keymap::RuntimeKeymap::defaults().pager,
    );
    pager
        .handle_event(&mut tui, TuiEvent::FocusLost)
        .expect("request native static input");
    assert!(tui.overlay_input == OverlayInput::StaticPager);
    tui.leave_alt_screen_for_handoff()
        .expect("yield static pager to editor");
    assert!(tui.overlay_input == OverlayInput::StaticPager);
    assert_eq!(tui.terminal.viewport_area, inline);
    tui.enter_alt_screen().expect("restore static pager");
    super::ALTERNATE_SCREEN
        .leave(tui.terminal.backend_mut())
        .expect("suspend static pager");
    super::job_control::PreparedResumeAction::RestoreAltScreen
        .apply(
            &mut tui.terminal,
            Size::new(/*width*/ 80, /*height*/ 24),
            /*owned*/ false,
            tui.overlay_input.captures_mouse(/*owned_screen*/ false),
        )
        .expect("resume static pager");
    pager
        .handle_event(
            &mut tui,
            TuiEvent::Key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)),
        )
        .expect("close static pager back to picker");
    assert!(pager.is_done());
    assert!(tui.overlay_input == OverlayInput::Default);
    assert!(tui.is_alt_screen_active());
    tui.set_owned_screen(/*owned*/ true)
        .expect("promote overlay");
    let owned = Rect::from(Size::new(/*width*/ 80, /*height*/ 24));
    for _ in 0..2 {
        tui.enter_alt_screen().expect("open overlay");
        tui.leave_alt_screen().expect("close overlay");
        assert_eq!(
            (
                tui.is_owned_screen(),
                tui.is_alt_screen_active(),
                tui.terminal.viewport_area,
                tui.alt_saved_viewport,
            ),
            (true, true, owned, Some(inline)),
        );
    }

    tui.leave_alt_screen_for_handoff().expect("yield to editor");
    assert_eq!(
        (
            tui.is_owned_screen(),
            tui.is_alt_screen_active(),
            tui.terminal.viewport_area,
            tui.alt_saved_viewport,
        ),
        (true, false, inline, None),
    );
    tui.enter_alt_screen().expect("return from editor");
    // Suspension leaves the real screen while retaining the session's ownership and viewport.
    super::ALTERNATE_SCREEN
        .leave(tui.terminal.backend_mut())
        .expect("suspend screen");
    let resumed = Size::new(/*width*/ 100, /*height*/ 30);
    super::job_control::PreparedResumeAction::RestoreAltScreen
        .apply(
            &mut tui.terminal,
            resumed,
            /*owned*/ true,
            /*capture_mouse*/ true,
        )
        .expect("resume owned screen");
    assert_eq!(
        (
            tui.is_owned_screen(),
            tui.terminal.viewport_area,
            tui.alt_saved_viewport
        ),
        (true, Rect::from(resumed), Some(inline)),
    );
    tui.set_alt_screen_enabled(/*enabled*/ false);
    assert_eq!(
        (
            tui.is_owned_screen(),
            tui.is_alt_screen_active(),
            tui.terminal.viewport_area,
            tui.alt_saved_viewport,
        ),
        (false, false, inline, None),
    );
    // Exercise direct owned entry as well as promotion of an existing overlay.
    tui.set_alt_screen_enabled(/*enabled*/ true);
    tui.set_owned_screen(/*owned*/ true)
        .expect("own session screen");
    tui.set_owned_screen(/*owned*/ false)
        .expect("release session screen");
    assert_eq!(tui.terminal.viewport_area, inline);
}
