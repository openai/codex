//! Bottom pane: shows the ChatComposer or a BottomPaneView, if one is active.
use std::path::PathBuf;

use crate::app_event_sender::AppEventSender;
use crate::tui::FrameRequester;
use bottom_pane_view::BottomPaneView;
use codex_file_search::FileMatch;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::widgets::WidgetRef;
use std::time::Duration;

mod approval_overlay;
pub(crate) use approval_overlay::ApprovalOverlay;
pub(crate) use approval_overlay::ApprovalRequest;
mod bottom_pane_view;
mod chat_composer;
mod chat_composer_history;
mod command_popup;
pub mod custom_prompt_view;
mod file_search_popup;
mod footer;
mod list_selection_view;
mod prompt_args;
pub(crate) use list_selection_view::SelectionViewParams;
mod paste_burst;
pub mod popup_consts;
mod scroll_state;
mod selection_popup_common;
mod textarea;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CancellationEvent {
    Handled,
    NotHandled,
}

pub(crate) use chat_composer::ChatComposer;
pub(crate) use chat_composer::InputResult;
use codex_protocol::custom_prompts::CustomPrompt;

use crate::status_indicator_widget::StatusIndicatorWidget;
pub(crate) use list_selection_view::SelectionAction;
pub(crate) use list_selection_view::SelectionItem;

/// Pane displayed in the lower half of the chat UI.
pub(crate) struct BottomPane {
    /// Composer is retained even when a BottomPaneView is displayed so the
    /// input state is retained when the view is closed.
    composer: ChatComposer,

    /// Stack of views displayed instead of the composer (e.g. popups/modals).
    view_stack: Vec<Box<dyn BottomPaneView>>,

    app_event_tx: AppEventSender,
    frame_requester: FrameRequester,

    has_input_focus: bool,
    is_task_running: bool,
    ctrl_c_quit_hint: bool,
    esc_backtrack_hint: bool,

    /// Inline status indicator shown above the composer while a task is running.
    status: Option<StatusIndicatorWidget>,
    /// Queued user messages to show under the status indicator.
    queued_user_messages: Vec<String>,
    context_window_percent: Option<u8>,
}

pub(crate) struct BottomPaneParams {
    pub(crate) app_event_tx: AppEventSender,
    pub(crate) frame_requester: FrameRequester,
    pub(crate) has_input_focus: bool,
    pub(crate) enhanced_keys_supported: bool,
    pub(crate) placeholder_text: String,
    pub(crate) disable_paste_burst: bool,
}

impl BottomPane {
    const BOTTOM_PAD_LINES: u16 = 1;
    pub fn new(params: BottomPaneParams) -> Self {
        let enhanced_keys_supported = params.enhanced_keys_supported;
        Self {
            composer: ChatComposer::new(
                params.has_input_focus,
                params.app_event_tx.clone(),
                enhanced_keys_supported,
                params.placeholder_text,
                params.disable_paste_burst,
            ),
            view_stack: Vec::new(),
            app_event_tx: params.app_event_tx,
            frame_requester: params.frame_requester,
            has_input_focus: params.has_input_focus,
            is_task_running: false,
            ctrl_c_quit_hint: false,
            status: None,
            queued_user_messages: Vec::new(),
            esc_backtrack_hint: false,
            context_window_percent: None,
        }
    }

    pub fn status_widget(&self) -> Option<&StatusIndicatorWidget> {
        self.status.as_ref()
    }

    fn active_view(&self) -> Option<&dyn BottomPaneView> {
        self.view_stack.last().map(std::convert::AsRef::as_ref)
    }

    fn push_view(&mut self, view: Box<dyn BottomPaneView>) {
        self.view_stack.push(view);
        self.request_redraw();
    }

