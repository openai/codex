use super::*;
use pretty_assertions::assert_eq;
use std::io::Read;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;

fn test_tty() -> io::Result<(File, Tty)> {
    let mut master_fd = -1;
    let mut slave_fd = -1;
    if unsafe {
        libc::openpty(
            &mut master_fd,
            &mut slave_fd,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    } == -1
    {
        return Err(io::Error::last_os_error());
    }
    let master = unsafe { File::from_raw_fd(master_fd) };
    let slave = unsafe { File::from_raw_fd(slave_fd) };

    let mut attributes = unsafe { std::mem::zeroed() };
    if unsafe { libc::tcgetattr(slave.as_raw_fd(), &mut attributes) } == -1 {
        return Err(io::Error::last_os_error());
    }
    unsafe { libc::cfmakeraw(&mut attributes) };
    if unsafe { libc::tcsetattr(slave.as_raw_fd(), libc::TCSANOW, &attributes) } == -1 {
        return Err(io::Error::last_os_error());
    }

    let reader = dup_file(slave.as_raw_fd())?;
    let writer = dup_file(slave.as_raw_fd())?;
    let tty = Tty::new(reader, writer)?;
    drop(slave);
    Ok((master, tty))
}

#[test]
fn split_startup_paste_keeps_control_bytes_as_data() {
    let mut buffer = b"\x1b[200~before".to_vec();
    let mut candidate = Vec::new();

    assert!(!append_open_startup_paste_chunk(
        &mut buffer,
        &mut candidate,
        b"a\x03b\x1b[20"
    ));
    assert!(append_open_startup_paste_chunk(
        &mut buffer,
        &mut candidate,
        b"1~after"
    ));
    assert_eq!(buffer, b"\x1b[200~beforea\x03b\x1b[201~after");
}

#[test]
fn ambiguous_alt_prefix_preserves_the_following_event() {
    let mut expected = vec![Event::Key(KeyEvent::new(
        KeyCode::Char(']'),
        KeyModifiers::ALT,
    ))];
    expected.extend(
        "10;ordinary"
            .chars()
            .map(startup_plain_key_event)
            .map(Event::Key),
    );

    assert_eq!(
        startup_input_events(
            settle_incomplete_input(b"\x1b]10;ordinary", IncompleteInputPhase::QueuedUserInput,)
                .expect("settled startup input"),
        ),
        expected,
    );
}

#[test]
fn queued_alt_right_bracket_input_settles_without_failing_startup() -> io::Result<()> {
    let (mut master, mut tty) = test_tty()?;
    master.write_all(b"draft\x1b]10;ordinary")?;
    if !tty.poll_readable(Duration::from_secs(/*secs*/ 5))? {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "queued key did not reach the PTY reader",
        ));
    }

    assert_eq!(
        read_pending_startup_input(&mut tty, Duration::from_millis(/*millis*/ 20), Vec::new(),)?,
        vec![
            StartupInput::Plain(b"draft".to_vec()),
            StartupInput::Key(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char(']'),
                crossterm::event::KeyModifiers::ALT,
            )),
            StartupInput::Plain(b"10;ordinary".to_vec()),
        ]
    );
    Ok(())
}

#[test]
fn incomplete_startup_sequence_preserves_preceding_text_and_is_quarantined() -> io::Result<()> {
    let (mut master, mut tty) = test_tty()?;
    master.write_all(b"draft\x1b[1;")?;

    assert_eq!(
        read_pending_startup_input(&mut tty, Duration::from_millis(/*millis*/ 20), Vec::new(),)?,
        vec![
            StartupInput::Plain(b"draft".to_vec()),
            StartupInput::UnknownAction,
        ]
    );
    Ok(())
}

