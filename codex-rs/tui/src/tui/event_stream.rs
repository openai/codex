//! Event stream plumbing for the TUI.
//!
//! - [`EventBroker`] holds the shared crossterm stream so sequential startup screens reuse the
//!   same input source, while external programs can still explicitly pause and recreate it.
//! - [`TuiEventStream`] wraps a draw event subscription plus the shared [`EventBroker`] and maps crossterm
//!   events into [`TuiEvent`].
//! - [`EventSource`] abstracts the underlying event producer; the real implementation is
//!   [`CrosstermEventSource`] and tests can swap in [`FakeEventSource`].
//!
//! The motivation for dropping/recreating the crossterm event stream is to enable the TUI to fully relinquish stdin.
//! If the stream is not dropped, it will continue to read from stdin even if it is not actively being polled
//! (due to how crossterm's EventStream is implemented), potentially stealing input from other processes reading stdin,
//! like terminal text editors. This race can cause missed input or capturing terminal query responses (for example, OSC palette/size queries)
//! that the other process expects to read. Stopping polling, instead of dropping the stream, is only sufficient when the
//! pause happens before the stream enters a pending state; otherwise the crossterm reader thread may keep reading
//! from stdin, so the safer approach is to drop and recreate the event stream when we need to hand off the terminal.
//!
//! See https://ratatui.rs/recipes/apps/spawn-vim/ and https://www.reddit.com/r/rust/comments/1f3o33u/myterious_crossterm_input_after_running_vim for more details.

use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::task::Context;
use std::task::Poll;
use std::thread::JoinHandle;
use std::time::Duration;

use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use tokio::sync::broadcast;
use tokio::sync::watch;
use tokio::time::Instant;
use tokio::time::Sleep;
use tokio_stream::Stream;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::wrappers::WatchStream;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;

use super::TuiEvent;
use super::startup::StartupActionLatch;
use super::startup::StartupBlockedAction;

mod initial_input;

use initial_input::InitialInputAction;
use initial_input::InitialInputFilter;
pub(super) use initial_input::InitialInputPolicy;

pub(super) struct InitialInputConfig {
    pub(super) policy: InitialInputPolicy,
    pub(super) start_quiet: bool,
    pub(super) pending_interrupt: bool,
    pub(super) pending_draw: bool,
    pub(super) pending_plain_whitespace: String,
    pub(super) trailing_action: Option<crate::key_hint::KeyBinding>,
    pub(super) trailing_action_from_raw_probe: bool,
}

impl InitialInputConfig {
    pub(super) fn new(policy: InitialInputPolicy) -> Self {
        Self {
            policy,
            start_quiet: false,
            pending_interrupt: false,
            pending_draw: false,
            pending_plain_whitespace: String::new(),
            trailing_action: None,
            trailing_action_from_raw_probe: false,
        }
    }
}

/// Result type produced by an event source.
pub type EventResult = std::io::Result<Event>;

/// Abstraction over a source of terminal events. Allows swapping in a fake for tests.
/// Value in production is [`CrosstermEventSource`].
pub trait EventSource: Send + 'static {
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<EventResult>>;

    fn request_input_drain(&mut self) {}

    fn take_input_drained(&mut self) -> bool {
        true
    }

    fn quiesce(&mut self) -> Vec<EventResult> {
        Vec::new()
    }
}

/// Shared crossterm input state for all [`TuiEventStream`] instances. A single crossterm EventStream
/// is reused so all streams still see the same input source.
///
/// This intermediate layer enables dropping/recreating the underlying EventStream (pause/resume) without rebuilding consumers.
pub struct EventBroker<S: EventSource = CrosstermEventSource> {
    state: Mutex<EventBrokerState<S>>,
    resume_events_tx: watch::Sender<()>,
}

/// Tracks state of underlying [`EventSource`].
enum EventBrokerState<S: EventSource> {
    Paused(usize), // Underlying event source dropped; count tracks overlapping owners
    Start,         // A new event source will be created on next poll
    Running(S),    // Event source is currently running
}

impl<S: EventSource + Default> EventBrokerState<S> {
    /// Return the running event source, starting it if needed; None when paused.
    fn active_event_source_mut(&mut self) -> Option<&mut S> {
        match self {
            EventBrokerState::Paused(_) => None,
            EventBrokerState::Start => {
                *self = EventBrokerState::Running(S::default());
                match self {
                    EventBrokerState::Running(events) => Some(events),
                    EventBrokerState::Paused(_) | EventBrokerState::Start => unreachable!(),
                }
            }
            EventBrokerState::Running(events) => Some(events),
        }
    }
}

impl<S: EventSource + Default> EventBroker<S> {
    pub fn new() -> Self {
        let (resume_events_tx, _resume_events_rx) = watch::channel(());
        Self {
            state: Mutex::new(EventBrokerState::Start),
            resume_events_tx,
        }
    }

    /// Drop the underlying event source and discard input it decoded ahead of its consumer.
    pub fn pause_events(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let EventBrokerState::Running(events) = &mut *state {
            let _ = events.quiesce();
        }
        *state = match &*state {
            EventBrokerState::Paused(count) => EventBrokerState::Paused(count + 1),
            EventBrokerState::Start | EventBrokerState::Running(_) => EventBrokerState::Paused(1),
        };
    }

    /// Pause stdin only if crossterm has already become its owner.
    pub(super) fn pause_running_events(&self) -> bool {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let EventBrokerState::Running(events) = &mut *state else {
            return false;
        };
        let _ = events.quiesce();
        *state = EventBrokerState::Paused(1);
        true
    }