    pub fn desired_height(&self, width: u16) -> u16 {
        // Always reserve one blank row above the pane for visual spacing.
        let top_margin = 1;

        // Base height depends on whether a modal/overlay is active.
        let base = match self.active_view().as_ref() {
            Some(view) => view.desired_height(width),
            None => self.composer.desired_height(width).saturating_add(
                self.status
                    .as_ref()
                    .map_or(0, |status| status.desired_height(width)),
            ),
        };
        // Account for bottom padding rows. Top spacing is handled in layout().
        base.saturating_add(Self::BOTTOM_PAD_LINES)
            .saturating_add(top_margin)
    }

    fn layout(&self, area: Rect) -> [Rect; 2] {
        // At small heights, bottom pane takes the entire height.
        let (top_margin, bottom_margin) = if area.height <= BottomPane::BOTTOM_PAD_LINES + 1 {
            (0, 0)
        } else {
            (1, BottomPane::BOTTOM_PAD_LINES)
        };

        let area = Rect {
            x: area.x,
            y: area.y + top_margin,
            width: area.width,
            height: area.height - top_margin - bottom_margin,
        };
        match self.active_view() {
            Some(_) => [Rect::ZERO, area],
            None => {
                let status_height = self
                    .status
                    .as_ref()
                    .map_or(0, |status| status.desired_height(area.width))
                    .min(area.height.saturating_sub(1));

                Layout::vertical([Constraint::Max(status_height), Constraint::Min(1)]).areas(area)
            }
        }
    }

