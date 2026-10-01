//! Exercise Windows console mouse capture in a separate process with its own console.

use super::AlternateScreen;
use pretty_assertions::assert_eq;
use std::fs::File;
use std::fs::OpenOptions;
use std::os::windows::io::AsRawHandle;
use std::os::windows::process::CommandExt;
use std::process::Command;
use windows_sys::Win32::System::Console::ENABLE_MOUSE_INPUT;
use windows_sys::Win32::System::Console::GetConsoleMode;
use windows_sys::Win32::System::Console::SetConsoleMode;
use windows_sys::Win32::System::Threading::CREATE_NEW_CONSOLE;

const CHILD_ENV: &str = "CODEX_TUI_MOUSE_CAPTURE_TEST_CHILD";

fn console_mode(input: &File) -> u32 {
    let mut mode = 0;
    // SAFETY: input is an open console handle and mode points to writable storage.
    let result = unsafe { GetConsoleMode(input.as_raw_handle(), &mut mode) };
    assert_ne!(result, 0, "{}", std::io::Error::last_os_error());
    mode
}

#[test]
fn mouse_capture_restores_console_mode_and_encoding() {
    if std::env::var_os(CHILD_ENV).is_none() {
        let output = Command::new(std::env::current_exe().expect("test binary"))
            .args([
                "--exact",
                "tui::alternate_screen::windows_tests::mouse_capture_restores_console_mode_and_encoding",
                "--nocapture",
            ])
            .env(CHILD_ENV, "1")
            .creation_flags(CREATE_NEW_CONSOLE)
            .output()
            .expect("run isolated console test");
        assert!(
            output.status.success() && String::from_utf8_lossy(&output.stdout).contains("1 passed"),
            "stdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        return;
    }

    let input = OpenOptions::new()
        .read(true)
        .write(true)
        .open("CONIN$")
        .expect("open console input");
    let initial_mode = console_mode(&input) & !ENABLE_MOUSE_INPUT;
    // SAFETY: input is an open console handle and initial_mode is a valid console input mode.
    let result = unsafe { SetConsoleMode(input.as_raw_handle(), initial_mode) };
    assert_ne!(result, 0, "{}", std::io::Error::last_os_error());
    let original_mode = console_mode(&input);
    assert_eq!(original_mode & ENABLE_MOUSE_INPUT, 0);
    let screen = AlternateScreen::default();
    let mut output = Vec::new();
    let mut terminal = vt100::Parser::new(
        /*rows*/ 24, /*cols*/ 80, /*scrollback_len*/ 0,
    );

    screen
        .configure_input(&mut output, /*capture_mouse*/ true)
        .unwrap();
    terminal.process(&std::mem::take(&mut output));
    assert_ne!(console_mode(&input) & ENABLE_MOUSE_INPUT, 0);
    assert_eq!(
        terminal.screen().mouse_protocol_encoding(),
        vt100::MouseProtocolEncoding::Sgr,
    );

    screen
        .configure_input(&mut output, /*capture_mouse*/ false)
        .unwrap();
    terminal.process(&output);
    assert_eq!(
        (
            console_mode(&input),
            terminal.screen().mouse_protocol_encoding()
        ),
        (original_mode, vt100::MouseProtocolEncoding::Default),
    );
}
