use super::*;
use PointerShape::Default as Arrow;
use PointerShape::Link;
use PointerShape::Resize;
use crossterm::event::KeyModifiers;
use crossterm::event::MouseButton;
use pretty_assertions::assert_eq;

#[test]
fn hover_transitions_are_deduplicated_and_restore_after_leaving_a_link() {
    let mut frames = Vec::new();
    for terminal in [TerminalName::Ghostty, TerminalName::Kitty] {
        let pointer = LinkPointer::default();
        let mut output = Vec::new();
        for shape in [Arrow, Link, Link, Resize, Resize, Arrow, Arrow, Link] {
            pointer.update(&mut output, shape).unwrap();
            frames.push(String::from_utf8(std::mem::take(&mut output)).unwrap());
        }
        pointer.restore(&mut output, terminal).unwrap();
        frames.push(String::from_utf8(std::mem::take(&mut output)).unwrap());
        pointer.restore(&mut output, terminal).unwrap();
        // Re-entering capture off a link must preserve the restored terminal policy.
        pointer.update(&mut output, Arrow).unwrap();
        assert_eq!(output, Vec::<u8>::new());
        pointer.update(&mut output, Link).unwrap();
        frames.push(String::from_utf8(output).unwrap());
    }
    insta::assert_debug_snapshot!(frames, @r#"
    [
        "",
        "\u{1b}]22;pointer\u{1b}\\",
        "",
        "\u{1b}]22;ew-resize\u{1b}\\",
        "",
        "\u{1b}]22;default\u{1b}\\",
        "",
        "\u{1b}]22;pointer\u{1b}\\",
        "\u{1b}]22;text\u{1b}\\",
        "\u{1b}]22;pointer\u{1b}\\",
        "",
        "\u{1b}]22;pointer\u{1b}\\",
        "",
        "\u{1b}]22;ew-resize\u{1b}\\",
        "",
        "\u{1b}]22;default\u{1b}\\",
        "",
        "\u{1b}]22;pointer\u{1b}\\",
        "\u{1b}]22;\u{1b}\\",
        "\u{1b}]22;pointer\u{1b}\\",
    ]
    "#);
}

#[test]
#[cfg(unix)]
fn capture_release_restores_pointer_even_after_moving_off_link() {
    for shape in [Link, Resize, Arrow] {
        let screen = super::super::alternate_screen::AlternateScreen::default();
        let mut output = Vec::new();
        screen
            .configure_input(&mut output, /*capture_mouse*/ true)
            .unwrap();
        screen.set_pointer_shape(&mut output, Link).unwrap();
        screen.set_pointer_shape(&mut output, shape).unwrap();
        output.clear();
        screen.release_input(&mut output).unwrap();
        let expected = if codex_terminal_detection::terminal_info().name == TerminalName::Kitty {
            b"\x1b]22;\x1b\\".as_slice()
        } else {
            b"\x1b]22;text\x1b\\".as_slice()
        };
        assert!(output.starts_with(expected));
        output.clear();
        screen.set_pointer_shape(&mut output, Link).unwrap();
        assert_eq!(output, Vec::<u8>::new());
    }
}

#[test]
fn failed_write_is_retried_and_cleanup_remains_armed() {
    struct Fails;
    impl Write for Fails {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("write failed"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    for (terminal, expected) in [
        (TerminalName::Ghostty, b"\x1b]22;text\x1b\\".as_slice()),
        (TerminalName::Kitty, b"\x1b]22;\x1b\\".as_slice()),
    ] {
        let pointer = LinkPointer::default();
        assert!(pointer.update(&mut Fails, Link).is_err());
        let mut output = Vec::new();
        pointer.update(&mut output, Link).unwrap();
        assert_eq!(output, b"\x1b]22;pointer\x1b\\");
        assert!(pointer.restore(&mut Fails, terminal).is_err());
        output.clear();
        pointer.restore(&mut output, terminal).unwrap();
        assert_eq!(output, expected);
    }
}

#[test]
fn motion_survives_repaint_but_not_drag_focus_loss_resize_or_resume() {
    let mouse = MouseEvent {
        kind: MouseEventKind::Moved,
        column: 8,
        row: 3,
        modifiers: KeyModifiers::NONE,
    };
    for event in [
        TuiEvent::Mouse(MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            ..mouse
        }),
        TuiEvent::FocusLost,
        TuiEvent::Resize(ratatui::layout::Size::new(
            /*width*/ 80, /*height*/ 24,
        )),
        TuiEvent::Resume,
    ] {
        let mut hover = LinkHover::default();
        hover.observe(&TuiEvent::Mouse(mouse));
        hover.observe(&TuiEvent::Draw);
        assert_eq!(hover.mouse, Some(mouse));
        hover.observe(&event);
        assert_eq!(hover.mouse, None);
    }
}