    pub fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        // Hide the cursor whenever an overlay view is active (e.g. the
        // status indicator shown while a task is running, or approval modal).
        // In these states the textarea is not interactable, so we should not
        // show its caret.
        let [_, content] = self.layout(area);
        if let Some(view) = self.active_view() {
            view.cursor_pos(content)
        } else {
            self.composer.cursor_pos(content)
        }
    }

    /// Forward a key event to the active view or the composer.
    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> InputResult {
        // If a modal/view is active, handle it here; otherwise forward to composer.
        if let Some(view) = self.view_stack.last_mut() {
            if key_event.code == KeyCode::Esc
                && matches!(view.on_ctrl_c(), CancellationEvent::Handled)
                && view.is_complete()
            {
                self.view_stack.pop();
                self.on_active_view_complete();
            } else {
                view.handle_key_event(key_event);
                if view.is_complete() {
                    self.view_stack.clear();
                    self.on_active_view_complete();
                }
            }
            self.request_redraw();
            InputResult::None
        } else {
            // If a task is running and a status line is visible, allow Esc to
            // send an interrupt even while the composer has focus.
            if matches!(key_event.code, crossterm::event::KeyCode::Esc)
                && self.is_task_running
                && let Some(status) = &self.status
            {
                // Send Op::Interrupt
                status.interrupt();
                self.request_redraw();
                return InputResult::None;
            }
            let (input_result, needs_redraw) = self.composer.handle_key_event(key_event);
            if needs_redraw {
                self.request_redraw();
            }
            if self.composer.is_in_paste_burst() {
                self.request_redraw_in(ChatComposer::recommended_paste_flush_delay());
            }
            input_result
        }
    }

    /// Handle Ctrl-C in the bottom pane. If a modal view is active it gets a
    /// chance to consume the event (e.g. to dismiss itself).
    pub(crate) fn on_ctrl_c(&mut self) -> CancellationEvent {
        if let Some(view) = self.view_stack.last_mut() {
            let event = view.on_ctrl_c();
            if matches!(event, CancellationEvent::Handled) {
                if view.is_complete() {
                    self.view_stack.pop();
                    self.on_active_view_complete();
                }
                self.show_ctrl_c_quit_hint();
            }
            event
        } else if self.composer_is_empty() {
            CancellationEvent::NotHandled
        } else {
            self.view_stack.pop();
            self.set_composer_text(String::new());
            self.show_ctrl_c_quit_hint();
            CancellationEvent::Handled
        }
    }

    pub fn handle_paste(&mut self, pasted: String) {
        if let Some(view) = self.view_stack.last_mut() {
            let needs_redraw = view.handle_paste(pasted);
            if view.is_complete() {
                self.on_active_view_complete();
            }
            if needs_redraw {
                self.request_redraw();
            }
        } else {
            let needs_redraw = self.composer.handle_paste(pasted);
            if needs_redraw {
                self.request_redraw();
            }
        }
    }

    pub(crate) fn insert_str(&mut self, text: &str) {
        self.composer.insert_str(text);
        self.request_redraw();
    }

    /// Replace the composer text with `text`.
    pub(crate) fn set_composer_text(&mut self, text: String) {
        self.composer.set_text_content(text);
        self.request_redraw();
    }

    /// Get the current composer text (for tests and programmatic checks).
    pub(crate) fn composer_text(&self) -> String {
        self.composer.current_text()
    }

    /// Update the animated header shown to the left of the brackets in the
    /// status indicator (defaults to "Working"). No-ops if the status
    /// indicator is not active.
    pub(crate) fn update_status_header(&mut self, header: String) {
        if let Some(status) = self.status.as_mut() {
            status.update_header(header);
            self.request_redraw();
        }
    }

    pub(crate) fn show_ctrl_c_quit_hint(&mut self) {
        self.ctrl_c_quit_hint = true;
        self.composer
            .set_ctrl_c_quit_hint(true, self.has_input_focus);
        self.request_redraw();
    }

    pub(crate) fn clear_ctrl_c_quit_hint(&mut self) {
        if self.ctrl_c_quit_hint {
            self.ctrl_c_quit_hint = false;
            self.composer
                .set_ctrl_c_quit_hint(false, self.has_input_focus);
            self.request_redraw();
        }
    }

    #[cfg(test)]
    pub(crate) fn ctrl_c_quit_hint_visible(&self) -> bool {
        self.ctrl_c_quit_hint
    }

    pub(crate) fn show_esc_backtrack_hint(&mut self) {
        self.esc_backtrack_hint = true;
        self.composer.set_esc_backtrack_hint(true);
        self.request_redraw();
    }

    pub(crate) fn clear_esc_backtrack_hint(&mut self) {
        if self.esc_backtrack_hint {
            self.esc_backtrack_hint = false;
            self.composer.set_esc_backtrack_hint(false);
            self.request_redraw();
        }
    }

    // esc_backtrack_hint_visible removed; hints are controlled internally.

    pub fn set_task_running(&mut self, running: bool) {
        self.is_task_running = running;
        self.composer.set_task_running(running);

        if running {
            if self.status.is_none() {
                self.status = Some(StatusIndicatorWidget::new(
                    self.app_event_tx.clone(),
                    self.frame_requester.clone(),
                ));
            }
            if let Some(status) = self.status.as_mut() {
                status.set_queued_messages(self.queued_user_messages.clone());
            }
            self.request_redraw();
        } else {
            // Hide the status indicator when a task completes, but keep other modal views.
            self.hide_status_indicator();
        }
    }

    /// Hide the status indicator while leaving task-running state untouched.
    pub(crate) fn hide_status_indicator(&mut self) {
        if self.status.take().is_some() {
            self.request_redraw();
        }
    }

    pub(crate) fn set_context_window_percent(&mut self, percent: Option<u8>) {
        if self.context_window_percent == percent {
            return;
        }

        self.context_window_percent = percent;
        self.composer.set_context_window_percent(percent);
        self.request_redraw();
    }

    /// Show a generic list selection view with the provided items.
    pub(crate) fn show_selection_view(&mut self, params: list_selection_view::SelectionViewParams) {
        let view = list_selection_view::ListSelectionView::new(params, self.app_event_tx.clone());
        self.push_view(Box::new(view));
    }

    /// Update the queued messages shown under the status header.
    pub(crate) fn set_queued_user_messages(&mut self, queued: Vec<String>) {
        self.queued_user_messages = queued.clone();
        if let Some(status) = self.status.as_mut() {
            status.set_queued_messages(queued);
        }
        self.request_redraw();
    }

    /// Update custom prompts available for the slash popup.
    pub(crate) fn set_custom_prompts(&mut self, prompts: Vec<CustomPrompt>) {
        self.composer.set_custom_prompts(prompts);
        self.request_redraw();
    }

    pub(crate) fn composer_is_empty(&self) -> bool {
        self.composer.is_empty()
    }

    pub(crate) fn is_task_running(&self) -> bool {
        self.is_task_running
    }

    /// Return true when the pane is in the regular composer state without any
    /// overlays or popups and not running a task. This is the safe context to
    /// use Esc-Esc for backtracking from the main view.
    pub(crate) fn is_normal_backtrack_mode(&self) -> bool {
        !self.is_task_running && self.view_stack.is_empty() && !self.composer.popup_active()
    }

    pub(crate) fn show_view(&mut self, view: Box<dyn BottomPaneView>) {
        self.push_view(view);
    }

    /// Called when the agent requests user approval.
    pub fn push_approval_request(&mut self, request: ApprovalRequest) {
        let request = if let Some(view) = self.view_stack.last_mut() {
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
        let modal = ApprovalOverlay::new(request, self.app_event_tx.clone());
        self.pause_status_timer_for_modal();
        self.push_view(Box::new(modal));
    }

    fn on_active_view_complete(&mut self) {
        self.resume_status_timer_after_modal();
    }

    fn pause_status_timer_for_modal(&mut self) {
        if let Some(status) = self.status.as_mut() {
            status.pause_timer();
        }
    }

    fn resume_status_timer_after_modal(&mut self) {
        if let Some(status) = self.status.as_mut() {
            status.resume_timer();
        }
    }

    /// Height (terminal rows) required by the current bottom pane.
    pub(crate) fn request_redraw(&self) {
        self.frame_requester.schedule_frame();
    }

    pub(crate) fn request_redraw_in(&self, dur: Duration) {
        self.frame_requester.schedule_frame_in(dur);
    }

    // --- History helpers ---

    pub(crate) fn set_history_metadata(&mut self, log_id: u64, entry_count: usize) {
        self.composer.set_history_metadata(log_id, entry_count);
    }

    pub(crate) fn flush_paste_burst_if_due(&mut self) -> bool {
        self.composer.flush_paste_burst_if_due()
    }

    pub(crate) fn is_in_paste_burst(&self) -> bool {
        self.composer.is_in_paste_burst()
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

    pub(crate) fn on_file_search_result(&mut self, query: String, matches: Vec<FileMatch>) {
        self.composer.on_file_search_result(query, matches);
        self.request_redraw();
    }

    pub(crate) fn attach_image(
        &mut self,
        path: PathBuf,
        width: u32,
        height: u32,
        format_label: &str,
    ) {
        if self.view_stack.is_empty() {
            self.composer
                .attach_image(path, width, height, format_label);
            self.request_redraw();
        }
    }

    pub(crate) fn take_recent_submission_images(&mut self) -> Vec<PathBuf> {
        self.composer.take_recent_submission_images()
    }
}

impl WidgetRef for &BottomPane {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let [status_area, content] = self.layout(area);

        // When a modal view is active, it owns the whole content area.
        if let Some(view) = self.active_view() {
            view.render(content, buf);
        } else {
            // No active modal:
            // If a status indicator is active, render it above the composer.
            if let Some(status) = &self.status {
                status.render_ref(status_area, buf);
            }

            // Render the composer in the remaining area.
            self.composer.render_ref(content, buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_event::AppEvent;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use tokio::sync::mpsc::unbounded_channel;

    fn exec_request() -> ApprovalRequest {
        ApprovalRequest::Exec {
            id: "1".to_string(),
            command: vec!["echo".into(), "ok".into()],
            reason: None,
        }
    }

    #[test]
    fn ctrl_c_on_modal_consumes_and_shows_quit_hint() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut pane = BottomPane::new(BottomPaneParams {
            app_event_tx: tx,
            frame_requester: FrameRequester::test_dummy(),
            has_input_focus: true,
            enhanced_keys_supported: false,
            placeholder_text: "Ask Codex to do anything".to_string(),
            disable_paste_burst: false,
        });
        pane.push_approval_request(exec_request());
        assert_eq!(CancellationEvent::Handled, pane.on_ctrl_c());
        assert!(pane.ctrl_c_quit_hint_visible());
        assert_eq!(CancellationEvent::NotHandled, pane.on_ctrl_c());
    }

    // live ring removed; related tests deleted.

    #[test]
    fn overlay_not_shown_above_approval_modal() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut pane = BottomPane::new(BottomPaneParams {
            app_event_tx: tx,
            frame_requester: FrameRequester::test_dummy(),
            has_input_focus: true,
            enhanced_keys_supported: false,
            placeholder_text: "Ask Codex to do anything".to_string(),
            disable_paste_burst: false,
        });

        // Create an approval modal (active view).
        pane.push_approval_request(exec_request());

        // Render and verify the top row does not include an overlay.
        let area = Rect::new(0, 0, 60, 6);
        let mut buf = Buffer::empty(area);
        (&pane).render_ref(area, &mut buf);

        let mut r0 = String::new();
        for x in 0..area.width {
            r0.push(buf[(x, 0)].symbol().chars().next().unwrap_or(' '));
        }
        assert!(
            !r0.contains("Working"),
            "overlay should not render above modal"
        );
    }

    #[test]
    fn composer_shown_after_denied_while_task_running() {
        let (tx_raw, rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut pane = BottomPane::new(BottomPaneParams {
            app_event_tx: tx,
            frame_requester: FrameRequester::test_dummy(),
            has_input_focus: true,
            enhanced_keys_supported: false,
            placeholder_text: "Ask Codex to do anything".to_string(),
            disable_paste_burst: false,
        });

        // Start a running task so the status indicator is active above the composer.
        pane.set_task_running(true);

        // Push an approval modal (e.g., command approval) which should hide the status view.
        pane.push_approval_request(exec_request());

        // Simulate pressing 'n' (No) on the modal.
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;
        pane.handle_key_event(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE));

        // After denial, since the task is still running, the status indicator should be
        // visible above the composer. The modal should be gone.
        assert!(
            pane.view_stack.is_empty(),
            "no active modal view after denial"
        );

        // Render and ensure the top row includes the Working header and a composer line below.
        // Give the animation thread a moment to tick.
        std::thread::sleep(Duration::from_millis(120));
        let area = Rect::new(0, 0, 40, 6);
        let mut buf = Buffer::empty(area);
        (&pane).render_ref(area, &mut buf);
        let mut row1 = String::new();
        for x in 0..area.width {
            row1.push(buf[(x, 1)].symbol().chars().next().unwrap_or(' '));
        }
        assert!(
            row1.contains("Working"),
            "expected Working header after denial on row 1: {row1:?}"
        );

        // Composer placeholder should be visible somewhere below.
        let mut found_composer = false;
        for y in 1..area.height {
            let mut row = String::new();
            for x in 0..area.width {
                row.push(buf[(x, y)].symbol().chars().next().unwrap_or(' '));
            }
            if row.contains("Ask Codex") {
                found_composer = true;
                break;
            }
        }
        assert!(
            found_composer,
            "expected composer visible under status line"
        );

        // Drain the channel to avoid unused warnings.
        drop(rx);
    }

    #[test]
    fn status_indicator_visible_during_command_execution() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut pane = BottomPane::new(BottomPaneParams {
            app_event_tx: tx,
            frame_requester: FrameRequester::test_dummy(),
            has_input_focus: true,
            enhanced_keys_supported: false,
            placeholder_text: "Ask Codex to do anything".to_string(),
            disable_paste_burst: false,
        });

        // Begin a task: show initial status.
        pane.set_task_running(true);

        // Use a height that allows the status line to be visible above the composer.
        let area = Rect::new(0, 0, 40, 6);
        let mut buf = Buffer::empty(area);
        (&pane).render_ref(area, &mut buf);

        let mut row0 = String::new();
        for x in 0..area.width {
            row0.push(buf[(x, 1)].symbol().chars().next().unwrap_or(' '));
        }
        assert!(
            row0.contains("Working"),
            "expected Working header: {row0:?}"
        );
    }

    #[test]
    fn bottom_padding_present_with_status_above_composer() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut pane = BottomPane::new(BottomPaneParams {
            app_event_tx: tx,
            frame_requester: FrameRequester::test_dummy(),
            has_input_focus: true,
            enhanced_keys_supported: false,
            placeholder_text: "Ask Codex to do anything".to_string(),
            disable_paste_burst: false,
        });

        // Activate spinner (status view replaces composer) with no live ring.
        pane.set_task_running(true);

        // Use height == desired_height; expect 1 status row at top and 2 bottom padding rows.
        let height = pane.desired_height(30);
        assert!(
            height >= 3,
            "expected at least 3 rows with bottom padding; got {height}"
        );
        let area = Rect::new(0, 0, 30, height);
        let mut buf = Buffer::empty(area);
        (&pane).render_ref(area, &mut buf);

        // Row 1 contains the status header (row 0 is the spacer)
        let mut top = String::new();
        for x in 0..area.width {
            top.push(buf[(x, 1)].symbol().chars().next().unwrap_or(' '));
        }
        assert!(
            top.trim_start().starts_with("• Working"),
            "expected top row to start with '• Working': {top:?}"
        );
        assert!(
            top.contains("Working"),
            "expected Working header on top row: {top:?}"
        );

        // Last row should be blank padding; the row above should generally contain composer content.
        let mut r_last = String::new();
        for x in 0..area.width {
            r_last.push(buf[(x, height - 1)].symbol().chars().next().unwrap_or(' '));
        }
        assert!(
            r_last.trim().is_empty(),
            "expected last row blank: {r_last:?}"
        );
    }

    #[test]
    fn bottom_padding_shrinks_when_tiny() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut pane = BottomPane::new(BottomPaneParams {
            app_event_tx: tx,
            frame_requester: FrameRequester::test_dummy(),
            has_input_focus: true,
            enhanced_keys_supported: false,
            placeholder_text: "Ask Codex to do anything".to_string(),
            disable_paste_burst: false,
        });

        pane.set_task_running(true);

        // Height=2 → status on one row, composer on the other.
        let area2 = Rect::new(0, 0, 20, 2);
        let mut buf2 = Buffer::empty(area2);
        (&pane).render_ref(area2, &mut buf2);
        let mut row0 = String::new();
        let mut row1 = String::new();
        for x in 0..area2.width {
            row0.push(buf2[(x, 0)].symbol().chars().next().unwrap_or(' '));
            row1.push(buf2[(x, 1)].symbol().chars().next().unwrap_or(' '));
        }
        let has_composer = row0.contains("Ask Codex") || row1.contains("Ask Codex");
        assert!(
            has_composer,
            "expected composer to be visible on one of the rows: row0={row0:?}, row1={row1:?}"
        );
        assert!(
            row0.contains("Working") || row1.contains("Working"),
            "expected status header to be visible at height=2: row0={row0:?}, row1={row1:?}"
        );

        // Height=1 → no padding; single row is the composer (status hidden).
        let area1 = Rect::new(0, 0, 20, 1);
        let mut buf1 = Buffer::empty(area1);
        (&pane).render_ref(area1, &mut buf1);
        let mut only = String::new();
        for x in 0..area1.width {
            only.push(buf1[(x, 0)].symbol().chars().next().unwrap_or(' '));
        }
        assert!(
            only.contains("Ask Codex"),
            "expected composer with no padding: {only:?}"
        );
    }
}
