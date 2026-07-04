use std::io;

use pretty_assertions::assert_eq;
use ratatui::layout::Rect;

use super::AltScreenInputMode;
use super::PhysicalAltScreenTracker;
use super::ScreenCommands;
use super::ScreenSession;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Effect {
    Enter,
    Leave,
    EnableScroll,
    DisableScroll,
    EnableMouse,
    DisableMouse,
}

const ENTER_AND_LEAVE: &[Effect] = &[
    Effect::Enter,
    Effect::EnableScroll,
    Effect::DisableScroll,
    Effect::Leave,
];
const LEAVE: &[Effect] = &[Effect::DisableScroll, Effect::Leave];
const OWNED_ENTER_AND_LEAVE: &[Effect] = &[
    Effect::Enter,
    Effect::EnableMouse,
    Effect::DisableMouse,
    Effect::Leave,
];

#[derive(Default)]
struct FakeCommands {
    effects: Vec<Effect>,
    failures: Vec<Effect>,
}

impl FakeCommands {
    fn failing(failures: Vec<Effect>) -> Self {
        Self {
            failures,
            ..Self::default()
        }
    }

    fn record(&mut self, effect: Effect) -> io::Result<()> {
        self.effects.push(effect);
        if let Some(index) = self.failures.iter().position(|failure| *failure == effect) {
            self.failures.remove(index);
            return Err(io::Error::other(format!("failed {effect:?}")));
        }
        Ok(())
    }
}

impl ScreenCommands for FakeCommands {
    fn enter_alternate_screen(&mut self) -> io::Result<()> {
        self.record(Effect::Enter)
    }

    fn leave_alternate_screen(&mut self) -> io::Result<()> {
        self.record(Effect::Leave)
    }

    fn enable_alternate_scroll(&mut self) -> io::Result<()> {
        self.record(Effect::EnableScroll)
    }

    fn disable_alternate_scroll(&mut self) -> io::Result<()> {
        self.record(Effect::DisableScroll)
    }

    fn enable_mouse_capture(&mut self) -> io::Result<()> {
        self.record(Effect::EnableMouse)
    }

    fn disable_mouse_capture(&mut self) -> io::Result<()> {
        self.record(Effect::DisableMouse)
    }
}

fn acquire(session: &ScreenSession, commands: &mut FakeCommands) {
    session
        .acquire(
            commands,
            Rect {
                x: 0,
                y: 12,
                width: 80,
                height: 12,
            },
            AltScreenInputMode::AlternateScroll,
        )
        .expect("acquire");
}

fn acquire_owned(session: &ScreenSession, commands: &mut FakeCommands) {
    session
        .acquire(commands, Rect::default(), AltScreenInputMode::MouseCapture)
        .expect("acquire owned");
}

fn assert_effects(commands: &FakeCommands, expected: &[Effect]) {
    assert_eq!(commands.effects, expected);
}

#[test]
fn nested_owners_only_transition_at_outer_boundaries() {
    let session = ScreenSession::new();
    let nested = session.clone();
    let mut commands = FakeCommands::default();
    acquire(&session, &mut commands);
    acquire(&nested, &mut commands);

    nested.release(&mut commands).expect("nested release");
    assert!(session.is_active());
    session.release(&mut commands).expect("outer release");

    assert!(!session.is_active());
    assert_effects(&commands, ENTER_AND_LEAVE);
}

#[test]
fn finish_clears_all_nested_owners_with_one_physical_transition() {
    let session = ScreenSession::new();
    let mut commands = FakeCommands::default();
    acquire(&session, &mut commands);
    acquire(&session, &mut commands);
    commands.effects.clear();

    session.finish_commands(&mut commands).expect("finish");
    session
        .finish_commands(&mut commands)
        .expect("repeat finish");

    assert!(!session.is_active());
    assert_effects(&commands, LEAVE);
}

#[test]
fn owned_root_keeps_mouse_capture_across_nested_overlay_ownership() {
    let session = ScreenSession::new();
    let mut commands = FakeCommands::default();
    acquire_owned(&session, &mut commands);
    acquire(&session, &mut commands);

    session.release(&mut commands).expect("nested release");
    session.release(&mut commands).expect("owned release");

    assert!(!session.is_active());
    assert_effects(&commands, OWNED_ENTER_AND_LEAVE);
}

#[test]
fn emergency_restore_only_leaves_a_physically_owned_screen() {
    let tracker = PhysicalAltScreenTracker::new();
    let mut inactive_output = Vec::new();
    tracker
        .restore(&mut inactive_output)
        .expect("restore inactive screen");
    assert_eq!(inactive_output, Vec::<u8>::new());

    tracker.mark_entered();
    tracker.mark_alternate_scroll_enabled();

    let mut restore_output = Vec::new();
    tracker
        .restore(&mut restore_output)
        .expect("restore active screen");
    let mut expected_output = Vec::new();
    ratatui::crossterm::execute!(
        expected_output,
        super::DisableAlternateScroll,
        crossterm::terminal::LeaveAlternateScreen
    )
    .expect("write expected restore sequence");
    assert_eq!(restore_output, expected_output);

    tracker
        .restore(&mut restore_output)
        .expect("repeat restore");
    assert_eq!(restore_output, expected_output);
}