    /// Ensure an event source is available, recreating it only after an explicit pause.
    pub fn resume_events(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match &mut *state {
            EventBrokerState::Paused(count) if *count > 1 => {
                *count -= 1;
                return;
            }
            EventBrokerState::Paused(_) | EventBrokerState::Start => {
                *state = EventBrokerState::Running(S::default());
            }
            EventBrokerState::Running(_) => {}
        }
        let _ = self.resume_events_tx.send(());
    }

    /// Subscribe to a notification that fires whenever [`Self::resume_events`] is called.
    ///
    /// This is used to wake `poll_crossterm_event` when it is paused and waiting for the
    /// underlying crossterm stream to be recreated.
    pub fn resume_events_rx(&self) -> watch::Receiver<()> {
        self.resume_events_tx.subscribe()
    }

    fn request_input_drain(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let EventBrokerState::Running(events) = &mut *state {
            events.request_input_drain();
        }
    }
}

/// Real crossterm-backed event source.
pub struct CrosstermEventSource {
    events: tokio::sync::mpsc::Receiver<CrosstermInputMessage>,
    drain_request: Arc<AtomicU64>,
    requested_drain: u64,
    observed_drain: u64,
    consumed_drain: u64,
    shutdown: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

enum CrosstermInputMessage {
    Event(EventResult),
    Drained(u64),
}

const CROSSTERM_INPUT_CHANNEL_CAPACITY: usize = 1024;

impl Default for CrosstermEventSource {
    fn default() -> Self {
        let (event_tx, events) = tokio::sync::mpsc::channel(CROSSTERM_INPUT_CHANNEL_CAPACITY);
        let shutdown = Arc::new(AtomicBool::new(false));
        let drain_request = Arc::new(AtomicU64::new(0));
        let worker_shutdown = shutdown.clone();
        let worker_drain_request = drain_request.clone();
        let worker = std::thread::Builder::new()
            .name("codex-tui-input".to_string())
            .spawn(move || {
                let mut acknowledged_drain = 0;
                while !worker_shutdown.load(Ordering::SeqCst) {
                    match crossterm::event::poll(Duration::from_millis(/*millis*/ 25)) {
                        Ok(true) => {
                            if event_tx
                                .blocking_send(CrosstermInputMessage::Event(
                                    crossterm::event::read(),
                                ))
                                .is_err()
                            {
                                break;
                            }
                        }
                        Ok(false) => {
                            let requested_drain = worker_drain_request.load(Ordering::SeqCst);
                            if requested_drain > acknowledged_drain
                                && event_tx
                                    .blocking_send(CrosstermInputMessage::Drained(requested_drain))
                                    .is_err()
                            {
                                break;
                            }
                            acknowledged_drain = requested_drain;
                        }
                        Err(err) => {
                            let _ = event_tx.blocking_send(CrosstermInputMessage::Event(Err(err)));
                            break;
                        }
                    }
                }
            })
            .unwrap_or_else(|err| panic!("failed to spawn terminal input reader: {err}"));
        Self {
            events,
            drain_request,
            requested_drain: 0,
            observed_drain: 0,
            consumed_drain: 0,
            shutdown,
            worker: Some(worker),
        }
    }
}

impl EventSource for CrosstermEventSource {
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<EventResult>> {
        let this = self.get_mut();
        loop {
            match this.events.poll_recv(cx) {
                Poll::Ready(Some(CrosstermInputMessage::Event(event))) => {
                    this.consumed_drain = this.observed_drain;
                    return Poll::Ready(Some(event));
                }
                Poll::Ready(Some(CrosstermInputMessage::Drained(drain))) => {
                    this.observed_drain = this.observed_drain.max(drain);
                }
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Pending => return Poll::Pending,
            }
        }
    }

    fn request_input_drain(&mut self) {
        if self.requested_drain == self.observed_drain {
            self.requested_drain += 1;
            self.drain_request
                .store(self.requested_drain, Ordering::SeqCst);
        }
    }

    fn take_input_drained(&mut self) -> bool {
        if self.observed_drain == self.consumed_drain {
            return false;
        }
        self.consumed_drain = self.observed_drain;
        true
    }

