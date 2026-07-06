use std::future::Future;
use std::pin::Pin;
use std::task::Context;

use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use tokio::time::Instant;
use tokio::time::Sleep;

use crate::key_hint;
use crate::key_hint::KeyBinding;

use super::super::STARTUP_INPUT_QUIET_PERIOD;
use super::super::startup::StartupBlockedAction;

#[derive(Clone, Copy)]
pub(in crate::tui) enum InitialInputPolicy {
    DiscardAll,
    PreserveText,
}

pub(super) enum InitialInputAction {
    Discard,
    Forward,
    ForwardToComposer,
    ForwardTextToComposer(String),
    PrependToComposer(String),
}

pub(super) struct InitialInputFilter {
    policy: InitialInputPolicy,
    drain_ready: bool,
    quiet_timer: Option<Pin<Box<Sleep>>>,
    pending_plain_whitespace: String,
    blocked_actions: Vec<StartupBlockedAction>,
    submission_bindings: Vec<KeyBinding>,
    enhanced_key_events: bool,
    quiet_complete: bool,
    source_drain_observed: bool,
    startup_draw_yielded: bool,
    post_draw_drain_pending: bool,
    post_edit_drain_pending: bool,
    settlement_ready: bool,
    settlement_emitted: bool,
    draw_requested: bool,
}

impl InitialInputFilter {
    pub(super) fn new(
        policy: InitialInputPolicy,
        start_quiet: bool,
        pending_plain_whitespace: String,
        trailing_action: Option<KeyBinding>,
        trailing_action_from_raw_probe: bool,
        enhanced_key_events: bool,
    ) -> Self {
        let quiet_timer = start_quiet.then(|| {
            Box::pin(tokio::time::sleep_until(
                Instant::now() + STARTUP_INPUT_QUIET_PERIOD,
            ))
        });
        let blocked_actions = trailing_action
            .map(|binding| StartupBlockedAction::captured(binding, trailing_action_from_raw_probe))
            .into_iter()
            .collect();
        Self {
            policy,
            drain_ready: matches!(policy, InitialInputPolicy::DiscardAll)
                && !start_quiet
                && trailing_action.is_none(),
            quiet_timer,
            pending_plain_whitespace,
            blocked_actions,
            submission_bindings: Vec::new(),
            enhanced_key_events,
            quiet_complete: !start_quiet && trailing_action.is_some(),
            source_drain_observed: false,
            startup_draw_yielded: false,
            post_draw_drain_pending: false,
            post_edit_drain_pending: false,
            settlement_ready: false,
            settlement_emitted: false,
            draw_requested: false,
        }
    }

    pub(super) fn add_submission_bindings(&mut self, bindings: Vec<KeyBinding>) {
        for binding in bindings {
            if !self.submission_bindings.contains(&binding) {
                self.submission_bindings.push(binding);
            }
        }
    }

    pub(super) fn add_blocked_actions(&mut self, actions: Vec<StartupBlockedAction>) {
        let submission_bindings = &self.submission_bindings;
        for action in actions.into_iter().filter(|action| {
            !matches!(self.policy, InitialInputPolicy::PreserveText)
                || !action.quiet_elapsed
                || startup_action_blocks_submission(submission_bindings, *action)
        }) {
            if let Some(existing) = self
                .blocked_actions
                .iter_mut()
                .find(|existing| existing.binding == action.binding)
            {
                *existing = action;
            } else {
                self.blocked_actions.push(action);
            }
        }
        if self
            .blocked_actions
            .iter()
            .any(|action| !action.quiet_elapsed)
        {
            self.reset_quiet_timer();
        } else if !self.blocked_actions.is_empty() {
            self.quiet_complete = true;
        }
    }

    pub(super) fn note_blocked_startup_action(&mut self) {
        self.settlement_ready = false;
        self.reset_quiet_timer();
    }

    pub(super) fn take_blocked_actions(&mut self) -> Vec<StartupBlockedAction> {
        std::mem::take(&mut self.blocked_actions)
    }

    pub(super) fn handle_event(&mut self, event: &Event) -> InitialInputAction {
        if matches!(self.policy, InitialInputPolicy::PreserveText) {
            return self.handle_preserved_event(event);
        }

        self.handle_discarded_event(event)
    }