#[cfg(unix)]
#[test]
fn emergency_restore_disables_tracked_mouse_capture() {
    let tracker = PhysicalAltScreenTracker::new();
    tracker.mark_entered();
    tracker.mark_mouse_capture_enabled();

    let mut restore_output = Vec::new();
    tracker
        .restore(&mut restore_output)
        .expect("restore mouse-captured screen");
    let mut expected_output = Vec::new();
    ratatui::crossterm::execute!(
        expected_output,
        crossterm::event::DisableMouseCapture,
        crossterm::terminal::LeaveAlternateScreen
    )
    .expect("write expected restore sequence");
    assert_eq!(restore_output, expected_output);
}

#[test]
fn temporary_suspend_preserves_nested_ownership_until_resume() {
    let session = ScreenSession::new();
    let mut commands = FakeCommands::default();
    acquire(&session, &mut commands);
    acquire(&session, &mut commands);
    commands.effects.clear();

    session.suspend_commands(&mut commands).expect("suspend");
    session
        .suspend_commands(&mut commands)
        .expect("repeat suspend");
    assert!(session.is_active() && session.is_suspended());
    session.resume_commands(&mut commands).expect("resume");
    session
        .resume_commands(&mut commands)
        .expect("repeat resume");
    session.release(&mut commands).expect("nested release");
    assert!(session.is_active());
    session.release(&mut commands).expect("outer release");

    assert_effects(
        &commands,
        &[
            Effect::DisableScroll,
            Effect::Leave,
            Effect::Enter,
            Effect::EnableScroll,
            Effect::DisableScroll,
            Effect::Leave,
        ],
    );
}

#[test]
fn owned_suspend_and_resume_restore_mouse_capture() {
    let session = ScreenSession::new();
    let mut commands = FakeCommands::default();
    acquire_owned(&session, &mut commands);
    commands.effects.clear();

    session.suspend_commands(&mut commands).expect("suspend");
    session.resume_commands(&mut commands).expect("resume");
    session.release(&mut commands).expect("release");

    assert_effects(
        &commands,
        &[
            Effect::DisableMouse,
            Effect::Leave,
            Effect::Enter,
            Effect::EnableMouse,
            Effect::DisableMouse,
            Effect::Leave,
        ],
    );
}

#[test]
fn disabled_session_ignores_ownership_requests() {
    let session = ScreenSession::new();
    let mut commands = FakeCommands::default();
    session.set_enabled(/*enabled*/ false);

    acquire(&session, &mut commands);
    session.release(&mut commands).expect("release");
    session.suspend_commands(&mut commands).expect("suspend");
    session.resume_commands(&mut commands).expect("resume");

    assert!(!session.is_active());
    assert_effects(&commands, &[]);
}

#[test]
fn partial_acquire_rolls_back_or_retains_ownership_for_retry() {
    let session = ScreenSession::new();
    let mut commands = FakeCommands::failing(vec![Effect::EnableScroll]);
    assert!(
        session
            .acquire(
                &mut commands,
                Rect::default(),
                AltScreenInputMode::AlternateScroll,
            )
            .is_err()
    );
    assert!(!session.is_active());
    assert_effects(&commands, ENTER_AND_LEAVE);

    let session = ScreenSession::new();
    let mut commands = FakeCommands::failing(vec![Effect::EnableScroll, Effect::Leave]);
    assert!(
        session
            .acquire(
                &mut commands,
                Rect::default(),
                AltScreenInputMode::AlternateScroll,
            )
            .is_err()
    );
    assert!(session.is_active());
    commands.effects.clear();
    session.release(&mut commands).expect("retry cleanup");
    assert!(!session.is_active());
    assert_effects(&commands, LEAVE);
}

#[test]
fn partial_owned_acquire_disables_mouse_before_leaving() {
    let session = ScreenSession::new();
    let mut commands = FakeCommands::failing(vec![Effect::EnableMouse]);

    assert!(
        session
            .acquire(
                &mut commands,
                Rect::default(),
                AltScreenInputMode::MouseCapture,
            )
            .is_err()
    );
    assert!(!session.is_active());
    assert_effects(&commands, OWNED_ENTER_AND_LEAVE);
}

#[test]
fn release_attempts_all_cleanup_and_retains_ownership_when_leave_fails() {
    let session = ScreenSession::new();
    let mut commands = FakeCommands::default();
    acquire(&session, &mut commands);
    commands.effects.clear();
    commands.failures = vec![Effect::DisableScroll, Effect::Leave];

    assert!(session.release(&mut commands).is_err());
    assert!(session.is_active());
    assert_effects(&commands, LEAVE);

    commands.effects.clear();
    session.release(&mut commands).expect("retry release");
    assert!(!session.is_active());
    assert_effects(&commands, LEAVE);
}
