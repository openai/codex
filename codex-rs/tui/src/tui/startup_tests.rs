use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;

use super::MAX_STARTUP_INPUT_CHARS;
use super::StartupActionLatch;
use super::StartupInputBuffer;
use super::startup_action_matches;
use crate::key_hint::KeyBinding;

trait StartupInputBufferTestExt {
    fn take_text(&mut self) -> Option<String>;
}

impl StartupInputBufferTestExt for StartupInputBuffer {
    fn take_text(&mut self) -> Option<String> {
        self.take_text_excluding_submission_bindings(&[
            crate::key_hint::plain(KeyCode::Enter),
            crate::key_hint::plain(KeyCode::Tab),
        ])
    }
}

#[cfg(unix)]
const PTY_CHILD_ENV: &str = "CODEX_TUI_STARTUP_PTY_CHILD";
#[cfg(unix)]
const PTY_SYNC_FD_ENV: &str = "CODEX_TUI_STARTUP_PTY_SYNC_FD";

#[test]
fn startup_input_keeps_text_without_replaying_actions() {
    let mut input = StartupInputBuffer::default();
    for event in [
        Event::Key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE)),
        Event::Key(KeyEvent::new(KeyCode::Char('I'), KeyModifiers::SHIFT)),
        Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
        Event::Key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE)),
        Event::Paste("ello\r\n\tworld".to_string()),
        Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        Event::Key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
        Event::Key(KeyEvent::new_with_kind(
            KeyCode::Char('!'),
            KeyModifiers::NONE,
            KeyEventKind::Release,
        )),
    ] {
        input.handle_event(event);
    }

    assert_eq!(input.take_text(), Some("hello\n\tworld".to_string()));
    let handoff = input.into_handoff();
    assert!(handoff.interrupt_requested);
    assert!(
        handoff
            .quarantined_actions
            .iter()
            .any(|action| { action.binding == crate::key_hint::plain(KeyCode::Tab) })
    );
}

#[test]
fn startup_input_filters_all_typed_custom_submit_keys_but_keeps_pasted_text() {
    let submit = crate::key_hint::plain(KeyCode::Char('x'));
    let mut input = StartupInputBuffer::default();
    input.handle_probe_input(b"axxb");
    input.handle_event(Event::Key(KeyEvent::new(
        KeyCode::Char('x'),
        KeyModifiers::NONE,
    )));
    input.push_text("x");

    assert_eq!(
        input.take_text_excluding_submission_bindings(&[submit]),
        Some("abx".to_string())
    );
    assert!(
        input
            .into_handoff()
            .quarantined_actions
            .iter()
            .any(|action| action.binding == submit)
    );
}

#[test]
fn windows_console_multiline_paste_is_coalesced_before_submission_filtering() {
    let events = ['a', '\n', 'b']
        .into_iter()
        .map(|ch| {
            Event::Key(KeyEvent::new(
                if ch == '\n' {
                    KeyCode::Enter
                } else {
                    KeyCode::Char(ch)
                },
                KeyModifiers::NONE,
            ))
        })
        .collect();

    assert_eq!(
        super::coalesce_windows_startup_pastes(events),
        vec![Event::Paste("a\nb".to_string())]
    );
}

#[test]
fn windows_console_single_enter_remains_a_submission_action() {
    let enter = Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_eq!(
        super::coalesce_windows_startup_pastes(vec![enter.clone()]),
        vec![enter]
    );
}

#[test]
fn startup_input_keeps_printable_list_accept_keys_as_composer_text() {
    let mut keymap = crate::keymap::RuntimeKeymap::defaults();
    keymap.list.accept = vec![crate::key_hint::plain(KeyCode::Char('x'))];
    let mut input = StartupInputBuffer::default();
    input.handle_probe_input(b"fix");

    assert_eq!(
        input.take_text_excluding_submission_bindings(&keymap.startup_submission_bindings()),
        Some("fix".to_string())
    );
}

#[test]
fn startup_input_retains_repeat_provenance_for_all_typed_keys() {
    let mut input = StartupInputBuffer::default();
    input.handle_probe_input(b"yfoo");

    assert_eq!(input.take_text(), Some("yfoo".to_string()));
    let repeat_actions = input.into_handoff().repeat_actions;
    assert!(repeat_actions.iter().any(|action| {
        action.binding == crate::key_hint::plain(KeyCode::Char('y')) && action.from_raw_probe
    }));
    assert!(repeat_actions.iter().any(|action| {
        action.binding == crate::key_hint::plain(KeyCode::Char('o')) && action.from_raw_probe
    }));
}

#[test]
fn startup_screen_handoff_retains_repeat_provenance_for_all_typed_keys() {
    let mut input = StartupInputBuffer::default();
    input.handle_probe_input(b"yx");

    let repeat_actions = input.into_handoff().repeat_actions;
    assert!(repeat_actions.iter().any(|action| {
        action.binding == crate::key_hint::plain(KeyCode::Char('y')) && action.from_raw_probe
    }));
    assert!(repeat_actions.iter().any(|action| {
        action.binding == crate::key_hint::plain(KeyCode::Char('x')) && action.from_raw_probe
    }));
}