    fn handle_preserved_event(&mut self, event: &Event) -> InitialInputAction {
        match event {
            Event::Key(key_event) => {
                let binding = KeyBinding::from_event(*key_event);
                if key_event.kind == KeyEventKind::Release {
                    if self.remove_matching_action(binding) && self.blocked_actions.is_empty() {
                        self.quiet_timer = None;
                        self.quiet_complete = true;
                    }
                    return InitialInputAction::Discard;
                }
                if !matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
                    return InitialInputAction::Discard;
                }
                // Press-only terminals cannot prove that a captured control key was released.
                // Keep controls live instead of trapping the user behind the startup latch;
                // submit-like actions remain latched because replaying those is unsafe.
                if is_interrupt(*key_event) || is_suspend(*key_event) {
                    self.pending_plain_whitespace.clear();
                    return InitialInputAction::Forward;
                }
                if let Some(index) = self.matching_action_index(binding) {
                    let action = self.blocked_actions[index];
                    let blocks_submission =
                        startup_action_blocks_submission(&self.submission_bindings, action);
                    if blocks_submission {
                        if action.from_raw_probe
                            && self.enhanced_key_events
                            && key_event.kind == KeyEventKind::Press
                            && action.quiet_elapsed
                            && self.settlement_emitted
                        {
                            self.blocked_actions.remove(index);
                            return InitialInputAction::Forward;
                        }
                        if !action.quiet_elapsed {
                            self.reset_quiet_timer();
                            return InitialInputAction::Discard;
                        }
                        self.reset_quiet_timer();
                        return InitialInputAction::Discard;
                    }
                    if action.preserve_after_quiet && is_text_input(*key_event) {
                        self.blocked_actions.remove(index);
                    } else {
                        self.reset_quiet_timer();
                        return InitialInputAction::Discard;
                    }
                }

                if startup_action_blocks_submission(
                    &self.submission_bindings,
                    StartupBlockedAction::captured(binding, /*from_raw_probe*/ false),
                ) {
                    self.block_action(binding);
                    self.settlement_ready = false;
                    self.reset_quiet_timer();
                    return InitialInputAction::Discard;
                }

                if is_plain_backspace(*key_event) {
                    self.note_text_input();
                    return if self.pending_plain_whitespace.pop().is_some() {
                        InitialInputAction::Discard
                    } else {
                        InitialInputAction::ForwardToComposer
                    };
                }

                if is_plain_enter_or_tab(*key_event) {
                    self.note_text_input();
                    let ch = match key_event.code {
                        KeyCode::Enter => '\n',
                        KeyCode::Tab => '\t',
                        _ => unreachable!("plain Enter/Tab checked above"),
                    };
                    self.pending_plain_whitespace.push(ch);
                    return InitialInputAction::ForwardTextToComposer(std::mem::take(
                        &mut self.pending_plain_whitespace,
                    ));
                }

                if is_text_input(*key_event) {
                    self.note_text_input();
                    return self.prepend_pending_whitespace();
                }

                if self.settlement_emitted && self.quiet_complete {
                    return InitialInputAction::Forward;
                }

                self.block_action(binding);
                self.settlement_ready = false;
                self.reset_quiet_timer();
                if !key_hint::has_ctrl_or_alt(key_event.modifiers)
                    && let Some(ch) = match key_event.code {
                        KeyCode::Enter => Some('\n'),
                        KeyCode::Tab => Some('\t'),
                        _ => None,
                    }
                    && self.pending_plain_whitespace.len()
                        < super::super::startup::MAX_STARTUP_INPUT_CHARS
                {
                    self.pending_plain_whitespace.push(ch);
                }
                InitialInputAction::Discard
            }
            Event::Paste(text) if !text.is_empty() => {
                self.note_text_input();
                self.prepend_pending_whitespace()
            }
            Event::Resize(_, _) | Event::FocusGained | Event::FocusLost => {
                InitialInputAction::Forward
            }
            _ => InitialInputAction::Discard,
        }
    }

    fn handle_discarded_event(&mut self, event: &Event) -> InitialInputAction {
        match event {
            Event::Key(key_event) => {
                let binding = KeyBinding::from_event(*key_event);
                if key_event.kind == KeyEventKind::Release {
                    if self.remove_matching_action(binding) && self.blocked_actions.is_empty() {
                        self.pending_plain_whitespace.clear();
                        self.quiet_timer = None;
                        self.quiet_complete = true;
                    }
                    return InitialInputAction::Discard;
                }
                if !matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
                    return InitialInputAction::Discard;
                }
                if self.matching_action_index(binding).is_some() {
                    self.reset_quiet_timer();
                    return InitialInputAction::Discard;
                }

                if self.quiet_complete && !self.post_draw_drain_pending {
                    self.pending_plain_whitespace.clear();
                    return InitialInputAction::Forward;
                }

                self.reset_quiet_timer();
                if is_interrupt(*key_event) {
                    self.pending_plain_whitespace.clear();
                    return InitialInputAction::Forward;
                }
                if is_suspend(*key_event) {
                    self.pending_plain_whitespace.clear();
                    return InitialInputAction::Forward;
                }
                self.pending_plain_whitespace.clear();
                self.block_action(binding);
                InitialInputAction::Discard
            }
            Event::Paste(_) => {
                if self.quiet_complete && !self.post_draw_drain_pending {
                    self.pending_plain_whitespace.clear();
                    return InitialInputAction::Forward;
                }
                self.reset_quiet_timer();
                InitialInputAction::Discard
            }
            Event::Resize(_, _) | Event::FocusGained | Event::FocusLost => {
                InitialInputAction::Forward
            }
            _ => InitialInputAction::Discard,
        }
    }

    pub(super) fn poll_ready(&mut self, cx: &mut Context<'_>) -> bool {
        if matches!(self.policy, InitialInputPolicy::PreserveText) {
            if self.post_draw_drain_pending {
                self.post_draw_drain_pending = false;
                self.settlement_ready = false;
            }
            if let Some(timer) = &mut self.quiet_timer
                && timer.as_mut().poll(cx).is_ready()
            {
                self.quiet_timer = None;
                self.quiet_complete = true;
                for action in &mut self.blocked_actions {
                    action.quiet_elapsed = true;
                }
                if self.settlement_emitted {
                    self.release_actions_after_quiet();
                } else {
                    let submission_bindings = &self.submission_bindings;
                    self.blocked_actions.retain(|action| {
                        startup_action_blocks_submission(submission_bindings, *action)
                    });
                }
            }
            if self.settlement_emitted
                && self.post_edit_drain_pending
                && self.startup_draw_yielded
                && self.quiet_timer.is_none()
            {
                self.blocked_actions.clear();
                self.post_edit_drain_pending = false;
            }
            if self.startup_draw_yielded && self.quiet_timer.is_none() {
                self.settlement_ready = true;
            }
            return self.is_finished();
        }
        if self.post_draw_drain_pending {
            self.post_draw_drain_pending = false;
        }
        if self.drain_ready {
            self.quiet_complete = true;
            return self.is_finished();
        }
        let Some(timer) = &mut self.quiet_timer else {
            return self.is_finished();
        };
        if timer.as_mut().poll(cx).is_ready() {
            self.quiet_timer = None;
            self.quiet_complete = true;
            for action in &mut self.blocked_actions {
                action.quiet_elapsed = true;
            }
            self.release_actions_after_quiet();
            self.pending_plain_whitespace.clear();
        }
        self.is_finished()
    }

    pub(super) fn note_source_drained(&mut self) {
        if self.source_drain_observed {
            return;
        }
        self.source_drain_observed = true;
        if let Some(timer) = &mut self.quiet_timer {
            timer
                .as_mut()
                .reset(Instant::now() + STARTUP_INPUT_QUIET_PERIOD);
        }
    }

    pub(super) fn is_finished(&self) -> bool {
        match self.policy {
            InitialInputPolicy::DiscardAll => {
                self.startup_draw_yielded
                    && !self.post_draw_drain_pending
                    && self.quiet_complete
                    && self.blocked_actions.is_empty()
            }
            InitialInputPolicy::PreserveText => {
                self.settlement_emitted
                    && self.quiet_timer.is_none()
                    && self.blocked_actions.is_empty()
            }
        }
    }

    pub(super) fn note_draw_yielded(&mut self) {
        self.startup_draw_yielded = true;
        self.post_draw_drain_pending = true;
        self.settlement_ready = false;
        if matches!(self.policy, InitialInputPolicy::DiscardAll) && !self.blocked_actions.is_empty()
        {
            self.reset_quiet_timer();
        }
    }

    pub(super) fn requires_input_first(&self) -> bool {
        matches!(self.policy, InitialInputPolicy::PreserveText)
            || !self.startup_draw_yielded
            || self.post_draw_drain_pending
    }

    pub(super) fn awaits_initial_source_drain(&self) -> bool {
        !self.source_drain_observed
    }

    pub(super) fn take_settlement(&mut self) -> bool {
        if !matches!(self.policy, InitialInputPolicy::PreserveText)
            || self.settlement_emitted
            || !self.startup_draw_yielded
            || !self.settlement_ready
        {
            return false;
        }
        self.settlement_ready = false;
        self.settlement_emitted = true;
        if !self.blocked_actions.is_empty() {
            // Keep submit-capable repeats quarantined briefly after the draft is unlocked. A
            // repeat resets this timer, while an idle latch expires without requiring a key-up
            // event from terminals that only report presses.
            self.reset_quiet_timer();
        }
        true
    }

    fn reset_quiet_timer(&mut self) {
        let deadline = Instant::now() + STARTUP_INPUT_QUIET_PERIOD;
        if let Some(timer) = &mut self.quiet_timer {
            timer.as_mut().reset(deadline);
        } else {
            self.quiet_timer = Some(Box::pin(tokio::time::sleep_until(deadline)));
        }
        self.drain_ready = false;
        self.quiet_complete = false;
        self.settlement_ready = false;
        for action in &mut self.blocked_actions {
            action.quiet_elapsed = false;
        }
    }

    fn matching_action_index(&self, binding: KeyBinding) -> Option<usize> {
        self.blocked_actions.iter().position(|action| {
            super::super::startup::startup_action_matches(
                action.binding,
                action.from_raw_probe,
                binding,
            )
        })
    }

    fn remove_matching_action(&mut self, binding: KeyBinding) -> bool {
        let original_len = self.blocked_actions.len();
        self.blocked_actions.retain(|action| {
            !super::super::startup::startup_action_matches(
                action.binding,
                action.from_raw_probe,
                binding,
            )
        });
        self.blocked_actions.len() != original_len
    }

    fn release_actions_after_quiet(&mut self) {
        let policy = self.policy;
        let submission_bindings = &self.submission_bindings;
        let enhanced_key_events = self.enhanced_key_events;
        let legacy_latch_can_expire = self.settlement_emitted
            || (matches!(policy, InitialInputPolicy::DiscardAll)
                && self.startup_draw_yielded
                && !self.post_draw_drain_pending);
        self.blocked_actions.retain(|action| {
            let requires_persistent_latch = matches!(policy, InitialInputPolicy::DiscardAll)
                || startup_action_blocks_submission(submission_bindings, *action);
            requires_persistent_latch && (enhanced_key_events || !legacy_latch_can_expire)
        });
    }

    fn block_action(&mut self, binding: KeyBinding) {
        if self.matching_action_index(binding).is_none() {
            self.blocked_actions.push(StartupBlockedAction::captured(
                binding, /*from_raw_probe*/ false,
            ));
        }
    }

    fn note_text_input(&mut self) {
        if self.startup_draw_yielded {
            self.finish_on_visible_input();
        } else {
            self.settlement_ready = false;
            self.reset_quiet_timer();
        }
    }

    fn finish_on_visible_input(&mut self) {
        let submission_bindings = &self.submission_bindings;
        self.blocked_actions.retain(|action| {
            !action.preserve_after_quiet
                || startup_action_blocks_submission(submission_bindings, *action)
        });
        if self.settlement_emitted || self.blocked_actions.is_empty() {
            self.quiet_timer = None;
            self.quiet_complete = true;
        }
        self.startup_draw_yielded = false;
        self.post_edit_drain_pending = true;
        self.settlement_ready = false;
        self.draw_requested = true;
    }

    pub(super) fn take_draw_request(&mut self) -> bool {
        std::mem::take(&mut self.draw_requested)
    }

    fn prepend_pending_whitespace(&mut self) -> InitialInputAction {
        if self.pending_plain_whitespace.is_empty() {
            InitialInputAction::ForwardToComposer
        } else {
            InitialInputAction::PrependToComposer(std::mem::take(
                &mut self.pending_plain_whitespace,
            ))
        }
    }
}