    fn quiesce(&mut self) -> Vec<EventResult> {
        self.stop_worker()
            .into_iter()
            .filter_map(|message| match message {
                CrosstermInputMessage::Event(event) => Some(event),
                CrosstermInputMessage::Drained(_) => None,
            })
            .collect()
    }
}

impl CrosstermEventSource {
    fn stop_worker(&mut self) -> Vec<CrosstermInputMessage> {
        self.shutdown.store(true, Ordering::SeqCst);
        let mut pending = Vec::new();
        if let Some(worker) = self.worker.take() {
            while !worker.is_finished() {
                while let Ok(message) = self.events.try_recv() {
                    pending.push(message);
                }
                std::thread::sleep(Duration::from_millis(/*millis*/ 1));
            }
            if worker.join().is_err() {
                tracing::warn!("terminal input reader panicked while stopping");
            }
        }
        while let Ok(message) = self.events.try_recv() {
            pending.push(message);
        }
        pending
    }
}

impl Drop for CrosstermEventSource {
    fn drop(&mut self) {
        let _ = self.stop_worker();
    }
}

/// TuiEventStream is a struct for reading TUI events (draws and user input).
/// Each instance has its own draw subscription (the draw channel is broadcast, so
/// multiple receivers are fine), while crossterm input is funneled through a
/// single shared [`EventBroker`] because crossterm uses a global stdin reader and
/// does not support fan-out. Multiple TuiEventStream instances can exist during the app lifetime
/// (for nested or sequential screens), but only one should be polled at a time,
/// otherwise one instance can consume ("steal") input events and the other will miss them.
pub struct TuiEventStream<S: EventSource + Default + Unpin = CrosstermEventSource> {
    broker: Arc<EventBroker<S>>,
    draw_stream: BroadcastStream<()>,
    resume_stream: WatchStream<()>,
    terminal_focused: Arc<AtomicBool>,
    poll_draw_first: bool,
    initial_input_filter: Option<InitialInputFilter>,
    pending_interrupt: bool,
    pending_draw: bool,
    pending_events: VecDeque<Event>,
    pending_tui_events: VecDeque<TuiEvent>,
    #[cfg(windows)]
    windows_initial_events: Vec<Event>,
    startup_repeat_actions: Vec<StartupBlockedAction>,
    legacy_repeat_timer: Option<Pin<Box<Sleep>>>,
    enhanced_key_events: bool,
    startup_action_latch: Option<Arc<Mutex<StartupActionLatch>>>,
    restore_startup_capture_on_drop: bool,
    #[cfg(unix)]
    suspend_context: crate::tui::job_control::SuspendContext,
    #[cfg(unix)]
    alt_screen_active: Arc<AtomicBool>,
}

impl<S: EventSource + Default + Unpin> TuiEventStream<S> {
    pub fn new(
        broker: Arc<EventBroker<S>>,
        draw_rx: broadcast::Receiver<()>,
        terminal_focused: Arc<AtomicBool>,
        #[cfg(unix)] suspend_context: crate::tui::job_control::SuspendContext,
        #[cfg(unix)] alt_screen_active: Arc<AtomicBool>,
    ) -> Self {
        let resume_stream = WatchStream::from_changes(broker.resume_events_rx());
        Self {
            broker,
            draw_stream: BroadcastStream::new(draw_rx),
            resume_stream,
            terminal_focused,
            poll_draw_first: false,
            initial_input_filter: None,
            pending_interrupt: false,
            pending_draw: false,
            pending_events: VecDeque::new(),
            pending_tui_events: VecDeque::new(),
            #[cfg(windows)]
            windows_initial_events: Vec::new(),
            startup_repeat_actions: Vec::new(),
            legacy_repeat_timer: None,
            enhanced_key_events: true,
            startup_action_latch: None,
            restore_startup_capture_on_drop: false,
            #[cfg(unix)]
            suspend_context,
            #[cfg(unix)]
            alt_screen_active,
        }
    }

    pub(super) fn recording_startup_actions(
        mut self,
        startup_action_latch: Arc<Mutex<StartupActionLatch>>,
    ) -> Self {
        self.startup_action_latch = Some(startup_action_latch);
        self
    }

    pub(super) fn with_enhanced_key_events(mut self, enhanced_key_events: bool) -> Self {
        self.enhanced_key_events = enhanced_key_events;
        self
    }

    pub(super) fn restoring_startup_capture_on_drop(mut self) -> Self {
        self.restore_startup_capture_on_drop = true;
        self
    }

    pub(super) fn restoring_startup_composer_text(mut self, text: Option<String>) -> Self {
        if let Some(text) = text {
            self.pending_tui_events
                .push_back(TuiEvent::StartupComposerPaste(text));
        }
        self
    }

    pub(super) fn filtering_initial_input(mut self, config: InitialInputConfig) -> Self {
        self.initial_input_filter = Some(InitialInputFilter::new(
            config.policy,
            config.start_quiet,
            config.pending_plain_whitespace,
            config.trailing_action,
            config.trailing_action_from_raw_probe,
            self.enhanced_key_events,
        ));
        self.pending_interrupt = config.pending_interrupt;
        self.pending_draw = config.pending_draw;
        self
    }

    pub(super) fn blocking_initial_actions(
        mut self,
        actions: Vec<super::startup::StartupBlockedAction>,
    ) -> Self {
        if let Some(filter) = self.initial_input_filter.as_mut() {
            filter.add_blocked_actions(actions);
        }
        self
    }

    pub(super) fn protecting_initial_submission_bindings(
        mut self,
        bindings: Vec<crate::key_hint::KeyBinding>,
    ) -> Self {
        if let Some(filter) = self.initial_input_filter.as_mut() {
            filter.add_submission_bindings(bindings);
        }
        self
    }

    pub(super) fn blocking_startup_repeats(mut self, actions: Vec<StartupBlockedAction>) -> Self {
        for action in actions {
            self.remember_startup_repeat_action(action);
        }
        self
    }

    fn remember_startup_repeat_action(&mut self, action: StartupBlockedAction) {
        if action.release_observed {
            return;
        }
        if !self.startup_repeat_actions.iter().any(|existing| {
            super::startup::startup_action_matches(
                existing.binding,
                existing.from_raw_probe,
                action.binding,
            )
        }) {
            self.startup_repeat_actions.push(action);
        }
    }

    fn release_startup_repeat_key(&mut self, key_event: KeyEvent) {
        if key_event.kind != KeyEventKind::Release {
            return;
        }
        let binding = crate::key_hint::KeyBinding::from_event(key_event);
        self.startup_repeat_actions.retain(|action| {
            !super::startup::startup_action_matches(action.binding, action.from_raw_probe, binding)
        });
        if self.startup_repeat_actions.is_empty() {
            self.legacy_repeat_timer = None;
        }
    }

    fn reset_legacy_repeat_timer(&mut self) {
        let deadline = Instant::now() + super::STARTUP_INPUT_QUIET_PERIOD;
        if let Some(timer) = &mut self.legacy_repeat_timer {
            timer.as_mut().reset(deadline);
        } else {
            self.legacy_repeat_timer = Some(Box::pin(tokio::time::sleep_until(deadline)));
        }
    }