#[test]
fn startup_input_filters_trailing_raw_whitespace_aliases_for_custom_submit() {
    let submit = crate::key_hint::ctrl(KeyCode::Char('j'));
    let mut trailing = StartupInputBuffer::default();
    trailing.handle_probe_input(b"a\n");

    assert_eq!(
        trailing.take_text_excluding_submission_bindings(&[submit]),
        Some("a".to_string())
    );
    assert!(trailing.into_handoff().pending_plain_whitespace.is_empty());

    let mut internal = StartupInputBuffer::default();
    internal.handle_probe_input(b"a\nb");
    assert_eq!(
        internal.take_text_excluding_submission_bindings(&[submit]),
        Some("a\nb".to_string())
    );
    assert!(internal.into_handoff().repeat_actions.iter().any(|action| {
        action.binding == crate::key_hint::plain(KeyCode::Enter)
            && action.from_raw_probe
            && action.preserve_after_quiet
    }));

    let mut internal_tab = StartupInputBuffer::default();
    internal_tab.handle_probe_input(b"a\tb");
    assert_eq!(
        internal_tab
            .take_text_excluding_submission_bindings(&[crate::key_hint::ctrl(KeyCode::Char('i'),)]),
        Some("a\tb".to_string())
    );

    for modifiers in [KeyModifiers::CONTROL, KeyModifiers::SHIFT] {
        let mut modified_enter = StartupInputBuffer::default();
        modified_enter.handle_probe_input(b"a\nb");
        assert_eq!(
            modified_enter.take_text_excluding_submission_bindings(&[KeyBinding::new(
                KeyCode::Enter,
                modifiers,
            )]),
            Some("a\nb".to_string())
        );
    }
}

#[test]
fn startup_input_preserves_internal_whitespace_for_default_submit_keys() {
    let mut input = StartupInputBuffer::default();
    input.handle_probe_input(b"a\nb\tc\n");

    assert_eq!(
        input.take_text_excluding_submission_bindings(&[
            crate::key_hint::plain(KeyCode::Enter),
            crate::key_hint::plain(KeyCode::Tab),
        ]),
        Some("a\nb\tc".to_string())
    );
}

#[test]
fn startup_backspace_removes_typed_key_provenance() {
    let submit = crate::key_hint::plain(KeyCode::Char('x'));
    let mut input = StartupInputBuffer::default();
    input.handle_probe_input(b"ax\x7f");

    assert_eq!(
        input.take_text_excluding_submission_bindings(&[submit]),
        Some("a".to_string())
    );
    let handoff = input.into_handoff();
    assert!(
        handoff
            .quarantined_actions
            .iter()
            .all(|action| action.binding != submit)
    );
    assert_ne!(handoff.trailing_printable_action, Some((submit, true)));
}

#[test]
fn startup_raw_backspace_records_repeat_provenance() {
    let mut input = StartupInputBuffer::default();
    input.handle_probe_input(b"a\x7f");

    assert_eq!(input.take_text(), None);
    let handoff = input.into_handoff();
    assert!(handoff.quarantined_actions.iter().any(|action| {
        action.binding == crate::key_hint::plain(KeyCode::Backspace) && action.from_raw_probe
    }));
    assert!(startup_action_matches(
        crate::key_hint::plain(KeyCode::Backspace),
        /*from_raw_probe*/ true,
        crate::key_hint::ctrl(KeyCode::Char('h')),
    ));
}

#[test]
fn startup_raw_unicode_uppercase_matches_crossterm_shift() {
    let mut input = StartupInputBuffer::default();
    input.handle_probe_input("É".as_bytes());

    assert_eq!(input.take_text(), Some("É".to_string()));
    assert!(input.into_handoff().repeat_actions.iter().any(|action| {
        action.binding
            == crate::key_hint::KeyBinding::from_event(KeyEvent::new(
                KeyCode::Char('É'),
                KeyModifiers::SHIFT,
            ))
            && action.from_raw_probe
    }));
}

#[test]
fn startup_backspace_removes_one_displayed_grapheme() {
    let mut input = StartupInputBuffer::default();
    input.handle_probe_input("a👍🏽".as_bytes());
    input.handle_probe_input(b"\x7f");

    assert_eq!(input.take_text(), Some("a".to_string()));
}

#[cfg(windows)]
#[test]
fn startup_input_treats_altgr_characters_as_text() {
    let mut input = StartupInputBuffer::default();
    for ch in ['@', 'c'] {
        input.handle_event(Event::Key(KeyEvent::new(
            KeyCode::Char(ch),
            KeyModifiers::CONTROL | KeyModifiers::ALT,
        )));
    }

    assert_eq!(input.take_text(), Some("@c".to_string()));
    let handoff = input.into_handoff();
    assert!(!handoff.interrupt_requested);
}

