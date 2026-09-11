//! Exercise caught-panic recovery through a real terminal and the production panic hook.

use std::fs::File;
use std::io::Read as _;
use std::io::Write as _;
use std::os::fd::FromRawFd;
use std::process::Command;
use std::process::Stdio;

use crossterm::cursor::SetCursorStyle;
use ratatui::layout::Rect;
use ratatui::style::Style;

const BEFORE_PANIC: &str = "\x1b]1337;codex-before-panic\x07";
const BEFORE_RECOVERY: &str = "\x1b]1337;codex-before-recovery\x07";
const BEFORE_VISIBLE_FRAME: &str = "\x1b]1337;codex-before-visible-frame\x07";
const AFTER_VISIBLE_FRAME: &str = "\x1b]1337;codex-after-visible-frame\x07";

#[test]
fn caught_panic_rehides_cursor_before_repainting_and_restores_its_shape() {
    let mut master = -1;
    let mut slave = -1;
    let mut size = libc::winsize {
        ws_row: 24,
        ws_col: 80,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // SAFETY: openpty initializes both file descriptors and reads the supplied window size.
    let result = unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::addr_of_mut!(size),
        )
    };
    assert!(
        result == 0,
        "open test PTY: {}",
        std::io::Error::last_os_error()
    );
    // SAFETY: a successful openpty call returned two owned file descriptors.
    let mut master = unsafe { File::from_raw_fd(master) };
    // SAFETY: the slave descriptor is distinct from the master and is likewise newly owned.
    let slave = unsafe { File::from_raw_fd(slave) };
    let child = Command::new(std::env::current_exe().expect("current test binary"))
        .args([
            "--exact",
            "tui::panic_tests::caught_panic_pty_child",
            "--ignored",
            "--nocapture",
        ])
        .env("CODEX_TUI_CAUGHT_PANIC_PTY_CHILD", "1")
        .stdin(Stdio::from(slave.try_clone().expect("clone PTY slave")))
        .stdout(Stdio::from(slave))
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn caught panic child");
    let reader = std::thread::spawn(move || {
        let mut output = Vec::new();
        let mut buffer = [0; 4096];
        loop {
            match master.read(&mut buffer) {
                Ok(0) => break,
                Ok(read) => output.extend_from_slice(&buffer[..read]),
                Err(err) if err.kind() == std::io::ErrorKind::Interrupted => {}
                // Linux reports EIO when the last PTY slave closes; other systems can return EOF.
                Err(err) if err.raw_os_error() == Some(libc::EIO) => break,
                Err(err) => panic!("read child PTY: {err}"),
            }
        }
        output
    });
    let child = child
        .wait_with_output()
        .expect("wait for caught panic child");
    let output = reader.join().expect("read caught panic terminal output");
    let output = String::from_utf8_lossy(&output);
    assert!(
        child.status.success(),
        "child stderr: {}\nterminal: {output}",
        String::from_utf8_lossy(&child.stderr)
    );

    let (_, output) = output.split_once(BEFORE_PANIC).expect("before panic");
    let (hook, output) = output.split_once(BEFORE_RECOVERY).expect("before recovery");
    let (hidden, output) = output
        .split_once(BEFORE_VISIBLE_FRAME)
        .expect("before visible frame");
    let (visible, _) = output
        .split_once(AFTER_VISIBLE_FRAME)
        .expect("after visible frame");

    let cursor_move = regex_lite::Regex::new(r"\x1b\[[0-9]+;[0-9]+H").expect("cursor move regex");
    assert!(
        hook.contains("\x1b[0 q") && hook.contains("\x1b[?25h"),
        "panic hook did not reset and show the cursor"
    );
    assert!(
        hidden.find("\x1b[?25l").expect("hide after panic")
            < cursor_move
                .find(hidden)
                .expect("repaint cursor movement")
                .start(),
        "repaint moved a visible cursor"
    );
    assert!(
        !hidden.contains("\x1b[?25h"),
        "hidden frame exposed the cursor"
    );
    assert!(
        visible.find("\x1b[6 q").expect("restore steady bar")
            < visible.find("\x1b[?25h").expect("show restored cursor")
    );

    let cursor_commands =
        regex_lite::Regex::new(r"\x1b\[\?25[hl]|\x1b\[[0-9] q|\x1b\[[0-9]+;[0-9]+H")
            .expect("cursor command regex");
    let phases = [
        ("panic hook", hook),
        ("hidden repaint", hidden),
        ("visible frame", visible),
    ]
    .map(|(name, bytes)| {
        let commands = cursor_commands
            .find_iter(bytes)
            .map(|m| m.as_str().escape_debug().to_string())
            .collect::<Vec<_>>();
        format!("{name}: {}", commands.join(" "))
    });
    insta::assert_snapshot!("caught_panic_cursor_commands", phases.join("\n"));
}

#[tokio::test(flavor = "current_thread")]
#[ignore]
async fn caught_panic_pty_child() {
    if std::env::var_os("CODEX_TUI_CAUGHT_PANIC_PTY_CHILD").is_none() {
        return;
    }
    super::set_modes().expect("set terminal modes");
    super::set_panic_hook();
    let mut tui = super::test_support::make_test_tui().expect("create TUI");
    tui.terminal.set_viewport_area(Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 12, /*height*/ 2,
    ));

    draw(&mut tui, "before panic", Some((4, 1))).expect("draw initial visible cursor");
    draw(&mut tui, "before panic", /*cursor*/ None).expect("hide initial cursor");
    mark(BEFORE_PANIC).expect("mark before panic");
    assert!(std::panic::catch_unwind(|| panic!("simulate caught startup panic")).is_err());
    mark(BEFORE_RECOVERY).expect("mark before recovery");
    tui.recover_after_caught_panic()
        .expect("recover TUI after panic");
    draw(&mut tui, "after panic", /*cursor*/ None).expect("draw recovered hidden cursor");
    mark(BEFORE_VISIBLE_FRAME).expect("mark before visible frame");
    draw(&mut tui, "after panic", Some((4, 1))).expect("draw recovered visible cursor");
    mark(AFTER_VISIBLE_FRAME).expect("mark after visible frame");
    super::restore_after_exit().expect("restore terminal after test");
}

fn draw(tui: &mut super::Tui, text: &str, cursor: Option<(u16, u16)>) -> std::io::Result<()> {
    tui.terminal.draw(|frame| {
        frame.buffer_mut().set_string(0, 0, text, Style::default());
        if let Some(cursor) = cursor {
            frame.set_cursor_style(SetCursorStyle::SteadyBar);
            frame.set_cursor_position(cursor);
        }
    })
}

fn mark(marker: &str) -> std::io::Result<()> {
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(marker.as_bytes())?;
    stdout.flush()
}