    fn poll_legacy_repeat_timer(&mut self, cx: &mut Context<'_>) {
        let Some(timer) = &mut self.legacy_repeat_timer else {
            return;
        };
        if timer.as_mut().poll(cx).is_pending() {
            return;
        }
        self.legacy_repeat_timer = None;
        self.startup_repeat_actions.clear();
    }

    fn blocks_startup_action(&mut self, key_event: KeyEvent) -> bool {
        if key_event.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key_event.code, KeyCode::Char('c') | KeyCode::Char('z'))
            && !crate::key_hint::is_altgr(key_event.modifiers)
        {
            return false;
        }
        let binding = crate::key_hint::KeyBinding::from_event(key_event);
        let action_matches = |action: &StartupBlockedAction| {
            super::startup::startup_action_matches(action.binding, action.from_raw_probe, binding)
        };
        let matches_startup_action = self.startup_repeat_actions.iter().any(action_matches);
        let matches_ordinary_text = self
            .startup_repeat_actions
            .iter()
            .any(|action| action_matches(action) && action.preserve_after_quiet);
        match key_event.kind {
            KeyEventKind::Repeat if self.enhanced_key_events => matches_startup_action,
            KeyEventKind::Press | KeyEventKind::Repeat if matches_ordinary_text => {
                self.startup_repeat_actions
                    .retain(|action| !action_matches(action));
                false
            }
            KeyEventKind::Repeat => {
                if matches_startup_action {
                    self.reset_legacy_repeat_timer();
                }
                matches_startup_action
            }
            KeyEventKind::Press if self.enhanced_key_events => {
                self.startup_repeat_actions
                    .retain(|action| !action_matches(action));
                false
            }
            KeyEventKind::Press => {
                if matches_startup_action {
                    self.reset_legacy_repeat_timer();
                }
                matches_startup_action
            }
            KeyEventKind::Release => false,
        }
    }

    fn poll_initial_input_filter(&mut self, cx: &mut Context<'_>) {
        if self
            .initial_input_filter
            .as_mut()
            .is_some_and(|filter| filter.poll_ready(cx))
        {
            self.initial_input_filter = None;
            self.poll_draw_first = true;
        }
    }

    fn take_initial_input_settlement(&mut self) -> bool {
        if !self.pending_events.is_empty() {
            return false;
        }
        let settled = self
            .initial_input_filter
            .as_mut()
            .is_some_and(InitialInputFilter::take_settlement);
        if settled
            && self
                .initial_input_filter
                .as_ref()
                .is_some_and(InitialInputFilter::is_finished)
        {
            self.initial_input_filter = None;
            self.poll_draw_first = true;
        }
        settled
    }

    fn note_yielded_event(&mut self, event: &Poll<Option<TuiEvent>>) {
        if matches!(event, Poll::Ready(Some(TuiEvent::Draw)))
            && let Some(filter) = self.initial_input_filter.as_mut()
        {
            filter.note_draw_yielded();
            self.broker.request_input_drain();
        }
    }