#[test]
fn startup_input_preserves_internal_plain_whitespace_but_drops_trailing_actions() {
    let mut input = StartupInputBuffer::default();
    for code in [
        KeyCode::Char('a'),
        KeyCode::Enter,
        KeyCode::Char('b'),
        KeyCode::Tab,
        KeyCode::Char('c'),
        KeyCode::Enter,
    ] {
        input.handle_event(Event::Key(KeyEvent::new(code, KeyModifiers::NONE)));
    }

    assert_eq!(input.take_text(), Some("a\nb\tc".to_string()));
}

#[test]
fn startup_key_release_clears_the_latch_but_keeps_pending_whitespace() {
    let mut input = StartupInputBuffer::default();
    input.handle_event(Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )));
    input.handle_event(Event::Key(KeyEvent::new_with_kind(
        KeyCode::Enter,
        KeyModifiers::NONE,
        KeyEventKind::Release,
    )));

    let handoff = input.into_handoff();
    assert!(handoff.quarantined_actions.is_empty());
    assert_eq!(handoff.pending_plain_whitespace, "\n");
}

#[test]
fn startup_key_release_does_not_recreate_submit_repeat_provenance() {
    let submit = crate::key_hint::plain(KeyCode::Enter);
    let mut input = StartupInputBuffer::default();
    input.handle_event(Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )));
    input.handle_event(Event::Key(KeyEvent::new_with_kind(
        KeyCode::Enter,
        KeyModifiers::NONE,
        KeyEventKind::Release,
    )));

    assert_eq!(
        input.take_text_excluding_submission_bindings(&[submit]),
        None
    );
    let handoff = input.into_handoff();
    assert!(handoff.quarantined_actions.is_empty());
    assert!(handoff.repeat_actions.is_empty());
}

#[test]
fn released_printable_submit_is_filtered_without_latching() {
    let submit = crate::key_hint::plain(KeyCode::Char('x'));
    let mut input = StartupInputBuffer::default();
    for event in [
        KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
        KeyEvent::new_with_kind(
            KeyCode::Char('x'),
            KeyModifiers::NONE,
            KeyEventKind::Release,
        ),
        KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE),
    ] {
        input.handle_event(Event::Key(event));
    }

    assert_eq!(
        input.take_text_excluding_submission_bindings(&[submit]),
        Some("ab".to_string())
    );
    let handoff = input.into_handoff();
    assert!(
        handoff
            .quarantined_actions
            .iter()
            .all(|action| action.binding != submit)
    );
    assert!(
        handoff
            .repeat_actions
            .iter()
            .all(|action| action.binding != submit)
    );
}

#[test]
fn startup_input_preserves_internal_whitespace_across_key_release() {
    let mut input = StartupInputBuffer::default();
    for event in [
        Event::Key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE)),
        Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        Event::Key(KeyEvent::new_with_kind(
            KeyCode::Enter,
            KeyModifiers::NONE,
            KeyEventKind::Release,
        )),
        Event::Key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE)),
    ] {
        input.handle_event(event);
    }

    assert_eq!(input.take_text(), Some("a\nb".to_string()));
}

#[test]
fn startup_control_release_does_not_reconstruct_a_control_latch() {
    let mut input = StartupInputBuffer::default();
    input.handle_event(Event::Key(KeyEvent::new(
        KeyCode::Char('c'),
        KeyModifiers::CONTROL,
    )));
    input.handle_event(Event::Key(KeyEvent::new_with_kind(
        KeyCode::Char('c'),
        KeyModifiers::CONTROL,
        KeyEventKind::Release,
    )));

    let handoff = input.into_handoff();
    assert!(handoff.interrupt_requested);
    assert!(handoff.quarantined_actions.is_empty());
}

#[test]
fn startup_distinct_actions_are_all_quarantined() {
    let mut input = StartupInputBuffer::default();
    input.handle_event(Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )));
    input.handle_event(Event::Key(KeyEvent::new(
        KeyCode::Right,
        KeyModifiers::NONE,
    )));
    input.handle_event(Event::Key(KeyEvent::new(
        KeyCode::Char('x'),
        KeyModifiers::NONE,
    )));

    assert_eq!(input.take_text(), Some("x".to_string()));
    let handoff = input.into_handoff();
    assert_eq!(handoff.quarantined_actions.len(), 2);
    assert!(
        handoff
            .quarantined_actions
            .iter()
            .any(|action| { action.binding == crate::key_hint::plain(KeyCode::Enter) })
    );
    assert!(
        handoff
            .quarantined_actions
            .iter()
            .any(|action| { action.binding == crate::key_hint::plain(KeyCode::Right) })
    );
    assert_eq!(
        handoff.trailing_printable_action,
        Some((crate::key_hint::plain(KeyCode::Char('x')), false))
    );
    assert_eq!(handoff.pending_plain_whitespace, "");
}

