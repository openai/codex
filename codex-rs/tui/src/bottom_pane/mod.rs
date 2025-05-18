//! Bottom pane: shows the ChatComposer or a BottomPaneView, if one is active.

use bottom_pane_view::BottomPaneView;
use bottom_pane_view::ConditionalUpdate;
use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::WidgetRef;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::user_approval_widget::ApprovalRequest;

mod approval_modal_view;
mod bottom_pane_view;
mod chat_composer;
mod chat_composer_history;
mod command_popup;
mod status_indicator_view;

pub(crate) use chat_composer::ChatComposer;
pub(crate) use chat_composer::InputResult;

use approval_modal_view::ApprovalModalView;
use status_indicator_view::StatusIndicatorView;

/// Pane displayed in the lower half of the chat UI.
pub(crate) struct BottomPane<'a> {
    /// Composer is retained even when a BottomPaneView is displayed so the
    /// input state is retained when the view is closed.
    composer: ChatComposer<'a>,

    /// If present, this is displayed instead of the `composer`.
    active_view: Option<Box<dyn BottomPaneView<'a> + 'a>>,

    app_event_tx: AppEventSender,
    has_input_focus: bool,
    is_task_running: bool,
}

pub(crate) struct BottomPaneParams {
    pub(crate) app_event_tx: AppEventSender,
    pub(crate) has_input_focus: bool,
}

impl BottomPane<'_> {
    pub fn new(params: BottomPaneParams) -> Self {
        Self {
            composer: ChatComposer::new(params.has_input_focus, params.app_event_tx.clone()),
            active_view: None,
            app_event_tx: params.app_event_tx,
            has_input_focus: params.has_input_focus,
            is_task_running: false,
        }
    }

    /// Forward a key event to the active view or the composer.
    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> InputResult {
        if let Some(mut view) = self.active_view.take() {
            view.handle_key_event(self, key_event);
            if !view.is_complete() {
                self.active_view = Some(view);
            } else if self.is_task_running {
                let height = self.composer.calculate_required_height(&Rect::default());
                self.active_view = Some(Box::new(StatusIndicatorView::new(
                    self.app_event_tx.clone(),
                    height,
                )));
            }
            self.request_redraw();
            InputResult::None
        } else {
            let (input_result, needs_redraw) = self.composer.handle_key_event(key_event);
            if needs_redraw {
                self.request_redraw();
            }
            input_result
        }
    }

    /// Update the status indicator text (only when the `StatusIndicatorView` is
    /// active).
    pub(crate) fn update_status_text(&mut self, text: String) {
        if let Some(view) = &mut self.active_view {
            match view.update_status_text(text) {
                ConditionalUpdate::NeedsRedraw => {
                    self.request_redraw();
                }
                ConditionalUpdate::NoRedraw => {
                    // No redraw needed.
                }
            }
        }
    }

    /// Update the UI to reflect whether this `BottomPane` has input focus.
    pub(crate) fn set_input_focus(&mut self, has_focus: bool) {
        self.has_input_focus = has_focus;
        self.composer.set_input_focus(has_focus);
    }

    pub fn set_task_running(&mut self, running: bool) {
        self.is_task_running = running;

        match (running, self.active_view.is_some()) {
            (true, false) => {
                // Show status indicator overlay.
                let height = self.composer.calculate_required_height(&Rect::default());
                self.active_view = Some(Box::new(StatusIndicatorView::new(
                    self.app_event_tx.clone(),
                    height,
                )));
                self.request_redraw();
            }
            (false, true) => {
                if let Some(mut view) = self.active_view.take() {
                    if view.should_hide_when_task_is_done() {
                        // Leave self.active_view as None.
                        self.request_redraw();
                    } else {
                        // Preserve the view.
                        self.active_view = Some(view);
                    }
                }
            }
            _ => {
                // No change.
            }
        }
    }

    /// Called when the agent requests user approval.
    pub fn push_approval_request(&mut self, request: ApprovalRequest) {
        let request = if let Some(view) = self.active_view.as_mut() {
            match view.try_consume_approval_request(request) {
                Some(request) => request,
                None => {
                    self.request_redraw();
                    return;
                }
            }
        } else {
            request
        };

        // Otherwise create a new approval modal overlay.
        let modal = ApprovalModalView::new(request, self.app_event_tx.clone());
        self.active_view = Some(Box::new(modal));
        self.request_redraw()
    }

    /// Height (terminal rows) required by the current bottom pane.
    pub fn calculate_required_height(&self, area: &Rect) -> u16 {
        if let Some(view) = &self.active_view {
            view.calculate_required_height(area)
        } else {
            self.composer.calculate_required_height(area)
        }
    }

    pub(crate) fn request_redraw(&self) {
        self.app_event_tx.send(AppEvent::Redraw)
    }

    /// Returns true when the slash-command popup inside the composer is visible.
    pub(crate) fn is_command_popup_visible(&self) -> bool {
        self.active_view.is_none() && self.composer.is_command_popup_visible()
    }

    // --- History helpers ---

    pub(crate) fn set_history_metadata(&mut self, log_id: u64, entry_count: usize) {
        self.composer.set_history_metadata(log_id, entry_count);
    }

    pub(crate) fn on_history_entry_response(
        &mut self,
        log_id: u64,
        offset: usize,
        entry: Option<String>,
    ) {
        let updated = self
            .composer
            .on_history_entry_response(log_id, offset, entry);

        if updated {
            self.request_redraw();
        }
    }
}

impl WidgetRef for &BottomPane<'_> {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        // Show BottomPaneView if present.
        if let Some(ov) = &self.active_view {
            ov.render(area, buf);
        } else {
            (&self.composer).render_ref(area, buf);
        }
    }
}