    /// Poll the shared crossterm stream for the next mapped `TuiEvent`.
    ///
    /// This skips events we don't use (mouse events, etc.) and keeps polling until it yields
    /// a mapped event, hits `Pending`, or sees EOF/error. When the broker is paused, it drops
    /// the underlying stream and returns `Pending` to fully release stdin.
    pub fn poll_crossterm_event(&mut self, cx: &mut Context<'_>) -> Poll<Option<TuiEvent>> {
        if std::mem::take(&mut self.pending_interrupt) {
            return Poll::Ready(Some(TuiEvent::Key(KeyEvent::new(
                KeyCode::Char('c'),
                KeyModifiers::CONTROL,
            ))));
        }
        // Some crossterm events map to None (e.g. FocusLost, mouse); loop so we keep polling
        // until we return a mapped event, hit Pending, or see EOF/error.
        loop {
            let poll_result = if let Some(event) = self.pending_events.pop_front() {
                Some(event)
            } else {
                let mut state = self
                    .broker
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let events = match state.active_event_source_mut() {
                    Some(events) => events,
                    None => {
                        drop(state);
                        // Poll resume_stream so resume_events wakes a stream paused here
                        match Pin::new(&mut self.resume_stream).poll_next(cx) {
                            Poll::Ready(Some(())) => continue,
                            Poll::Ready(None) => return Poll::Ready(None),
                            Poll::Pending => return Poll::Pending,
                        }
                    }
                };
                match Pin::new(&mut *events).poll_next(cx) {
                    Poll::Ready(Some(Ok(event))) => Some(event),
                    Poll::Ready(Some(Err(_))) | Poll::Ready(None) => {
                        *state = EventBrokerState::Start;
                        return Poll::Ready(None);
                    }
                    Poll::Pending => {
                        let input_drained = events.take_input_drained();
                        if !input_drained && self.initial_input_filter.is_some() {
                            events.request_input_drain();
                        }
                        drop(state);
                        if input_drained {
                            #[cfg(windows)]
                            let initial_events_ready = if self.windows_initial_events.is_empty() {
                                false
                            } else {
                                let events = super::startup::coalesce_windows_startup_pastes(
                                    std::mem::take(&mut self.windows_initial_events),
                                );
                                self.pending_events.extend(events);
                                true
                            };
                            if let Some(startup_action_latch) = &self.startup_action_latch {
                                startup_action_latch
                                    .lock()
                                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                                    .note_input_drained();
                            }
                            if let Some(filter) = self.initial_input_filter.as_mut() {
                                filter.note_source_drained();
                            }
                            if !self.enhanced_key_events
                                && self.legacy_repeat_timer.is_none()
                                && !self.startup_repeat_actions.is_empty()
                            {
                                self.reset_legacy_repeat_timer();
                            }
                            #[cfg(windows)]
                            if initial_events_ready {
                                continue;
                            }
                        }
                        // Poll resume_stream so resume_events can wake us even while waiting on stdin
                        match Pin::new(&mut self.resume_stream).poll_next(cx) {
                            Poll::Ready(Some(())) => continue,
                            Poll::Ready(None) => return Poll::Ready(None),
                            Poll::Pending => {
                                if input_drained {
                                    self.poll_initial_input_filter(cx);
                                    self.poll_legacy_repeat_timer(cx);
                                    if self.take_initial_input_settlement() {
                                        return Poll::Ready(Some(TuiEvent::StartupInputSettled));
                                    }
                                }
                                return Poll::Pending;
                            }
                        }
                    }
                }
            };

            let Some(event) = poll_result else {
                continue;
            };
            #[cfg(windows)]
            if self
                .initial_input_filter
                .as_ref()
                .is_some_and(InitialInputFilter::awaits_initial_source_drain)
            {
                const MAX_INITIAL_EVENTS: usize =
                    super::startup::MAX_STARTUP_INPUT_CHARS + 32 * 1024;
                if self.windows_initial_events.len() < MAX_INITIAL_EVENTS {
                    self.windows_initial_events.push(event);
                }
                continue;
            }
            if let Event::Key(key_event) = event {
                let blocks_startup_action = self.blocks_startup_action(key_event);
                self.release_startup_repeat_key(key_event);
                if blocks_startup_action {
                    if let Some(filter) = self.initial_input_filter.as_mut() {
                        filter.note_blocked_startup_action();
                    }
                    continue;
                }
            }
            let initial_input_action = self
                .initial_input_filter
                .as_mut()
                .map(|filter| filter.handle_event(&event));
            if self
                .initial_input_filter
                .as_mut()
                .is_some_and(InitialInputFilter::take_draw_request)
            {
                self.pending_draw = true;
            }
            let initial_input_finished = self
                .initial_input_filter
                .as_ref()
                .is_some_and(InitialInputFilter::is_finished);
            if initial_input_finished {
                self.initial_input_filter = None;
                self.poll_draw_first = true;
            }
            match initial_input_action {
                Some(InitialInputAction::Discard) => continue,
                Some(InitialInputAction::PrependToComposer(text)) => {
                    if let Some(mapped) = self.map_startup_composer_event(event) {
                        self.pending_tui_events.push_back(mapped);
                    }
                    if let Some(mapped) = self.map_startup_composer_event(Event::Paste(text)) {
                        return Poll::Ready(Some(mapped));
                    }
                    continue;
                }
                Some(InitialInputAction::ForwardToComposer) => {
                    if let Some(mapped) = self.map_startup_composer_event(event) {
                        return Poll::Ready(Some(mapped));
                    }
                    continue;
                }
                Some(InitialInputAction::ForwardTextToComposer(text)) => {
                    if let Some(mapped) = self.map_startup_composer_event(Event::Paste(text)) {
                        return Poll::Ready(Some(mapped));
                    }
                    continue;
                }
                Some(InitialInputAction::Forward) | None => {}
            }
            if let Some(mapped) = self.map_crossterm_event(event) {
                return Poll::Ready(Some(mapped));
            }
        }
    }

    /// Poll the draw broadcast stream for the next draw event. Draw events are used to trigger a redraw of the TUI.
    pub fn poll_draw_event(&mut self, cx: &mut Context<'_>) -> Poll<Option<TuiEvent>> {
        if std::mem::take(&mut self.pending_draw) {
            return Poll::Ready(Some(TuiEvent::Draw));
        }
        match Pin::new(&mut self.draw_stream).poll_next(cx) {
            Poll::Ready(Some(Ok(()))) => Poll::Ready(Some(TuiEvent::Draw)),
            Poll::Ready(Some(Err(BroadcastStreamRecvError::Lagged(_)))) => {
                Poll::Ready(Some(TuiEvent::Draw))
            }
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }

    /// Map a crossterm event to a [`TuiEvent`], skipping events we don't use (mouse events, etc.).
    fn map_crossterm_event(&mut self, event: Event) -> Option<TuiEvent> {
        match event {
            Event::Key(key_event) => {
                if matches!(
                    key_event.kind,
                    KeyEventKind::Press | KeyEventKind::Repeat | KeyEventKind::Release
                ) && let Some(startup_action_latch) = &self.startup_action_latch
                {
                    startup_action_latch
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .record(key_event);
                    self.broker.request_input_drain();
                }
                #[cfg(unix)]
                if crate::tui::job_control::SUSPEND_KEY.is_press(key_event) {
                    self.broker.pause_events();
                    let suspend_result = self.suspend_context.suspend(&self.alt_screen_active);
                    self.broker.resume_events();
                    if let Err(err) = suspend_result {
                        tracing::warn!(
                            event = "tui_suspend_failed",
                            error = %err,
                            "failed to suspend TUI process"
                        );
                    }
                    return Some(TuiEvent::Draw);
                }
                Some(TuiEvent::Key(key_event))
            }
            Event::Resize(_, _) => Some(TuiEvent::Resize),
            Event::Paste(pasted) => Some(TuiEvent::Paste(pasted)),
            Event::FocusGained => {
                self.terminal_focused.store(true, Ordering::Relaxed);
                crate::terminal_palette::requery_default_colors();
                Some(TuiEvent::Draw)
            }
            Event::FocusLost => {
                self.terminal_focused.store(false, Ordering::Relaxed);
                None
            }
            _ => None,
        }
    }

    fn map_startup_composer_event(&mut self, event: Event) -> Option<TuiEvent> {
        match self.map_crossterm_event(event) {
            Some(TuiEvent::Key(key_event))
                if matches!(key_event.code, KeyCode::Char(_) | KeyCode::Backspace) =>
            {
                Some(TuiEvent::StartupComposerKey(key_event))
            }
            Some(TuiEvent::Key(key_event)) => Some(TuiEvent::StartupComposerAction(key_event)),
            Some(TuiEvent::Paste(pasted)) => Some(TuiEvent::StartupComposerPaste(pasted)),
            other => other,
        }
    }
}

impl<S: EventSource + Default + Unpin> Unpin for TuiEventStream<S> {}

impl<S: EventSource + Default + Unpin> Drop for TuiEventStream<S> {
    fn drop(&mut self) {
        if !self.restore_startup_capture_on_drop {
            return;
        }
        if let Some(filter) = self.initial_input_filter.as_mut()
            && let Some(startup_action_latch) = &self.startup_action_latch
        {
            startup_action_latch
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .retain_blocked(filter.take_blocked_actions());
        }
        if let Err(err) = super::restore_startup_screen() {
            tracing::warn!("failed to restore startup capture after screen: {err}");
        }
    }
}

impl<S: EventSource + Default + Unpin> Stream for TuiEventStream<S> {
    type Item = TuiEvent;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Some(event) = self.pending_tui_events.pop_front() {
            return Poll::Ready(Some(event));
        }
        if self.take_initial_input_settlement() {
            return Poll::Ready(Some(TuiEvent::StartupInputSettled));
        }