#[test]
fn immediate_printable_screen_repeats_remain_quarantined() {
    let key_event = KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE);
    let mut latch = StartupActionLatch::default();
    latch.record(key_event);
    let mut input = StartupInputBuffer::default();

    assert!(latch.drain_into(&mut input));
    input.handle_probe_input(b"lll");
    assert_eq!(input.take_text(), None);
    assert_eq!(input.into_handoff().quarantined_actions.len(), 1);
}

#[test]
fn startup_screen_actions_age_at_a_reader_drained_boundary() {
    let key_event = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
    let mut latch = StartupActionLatch::default();
    latch.record(key_event);
    latch.note_input_drained();
    let mut input = StartupInputBuffer::default();

    assert!(latch.drain_into(&mut input));
    assert_eq!(input.quarantined_actions.len(), 1);
    assert!(input.quarantined_actions[0].quiet_elapsed);

    latch.record(key_event);
    let mut repeated = StartupInputBuffer::default();
    assert!(latch.drain_into(&mut repeated));
    assert!(!repeated.quarantined_actions[0].quiet_elapsed);
}

#[test]
fn printable_screen_text_does_not_clear_another_held_action() {
    let mut latch = StartupActionLatch::default();
    latch.record(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE));
    latch.record(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    let mut input = StartupInputBuffer::default();

    assert!(latch.drain_into(&mut input));
    for action in &mut input.quarantined_actions {
        action.quiet_elapsed = true;
    }
    input.handle_probe_input(b"l");
    assert_eq!(input.take_text(), Some("l".to_string()));
    let handoff = input.into_handoff();
    assert_eq!(handoff.quarantined_actions.len(), 1);
    assert!(
        handoff
            .quarantined_actions
            .iter()
            .any(|action| { action.binding == crate::key_hint::plain(KeyCode::Enter) })
    );
    assert_eq!(
        handoff.trailing_printable_action,
        Some((crate::key_hint::plain(KeyCode::Char('l')), true))
    );
}

#[test]
fn startup_probe_input_preserves_internal_plain_whitespace_across_phases() {
    let mut input = StartupInputBuffer::default();
    input.handle_probe_input(b"a\r\n");
    input.handle_event(Event::Key(KeyEvent::new(
        KeyCode::Char('b'),
        KeyModifiers::NONE,
    )));
    input.handle_probe_input(b"\t");

    assert_eq!(input.take_text(), Some("a\nb".to_string()));
}

#[cfg(unix)]
#[test]
fn startup_probe_preserves_bracketed_paste_whitespace() {
    let mut input = StartupInputBuffer::default();
    input.handle_startup_probe_input(&[crate::terminal_probe::StartupInput::Paste(
        b"a\r\n\t".to_vec(),
    )]);

    assert_eq!(input.take_text(), Some("a\n\t".to_string()));
}

#[test]
fn startup_input_is_bounded() {
    let mut input = StartupInputBuffer::default();
    input.handle_event(Event::Paste("x".repeat(MAX_STARTUP_INPUT_CHARS + 1)));

    assert_eq!(input.take_text(), Some("x".repeat(MAX_STARTUP_INPUT_CHARS)));
}

#[test]
fn startup_input_applies_edits_across_capture_phases() {
    let mut input = StartupInputBuffer::default();
    input.handle_event(Event::Paste("draft".to_string()));

    input.handle_probe_input(b"\x7f\x7fph");
    input.handle_event(Event::Key(KeyEvent::new(
        KeyCode::Backspace,
        KeyModifiers::NONE,
    )));

    assert_eq!(input.take_text(), Some("drap".to_string()));
}

#[test]
fn startup_probe_controls_survive_text_extraction() {
    let mut input = StartupInputBuffer::default();
    input.handle_probe_input(b"draft\x03\x1a");

    assert_eq!(input.take_text(), Some("draft".to_string()));
    let handoff = input.into_handoff();
    assert!(handoff.interrupt_requested);
    assert!(handoff.suspend_requested);
    assert!(handoff.quarantined_actions.iter().any(|action| {
        action.binding == crate::key_hint::ctrl(KeyCode::Char('c')) && action.from_raw_probe
    }));
    assert!(handoff.quarantined_actions.iter().any(|action| {
        action.binding == crate::key_hint::ctrl(KeyCode::Char('z')) && action.from_raw_probe
    }));
}

#[cfg(unix)]
#[test]
fn startup_probe_preserves_enhanced_text_and_action_identity() {
    let mut input = StartupInputBuffer::default();
    input.handle_startup_probe_input(&[
        crate::terminal_probe::StartupInput::Key(KeyEvent::new(
            KeyCode::Char('a'),
            KeyModifiers::NONE,
        )),
        crate::terminal_probe::StartupInput::Key(KeyEvent::new(KeyCode::F(3), KeyModifiers::SHIFT)),
    ]);

    assert_eq!(input.take_text(), Some("a".to_string()));
    let handoff = input.into_handoff();
    assert_eq!(
        handoff.trailing_printable_action,
        Some((crate::key_hint::plain(KeyCode::Char('a')), false))
    );
    assert!(
        handoff
            .quarantined_actions
            .iter()
            .any(|action| { action.binding == crate::key_hint::shift(KeyCode::F(3)) })
    );
}