fn is_interrupt(key_event: KeyEvent) -> bool {
    matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat)
        && key_event.code == KeyCode::Char('c')
        && key_event.modifiers.contains(KeyModifiers::CONTROL)
        && !crate::key_hint::is_altgr(key_event.modifiers)
}

fn startup_action_blocks_submission(
    submission_bindings: &[KeyBinding],
    action: StartupBlockedAction,
) -> bool {
    submission_bindings.iter().copied().any(|binding| {
        super::super::startup::startup_action_matches(
            action.binding,
            action.from_raw_probe,
            binding,
        )
    })
}

fn is_suspend(key_event: KeyEvent) -> bool {
    #[cfg(unix)]
    {
        crate::tui::job_control::SUSPEND_KEY.is_press(key_event)
    }
    #[cfg(not(unix))]
    {
        let _ = key_event;
        false
    }
}

fn is_text_input(key_event: KeyEvent) -> bool {
    matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat)
        && match key_event.code {
            KeyCode::Char(ch) => {
                !ch.is_control() && !crate::key_hint::has_ctrl_or_alt(key_event.modifiers)
            }
            KeyCode::Backspace => is_plain_backspace(key_event),
            _ => false,
        }
}

fn is_plain_backspace(key_event: KeyEvent) -> bool {
    matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat)
        && key_event.code == KeyCode::Backspace
        && !key_event
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
}

fn is_plain_enter_or_tab(key_event: KeyEvent) -> bool {
    matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat)
        && matches!(key_event.code, KeyCode::Enter | KeyCode::Tab)
        && !key_hint::has_ctrl_or_alt(key_event.modifiers)
}

#[cfg(test)]
#[path = "initial_input_tests.rs"]
mod tests;
