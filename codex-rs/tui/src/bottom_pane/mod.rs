//! Bottom pane: shows the ChatComposer or a BottomPaneView, if one is active.
use std::path::PathBuf;

use crate::app_event_sender::AppEventSender;
use crate::tui::FrameRequester;
use crate::user_approval_widget::ApprovalRequest;
use bottom_pane_view::BottomPaneView;
use codex_core::protocol::TokenUsage;
use codex_file_search::FileMatch;
use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::widgets::WidgetRef;

mod approval_modal_view;
mod bottom_pane_view;
mod chat_composer;
mod chat_composer_history;
mod command_popup;
mod file_search_popup;
mod list_selection_view;
mod popup_consts;
mod scroll_state;
mod selection_popup_common;
mod status_indicator_view;
mod textarea;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CancellationEvent {
    Ignored,
    Handled,
}

pub(crate) use chat_composer::ChatComposer;
pub(crate) use chat_composer::InputResult;

use approval_modal_view::ApprovalModalView;
pub(crate) use list_selection_view::SelectionAction;
pub(crate) use list_selection_view::SelectionItem;
use status_indicator_view::StatusIndicatorView;

/// Pane displayed in the lower half of the chat UI.
pub(crate) struct BottomPane {
    /// Composer is retained even when a BottomPaneView is displayed so the
    /// input state is retained when the view is closed.
    composer: ChatComposer,

    /// If present, this is displayed instead of the `composer`.
    active_view: Option<Box<dyn BottomPaneView>>,

    app_event_tx: AppEventSender,
    frame_requester: FrameRequester,

    has_input_focus: bool,
    is_task_running: bool,
    ctrl_c_quit_hint: bool,
    esc_backtrack_hint: bool,

    /// True if the active view is the StatusIndicatorView that replaces the
    /// composer during a running task.
    status_view_active: bool,
}

pub(crate) struct BottomPaneParams {
    pub(crate) app_event_tx: AppEventSender,
    pub(crate) frame_requester: FrameRequester,
    pub(crate) has_input_focus: bool,
    pub(crate) enhanced_keys_supported: bool,
    pub(crate) placeholder_text: String,
}

impl BottomPane {
    const BOTTOM_PAD_LINES: u16 = 2;
    pub fn new(params: BottomPaneParams) -> Self {
        let enhanced_keys_supported = params.enhanced_keys_supported;
        Self {
            composer: ChatComposer::new(
                params.has_input_focus,
                params.app_event_tx.clone(),
                enhanced_keys_supported,
                params.placeholder_text,
            ),
            active_view: None,
            app_event_tx: params.app_event_tx,
            frame_requester: params.frame_requester,
            has_input_focus: params.has_input_focus,
            is_task_running: false,
            ctrl_c_quit_hint: false,
            esc_backtrack_hint: false,
            status_view_active: false,
        }
    }

    pub fn desired_height(&self, width: u16) -> u16 {
        let view_height = if let Some(view) = self.active_view.as_ref() {
            view.desired_height(width)
        } else {
            self.composer.desired_height(width)
        };
        let top_pad = if self.active_view.is_none() || self.status_view_active {
            1
        } else {
            0
        };
        view_height
            .saturating_add(Self::BOTTOM_PAD_LINES)
            .saturating_add(top_pad)
    }

    fn layout(&self, area: Rect) -> Rect {
        let top = if self.active_view.is_none() || self.status_view_active {
            1
        } else {
            0
        };

        let [_, content, _] = Layout::vertical([
            Constraint::Max(top),
            Constraint::Min(1),
            Constraint::Max(BottomPane::BOTTOM_PAD_LINES),
        ])
        .areas(area);

        content
    }