#[cfg(unix)]
#[test]
fn unknown_probe_action_keeps_startup_submission_quarantined() {
    let mut input = StartupInputBuffer::default();
    input.handle_startup_probe_input(&[
        crate::terminal_probe::StartupInput::Plain(b"draft".to_vec()),
        crate::terminal_probe::StartupInput::UnknownAction,
    ]);

    assert_eq!(input.take_text(), Some("draft".to_string()));
    let handoff = input.into_handoff();
    assert!(handoff.unknown_action_seen);
    assert!(
        handoff
            .quarantined_actions
            .iter()
            .all(|action| { action.binding != crate::key_hint::plain(KeyCode::Null) })
    );
}

#[test]
fn raw_probe_action_latches_match_protocol_aliases() {
    let enter = crate::key_hint::plain(KeyCode::Enter);
    let tab = crate::key_hint::plain(KeyCode::Tab);

    assert!(startup_action_matches(
        enter,
        /*from_raw_probe*/ true,
        crate::key_hint::ctrl(KeyCode::Char('m')),
    ));
    assert!(startup_action_matches(
        enter,
        /*from_raw_probe*/ true,
        crate::key_hint::ctrl(KeyCode::Char('j')),
    ));
    assert!(startup_action_matches(
        enter,
        /*from_raw_probe*/ true,
        KeyBinding::new(KeyCode::Enter, KeyModifiers::CONTROL),
    ));
    assert!(startup_action_matches(
        enter,
        /*from_raw_probe*/ true,
        KeyBinding::new(KeyCode::Enter, KeyModifiers::SHIFT),
    ));
    assert!(startup_action_matches(
        tab,
        /*from_raw_probe*/ true,
        crate::key_hint::ctrl(KeyCode::Char('i')),
    ));
    assert!(!startup_action_matches(
        enter,
        /*from_raw_probe*/ false,
        crate::key_hint::ctrl(KeyCode::Char('j')),
    ));
}

#[cfg(unix)]
#[test]
fn startup_paste_control_bytes_are_not_replayed_as_actions() {
    let mut input = StartupInputBuffer::default();
    input.handle_startup_probe_input(&[crate::terminal_probe::StartupInput::Paste(
        b"a\x03\x1ab".to_vec(),
    )]);

    assert_eq!(input.take_text(), Some("ab".to_string()));
    let handoff = input.into_handoff();
    assert!(!handoff.interrupt_requested);
    assert!(!handoff.suspend_requested);
}

