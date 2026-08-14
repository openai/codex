use std::fs::File;
use std::io::Read as _;
use std::io::Write as _;
use std::os::fd::FromRawFd;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;

use crossterm::event::Event;
use crossterm::event::KeyCode;

use super::InitializedTerminal;
use super::Tui;

const STARTUP_INPUT_CHILD_TEST: &str = "tui::startup_tests::startup_typeahead_pty_child";

#[test]
fn startup_preserves_typeahead_and_discards_buffered_action_key_after_first_draw() {
    let mut master = -1;
    let mut slave = -1;
    let mut size = libc::winsize {
        ws_row: 24,
        ws_col: 80,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // Safety: `openpty` initializes the two file descriptors and reads the supplied size.
    assert_eq!(
        unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::addr_of_mut!(size),
            )
        },
        0,
        "failed to create a test pty: {}",
        std::io::Error::last_os_error()
    );
    // Safety: `openpty` returned owned descriptors on success.
    let mut master = unsafe { File::from_raw_fd(master) };
    // Safety: `openpty` returned owned descriptors on success.
    let slave = unsafe { File::from_raw_fd(slave) };

    // Queue canonical-mode typeahead before raw mode. Reinserting the old startup `tcflush`
    // drops this input before the bounded probe can preserve it.
    master
        .write_all(b"helo\x7Flo\x04")
        .expect("queue typeahead");

    let signals = tempfile::tempdir().expect("create pty synchronization directory");
    let action_ready = signals.path().join("action-ready");
    let action_queued = signals.path().join("action-queued");
    let fresh_ready = signals.path().join("fresh-ready");
    let fresh_queued = signals.path().join("fresh-queued");
    let boundary_ready = signals.path().join("boundary-ready");
    let boundary_queued = signals.path().join("boundary-queued");
    let boundary_consumed = signals.path().join("boundary-consumed");
    let boundary_action_queued = signals.path().join("boundary-action-queued");
    let control_ready = signals.path().join("control-ready");
    let control_queued = signals.path().join("control-queued");
    let mut child = Command::new(std::env::current_exe().expect("current test binary"))
        .arg("--exact")
        .arg(STARTUP_INPUT_CHILD_TEST)
        .arg("--ignored")
        .arg("--nocapture")
        .env("CODEX_TUI_STARTUP_INPUT_PTY_DIR", signals.path())
        .stdin(Stdio::from(slave.try_clone().expect("clone pty slave")))
        .stdout(Stdio::from(slave))
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn startup input pty child");

    let result = (|| -> Result<(), String> {
        for suffix in ["y", "1"] {
            let ready = signals.path().join(format!("alt-{suffix}-ready"));
            wait_for_file(&ready)?;
            std::thread::sleep(Duration::from_millis(/*millis*/ 60));
            master
                .write_all(suffix.as_bytes())
                .map_err(|err| format!("failed to queue delayed modifier suffix: {err}"))?;
            std::fs::write(signals.path().join(format!("alt-{suffix}-queued")), [])
                .map_err(|err| format!("failed to signal delayed modifier suffix: {err}"))?;
        }

        wait_for_file(&action_ready)?;
        master
            .write_all(b"y")
            .map_err(|err| format!("failed to queue buffered action key: {err}"))?;
        std::fs::write(&action_queued, [])
            .map_err(|err| format!("failed to signal buffered action key: {err}"))?;
        std::thread::sleep(Duration::from_millis(/*millis*/ 30));
        master
            .write_all(b"1y\r\x1b[201~1\r")
            .map_err(|err| format!("failed to queue delayed bracketed-paste suffix: {err}"))?;
        wait_for_file(&fresh_ready)?;
        master
            .write_all(b"n")
            .map_err(|err| format!("failed to queue fresh key: {err}"))?;
        std::fs::write(&fresh_queued, [])
            .map_err(|err| format!("failed to signal fresh key: {err}"))?;

        wait_for_file(&boundary_ready)?;
        master
            .write_all(b"\x1b[201~\x1b[12;3R")
            .map_err(|err| format!("failed to queue completed paste boundary: {err}"))?;
        std::fs::write(&boundary_queued, [])
            .map_err(|err| format!("failed to signal queued paste boundary: {err}"))?;
        wait_for_file(&boundary_consumed)?;
        master
            .write_all(b"1\r")
            .map_err(|err| format!("failed to queue unread action after paste boundary: {err}"))?;
        std::fs::write(&boundary_action_queued, [])
            .map_err(|err| format!("failed to signal queued unread action: {err}"))?;

        wait_for_file(&control_ready)?;
        master
            .write_all(b"1y\r\x1b")
            .map_err(|err| format!("failed to queue delayed control-sequence payload: {err}"))?;
        std::fs::write(&control_queued, [])
            .map_err(|err| format!("failed to signal queued control-sequence payload: {err}"))?;
        std::thread::sleep(Duration::from_millis(/*millis*/ 30));
        master
            .write_all(b"\\")
            .map_err(|err| format!("failed to terminate the delayed control sequence: {err}"))?;

        let started_at = Instant::now();
        while child
            .try_wait()
            .map_err(|err| format!("failed to poll startup input pty child: {err}"))?
            .is_none()
        {
            if started_at.elapsed() >= Duration::from_secs(/*secs*/ 5) {
                return Err("timed out waiting for startup input pty child".to_string());
            }
            std::thread::sleep(Duration::from_millis(/*millis*/ 10));
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = child.kill();
    }
    let status = child.wait().expect("wait for startup input pty child");
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .expect("child stderr")
        .read_to_string(&mut stderr)
        .expect("read child stderr");
    assert!(
        result.is_ok() && status.success(),
        "startup input pty child failed: {}\n{stderr}",
        result
            .err()
            .unwrap_or_else(|| "child exited unsuccessfully".to_string())
    );
}

#[tokio::test(flavor = "current_thread")]
#[ignore]
async fn startup_typeahead_pty_child() {
    let Some(signals) = std::env::var_os("CODEX_TUI_STARTUP_INPUT_PTY_DIR") else {
        return;
    };
    let signals = PathBuf::from(signals);
    let InitializedTerminal {
        terminal,
        enhanced_keys_supported,
        stderr_guard,
    } = super::init().expect("initialize terminal");

    let mut typeahead = Vec::new();
    for _ in 0..5 {
        assert!(
            crossterm::event::poll(Duration::from_secs(/*secs*/ 1))
                .expect("poll startup typeahead"),
            "startup typeahead was lost"
        );
        match crossterm::event::read().expect("read startup typeahead") {
            Event::Key(key) => typeahead.push(key.code),
            event => panic!("unexpected startup event: {event:?}"),
        }
    }
    assert_eq!(
        typeahead,
        vec![
            KeyCode::Char('h'),
            KeyCode::Char('e'),
            KeyCode::Char('l'),
            KeyCode::Char('l'),
            KeyCode::Char('o'),
        ]
    );
    if crossterm::event::poll(Duration::ZERO).expect("poll the canonical-mode EOF") {
        // Canonical EOF does not have the same decoded representation on every Unix platform.
        crossterm::event::read().expect("read the canonical-mode EOF");
    }
    crossterm::event::buffer_input(b"\x1b").expect("buffer an incomplete escape sequence");
    crossterm::event::buffer_input(b"[A").expect("complete the buffered escape sequence");
    assert!(crossterm::event::poll(Duration::from_secs(/*secs*/ 1)).expect("poll split arrow"));
    assert!(matches!(
        crossterm::event::read().expect("read split arrow"),
        Event::Key(key) if key.code == KeyCode::Up
    ));

    let mut tui = Tui::new(terminal, enhanced_keys_supported, stderr_guard);
    tui.draw(u16::MAX, |_| {}).expect("draw actionable screen");
    crossterm::event::buffer_input(b"\x1b").expect("buffer a standalone escape");
    let escape_started_at = Instant::now();
    assert!(
        crossterm::event::poll(Duration::from_millis(/*millis*/ 250))
            .expect("poll the standalone escape")
    );
    assert!(matches!(
        crossterm::event::read().expect("read the standalone escape"),
        Event::Key(key) if key.code == KeyCode::Esc
    ));
    assert!(escape_started_at.elapsed() < Duration::from_millis(/*millis*/ 250));

    for suffix in ["y", "1"] {
        crossterm::event::buffer_input(b"\x1b")
            .expect("buffer an incomplete modifier before the security screen");
        std::fs::write(signals.join(format!("alt-{suffix}-ready")), [])
            .expect("signal incomplete modifier");
        tui.discard_pending_input_before_interactive_screen()
            .expect("discard the delayed modifier suffix");
        wait_for_file(&signals.join(format!("alt-{suffix}-queued")))
            .expect("wait for the delayed modifier suffix");
        assert!(
            !crossterm::event::poll(Duration::from_millis(/*millis*/ 50))
                .expect("poll discarded modifier suffix"),
            "delayed modifier suffix became an unmodified action key"
        );
    }

    crossterm::event::buffer_input(b"y\x1b[200~unfinished")
        .expect("buffer action key and incomplete bracketed paste before the security screen");
    std::fs::write(signals.join("action-ready"), []).expect("signal actionable screen");
    wait_for_file(&signals.join("action-queued")).expect("wait for the buffered action key");
    tui.discard_pending_input_before_interactive_screen()
        .expect("discard buffered action key");
    assert!(
        !crossterm::event::poll(Duration::from_millis(/*millis*/ 50))
            .expect("poll discarded action key"),
        "buffered action key was delivered after the first draw"
    );

    std::fs::write(signals.join("fresh-ready"), []).expect("signal fresh input readiness");
    wait_for_file(&signals.join("fresh-queued")).expect("wait for the fresh input");
    assert!(crossterm::event::poll(Duration::from_secs(/*secs*/ 1)).expect("poll fresh key"));
    assert!(matches!(
        crossterm::event::read().expect("read fresh key"),
        Event::Key(key) if key.code == KeyCode::Char('n')
    ));

    crossterm::event::buffer_input(b"\x1b[200~unfinished")
        .expect("buffer bracketed paste before an unread action boundary");
    assert_eq!(
        crossterm::event::discard_buffered_input().expect("quarantine unfinished paste"),
        crossterm::event::InputDiscardStatus::BracketedPasteInProgress
    );
    std::fs::write(signals.join("boundary-ready"), []).expect("signal paste-boundary readiness");
    wait_for_file(&signals.join("boundary-queued")).expect("wait for the paste boundary");
    assert!(
        !crossterm::event::poll(Duration::ZERO).expect("poll a completed paste with unread input")
    );
    assert_eq!(
        crossterm::event::discard_buffered_input().expect("discard completed paste boundary"),
        crossterm::event::InputDiscardStatus::Complete
    );
    std::fs::write(signals.join("boundary-consumed"), []).expect("signal completed paste boundary");
    wait_for_file(&signals.join("boundary-action-queued")).expect("wait for the unread action");
    assert!(super::terminal_input_is_readable().expect("check unread input after an elapsed poll"));
    tui.discard_pending_input_before_interactive_screen()
        .expect("discard actionable unread input after the paste boundary");
    assert!(
        !crossterm::event::poll(Duration::from_millis(/*millis*/ 50))
            .expect("poll discarded unread action"),
        "actionable input queued after the paste boundary survived the security drain"
    );

    crossterm::event::buffer_input(b"\x1b]10;unfinished")
        .expect("buffer an unfinished control sequence before the security screen");
    assert_eq!(
        crossterm::event::discard_buffered_input().expect("quarantine unfinished control sequence"),
        crossterm::event::InputDiscardStatus::ControlSequenceInProgress
    );
    std::fs::write(signals.join("control-ready"), []).expect("signal control-sequence readiness");
    wait_for_file(&signals.join("control-queued")).expect("wait for the control sequence");
    tui.discard_pending_input_before_interactive_screen()
        .expect("discard delayed control-sequence payload");
    assert!(
        !crossterm::event::poll(Duration::from_millis(/*millis*/ 50))
            .expect("poll discarded control-sequence payload"),
        "control-sequence payload survived the security drain"
    );

    crossterm::event::buffer_input(b"\x1b")
        .expect("buffer an ambiguous escape before the security screen");
    let err = tui
        .discard_pending_input_before_interactive_screen()
        .expect_err("an unresolved escape at a protected boundary must fail closed");
    assert_eq!(err.kind(), std::io::ErrorKind::TimedOut);

    super::restore_after_exit().expect("restore terminal");
}

fn wait_for_file(path: &Path) -> Result<(), String> {
    let started_at = Instant::now();
    while !path.exists() {
        if started_at.elapsed() >= Duration::from_secs(/*secs*/ 5) {
            return Err(format!("timed out waiting for {}", path.display()));
        }
        std::thread::sleep(Duration::from_millis(/*millis*/ 10));
    }
    Ok(())
}
