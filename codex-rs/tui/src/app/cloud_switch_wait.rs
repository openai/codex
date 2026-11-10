//! Keeps the composer editable while a Cloud backend switch owns navigation.
//!
//! Handles editing and redraws. The original handoff owns
//! session preparation and adoption, so no competing navigation can install a Cloud stream.

use super::App;
use crate::bottom_pane::RestrictedInputMode;
use crate::chatwidget::ClipboardPasteContext;
use crate::tui;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use ratatui::layout::Size;
use std::future::Future;
use tokio_stream::Stream;
use tokio_stream::StreamExt;

#[derive(Clone, Copy)]
pub(crate) enum ClipboardHandoff {
    ContinueEditing,
    SettleBeforeNextView,
}

#[derive(Clone, Copy)]
enum CloudSwitchProgress {
    Connecting(ClipboardHandoff),
    Failed,
}

/// Process editor and repaint events while a Cloud switch owns navigation.
fn handle_cloud_switch_event(
    app: &mut App,
    tui: &mut tui::Tui,
    event: tui::TuiEvent,
    progress: CloudSwitchProgress,
) {
    let screen_size = match tui.screen_size_for_event(&event) {
        Ok(size) => size,
        Err(error) => {
            tracing::warn!(%error, "failed to inspect terminal size during cloud switch");
            return;
        }
    };
    // Clipboard text is delivered on Draw after ordinary input invalidation and target checks.
    app.invalidate_right_click_paste(&event);
    // Polling retains copy feedback for the ordinary event loop.
    tui.clipboard.poll();
    let event = app.finish_right_click_paste(tui, event);
    if app.pending_right_click_paste.is_some() {
        if matches!(event, tui::TuiEvent::Draw) {
            // A PRIMARY read may be waiting for an earlier copy or publication.
            tui.clipboard.advance(tui.frame_requester());
        } else if matches!(event, tui::TuiEvent::Resize(_)) {
            // A size change can consume the worker's completion frame.
            tui.frame_requester().schedule_frame();
        }
    }
    match event {
        tui::TuiEvent::Key(mut key) => {
            let escape = if app.should_recover_vim_insert_escape(tui, key)
                && !(tui.is_owned_screen()
                    && crate::transcript_view::JumpTarget::from_key(key).is_some())
            {
                key.modifiers.remove(KeyModifiers::ALT);
                Some(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
            } else {
                None
            };
            for key in escape.into_iter().chain([key]) {
                if let Some(key) = app.route_key_chord_event(tui, key) {
                    let action = app
                        .chat_widget
                        .handle_restricted_key(key, RestrictedInputMode::DraftHandoff);
                    app.handle_clipboard_key_action(tui, action);
                }
            }
            show_cloud_switch_hint(app, progress);
        }
        tui::TuiEvent::Paste(text) => {
            app.cancel_pending_key_chord();
            app.chat_widget
                .handle_paste(text.replace("\r\n", "\n").replace('\r', "\n"));
            show_cloud_switch_hint(app, progress);
        }
        tui::TuiEvent::Draw
        | tui::TuiEvent::Resize(_)
        | tui::TuiEvent::Resume
        | tui::TuiEvent::FocusGained => {
            if app
                .chat_widget
                .handle_handoff_paste_tick(tui.frame_requester())
            {
                tui.defer_screen_size(screen_size);
                return;
            }
            app.chat_widget.pre_draw_tick();
            draw_cloud_switch(app, tui, screen_size);
        }
        tui::TuiEvent::FocusLost => {
            app.cancel_pending_key_chord();
            show_cloud_switch_hint(app, progress);
        }
        tui::TuiEvent::Mouse(mouse) => {
            app.start_right_click_paste(tui, mouse, ClipboardPasteContext::DraftHandoff);
        }
    }
}

fn show_cloud_switch_hint(app: &mut App, progress: CloudSwitchProgress) {
    let (label, action) = match progress {
        CloudSwitchProgress::Connecting(_) => ("Connecting to Cloud…", ""),
        CloudSwitchProgress::Failed => ("Cloud conversation did not open", "draft kept"),
    };
    app.chat_widget
        .set_footer_hint_override(Some(vec![(label.into(), action.into())]));
}

fn draw_cloud_switch(app: &mut App, tui: &mut tui::Tui, screen_size: Size) {
    if let Err(error) = app.render_chat_widget_frame(tui, screen_size) {
        tracing::warn!(%error, "failed to render during cloud switch");
    }
}

/// Keep input restricted through the failure frame and any input that arrived during its paint.
pub(super) async fn finish_failed_cloud_switch(
    app: &mut App,
    tui: &mut tui::Tui,
    mut events: impl Stream<Item = tui::TuiEvent> + Unpin,
) {
    let progress = CloudSwitchProgress::Failed;
    show_cloud_switch_hint(app, progress);
    draw_cloud_switch(app, tui, tui.terminal.last_known_screen_size);
    drain_cloud_switch_events(app, tui, &mut events, progress).await;
}

async fn drain_cloud_switch_events(
    app: &mut App,
    tui: &mut tui::Tui,
    events: &mut (impl Stream<Item = tui::TuiEvent> + Unpin),
    progress: CloudSwitchProgress,
) {
    // The current draft owns paste keys and any clipboard text until adoption.
    let settle_clipboard = matches!(
        progress,
        CloudSwitchProgress::Connecting(ClipboardHandoff::SettleBeforeNextView)
    );
    if settle_clipboard && app.pending_right_click_paste.is_some() {
        tui.frame_requester().schedule_frame();
    }
    loop {
        let clipboard_deadline = if settle_clipboard {
            app.pending_right_click_paste
                .as_ref()
                .and_then(|pending| tui.clipboard.text_read_deadline(&pending.request))
        } else {
            None
        };
        let deadline = app
            .chat_widget
            .composer_paste_deadline()
            .max(clipboard_deadline)
            .map(tokio::time::Instant::from_std)
            .unwrap_or_else(tokio::time::Instant::now);
        let Ok(Some(event)) = tokio::time::timeout_at(deadline, events.next()).await else {
            if clipboard_deadline.is_some() {
                // The terminal stream can end or the deadline can beat its scheduled Draw.
                handle_cloud_switch_event(app, tui, tui::TuiEvent::Draw, progress);
                app.pending_right_click_paste = None;
            }
            break;
        };
        handle_cloud_switch_event(app, tui, event, progress);
    }
    app.cancel_pending_key_chord();
    app.chat_widget.set_footer_hint_override(/*items*/ None);
}

/// Keep the draft editable while Cloud prepares its destination. Other app actions wait
/// until the handoff completes, so a second navigation cannot replace the pending stream.
pub(crate) async fn wait_for_cloud_switch<T, E>(
    app: &mut App,
    tui: &mut tui::Tui,
    operation: impl Future<Output = Result<T, E>>,
    events: impl Stream<Item = tui::TuiEvent> + Unpin,
    clipboard_handoff: ClipboardHandoff,
) -> Result<T, E> {
    let progress = CloudSwitchProgress::Connecting(clipboard_handoff);
    app.cancel_pending_key_chord();
    show_cloud_switch_hint(app, progress);
    draw_cloud_switch(app, tui, tui.terminal.last_known_screen_size);
    if app.pending_right_click_paste.is_some() {
        // A completion frame can arrive between Cloud preparation phases.
        tui.frame_requester().schedule_frame();
    }
    let mut events = events.fuse();
    tokio::pin!(operation);
    let result = loop {
        tokio::select! {
            biased;
            result = &mut operation => break result,
            Some(event) = events.next() => handle_cloud_switch_event(app, tui, event, progress),
        }
    };
    if result.is_err() {
        finish_failed_cloud_switch(app, tui, events).await;
    } else {
        drain_cloud_switch_events(app, tui, &mut events, progress).await;
    }
    result
}