    pub fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        // Hide the cursor whenever an overlay view is active (e.g. the
        // status indicator shown while a task is running, or approval modal).
        // In these states the textarea is not interactable, so we should not
        // show its caret.
        if self.active_view.is_some() || self.status_view_active {
            None
        } else {
            let content = self.layout(area);
            self.composer.cursor_pos(content)
        }
    }

    /// Forward a key event to the active view or the composer.
    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> InputResult {
        if let Some(mut view) = self.active_view.take() {
            view.handle_key_event(self, key_event);
            if !view.is_complete() {
                self.active_view = Some(view);
            } else if self.is_task_running {
                let mut v = StatusIndicatorView::new(
                    self.app_event_tx.clone(),
                    self.frame_requester.clone(),
                );
                v.update_text("waiting for model".to_string());
                self.active_view = Some(Box::new(v));
                self.status_view_active = true;
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

    /// Handle Ctrl-C in the bottom pane. If a modal view is active it gets a
    /// chance to consume the event (e.g. to dismiss itself).
    pub(crate) fn on_ctrl_c(&mut self) -> CancellationEvent {
        let mut view = match self.active_view.take() {
            Some(view) => view,
            None => return CancellationEvent::Ignored,
        };

        let event = view.on_ctrl_c(self);
        match event {
            CancellationEvent::Handled => {
                if !view.is_complete() {
                    self.active_view = Some(view);
                } else if self.is_task_running {
                    // Modal aborted but task still running – restore status indicator.
                    let mut v = StatusIndicatorView::new(
                        self.app_event_tx.clone(),
                        self.frame_requester.clone(),
                    );
                    v.update_text("waiting for model".to_string());
                    self.active_view = Some(Box::new(v));
                    self.status_view_active = true;
                }
                self.show_ctrl_c_quit_hint();
            }
            CancellationEvent::Ignored => {
                self.active_view = Some(view);
            }
        }
        event
    }

    pub fn handle_paste(&mut self, pasted: String) {
        if self.active_view.is_none() {
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

    /// Update the animated header shown to the left of the brackets in the
    /// status indicator (defaults to "Working"). This will update the active
    /// StatusIndicatorView if present; otherwise, if a live overlay is active,
    /// it will update that. If neither is present, this call is a no-op.
    pub(crate) fn update_status_header(&mut self, header: String) {
        if let Some(view) = self.active_view.as_mut() {
            view.update_status_header(header.clone());
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

        if running {
            if self.active_view.is_none() {
                self.active_view = Some(Box::new(StatusIndicatorView::new(
                    self.app_event_tx.clone(),
                    self.frame_requester.clone(),
                )));
                self.status_view_active = true;
            }
            self.request_redraw();
        } else {
            // Drop the status view when a task completes, but keep other
            // modal views (e.g. approval dialogs).
            if let Some(mut view) = self.active_view.take() {
                if !view.should_hide_when_task_is_done() {
                    self.active_view = Some(view);
                }
                self.status_view_active = false;
            }
        }
    }

    /// Show a generic list selection view with the provided items.
    pub(crate) fn show_selection_view(
        &mut self,
        title: String,
        subtitle: Option<String>,
        footer_hint: Option<String>,
        items: Vec<SelectionItem>,
    ) {
        let view = list_selection_view::ListSelectionView::new(
            title,
            subtitle,
            footer_hint,
            items,
            self.app_event_tx.clone(),
        );
        self.active_view = Some(Box::new(view));
        self.status_view_active = false;
        self.request_redraw();
    }

    /// Update the live status text shown while a task is running.
    /// If a modal view is active (i.e., not the status indicator), this is a no‑op.
    pub(crate) fn update_status_text(&mut self, text: String) {
        if !self.is_task_running || !self.status_view_active {
            return;
        }
        if let Some(mut view) = self.active_view.take() {
            view.update_status_text(text);
            self.active_view = Some(view);
            self.request_redraw();
        }
    }

    pub(crate) fn composer_is_empty(&self) -> bool {
        self.composer.is_empty()
    }

    pub(crate) fn is_task_running(&self) -> bool {
        self.is_task_running
    }

    /// Update the *context-window remaining* indicator in the composer. This
    /// is forwarded directly to the underlying `ChatComposer`.
    pub(crate) fn set_token_usage(
        &mut self,
        total_token_usage: TokenUsage,
        last_token_usage: TokenUsage,
        model_context_window: Option<u64>,
    ) {
        self.composer
            .set_token_usage(total_token_usage, last_token_usage, model_context_window);
        self.request_redraw();
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
        self.status_view_active = false;
        self.request_redraw()
    }

    /// Height (terminal rows) required by the current bottom pane.
    pub(crate) fn request_redraw(&self) {
        self.frame_requester.schedule_frame();
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
        if self.active_view.is_none() {
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
        let content = self.layout(area);

        if let Some(view) = &self.active_view {
            view.render(content, buf);
        } else {
            (&self.composer).render_ref(content, buf);
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
            frame_requester: crate::tui::FrameRequester::test_dummy(),
            has_input_focus: true,
            enhanced_keys_supported: false,
            placeholder_text: "Ask Codex to do anything".to_string(),
        });
        pane.push_approval_request(exec_request());
        assert_eq!(CancellationEvent::Handled, pane.on_ctrl_c());
        assert!(pane.ctrl_c_quit_hint_visible());
        assert_eq!(CancellationEvent::Ignored, pane.on_ctrl_c());
    }

    // live ring removed; related tests deleted.

    #[test]
    fn overlay_not_shown_above_approval_modal() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut pane = BottomPane::new(BottomPaneParams {
            app_event_tx: tx,
            frame_requester: crate::tui::FrameRequester::test_dummy(),
            has_input_focus: true,
            enhanced_keys_supported: false,
            placeholder_text: "Ask Codex to do anything".to_string(),
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
    fn composer_not_shown_after_denied_if_task_running() {
        let (tx_raw, rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut pane = BottomPane::new(BottomPaneParams {
            app_event_tx: tx.clone(),
            frame_requester: crate::tui::FrameRequester::test_dummy(),
            has_input_focus: true,
            enhanced_keys_supported: false,
            placeholder_text: "Ask Codex to do anything".to_string(),
        });

        // Start a running task so the status indicator replaces the composer.
        pane.set_task_running(true);

        // Push an approval modal (e.g., command approval) which should hide the status view.
        pane.push_approval_request(exec_request());

        // Simulate pressing 'n' (deny) on the modal.
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;
        pane.handle_key_event(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE));

        // After denial, since the task is still running, the status indicator
        // should be restored as the active view; the composer should NOT be visible.
        assert!(
            pane.status_view_active,
            "status view should be active after denial"
        );
        assert!(pane.active_view.is_some(), "active view should be present");

        // Render and ensure the top row includes the Working header instead of the composer.
        let area = Rect::new(0, 0, 40, 3);
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

        // Drain the channel to avoid unused warnings.
        drop(rx);
    }

    #[test]
    fn status_indicator_visible_during_command_execution() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut pane = BottomPane::new(BottomPaneParams {
            app_event_tx: tx,
            frame_requester: crate::tui::FrameRequester::test_dummy(),
            has_input_focus: true,
            enhanced_keys_supported: false,
            placeholder_text: "Ask Codex to do anything".to_string(),
        });

        // Begin a task: show initial status.
        pane.set_task_running(true);

        let area = Rect::new(0, 0, 40, 3);
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
    fn bottom_padding_present_for_status_view() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut pane = BottomPane::new(BottomPaneParams {
            app_event_tx: tx,
            frame_requester: crate::tui::FrameRequester::test_dummy(),
            has_input_focus: true,
            enhanced_keys_supported: false,
            placeholder_text: "Ask Codex to do anything".to_string(),
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
        assert_eq!(buf[(0, 1)].symbol().chars().next().unwrap_or(' '), '▌');
        assert!(
            top.contains("Working"),
            "expected Working header on top row: {top:?}"
        );

        // Bottom two rows are blank padding
        let mut r_last = String::new();
        let mut r_last2 = String::new();
        for x in 0..area.width {
            r_last.push(buf[(x, height - 1)].symbol().chars().next().unwrap_or(' '));
            r_last2.push(buf[(x, height - 2)].symbol().chars().next().unwrap_or(' '));
        }
        assert!(
            r_last.trim().is_empty(),
            "expected last row blank: {r_last:?}"
        );
        assert!(
            r_last2.trim().is_empty(),
            "expected second-to-last row blank: {r_last2:?}"
        );
    }

    #[test]
    fn bottom_padding_shrinks_when_tiny() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut pane = BottomPane::new(BottomPaneParams {
            app_event_tx: tx,
            frame_requester: crate::tui::FrameRequester::test_dummy(),
            has_input_focus: true,
            enhanced_keys_supported: false,
            placeholder_text: "Ask Codex to do anything".to_string(),
        });

        pane.set_task_running(true);

        // Height=2 → with spacer, spinner on row 1; no bottom padding.
        let area2 = Rect::new(0, 0, 20, 2);
        let mut buf2 = Buffer::empty(area2);
        (&pane).render_ref(area2, &mut buf2);
        let mut row0 = String::new();
        let mut row1 = String::new();
        for x in 0..area2.width {
            row0.push(buf2[(x, 0)].symbol().chars().next().unwrap_or(' '));
            row1.push(buf2[(x, 1)].symbol().chars().next().unwrap_or(' '));
        }
        assert!(row0.trim().is_empty(), "expected spacer on row 0: {row0:?}");
        assert!(
            row1.contains("Working"),
            "expected Working on row 1: {row1:?}"
        );

        // Height=1 → no padding; single row is the spinner.
        let area1 = Rect::new(0, 0, 20, 1);
        let mut buf1 = Buffer::empty(area1);
        (&pane).render_ref(area1, &mut buf1);
        let mut only = String::new();
        for x in 0..area1.width {
            only.push(buf1[(x, 0)].symbol().chars().next().unwrap_or(' '));
        }
        assert!(
            only.contains("Working"),
            "expected Working header with no padding: {only:?}"
        );
    }
}