        // During startup, drain input before every pre-settlement draw so an event that became
        // readable at the ownership boundary cannot slip past the protection filter.
        let input_handoff_active = self
            .initial_input_filter
            .as_ref()
            .is_some_and(InitialInputFilter::requires_input_first);
        let draw_first = !input_handoff_active && self.poll_draw_first;
        if !input_handoff_active {
            self.poll_draw_first = !self.poll_draw_first;
        }

        if draw_first {
            if let Poll::Ready(event) = self.poll_draw_event(cx) {
                let event = Poll::Ready(event);
                self.note_yielded_event(&event);
                return event;
            }
            if let Poll::Ready(event) = self.poll_crossterm_event(cx) {
                let event = Poll::Ready(event);
                self.note_yielded_event(&event);
                return event;
            }
        } else {
            if let Poll::Ready(event) = self.poll_crossterm_event(cx) {
                let event = Poll::Ready(event);
                self.note_yielded_event(&event);
                return event;
            }
            if self
                .initial_input_filter
                .as_ref()
                .is_some_and(InitialInputFilter::awaits_initial_source_drain)
            {
                return Poll::Pending;
            }
            if let Poll::Ready(event) = self.poll_draw_event(cx) {
                let event = Poll::Ready(event);
                self.note_yielded_event(&event);
                return event;
            }
        }

        Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::Event;
    use crossterm::event::KeyCode;
    use crossterm::event::KeyEvent;
    use crossterm::event::KeyModifiers;
    use pretty_assertions::assert_eq;
    use std::task::Context;
    use std::task::Poll;
    use std::time::Duration;
    use tokio::sync::broadcast;
    use tokio::sync::mpsc;
    use tokio::time::timeout;
    use tokio_stream::StreamExt;

    /// Simple fake event source for tests; feed events via the handle.
    pub(super) struct FakeEventSource {
        rx: mpsc::UnboundedReceiver<EventResult>,
        tx: mpsc::UnboundedSender<EventResult>,
        drain_acknowledged: bool,
        auto_acknowledge_drain: bool,
    }

    pub(super) struct FakeEventSourceHandle {
        broker: Arc<EventBroker<FakeEventSource>>,
    }

    impl FakeEventSource {
        fn new() -> Self {
            let (tx, rx) = mpsc::unbounded_channel();
            Self {
                rx,
                tx,
                drain_acknowledged: false,
                auto_acknowledge_drain: true,
            }
        }
    }

    impl Default for FakeEventSource {
        fn default() -> Self {
            Self::new()
        }
    }

    impl FakeEventSourceHandle {
        fn new(broker: Arc<EventBroker<FakeEventSource>>) -> Self {
            Self { broker }
        }

        pub(super) fn send(&self, event: EventResult) {
            let mut state = self
                .broker
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let Some(source) = state.active_event_source_mut() else {
                return;
            };
            let _ = source.tx.send(event);
        }

        fn set_auto_acknowledge_drain(&self, enabled: bool) {
            let mut state = self
                .broker
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let Some(source) = state.active_event_source_mut() else {
                return;
            };
            source.auto_acknowledge_drain = enabled;
        }

        fn acknowledge_drain(&self) {
            let mut state = self
                .broker
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let Some(source) = state.active_event_source_mut() else {
                return;
            };
            source.drain_acknowledged = true;
        }
    }

    impl EventSource for FakeEventSource {
        fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<EventResult>> {
            let this = self.get_mut();
            let event = Pin::new(&mut this.rx).poll_recv(cx);
            match &event {
                Poll::Ready(Some(_)) => this.drain_acknowledged = false,
                Poll::Pending if this.auto_acknowledge_drain => {
                    this.drain_acknowledged = true;
                }
                Poll::Ready(None) | Poll::Pending => {}
            }
            event
        }

        fn request_input_drain(&mut self) {
            if self.auto_acknowledge_drain {
                self.drain_acknowledged = true;
            }
        }

        fn take_input_drained(&mut self) -> bool {
            std::mem::take(&mut self.drain_acknowledged)
        }

        fn quiesce(&mut self) -> Vec<EventResult> {
            let mut pending = Vec::new();
            while let Ok(event) = self.rx.try_recv() {
                pending.push(event);
            }
            pending
        }
    }

