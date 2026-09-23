//! Replay emitted screen transitions against independent main/alternate keyboard stacks.

#![cfg(unix)]

use super::*;
use crossterm::event::KeyboardEnhancementFlags;
use crossterm::event::PushKeyboardEnhancementFlags;
use pretty_assertions::assert_eq;

#[derive(Default)]
struct KeyboardScreens {
    main: Vec<u8>,
    alternate: Vec<u8>,
    alternate_active: bool,
}

#[test]
fn refreshed_mouse_policy_applies_to_fullscreen_and_overlays() {
    use super::super::OverlayInput;

    for owned in [false, true] {
        let screen = AlternateScreen::default();
        let mut output = Vec::new();
        let mut terminal = vt100::Parser::new(
            /*rows*/ 24, /*cols*/ 80, /*scrollback_len*/ 0,
        );
        let mut input = OverlayInput::Default;
        screen
            .enter(&mut output, input.captures_mouse(owned))
            .unwrap();
        for disabled in [true, false, true] {
            screen
                .mouse_capture_disabled
                .store(disabled, Ordering::Relaxed);
            for next in [
                OverlayInput::Transcript,
                OverlayInput::Usage,
                OverlayInput::StaticPager,
                OverlayInput::Default,
            ] {
                input.apply(&screen, &mut output, next, owned).unwrap();
                terminal.process(&std::mem::take(&mut output));
                let expected = if !disabled && next.captures_mouse(owned) {
                    vt100::MouseProtocolMode::AnyMotion
                } else {
                    vt100::MouseProtocolMode::None
                };
                assert_eq!(
                    (
                        terminal.screen().alternate_screen(),
                        terminal.screen().mouse_protocol_mode()
                    ),
                    (true, expected),
                );
            }
        }
        screen.leave(&mut output).unwrap();
    }
}

impl KeyboardScreens {
    fn process(&mut self, output: &[u8]) {
        for sequence in std::str::from_utf8(output).unwrap().split('\x1b') {
            match sequence {
                "[?1049h" => self.alternate_active = true,
                "[?1049l" => self.alternate_active = false,
                _ => {
                    let stack = if self.alternate_active {
                        &mut self.alternate
                    } else {
                        &mut self.main
                    };
                    if let Some(flags) = sequence
                        .strip_prefix("[>")
                        .and_then(|value| value.strip_suffix('u'))
                    {
                        stack.push(flags.parse().unwrap());
                    } else if let Some(count) = sequence
                        .strip_prefix("[<")
                        .and_then(|value| value.strip_suffix('u'))
                    {
                        let count = count.parse().unwrap_or(/*default*/ 1);
                        stack.truncate(stack.len().saturating_sub(count));
                    }
                }
            }
        }
    }

    fn state(&self) -> (bool, &[u8], &[u8]) {
        (self.alternate_active, &self.main, &self.alternate)
    }
}

// Capture-specific tests must not inherit the tmux session running the test binary.
fn enter_with_mouse_enabled(
    screen: &AlternateScreen,
    writer: &mut impl Write,
    capture_mouse: bool,
) -> Result<()> {
    screen.enter(writer, capture_mouse)?;
    if screen
        .mouse_capture_disabled
        .swap(/*val*/ false, Ordering::Relaxed)
        && capture_mouse
    {
        screen.configure_input(writer, capture_mouse)?;
    }
    Ok(())
}

struct FailOnce {
    output: Vec<u8>,
    sequence: &'static [u8],
    failed: bool,
}