#[test]
fn open_startup_paste_timeout_tracks_inactivity() -> io::Result<()> {
    let (mut master, mut tty) = test_tty()?;
    let responder = std::thread::spawn(move || -> io::Result<()> {
        std::thread::sleep(Duration::from_millis(/*millis*/ 150));
        master.write_all(b"middle")?;
        std::thread::sleep(Duration::from_millis(/*millis*/ 150));
        master.write_all(b"\x1b[201~after")
    });
    let mut buffer = b"\x1b[200~before".to_vec();

    let result = finish_open_startup_paste_with_timeout(
        &mut tty,
        &mut buffer,
        Duration::from_millis(/*millis*/ 250),
    );
    responder
        .join()
        .map_err(|_| io::Error::other("terminal responder panicked"))??;
    result?;

    assert_eq!(buffer, b"\x1b[200~beforemiddle\x1b[201~after");
    Ok(())
}

#[test]
fn startup_probe_handoff_timeout_tracks_input_progress() -> io::Result<()> {
    let (mut master, mut tty) = test_tty()?;
    master.write_all(
        b"\x1b[20;1R\x1b]10;rgb:eeee/eeee/eeee\x1b\\\x1b]11;rgb:1111/1111/1111\x1b\\draft\x1b[",
    )?;
    let responder = std::thread::spawn(move || -> io::Result<()> {
        std::thread::sleep(Duration::from_millis(/*millis*/ 150));
        master.write_all(b"1;")?;
        std::thread::sleep(Duration::from_millis(/*millis*/ 150));
        master.write_all(b"2A")
    });

    let probe = read_startup_probe(
        &mut tty,
        Duration::from_millis(/*millis*/ 250),
        StartupKeyboardEnhancementProbe::Skip,
    );
    responder
        .join()
        .map_err(|_| io::Error::other("terminal responder panicked"))??;
    let probe = probe?;

    assert_eq!(
        probe.input,
        vec![
            StartupInput::Plain(b"draft".to_vec()),
            StartupInput::Key(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Up,
                crossterm::event::KeyModifiers::SHIFT,
            )),
        ]
    );
    Ok(())
}

#[test]
fn queued_modified_f3_cannot_satisfy_the_cursor_probe() -> io::Result<()> {
    let (mut master, mut tty) = test_tty()?;

    master.write_all(b"\x1b[1;2Rdraft")?;
    if !tty.poll_readable(Duration::from_secs(/*secs*/ 5))? {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "queued key did not reach the PTY reader",
        ));
    }
    let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(/*bound*/ 0);
    let responder = std::thread::spawn(move || -> io::Result<()> {
        ready_tx
            .send(())
            .map_err(|_| io::Error::other("probe test stopped before responder was ready"))?;
        let mut query = Vec::new();
        let mut chunk = [0_u8; 128];
        while !query
            .windows(b"\x1b[c".len())
            .any(|window| window == b"\x1b[c")
        {
            let count = master.read(&mut chunk)?;
            if count == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "terminal query stream closed",
                ));
            }
            query.extend_from_slice(&chunk[..count]);
        }
        master.write_all(
            b"\x1b[1;2R\x1b[20;1R\x1b]10;rgb:eeee/eeee/eeee\x1b\\\x1b]11;rgb:1111/1111/1111\x1b\\\x1b[?7u\x1b[?64;1;2c",
        )?;
        std::thread::sleep(Duration::from_millis(/*millis*/ 50));
        Ok(())
    });
    ready_rx
        .recv()
        .map_err(|_| io::Error::other("terminal responder stopped before startup probe"))?;

    let probe = startup_with_tty(
        &mut tty,
        Duration::from_secs(/*secs*/ 5),
        StartupKeyboardEnhancementProbe::Query,
        b"early".to_vec(),
        /*initial_input_truncated*/ false,
    )?;
    responder
        .join()
        .map_err(|_| io::Error::other("terminal responder panicked"))??;

    assert_eq!(probe.cursor_position, Some(Position { x: 0, y: 19 }));
    assert_eq!(
        probe.input,
        vec![
            StartupInput::Plain(b"early".to_vec()),
            StartupInput::Key(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::F(3),
                crossterm::event::KeyModifiers::SHIFT,
            )),
            StartupInput::Plain(b"draft".to_vec()),
            StartupInput::Key(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::F(3),
                crossterm::event::KeyModifiers::SHIFT,
            )),
        ]
    );
    Ok(())
}