    #[test]
    fn bounded_input_worker_quiesces_without_losing_its_blocked_event() {
        let (event_tx, events) = mpsc::channel(/*max_capacity*/ 1);
        let (channel_full_tx, channel_full_rx) = std::sync::mpsc::channel();
        let first = Event::Key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
        let second = Event::Key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));
        let expected = vec![first.clone(), second.clone()];
        let worker = std::thread::spawn(move || {
            event_tx
                .blocking_send(CrosstermInputMessage::Event(Ok(first)))
                .expect("send first event");
            channel_full_tx.send(()).expect("signal full channel");
            event_tx
                .blocking_send(CrosstermInputMessage::Event(Ok(second)))
                .expect("send blocked event");
        });
        channel_full_rx
            .recv_timeout(Duration::from_secs(/*secs*/ 1))
            .expect("input channel should fill");

        let mut source = CrosstermEventSource {
            events,
            drain_request: Arc::new(AtomicU64::new(0)),
            requested_drain: 0,
            observed_drain: 0,
            consumed_drain: 0,
            shutdown: Arc::new(AtomicBool::new(false)),
            worker: Some(worker),
        };
        let (result_tx, result_rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let events = source
                .quiesce()
                .into_iter()
                .collect::<std::io::Result<Vec<_>>>();
            result_tx.send(events).expect("return pending events");
        });

        assert_eq!(
            result_rx
                .recv_timeout(Duration::from_secs(/*secs*/ 1))
                .expect("bounded worker should quiesce")
                .expect("pending events should be valid"),
            expected
        );
    }

    pub(super) fn make_stream(
        broker: Arc<EventBroker<FakeEventSource>>,
        draw_rx: broadcast::Receiver<()>,
        terminal_focused: Arc<AtomicBool>,
    ) -> TuiEventStream<FakeEventSource> {
        TuiEventStream::new(
            broker,
            draw_rx,
            terminal_focused,
            #[cfg(unix)]
            crate::tui::job_control::SuspendContext::new(),
            #[cfg(unix)]
            Arc::new(AtomicBool::new(false)),
        )
    }

    pub(super) type SetupState = (
        Arc<EventBroker<FakeEventSource>>,
        FakeEventSourceHandle,
        broadcast::Sender<()>,
        broadcast::Receiver<()>,
        Arc<AtomicBool>,
    );

    pub(super) fn setup() -> SetupState {
        let source = FakeEventSource::new();
        let broker = Arc::new(EventBroker::new());
        *broker.state.lock().unwrap() = EventBrokerState::Running(source);
        let handle = FakeEventSourceHandle::new(broker.clone());

        let (draw_tx, draw_rx) = broadcast::channel(1);
        let terminal_focused = Arc::new(AtomicBool::new(true));
        (broker, handle, draw_tx, draw_rx, terminal_focused)
    }

    #[tokio::test(flavor = "current_thread")]
    async fn resume_keeps_events_queued_by_a_running_startup_reader() {
        let (broker, handle, draw_tx, draw_rx, terminal_focused) = setup();
        let stream = make_stream(broker.clone(), draw_rx, terminal_focused.clone());
        let key = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
        handle.send(Ok(Event::Key(key)));

        broker.resume_events();
        drop(stream);
        let mut next_stream = make_stream(broker, draw_tx.subscribe(), terminal_focused);

        assert!(matches!(next_stream.next().await, Some(TuiEvent::Key(actual)) if actual == key));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn relinquishing_input_discards_decoded_events() {
        let (broker, handle, draw_tx, draw_rx, terminal_focused) = setup();
        let stream = make_stream(broker.clone(), draw_rx, terminal_focused.clone());
        handle.send(Ok(Event::Key(KeyEvent::new(
            KeyCode::Char('x'),
            KeyModifiers::NONE,
        ))));

        broker.pause_events();
        broker.resume_events();
        drop(stream);
        let mut next_stream = make_stream(broker, draw_tx.subscribe(), terminal_focused);
        assert!(
            timeout(Duration::from_nanos(1), next_stream.next())
                .await
                .is_err()
        );

        let key = KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE);
        handle.send(Ok(Event::Key(key)));
        assert!(matches!(next_stream.next().await, Some(TuiEvent::Key(actual)) if actual == key));
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn startup_screen_requires_fresh_drains_before_and_after_its_first_draw() {
        let (broker, handle, _draw_tx, draw_rx, terminal_focused) = setup();
        handle.set_auto_acknowledge_drain(false);
        let mut stream = make_stream(broker, draw_rx, terminal_focused).filtering_initial_input(
            InitialInputConfig {
                pending_draw: true,
                ..InitialInputConfig::new(InitialInputPolicy::DiscardAll)
            },
        );

        assert!(
            timeout(Duration::from_nanos(1), stream.next())
                .await
                .is_err()
        );
        handle.acknowledge_drain();
        assert!(matches!(stream.next().await, Some(TuiEvent::Draw)));

        handle.send(Ok(Event::Key(KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::NONE,
        ))));
        assert!(
            timeout(Duration::from_nanos(1), stream.next())
                .await
                .is_err()
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn startup_screen_accepts_input_after_its_first_render_boundary() {
        let (broker, handle, _draw_tx, draw_rx, terminal_focused) = setup();
        let mut stream = make_stream(broker, draw_rx, terminal_focused).filtering_initial_input(
            InitialInputConfig {
                pending_draw: true,
                ..InitialInputConfig::new(InitialInputPolicy::DiscardAll)
            },
        );
        assert!(matches!(stream.next().await, Some(TuiEvent::Draw)));
        assert!(
            timeout(Duration::from_nanos(1), stream.next())
                .await
                .is_err()
        );

        let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        handle.send(Ok(Event::Key(key)));

        assert!(matches!(stream.next().await, Some(TuiEvent::Key(actual)) if actual == key));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn key_event_skips_unmapped() {
        let (broker, handle, _draw_tx, draw_rx, terminal_focused) = setup();
        let mut stream = make_stream(broker, draw_rx, terminal_focused);

        handle.send(Ok(Event::FocusLost));
        handle.send(Ok(Event::Key(KeyEvent::new(
            KeyCode::Char('a'),
            KeyModifiers::NONE,
        ))));

        let next = stream.next().await.unwrap();
        match next {
            TuiEvent::Key(key) => {
                assert_eq!(key, KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
            }
            other => panic!("expected key event, got {other:?}"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn draw_and_key_events_yield_both() {
        let (broker, handle, draw_tx, draw_rx, terminal_focused) = setup();
        let mut stream = make_stream(broker, draw_rx, terminal_focused);

        let expected_key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        let _ = draw_tx.send(());
        handle.send(Ok(Event::Key(expected_key)));

        let first = stream.next().await.unwrap();
        let second = stream.next().await.unwrap();

        let mut saw_draw = false;
        let mut saw_key = false;
        for event in [first, second] {
            match event {
                TuiEvent::Draw => {
                    saw_draw = true;
                }
                TuiEvent::Key(key) => {
                    assert_eq!(key, expected_key);
                    saw_key = true;
                }
                other => panic!("expected draw or key event, got {other:?}"),
            }
        }

        assert!(saw_draw && saw_key, "expected both draw and key events");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lagged_draw_maps_to_draw() {
        let (broker, _handle, draw_tx, draw_rx, terminal_focused) = setup();
        let mut stream = make_stream(broker, draw_rx.resubscribe(), terminal_focused);

        // Fill channel to force Lagged on the receiver.
        let _ = draw_tx.send(());
        let _ = draw_tx.send(());

        let first = stream.next().await;
        assert!(matches!(first, Some(TuiEvent::Draw)));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn resize_event_maps_to_resize() {
        let (broker, handle, _draw_tx, draw_rx, terminal_focused) = setup();
        let mut stream = make_stream(broker, draw_rx, terminal_focused);

        handle.send(Ok(Event::Resize(80, 24)));

        let next = stream.next().await;
        assert!(matches!(next, Some(TuiEvent::Resize)));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn error_or_eof_ends_stream() {
        let (broker, handle, _draw_tx, draw_rx, terminal_focused) = setup();
        let mut stream = make_stream(broker, draw_rx, terminal_focused);

        handle.send(Err(std::io::Error::other("boom")));

        let next = stream.next().await;
        assert!(next.is_none());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn resume_wakes_paused_stream() {
        let (broker, handle, _draw_tx, draw_rx, terminal_focused) = setup();
        let mut stream = make_stream(broker.clone(), draw_rx, terminal_focused);

        broker.pause_events();

        let task = tokio::spawn(async move { stream.next().await });
        tokio::task::yield_now().await;

        broker.resume_events();
        let expected_key = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE);
        handle.send(Ok(Event::Key(expected_key)));

        let event = timeout(Duration::from_millis(100), task)
            .await
            .expect("timed out waiting for resumed event")
            .expect("join failed");
        match event {
            Some(TuiEvent::Key(key)) => assert_eq!(key, expected_key),
            other => panic!("expected key event, got {other:?}"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn overlapping_pauses_require_matching_resumes() {
        let (broker, handle, _draw_tx, draw_rx, terminal_focused) = setup();
        let mut stream = make_stream(broker.clone(), draw_rx, terminal_focused);
        broker.pause_events();
        broker.pause_events();

        broker.resume_events();
        let key = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
        handle.send(Ok(Event::Key(key)));
        assert!(
            timeout(Duration::from_nanos(1), stream.next())
                .await
                .is_err()
        );

        broker.resume_events();
        handle.send(Ok(Event::Key(key)));
        assert!(matches!(stream.next().await, Some(TuiEvent::Key(actual)) if actual == key));
    }

    #[test]
    fn pausing_an_unstarted_broker_does_not_claim_stdin() {
        let broker = EventBroker::<FakeEventSource>::new();

        assert!(!broker.pause_running_events());
        assert!(matches!(
            *broker
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            EventBrokerState::Start
        ));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn resume_wakes_pending_stream() {
        let (broker, handle, _draw_tx, draw_rx, terminal_focused) = setup();
        let mut stream = make_stream(broker.clone(), draw_rx, terminal_focused);

        let task = tokio::spawn(async move { stream.next().await });
        tokio::task::yield_now().await;

        broker.pause_events();
        broker.resume_events();
        let expected_key = KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE);
        handle.send(Ok(Event::Key(expected_key)));

        let event = timeout(Duration::from_millis(100), task)
            .await
            .expect("timed out waiting for resumed event")
            .expect("join failed");
        match event {
            Some(TuiEvent::Key(key)) => assert_eq!(key, expected_key),
            other => panic!("expected key event, got {other:?}"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dropping_startup_screen_stream_keeps_the_event_broker_running() {
        let (broker, handle, draw_tx, draw_rx, terminal_focused) = setup();
        let mut stream = make_stream(broker.clone(), draw_rx, terminal_focused.clone())
            .restoring_startup_capture_on_drop();
        assert!(
            timeout(Duration::from_nanos(1), stream.next())
                .await
                .is_err()
        );

        drop(stream);
        assert!(matches!(
            *broker
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            EventBrokerState::Running(_)
        ));

        let expected = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
        handle.send(Ok(Event::Key(expected)));
        let mut next_stream = make_stream(broker, draw_tx.subscribe(), terminal_focused);
        assert!(
            matches!(next_stream.next().await, Some(TuiEvent::Key(actual)) if actual == expected)
        );
    }
}