impl Write for FailOnce {
    fn write(&mut self, bytes: &[u8]) -> Result<usize> {
        if bytes == self.sequence && !self.failed {
            self.failed = true;
            return Err(std::io::Error::other("injected cleanup write failure"));
        }
        self.output.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

#[test]
fn emergency_restore_cleans_up_alternate_screen_overlays() {
    if keyboard_modes::keyboard_enhancement_disabled() {
        return;
    }
    for close_before_restore in [false, true] {
        for keyboard_restore in [KeyboardRestore::PopStack, KeyboardRestore::ResetAfterExit] {
            let screen = AlternateScreen::default();
            let mut output = Vec::new();
            let mut terminal = KeyboardScreens::default();
            execute!(
                output,
                PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
            )
            .unwrap();
            keyboard_modes::enable_keyboard_enhancement(&mut output);
            // Exercise repeated overlays before the final exit, including the inline path
            // where normal overlay closing already returned to the main screen.
            screen.enter(&mut output, /*capture_mouse*/ false).unwrap();
            screen.leave(&mut output).unwrap();
            screen.enter(&mut output, /*capture_mouse*/ false).unwrap();
            if close_before_restore {
                screen.leave(&mut output).unwrap();
            }
            screen.restore(&mut output, keyboard_restore).unwrap();
            terminal.process(&output);
            let parent_stack = match keyboard_restore {
                KeyboardRestore::PopStack => &[1][..],
                KeyboardRestore::ResetAfterExit => &[][..],
            };
            assert_eq!(
                (terminal.state(), screen.active.load(Ordering::Relaxed)),
                ((false, parent_stack, &[][..]), false),
            );
        }
    }
}

#[test]
fn cleanup_write_failure_does_not_pop_the_wrong_screen() {
    if keyboard_modes::keyboard_enhancement_disabled() {
        return;
    }
    for (sequence, stays_in_alternate) in [
        (b"\x1b[?1049h".as_slice(), false),
        (b"\x1b[?1007l".as_slice(), false),
        (b"\x1b[?1049l".as_slice(), true),
    ] {
        let screen = AlternateScreen::default();
        let mut output = Vec::new();
        let mut terminal = KeyboardScreens::default();
        execute!(
            output,
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        )
        .unwrap();
        keyboard_modes::enable_keyboard_enhancement(&mut output);
        terminal.process(&std::mem::take(&mut output));
        let main_stack = terminal.main.clone();
        let mut writer = std::io::LineWriter::new(FailOnce {
            output,
            sequence,
            failed: false,
        });
        let enter_failed = sequence == b"\x1b[?1049h";
        assert_eq!(
            screen.enter(&mut writer, /*capture_mouse*/ false).is_err(),
            enter_failed
        );
        let result = screen.restore(&mut writer, KeyboardRestore::PopStack);
        terminal.process(&writer.get_ref().output);
        let expected_main = if stays_in_alternate {
            main_stack.as_slice()
        } else {
            &[1][..]
        };
        assert_eq!(
            (
                result.is_err(),
                writer.get_ref().failed,
                screen.active.load(Ordering::Relaxed),
                terminal.state(),
            ),
            (
                !enter_failed,
                true,
                stays_in_alternate,
                (stays_in_alternate, expected_main, &[][..]),
            ),
        );
    }
}

#[test]
fn owned_overlay_promotion_and_handoff_balance_both_keyboard_stacks() {
    if keyboard_modes::keyboard_enhancement_disabled() {
        return;
    }
    let screen = AlternateScreen::default();
    let mut output = Vec::new();
    let mut keyboard = KeyboardScreens::default();
    let mut terminal = vt100::Parser::new(
        /*rows*/ 24, /*cols*/ 80, /*scrollback_len*/ 0,
    );
    keyboard_modes::enable_keyboard_enhancement(&mut output);
    enter_with_mouse_enabled(&screen, &mut output, /*capture_mouse*/ false).unwrap();
    keyboard.process(&output);
    terminal.process(&std::mem::take(&mut output));
    let stacks = (keyboard.main.clone(), keyboard.alternate.clone());

    for _ in 0..2 {
        screen
            .configure_input(&mut output, /*capture_mouse*/ true)
            .unwrap();
        keyboard.process(&output);
        terminal.process(&std::mem::take(&mut output));
        assert_eq!(
            (
                keyboard.state(),
                terminal.screen().mouse_protocol_mode(),
                terminal.screen().mouse_protocol_encoding(),
            ),
            (
                (true, stacks.0.as_slice(), stacks.1.as_slice()),
                vt100::MouseProtocolMode::AnyMotion,
                vt100::MouseProtocolEncoding::Sgr,
            ),
        );
        screen.leave(&mut output).unwrap();
        keyboard.process(&output);
        terminal.process(&std::mem::take(&mut output));
        assert_eq!(
            (
                keyboard.state(),
                terminal.screen().mouse_protocol_mode(),
                terminal.screen().mouse_protocol_encoding(),
            ),
            (
                (false, stacks.0.as_slice(), &[][..]),
                vt100::MouseProtocolMode::None,
                vt100::MouseProtocolEncoding::Default,
            ),
        );
        enter_with_mouse_enabled(&screen, &mut output, /*capture_mouse*/ true).unwrap();
        keyboard.process(&output);
        terminal.process(&std::mem::take(&mut output));
    }
    screen
        .restore(&mut output, KeyboardRestore::ResetAfterExit)
        .unwrap();
    keyboard.process(&output);
    terminal.process(&output);
    assert_eq!(
        (keyboard.state(), terminal.screen().mouse_protocol_mode()),
        ((false, &[][..], &[][..]), vt100::MouseProtocolMode::None),
    );
}

#[test]
fn failed_mouse_cleanup_still_leaves_owned_screen_and_remains_retryable() {
    let screen = AlternateScreen::default();
    let mut output = Vec::new();
    enter_with_mouse_enabled(&screen, &mut output, /*capture_mouse*/ true).unwrap();
    let mut writer = FailOnce {
        output,
        sequence: b"\x1b[?1006l\x1b[?1015l\x1b[?1003l\x1b[?1002l\x1b[?1000l",
        failed: false,
    };
    assert!(screen.leave(&mut writer).is_err());
    for sequence in [
        b"\x1b[?1003l",
        b"\x1b[?1002l",
        b"\x1b[?1000l",
        b"\x1b[?1006l",
    ] {
        assert!(
            writer
                .output
                .windows(sequence.len())
                .any(|bytes| bytes == sequence)
        );
    }
    assert_eq!(
        (
            screen.is_active(),
            screen.mouse_active.load(Ordering::Relaxed)
        ),
        (false, true),
    );
    screen
        .restore(&mut writer, KeyboardRestore::ResetAfterExit)
        .unwrap();
    let mut terminal = vt100::Parser::new(
        /*rows*/ 24, /*cols*/ 80, /*scrollback_len*/ 0,
    );
    terminal.process(&writer.output);
    assert_eq!(
        (
            screen.is_active(),
            screen.mouse_active.load(Ordering::Relaxed),
            terminal.screen().alternate_screen(),
            terminal.screen().mouse_protocol_mode(),
        ),
        (false, false, false, vt100::MouseProtocolMode::None),
    );
}

#[cfg(not(windows))]
#[test]
fn pager_capture_restores_picker_input_without_a_screen_transition() {
    let screen = AlternateScreen::default();
    let mut output = Vec::new();
    let mut terminal = vt100::Parser::new(
        /*rows*/ 12, /*cols*/ 40, /*scrollback_len*/ 0,
    );
    enter_with_mouse_enabled(&screen, &mut output, /*capture_mouse*/ false).unwrap();
    terminal.process(&std::mem::take(&mut output));
    assert_eq!(
        terminal.screen().mouse_protocol_mode(),
        vt100::MouseProtocolMode::None
    );
    let mut input = super::super::OverlayInput::Default;
    for requested in [
        super::super::OverlayInput::Transcript,
        super::super::OverlayInput::StaticPager,
        super::super::OverlayInput::Usage,
    ] {
        input
            .apply(&screen, &mut output, requested, /*owned*/ false)
            .unwrap();
        let capture = requested != super::super::OverlayInput::StaticPager;
        assert_eq!(
            output
                .windows(b"\x1b[?1003h".len())
                .any(|bytes| bytes == b"\x1b[?1003h"),
            capture
        );
        terminal.process(&std::mem::take(&mut output));
        assert_eq!(
            (
                screen.is_active(),
                terminal.screen().mouse_protocol_mode(),
                terminal.screen().mouse_protocol_encoding()
            ),
            (
                true,
                if capture {
                    vt100::MouseProtocolMode::AnyMotion
                } else {
                    vt100::MouseProtocolMode::None
                },
                if capture {
                    vt100::MouseProtocolEncoding::Sgr
                } else {
                    vt100::MouseProtocolEncoding::Default
                }
            ),
        );
        input
            .apply(
                &screen,
                &mut output,
                super::super::OverlayInput::Default,
                /*owned*/ false,
            )
            .unwrap();
        terminal.process(&std::mem::take(&mut output));
        assert_eq!(
            (screen.is_active(), terminal.screen().mouse_protocol_mode()),
            (true, vt100::MouseProtocolMode::None)
        );
    }
    screen.leave(&mut output).unwrap();
}

#[cfg(not(windows))]
#[test]
fn failed_pager_capture_setup_restores_requested_and_actual_picker_policy() {
    for requested in [
        super::super::OverlayInput::Transcript,
        super::super::OverlayInput::Usage,
    ] {
        let screen = AlternateScreen::default();
        let mut output = Vec::new();
        enter_with_mouse_enabled(&screen, &mut output, /*capture_mouse*/ false).unwrap();
        let mut writer = FailOnce {
            output,
            sequence: b"\x1b[?1000h\x1b[?1002h\x1b[?1006h\x1b[?1003h",
            failed: false,
        };
        let mut input = super::super::OverlayInput::Default;
        let error = input
            .apply(&screen, &mut writer, requested, /*owned*/ false)
            .unwrap_err();
        assert_eq!(error.to_string(), "injected cleanup write failure");
        assert!(input == super::super::OverlayInput::Default);
        assert!(writer.failed);
        assert!(!screen.mouse_active.load(Ordering::Relaxed));
        let mut terminal = vt100::Parser::new(
            /*rows*/ 12, /*cols*/ 40, /*scrollback_len*/ 0,
        );
        terminal.process(&writer.output);
        assert_eq!(
            (
                terminal.screen().alternate_screen(),
                terminal.screen().mouse_protocol_mode()
            ),
            (true, vt100::MouseProtocolMode::None)
        );
        assert!(writer.output.ends_with(b"\x1b[?1007h"));
        screen.leave(&mut writer).unwrap();
    }
}

#[cfg(not(windows))]
#[test]
fn identical_default_request_retries_after_failed_fallback_cleanup() {
    struct PartialFailure {
        output: Vec<u8>,
        failed_setup: bool,
        failed_cleanup: bool,
    }
    impl Write for PartialFailure {
        fn write(&mut self, bytes: &[u8]) -> Result<usize> {
            if bytes == b"\x1b[?1000h\x1b[?1002h\x1b[?1006h\x1b[?1003h" && !self.failed_setup {
                self.output.extend_from_slice(b"\x1b[?1000h\x1b[?1002h");
                self.failed_setup = true;
                return Err(std::io::Error::other("partial setup"));
            }
            if bytes == b"\x1b[?1006l\x1b[?1015l\x1b[?1003l\x1b[?1002l\x1b[?1000l"
                && !self.failed_cleanup
            {
                self.failed_cleanup = true;
                return Err(std::io::Error::other("partial cleanup"));
            }
            self.output.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> Result<()> {
            Ok(())
        }
    }
    let screen = AlternateScreen::default();
    let mut writer = PartialFailure {
        output: Vec::new(),
        failed_setup: false,
        failed_cleanup: false,
    };
    enter_with_mouse_enabled(&screen, &mut writer, /*capture_mouse*/ false).unwrap();
    let mut input = super::super::OverlayInput::Default;
    let result = input.apply(
        &screen,
        &mut writer,
        super::super::OverlayInput::Transcript,
        /*owned*/ false,
    );
    assert_eq!(result.unwrap_err().to_string(), "partial setup");
    assert!(input == super::super::OverlayInput::Default);
    assert!(screen.mouse_active.load(Ordering::Relaxed));
    assert!(!screen.input_configured.load(Ordering::Relaxed));
    input
        .apply(
            &screen,
            &mut writer,
            super::super::OverlayInput::Default,
            /*owned*/ false,
        )
        .unwrap();
    assert!(!screen.mouse_active.load(Ordering::Relaxed));
    assert!(screen.input_configured.load(Ordering::Relaxed));
    let mut terminal = vt100::Parser::new(
        /*rows*/ 12, /*cols*/ 40, /*scrollback_len*/ 0,
    );
    terminal.process(&writer.output);
    assert_eq!(
        terminal.screen().mouse_protocol_mode(),
        vt100::MouseProtocolMode::None
    );
    screen.leave(&mut writer).unwrap();
}