#[cfg(unix)]
#[test]
fn prepared_terminal_preserves_and_restores_real_pty_state() {
    use std::fs::File;
    use std::io::Read;
    use std::io::Write;
    use std::os::fd::AsRawFd;
    use std::os::fd::FromRawFd;
    use std::process::Command;
    use std::process::Stdio;
    use std::time::Duration;
    use std::time::Instant;

    let mut master_fd = -1;
    let mut slave_fd = -1;
    let open_result = unsafe {
        libc::openpty(
            &mut master_fd,
            &mut slave_fd,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    assert_eq!(
        open_result,
        0,
        "openpty failed: {}",
        std::io::Error::last_os_error()
    );
    let mut master = unsafe { File::from_raw_fd(master_fd) };
    let slave = unsafe { File::from_raw_fd(slave_fd) };
    let mut original_terminal = unsafe { std::mem::zeroed() };
    assert_eq!(
        unsafe { libc::tcgetattr(slave.as_raw_fd(), &mut original_terminal) },
        0,
        "tcgetattr failed: {}",
        std::io::Error::last_os_error()
    );

    let mut sync_pipe = [-1; 2];
    let pipe_result = unsafe { libc::pipe(sync_pipe.as_mut_ptr()) };
    assert_eq!(
        pipe_result,
        0,
        "pipe failed: {}",
        std::io::Error::last_os_error()
    );
    let sync_reader = unsafe { File::from_raw_fd(sync_pipe[0]) };
    let mut sync_writer = unsafe { File::from_raw_fd(sync_pipe[1]) };

    let duplicate_stdio = || {
        let fd = unsafe { libc::dup(slave.as_raw_fd()) };
        assert_ne!(fd, -1, "dup failed: {}", std::io::Error::last_os_error());
        Stdio::from(unsafe { File::from_raw_fd(fd) })
    };
    let mut child = Command::new(std::env::current_exe().expect("current test executable"))
        .arg("--exact")
        .arg("tui::startup::tests::prepared_terminal_pty_child")
        .arg("--nocapture")
        .env(PTY_CHILD_ENV, "1")
        .env(PTY_SYNC_FD_ENV, sync_reader.as_raw_fd().to_string())
        .stdin(duplicate_stdio())
        .stdout(duplicate_stdio())
        .stderr(duplicate_stdio())
        .spawn()
        .expect("spawn PTY child");
    drop(sync_reader);

    let original_flags = unsafe { libc::fcntl(master.as_raw_fd(), libc::F_GETFL) };
    assert_ne!(original_flags, -1);
    assert_ne!(
        unsafe {
            libc::fcntl(
                master.as_raw_fd(),
                libc::F_SETFL,
                original_flags | libc::O_NONBLOCK,
            )
        },
        -1
    );

    let mut transcript = Vec::new();
    wait_for_pty_marker(
        &mut master,
        &mut transcript,
        b"CODEX_PTY_PREPARED",
        Duration::from_secs(/*secs*/ 5),
    );
    master.write_all(b"draft").expect("queue startup draft");
    sync_writer.write_all(b"1").expect("release activation");

    wait_for_pty_marker(
        &mut master,
        &mut transcript,
        b"\x1b[6n",
        Duration::from_secs(/*secs*/ 5),
    );
    assert_eq!(
        unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGTSTP) },
        0,
        "failed to suspend PTY child: {}",
        std::io::Error::last_os_error()
    );
    let deadline = Instant::now() + Duration::from_secs(/*secs*/ 5);
    loop {
        let mut status = 0;
        let result = unsafe {
            libc::waitpid(
                child.id() as libc::pid_t,
                &mut status,
                libc::WUNTRACED | libc::WNOHANG,
            )
        };
        assert_ne!(
            result,
            -1,
            "waitpid failed: {}",
            std::io::Error::last_os_error()
        );
        if result != 0 && libc::WIFSTOPPED(status) {
            assert_eq!(libc::WSTOPSIG(status), libc::SIGSTOP);
            break;
        }
        assert!(Instant::now() < deadline, "PTY child did not suspend");
        std::thread::sleep(Duration::from_millis(/*millis*/ 10));
    }
    wait_for_pty_marker(
        &mut master,
        &mut transcript,
        b"\x1b[?2004l",
        Duration::from_secs(/*secs*/ 5),
    );
    let mut suspended_terminal = unsafe { std::mem::zeroed() };
    assert_eq!(
        unsafe { libc::tcgetattr(slave.as_raw_fd(), &mut suspended_terminal) },
        0
    );
    assert_eq!(
        suspended_terminal.c_lflag & (libc::ICANON | libc::ECHO | libc::ISIG),
        original_terminal.c_lflag & (libc::ICANON | libc::ECHO | libc::ISIG)
    );
    assert_eq!(
        suspended_terminal.c_oflag & libc::OPOST,
        original_terminal.c_oflag & libc::OPOST
    );
    assert_eq!(
        unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGCONT) },
        0,
        "failed to resume PTY child: {}",
        std::io::Error::last_os_error()
    );
    let deadline = Instant::now() + Duration::from_secs(/*secs*/ 5);
    loop {
        let mut resumed_terminal = unsafe { std::mem::zeroed() };
        assert_eq!(
            unsafe { libc::tcgetattr(slave.as_raw_fd(), &mut resumed_terminal) },
            0
        );
        if resumed_terminal.c_lflag & (libc::ICANON | libc::ECHO) == 0
            && resumed_terminal.c_lflag & libc::ISIG != 0
            && resumed_terminal.c_oflag & libc::OPOST != 0
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "PTY child did not restore startup capture after resume"
        );
        std::thread::sleep(Duration::from_millis(/*millis*/ 10));
    }
    master
        .write_all(
            b"\x1b[1;1R\x1b]10;rgb:ffff/ffff/ffff\x1b\\\x1b]11;rgb:0000/0000/0000\x1b\\\x1b[?7u\x1b[?64;1;2c",
        )
        .expect("write terminal probe responses");

    wait_for_pty_marker(
        &mut master,
        &mut transcript,
        b"CODEX_PTY_ACTIVATED_GAP",
        Duration::from_secs(/*secs*/ 5),
    );
    master
        .write_all(b" later")
        .expect("queue post-activation startup draft");
    sync_writer
        .write_all(b"5")
        .expect("release final input handoff");

    wait_for_pty_marker(
        &mut master,
        &mut transcript,
        b"CODEX_PTY_ACTIVE",
        Duration::from_secs(/*secs*/ 5),
    );
    master.write_all(b"\n").expect("queue exit action");
    sync_writer.write_all(b"2").expect("release restore");

    wait_for_pty_marker(
        &mut master,
        &mut transcript,
        b"CODEX_PTY_PREPARED_DROP",
        Duration::from_secs(/*secs*/ 5),
    );
    master
        .write_all(b"discarded\n")
        .expect("queue abandoned input");
    sync_writer.write_all(b"3").expect("release guard drop");

    wait_for_pty_marker(
        &mut master,
        &mut transcript,
        b"CODEX_PTY_PARTIAL_INPUT",
        Duration::from_secs(/*secs*/ 5),
    );
    master
        .write_all(b"\xc3")
        .expect("queue incomplete startup input");
    sync_writer
        .write_all(b"4")
        .expect("release partial-input activation");

    wait_for_pty_marker(
        &mut master,
        &mut transcript,
        b"CODEX_PTY_OK",
        Duration::from_secs(/*secs*/ 5),
    );
    drop(slave);
    let deadline = Instant::now() + Duration::from_secs(/*secs*/ 5);
    let status = loop {
        if let Some(status) = child.try_wait().expect("poll PTY child") {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "PTY child did not exit: {}",
            String::from_utf8_lossy(&transcript)
        );
        std::thread::sleep(Duration::from_millis(/*millis*/ 10));
    };
    assert!(
        status.success(),
        "PTY child failed with {status}: {}",
        String::from_utf8_lossy(&transcript)
    );

    fn wait_for_pty_marker(
        master: &mut File,
        transcript: &mut Vec<u8>,
        marker: &[u8],
        timeout: Duration,
    ) {
        let deadline = Instant::now() + timeout;
        let mut chunk = [0_u8; 1024];
        while !transcript
            .windows(marker.len())
            .any(|window| window == marker)
        {
            match master.read(&mut chunk) {
                Ok(0) => panic!(
                    "PTY closed before marker {:?}: {}",
                    String::from_utf8_lossy(marker),
                    String::from_utf8_lossy(transcript)
                ),
                Ok(read) => transcript.extend_from_slice(&chunk[..read]),
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(
                        Instant::now() < deadline,
                        "timed out waiting for PTY marker {:?}: {}",
                        String::from_utf8_lossy(marker),
                        String::from_utf8_lossy(transcript)
                    );
                    std::thread::sleep(Duration::from_millis(/*millis*/ 10));
                }
                Err(err) => panic!("failed reading PTY: {err}"),
            }
        }
    }
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prepared_terminal_pty_child() {
    use std::fs::File;
    use std::io::Read;
    use std::io::Write;
    use std::os::fd::FromRawFd;
    use std::time::Duration;
    use std::time::Instant;

    if std::env::var_os(PTY_CHILD_ENV).is_none() {
        return;
    }
    let sync_fd = std::env::var(PTY_SYNC_FD_ENV)
        .expect("sync fd")
        .parse::<libc::c_int>()
        .expect("numeric sync fd");
    let mut sync = unsafe { File::from_raw_fd(sync_fd) };
    let original = terminal_attributes();
    let original_fd_flags = unsafe { libc::fcntl(libc::STDIN_FILENO, libc::F_GETFD) };
    assert_ne!(original_fd_flags, -1);
    let original_file_status_flags = unsafe { libc::fcntl(libc::STDIN_FILENO, libc::F_GETFL) };
    assert_ne!(original_file_status_flags, -1);
    assert_terminal_flags_enabled(&original);

    let prepared = super::PreparedTerminal::prepare().expect("prepare terminal");
    assert_startup_capture_flags(&terminal_attributes());
    assert_ne!(
        unsafe { libc::fcntl(libc::STDIN_FILENO, libc::F_GETFD) } & libc::FD_CLOEXEC,
        0
    );
    write_marker("CODEX_PTY_PREPARED");
    wait_for_parent(&mut sync, b'1');
    let deadline = Instant::now() + Duration::from_secs(/*secs*/ 1);
    while pending_terminal_bytes() != 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(/*millis*/ 10));
    }
    assert_eq!(
        pending_terminal_bytes(),
        0,
        "early startup reader did not continuously drain the tty"
    );

    let mut initialized = prepared.activate().expect("activate terminal");
    assert_eq!(
        unsafe { libc::fcntl(libc::STDIN_FILENO, libc::F_GETFL) },
        original_file_status_flags
    );
    let deadline = Instant::now() + Duration::from_secs(/*secs*/ 1);
    while !startup_capture_flags_enabled(&terminal_attributes()) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(/*millis*/ 10));
    }
    assert_startup_capture_flags(&terminal_attributes());
    write_marker("CODEX_PTY_ACTIVATED_GAP");
    wait_for_parent(&mut sync, b'5');
    let deadline = Instant::now() + Duration::from_secs(/*secs*/ 1);
    while pending_terminal_bytes() != 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(/*millis*/ 10));
    }
    assert_eq!(
        pending_terminal_bytes(),
        0,
        "post-activation startup reader did not continuously drain the tty"
    );
    super::capture_startup_input_for_full_modes(&mut initialized.startup_input)
        .expect("capture post-activation input and activate full terminal modes");
    super::finish_startup_input_capture().expect("finish startup input capture");
    assert_eq!(
        unsafe { libc::fcntl(libc::STDIN_FILENO, libc::F_GETFD) } & libc::FD_CLOEXEC,
        0
    );
    let active = terminal_attributes();
    assert_eq!(active.c_lflag & (libc::ICANON | libc::ECHO | libc::ISIG), 0);
    assert_eq!(active.c_oflag & libc::OPOST, 0);
    assert_eq!(
        initialized.startup_input.take_text().as_deref(),
        Some("draft later")
    );
    write_marker("CODEX_PTY_ACTIVE");
    wait_for_parent(&mut sync, b'2');

    crate::tui::restore_after_exit_best_effort().expect("restore terminal");
    assert_terminal_flags_enabled(&terminal_attributes());
    assert_eq!(
        unsafe { libc::fcntl(libc::STDIN_FILENO, libc::F_GETFL) },
        original_file_status_flags
    );
    assert_terminal_input_empty();
    crate::tui::restore_after_exit_best_effort().expect("repeat terminal restore");
    assert_terminal_flags_enabled(&terminal_attributes());
    assert_terminal_input_empty();
    drop(initialized);

    let prepared = super::PreparedTerminal::prepare().expect("prepare terminal for drop");
    assert_startup_capture_flags(&terminal_attributes());
    write_marker("CODEX_PTY_PREPARED_DROP");
    wait_for_parent(&mut sync, b'3');
    drop(prepared);
    assert_terminal_flags_enabled(&terminal_attributes());
    assert_terminal_input_empty();

    let prepared = super::PreparedTerminal::prepare().expect("prepare terminal for partial input");
    assert_startup_capture_flags(&terminal_attributes());
    write_marker("CODEX_PTY_PARTIAL_INPUT");
    wait_for_parent(&mut sync, b'4');
    let initialized = prepared
        .activate()
        .expect("incomplete user input must not abort activation");
    assert_startup_capture_flags(&terminal_attributes());
    crate::tui::restore_after_exit_best_effort().expect("restore after partial input");
    drop(initialized);
    assert_terminal_flags_enabled(&terminal_attributes());
    assert_terminal_input_empty();
    write_marker("CODEX_PTY_OK");

    fn write_marker(marker: &str) {
        let mut stdout = std::io::stdout().lock();
        writeln!(stdout, "{marker}")
            .unwrap_or_else(|err| panic!("failed to write PTY marker: {err}"));
        stdout
            .flush()
            .unwrap_or_else(|err| panic!("failed to flush PTY marker: {err}"));
    }

    fn wait_for_parent(sync: &mut File, expected: u8) {
        let mut byte = [0_u8; 1];
        sync.read_exact(&mut byte).expect("read parent sync byte");
        assert_eq!(byte[0], expected);
    }

    fn terminal_attributes() -> libc::termios {
        let mut attributes = unsafe { std::mem::zeroed() };
        assert_eq!(
            unsafe { libc::tcgetattr(libc::STDIN_FILENO, &mut attributes) },
            0,
            "tcgetattr failed: {}",
            std::io::Error::last_os_error()
        );
        attributes
    }

    fn pending_terminal_bytes() -> libc::c_int {
        let mut pending = 0;
        assert_eq!(
            unsafe { libc::ioctl(libc::STDIN_FILENO, libc::FIONREAD, &mut pending) },
            0,
            "ioctl(FIONREAD) failed: {}",
            std::io::Error::last_os_error()
        );
        pending
    }

    fn assert_terminal_flags_enabled(attributes: &libc::termios) {
        assert_eq!(
            attributes.c_lflag & (libc::ICANON | libc::ECHO | libc::ISIG),
            libc::ICANON | libc::ECHO | libc::ISIG
        );
        assert_ne!(attributes.c_oflag & libc::OPOST, 0);
    }

    fn assert_startup_capture_flags(attributes: &libc::termios) {
        assert!(startup_capture_flags_enabled(attributes));
    }

    fn startup_capture_flags_enabled(attributes: &libc::termios) -> bool {
        attributes.c_lflag & (libc::ICANON | libc::ECHO) == 0
            && attributes.c_lflag & libc::ISIG != 0
            && attributes.c_oflag & libc::OPOST != 0
    }

    fn assert_terminal_input_empty() {
        let original_flags = unsafe { libc::fcntl(libc::STDIN_FILENO, libc::F_GETFL) };
        assert_ne!(original_flags, -1);
        assert_ne!(
            unsafe {
                libc::fcntl(
                    libc::STDIN_FILENO,
                    libc::F_SETFL,
                    original_flags | libc::O_NONBLOCK,
                )
            },
            -1
        );
        let mut byte = [0_u8; 1];
        let read = unsafe {
            libc::read(
                libc::STDIN_FILENO,
                byte.as_mut_ptr().cast::<libc::c_void>(),
                byte.len(),
            )
        };
        let error = std::io::Error::last_os_error();
        assert_ne!(
            unsafe { libc::fcntl(libc::STDIN_FILENO, libc::F_SETFL, original_flags) },
            -1
        );
        assert_eq!(read, -1, "unexpected queued terminal byte: {byte:?}");
        assert_eq!(error.kind(), std::io::ErrorKind::WouldBlock);
    }
}