#[test]
fn parses_cursor_position_as_zero_based() {
    assert_eq!(
        parse_cursor_position(b"\x1B[20;10R"),
        Some(Position { x: 9, y: 19 })
    );
    assert_eq!(
        parse_cursor_position(b"\x1B[I\x1B[20;10R"),
        Some(Position { x: 9, y: 19 })
    );
}

#[test]
fn startup_cursor_position_ignores_response_shaped_function_keys() {
    assert_eq!(
        parse_cursor_position_with_column(b"\x1B[1;2R\x1B[20;1R", Some(/*column*/ 1)),
        Some(Position { x: 0, y: 19 })
    );
}

#[test]
fn startup_cursor_position_ignores_responses_inside_paste() {
    let pasted_response = b"\x1b[200~log: \x1b[999;1R\x1b[201~";
    assert_eq!(
        parse_cursor_position_with_column(pasted_response, Some(/*column*/ 1)),
        None
    );

    let mut buffer = pasted_response.to_vec();
    buffer.extend_from_slice(b"\x1b[20;1R");
    assert_eq!(
        parse_cursor_position_with_column(&buffer, Some(/*column*/ 1)),
        Some(Position { x: 0, y: 19 })
    );
}

#[test]
fn parses_keyboard_enhancement_flags_and_pda_fallback() {
    assert_eq!(
        parse_keyboard_enhancement_support(b"\x1B[?7u"),
        KeyboardProbeState::Supported
    );
    assert_eq!(
        parse_keyboard_enhancement_support(b"\x1B[?64;1;2c"),
        KeyboardProbeState::UnsupportedFallback
    );
    assert_eq!(
        parse_keyboard_enhancement_support(b"\x1B[?64;1;2c\x1B[?7u"),
        KeyboardProbeState::SupportedAndFallback
    );
    assert_eq!(
        parse_keyboard_enhancement_support(b"\x1B[?7u\x1B[?64;1;2c"),
        KeyboardProbeState::SupportedAndFallback
    );
    assert_eq!(
        parse_keyboard_enhancement_support(b""),
        KeyboardProbeState::Pending
    );
}

#[test]
fn keyboard_probe_ignores_responses_inside_bracketed_paste() {
    let pasted_responses = b"\x1b[200~\x1B[?7u\x1B[?64;1;2c\x1b[201~";
    assert_eq!(
        parse_keyboard_enhancement_support(pasted_responses),
        KeyboardProbeState::Pending
    );

    let mut buffer = pasted_responses.to_vec();
    buffer.extend_from_slice(b"\x1B[?7u\x1B[?64;1;2c");
    assert_eq!(
        parse_keyboard_enhancement_support(&buffer),
        KeyboardProbeState::SupportedAndFallback
    );
}

#[test]
fn startup_probe_parses_batched_terminal_responses() {
    let mut probe = StartupProbe {
        cursor_position: None,
        default_colors: None,
        keyboard_enhancement_supported: None,
        input: Vec::new(),
    };
    let mut saw_supported_keyboard = false;
    update_startup_probe(
            &mut probe,
            &mut saw_supported_keyboard,
            b"draft\x1B[20;1R\x1B]11;rgb:1111/1111/1111\x07\x1B[?64;1;2c\x1B]10;rgb:eeee/eeee/eeee\x1B\\\x1B[?7u",
            StartupKeyboardEnhancementProbe::Query,
        );

    assert_eq!(
        probe,
        StartupProbe {
            cursor_position: Some(Position { x: 0, y: 19 }),
            default_colors: Some(DefaultColors {
                fg: (238, 238, 238),
                bg: (17, 17, 17),
            }),
            keyboard_enhancement_supported: Some(true),
            input: Vec::new(),
        }
    );
    assert!(startup_probe_complete(
        &probe,
        StartupKeyboardEnhancementProbe::Query
    ));
}
