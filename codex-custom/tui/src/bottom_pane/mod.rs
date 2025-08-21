//! Bottom pane: shows the ChatComposer or a BottomPaneView, if one is active.

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::user_approval_widget::ApprovalRequest;
use bottom_pane_view::BottomPaneView;
use codex_core::protocol::TokenUsage;
use codex_file_search::FileMatch;
use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::WidgetRef;

mod approval_modal_view;
mod bottom_pane_view;
mod chat_composer;
mod chat_composer_history;
mod command_popup;
mod file_search_popup;
mod live_ring_widget;
mod past_inputs_popup;
mod popup_consts;
mod prompts_popup;
mod resume_popup;
mod scroll_state;
mod selection_popup_common;
mod textarea;

/// Image attachments captured in the composer while the user is editing.
/// These are added when the user pastes an image path/URL/data URL and are
/// sent with the next submitted message.
#[derive(Debug, Clone)]
pub(crate) enum ImageSource {
    Local(std::path::PathBuf),
    Url(String),
    DataUrl(String),
}

#[derive(Debug, Clone)]
pub(crate) struct AttachedImage {
    pub(crate) source: ImageSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CancellationEvent {
    Ignored,
    Handled,
}

pub(crate) use chat_composer::ChatComposer;
pub(crate) use chat_composer::InputResult;

use crate::status_indicator_widget::StatusIndicatorWidget;
use approval_modal_view::ApprovalModalView;

/// Pane displayed in the lower half of the chat UI.
pub(crate) struct BottomPane<'a> {
    /// Composer is retained even when a BottomPaneView is displayed so the
    /// input state is retained when the view is closed.
    composer: ChatComposer,

    /// If present, this is displayed instead of the `composer`.
    active_view: Option<Box<dyn BottomPaneView<'a> + 'a>>,

    app_event_tx: AppEventSender,
    has_input_focus: bool,
    is_task_running: bool,
    ctrl_c_quit_hint: bool,

    /// Optional live, multi‑line status/"live cell" rendered directly above
    /// the composer while a task is running. Unlike `active_view`, this does
    /// not replace the composer; it augments it.
    live_status: Option<StatusIndicatorWidget>,

    /// Optional transient ring shown above the composer. This is a rendering-only
    /// container used during development before we wire it to ChatWidget events.
    live_ring: Option<live_ring_widget::LiveRingWidget>,

    /// True if the active view is the StatusIndicatorView that replaces the
    /// composer during a running task.
    status_view_active: bool,

    /// Optional single-line preview of the queued message (first line), shown
    /// above the composer while a task is running so users can manage it.
    queued_preview: Option<String>,
    /// When true, the queued preview line is highlighted (selected) to signal
    /// that ESC will cancel the queued message.
    queued_selected: bool,
    /// Current sandbox policy (for footer status display and quick toggle).
    sandbox_policy: codex_core::protocol::SandboxPolicy,
}

pub(crate) struct BottomPaneParams {
    pub(crate) app_event_tx: AppEventSender,
    pub(crate) has_input_focus: bool,
    pub(crate) enhanced_keys_supported: bool,
    pub(crate) approval_policy: codex_core::protocol::AskForApproval,
    pub(crate) sandbox_policy: codex_core::protocol::SandboxPolicy,
}

impl BottomPane<'_> {
    const BOTTOM_PAD_LINES: u16 = 2;
    pub fn new(params: BottomPaneParams) -> Self {
        let enhanced_keys_supported = params.enhanced_keys_supported;
        Self {
            composer: ChatComposer::new(
                params.has_input_focus,
                params.app_event_tx.clone(),
                enhanced_keys_supported,
                params.approval_policy,
                params.sandbox_policy.clone(),
            ),
            active_view: None,
            app_event_tx: params.app_event_tx,
            has_input_focus: params.has_input_focus,
            is_task_running: false,
            ctrl_c_quit_hint: false,
            live_status: None,
            live_ring: None,
            status_view_active: false,
            queued_preview: None,
            queued_selected: false,
            sandbox_policy: params.sandbox_policy,
        }
    }

    /// Attempt to read an image from the system clipboard and attach it to the
    /// composer as a data URL. Returns true if an image was attached.
    pub fn attach_image_from_clipboard(&mut self) -> bool {
        self.composer.attach_image_from_clipboard()
    }

    pub fn desired_height(&self, width: u16) -> u16 {
        let overlay_status_h = self
            .live_status
            .as_ref()
            .map(|s| s.desired_height(width))
            .unwrap_or(0);
        let ring_h = self
            .live_ring
            .as_ref()
            .map(|r| r.desired_height(width))
            .unwrap_or(0);

        let view_height = if let Some(view) = self.active_view.as_ref() {
            // Add a single blank spacer line between live ring and status view when active.
            let spacer = if self.live_ring.is_some() && self.status_view_active {
                1
            } else {
                0
            };
            spacer + view.desired_height(width)
        } else {
            let mut h = self.composer.desired_height(width);
            // Add one line for the queued preview banner when present.
            if self.queued_preview.is_some() {
                h = h.saturating_add(1);
            }
            h
        };

        overlay_status_h
            .saturating_add(ring_h)
            .saturating_add(view_height)
            .saturating_add(Self::BOTTOM_PAD_LINES)
    }

    pub fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        // Hide the cursor whenever a modal/overlay view is active (e.g. an
        // approval dialog). In these states the textarea is not interactable.
        if self.active_view.is_some() {
            return None;
        }

        // Compute the exact rectangle used to render the composer, mirroring
        // the layout logic in render_ref() so the cursor sits inside the
        // textarea (not over the status line).
        let mut y_offset = 0u16;
        if let Some(ring) = &self.live_ring {
            let live_h = ring.desired_height(area.width).min(area.height);
            y_offset = y_offset.saturating_add(live_h);
        }
        // Spacer between live ring and a status view when active – not used
        // for overlays, but keep the logic consistent with render_ref.
        if self.live_ring.is_some() && self.status_view_active && y_offset < area.height {
            y_offset = y_offset.saturating_add(1);
        }
        if let Some(status) = &self.live_status {
            let live_h = status
                .desired_height(area.width)
                .min(area.height.saturating_sub(y_offset));
            y_offset = y_offset.saturating_add(live_h);
        }

        // Account for the queued preview banner if present.
        if self.queued_preview.is_some() && y_offset < area.height {
            y_offset = y_offset.saturating_add(1);
        }

        // If no vertical space remains for the composer, we cannot place a cursor.
        if y_offset >= area.height {
            return None;
        }

        // Reserve bottom padding lines; keep at least 1 line for the composer when possible.
        let avail = area.height - y_offset;
        let pad = BottomPane::BOTTOM_PAD_LINES.min(avail.saturating_sub(1));
        let comp_h = avail.saturating_sub(pad);
        if comp_h == 0 {
            return None;
        }
        let composer_rect = Rect {
            x: area.x,
            y: area.y + y_offset,
            width: area.width,
            height: comp_h,
        };
        self.composer.cursor_pos(composer_rect)
    }

    /// Forward a key event to the active view or the composer.
    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> InputResult {
        if let Some(mut view) = self.active_view.take() {
            view.handle_key_event(self, key_event);
            if !view.is_complete() {
                self.active_view = Some(view);
            } else if self.is_task_running {
                // Task still running after modal completes – restore the
                // status as an overlay while keeping the composer visible.
                self.update_status_text("waiting for model".to_string());
                self.status_view_active = false;
            }
            self.request_redraw();
            InputResult::None
        } else {
            // Intercept management of queued message while a task is running:
            // Up selects/highlights the queued banner; ESC cancels it; Down
            // deselects.
            use crossterm::event::KeyCode;
            if self.is_task_running && self.queued_preview.is_some() {
                match key_event.code {
                    KeyCode::Up => {
                        if !self.queued_selected {
                            self.queued_selected = true;
                            self.request_redraw();
                            return InputResult::None;
                        }
                        // Already selected – keep focus on the banner.
                        return InputResult::None;
                    }
                    KeyCode::Enter => {
                        if self.queued_selected {
                            self.queued_selected = false;
                            self.request_redraw();
                            return InputResult::CancelQueued;
                        }
                    }
                    KeyCode::Esc => {
                        // ESC no longer cancels; it just deselects the banner.
                        if self.queued_selected {
                            self.queued_selected = false;
                            self.request_redraw();
                            return InputResult::None;
                        }
                    }
                    KeyCode::Down => {
                        if self.queued_selected {
                            self.queued_selected = false;
                            self.request_redraw();
                            return InputResult::None;
                        }
                    }
                    _ => {
                        if self.queued_selected {
                            // Any other input deselects and falls through to the composer.
                            self.queued_selected = false;
                            self.request_redraw();
                        }
                    }
                }
            }
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
                    // Modal aborted but task still running – show the status as
                    // a non-blocking overlay and keep the composer visible.
                    self.update_status_text("waiting for model".to_string());
                    self.status_view_active = false;
                    self.active_view = None;
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

    /// Send an AppEvent to the application layer.
    pub(crate) fn send_app_event(&self, event: AppEvent) {
        self.app_event_tx.send(event);
    }

    /// Update the approval policy shown in the composer footer (workflow status).
    pub(crate) fn set_approval_policy(&mut self, policy: codex_core::protocol::AskForApproval) {
        self.composer.set_approval_policy(policy);
        self.request_redraw();
    }

    /// Update the sandbox policy shown in the composer footer (workflow status).
    pub(crate) fn set_sandbox_policy(&mut self, policy: codex_core::protocol::SandboxPolicy) {
        self.sandbox_policy = policy.clone();
        self.composer.set_sandbox_policy(policy);
        self.request_redraw();
    }

    /// Consume any image attachments referenced by placeholders in `text`,
    /// ordered by the position they appear in the text, and clear them from
    /// the composer state.
    pub(crate) fn take_image_attachments_in_text_order(
        &mut self,
        text: &str,
    ) -> Vec<AttachedImage> {
        self.composer.take_image_attachments_in_text_order(text)
    }

    /// Show the resume popup to select a previous session.
    pub(crate) fn show_resume_popup(
        &mut self,
        codex_home: std::path::PathBuf,
        cwd: std::path::PathBuf,
    ) {
        let view = resume_popup::ResumePopup::new(self.app_event_tx.clone(), codex_home, cwd);
        self.active_view = Some(Box::new(view));
        self.status_view_active = false;
        self.request_redraw();
    }

    /// Show the "Past Inputs" popup for the current session so the user can
    /// pick a previous input to branch from.
    pub(crate) fn show_past_inputs_popup(
        &mut self,
        codex_home: std::path::PathBuf,
        session_id: uuid::Uuid,
    ) {
        let view = past_inputs_popup::PastInputsPopup::new(
            self.app_event_tx.clone(),
            codex_home,
            session_id,
        );
        self.active_view = Some(Box::new(view));
        self.status_view_active = false;
        self.request_redraw();
    }

    /// Show the prompts manager popup so users can insert/manage saved prompts.
    pub(crate) fn show_prompts_popup(&mut self, codex_home: std::path::PathBuf) {
        let view = prompts_popup::PromptsPopup::new(self.app_event_tx.clone(), codex_home);
        self.active_view = Some(Box::new(view));
        self.status_view_active = false;
        self.request_redraw();
    }

    /// Update the status indicator text. Prefer replacing the composer with
    /// the StatusIndicatorView so the input pane shows a single-line status
    /// like: `▌ Working waiting for model`.
    pub(crate) fn update_status_text(&mut self, text: String) {
        // If a modal view is active, forward the update to it so it can decide
        // whether to consume the change (e.g., approval dialog ignoring status).
        let mut handled_by_view = false;
        if let Some(view) = self.active_view.as_mut() {
            if view.update_status_text(text.clone()) {
                handled_by_view = true;
            }
        }

        // Prefer an overlay above the composer so users can continue typing
        // while the agent is working. Only show overlay when NO modal is
        // active; otherwise, clear it to avoid drawing over dialogs.
        if !handled_by_view && self.active_view.is_none() {
            if self.live_status.is_none() {
                self.live_status = Some(StatusIndicatorWidget::new(self.app_event_tx.clone()));
            }
            if let Some(status) = &mut self.live_status {
                status.update_text(text);
            }
            self.status_view_active = false;
        } else if !handled_by_view {
            // Ensure any previous overlay is cleared when a modal becomes active.
            self.live_status = None;
            self.status_view_active = false;
        }
        self.request_redraw();
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

    pub fn set_task_running(&mut self, running: bool) {
        self.is_task_running = running;

        if running {
            // Do not replace the composer; show status as an overlay so the
            // user can keep typing during long operations. The overlay will be
            // created/updated by update_status_text().
            self.status_view_active = false;
            self.request_redraw();
        } else {
            self.live_status = None;
            // Drop the status view when a task completes, but keep other
            // modal views (e.g. approval dialogs).
            if let Some(mut view) = self.active_view.take() {
                if !view.should_hide_when_task_is_done() {
                    self.active_view = Some(view);
                }
            }
            self.status_view_active = false;
            // Clear any queued selection state when a task ends.
            self.queued_selected = false;
        }
    }

    pub(crate) fn composer_is_empty(&self) -> bool {
        self.composer.is_empty()
    }

    pub(crate) fn is_task_running(&self) -> bool {
        self.is_task_running
    }

    // queued preview selection state is internal-only

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

    /// Control whether the composer shows a subtle "queued" tag in its footer.
    pub(crate) fn set_queued_indicator(&mut self, queued: bool) {
        self.composer.set_queued_indicator(queued);
        self.request_redraw();
    }

    /// Provide a preview (first line) of the queued message so the bottom
    /// pane can render and manage it while a task is running.
    pub(crate) fn set_queued_message_preview(&mut self, preview: Option<String>) {
        self.queued_preview = preview;
        if self.queued_preview.is_none() {
            self.queued_selected = false;
        }
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
        // Hide any overlay status while a modal is visible.
        self.live_status = None;
        self.status_view_active = false;
        self.request_redraw()
    }

    /// Height (terminal rows) required by the current bottom pane.
    pub(crate) fn request_redraw(&self) {
        self.app_event_tx.send(AppEvent::RequestRedraw)
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

    /// Set the rows and cap for the transient live ring overlay.
    pub(crate) fn set_live_ring_rows(&mut self, max_rows: u16, rows: Vec<Line<'static>>) {
        let mut w = live_ring_widget::LiveRingWidget::new();
        w.set_max_rows(max_rows);
        w.set_rows(rows);
        self.live_ring = Some(w);
    }

    /// True if a modal/overlay view is currently active (approval dialog,
    /// resume, past inputs, etc.). In these states the composer should not
    /// receive keyboard input and ESC should be handled by the modal.
    pub(crate) fn is_modal_active(&self) -> bool {
        self.active_view.is_some()
    }

    /// Programmatically replace the contents of the composer and move the
    /// cursor to the end of the inserted text.
    pub(crate) fn set_composer_text(&mut self, text: String) {
        self.composer.set_text(&text);
        self.request_redraw();
    }

    pub(crate) fn clear_live_ring(&mut self) {
        self.live_ring = None;
    }

    // Removed restart_live_status_with_text – no longer used by the current streaming UI.
}

impl WidgetRef for &BottomPane<'_> {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let mut y_offset = 0u16;
        if let Some(ring) = &self.live_ring {
            let live_h = ring.desired_height(area.width).min(area.height);
            if live_h > 0 {
                let live_rect = Rect {
                    x: area.x,
                    y: area.y,
                    width: area.width,
                    height: live_h,
                };
                ring.render_ref(live_rect, buf);
                y_offset = live_h;
            }
        }
        // Spacer between live ring and status view when active
        if self.live_ring.is_some() && self.status_view_active && y_offset < area.height {
            // Leave one empty line
            y_offset = y_offset.saturating_add(1);
        }
        if let Some(status) = &self.live_status {
            let live_h = status
                .desired_height(area.width)
                .min(area.height.saturating_sub(y_offset));
            if live_h > 0 {
                let live_rect = Rect {
                    x: area.x,
                    y: area.y + y_offset,
                    width: area.width,
                    height: live_h,
                };
                status.render_ref(live_rect, buf);
                y_offset = y_offset.saturating_add(live_h);
            }
        }

        // Queued preview banner renders directly above the composer when present.
        if let Some(preview) = &self.queued_preview {
            if y_offset < area.height {
                let banner_rect = Rect {
                    x: area.x,
                    y: area.y + y_offset,
                    width: area.width,
                    height: 1,
                };
                let mut spans: Vec<ratatui::text::Span<'static>> = Vec::new();
                spans.push(ratatui::text::Span::styled(
                    "▌ ",
                    Style::default().fg(Color::Cyan),
                ));
                spans.push(ratatui::text::Span::styled(
                    "Queued",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ));
                spans.push(ratatui::text::Span::raw(" — "));
                spans.push(ratatui::text::Span::styled(
                    preview.clone(),
                    Style::default().fg(Color::Gray),
                ));
                spans.push(ratatui::text::Span::raw("  "));
                spans.push(ratatui::text::Span::styled(
                    "Enter",
                    Style::default()
                        .fg(Color::Gray)
                        .add_modifier(Modifier::BOLD | Modifier::DIM),
                ));
                spans.push(ratatui::text::Span::styled(
                    " to cancel",
                    Style::default().fg(Color::Gray).add_modifier(Modifier::DIM),
                ));

                let mut line = Line::from(spans);
                if self.queued_selected {
                    line = line.style(Style::default().bg(Color::DarkGray));
                } else {
                    line = line.style(Style::default().add_modifier(Modifier::DIM));
                }
                line.render_ref(banner_rect, buf);
                y_offset = y_offset.saturating_add(1);
            }
        }

        if let Some(view) = &self.active_view {
            if y_offset < area.height {
                // Reserve bottom padding lines; keep at least 1 line for the view.
                let avail = area.height - y_offset;
                let pad = BottomPane::BOTTOM_PAD_LINES.min(avail.saturating_sub(1));
                let view_rect = Rect {
                    x: area.x,
                    y: area.y + y_offset,
                    width: area.width,
                    height: avail - pad,
                };
                view.render(view_rect, buf);
            }
        } else if y_offset < area.height {
            let composer_rect = Rect {
                x: area.x,
                y: area.y + y_offset,
                width: area.width,
                // Reserve bottom padding
                height: (area.height - y_offset)
                    - BottomPane::BOTTOM_PAD_LINES.min((area.height - y_offset).saturating_sub(1)),
            };
            (&self.composer).render_ref(composer_rect, buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_event::AppEvent;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::text::Line;
    use std::path::PathBuf;
    use std::sync::mpsc::channel;

    fn exec_request() -> ApprovalRequest {
        ApprovalRequest::Exec {
            id: "1".to_string(),
            command: vec!["echo".into(), "ok".into()],
            cwd: PathBuf::from("."),
            reason: None,
        }
    }

    #[test]
    fn ctrl_c_on_modal_consumes_and_shows_quit_hint() {
        let (tx_raw, _rx) = channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut pane = BottomPane::new(BottomPaneParams {
            app_event_tx: tx,
            has_input_focus: true,
            enhanced_keys_supported: false,
            approval_policy: codex_core::protocol::AskForApproval::OnRequest,
            sandbox_policy: codex_core::protocol::SandboxPolicy::new_workspace_write_policy(),
        });
        pane.push_approval_request(exec_request());
        assert_eq!(CancellationEvent::Handled, pane.on_ctrl_c());
        assert!(pane.ctrl_c_quit_hint_visible());
        assert_eq!(CancellationEvent::Ignored, pane.on_ctrl_c());
    }

    #[test]
    fn live_ring_renders_above_composer() {
        let (tx_raw, _rx) = channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut pane = BottomPane::new(BottomPaneParams {
            app_event_tx: tx,
            has_input_focus: true,
            enhanced_keys_supported: false,
            approval_policy: codex_core::protocol::AskForApproval::OnRequest,
            sandbox_policy: codex_core::protocol::SandboxPolicy::new_workspace_write_policy(),
        });

        // Provide 4 rows with max_rows=3; only the last 3 should be visible.
        pane.set_live_ring_rows(
            3,
            vec![
                Line::from("one".to_string()),
                Line::from("two".to_string()),
                Line::from("three".to_string()),
                Line::from("four".to_string()),
            ],
        );

        let area = Rect::new(0, 0, 10, 5);
        let mut buf = Buffer::empty(area);
        (&pane).render_ref(area, &mut buf);

        // Extract the first 3 rows and assert they contain the last three lines.
        let mut lines: Vec<String> = Vec::new();
        for y in 0..3 {
            let mut s = String::new();
            for x in 0..area.width {
                s.push(buf[(x, y)].symbol().chars().next().unwrap_or(' '));
            }
            lines.push(s.trim_end().to_string());
        }
        assert_eq!(lines, vec!["two", "three", "four"]);
    }

    #[test]
    fn status_indicator_visible_with_live_ring() {
        let (tx_raw, _rx) = channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut pane = BottomPane::new(BottomPaneParams {
            app_event_tx: tx,
            has_input_focus: true,
            enhanced_keys_supported: false,
            approval_policy: codex_core::protocol::AskForApproval::OnRequest,
            sandbox_policy: codex_core::protocol::SandboxPolicy::new_workspace_write_policy(),
        });

        // Simulate task running which replaces composer with the status indicator.
        pane.set_task_running(true);
        pane.update_status_text("waiting for model".to_string());

        // Provide 2 rows in the live ring (e.g., streaming CoT) and ensure the
        // status indicator remains visible below them.
        pane.set_live_ring_rows(
            2,
            vec![
                Line::from("cot1".to_string()),
                Line::from("cot2".to_string()),
            ],
        );

        // Allow some frames so the dot animation is present.
        std::thread::sleep(std::time::Duration::from_millis(120));

        // Height should include both ring rows and the 1-line status overlay.
        let area = Rect::new(0, 0, 30, 4);
        let mut buf = Buffer::empty(area);
        (&pane).render_ref(area, &mut buf);

        // Top two rows are the live ring.
        let mut r0 = String::new();
        let mut r1 = String::new();
        for x in 0..area.width {
            r0.push(buf[(x, 0)].symbol().chars().next().unwrap_or(' '));
            r1.push(buf[(x, 1)].symbol().chars().next().unwrap_or(' '));
        }
        assert!(r0.contains("cot1"), "expected first live row: {r0:?}");
        assert!(r1.contains("cot2"), "expected second live row: {r1:?}");

        // Next row should be the status overlay; it should contain the left bar and "Working".
        let mut r2 = String::new();
        for x in 0..area.width {
            r2.push(buf[(x, 2)].symbol().chars().next().unwrap_or(' '));
        }
        assert_eq!(buf[(0, 2)].symbol().chars().next().unwrap_or(' '), '▌');
        assert!(
            r2.contains("Working"),
            "expected Working header in status line: {r2:?}"
        );
    }

    #[test]
    fn overlay_not_shown_above_approval_modal() {
        let (tx_raw, _rx) = channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut pane = BottomPane::new(BottomPaneParams {
            app_event_tx: tx,
            has_input_focus: true,
            enhanced_keys_supported: false,
            approval_policy: codex_core::protocol::AskForApproval::OnRequest,
            sandbox_policy: codex_core::protocol::SandboxPolicy::new_workspace_write_policy(),
        });

        // Create an approval modal (active view).
        pane.push_approval_request(exec_request());
        // Attempt to update status; this should NOT create an overlay while modal is visible.
        pane.update_status_text("running command".to_string());

        // Render and verify the top row does not include the Working header overlay.
        let area = Rect::new(0, 0, 60, 6);
        let mut buf = Buffer::empty(area);
        (&pane).render_ref(area, &mut buf);

        let mut r0 = String::new();
        for x in 0..area.width {
            r0.push(buf[(x, 0)].symbol().chars().next().unwrap_or(' '));
        }
        assert!(
            !r0.contains("Working"),
            "overlay Working header should not render above modal"
        );
    }

    #[test]
    fn composer_shown_after_denied_if_task_running() {
        let (tx_raw, rx) = channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut pane = BottomPane::new(BottomPaneParams {
            app_event_tx: tx.clone(),
            has_input_focus: true,
            enhanced_keys_supported: false,
            approval_policy: codex_core::protocol::AskForApproval::OnRequest,
            sandbox_policy: codex_core::protocol::SandboxPolicy::new_workspace_write_policy(),
        });

        // Start a running task so the status indicator replaces the composer.
        pane.set_task_running(true);
        pane.update_status_text("waiting for model".to_string());

        // Push an approval modal (e.g., command approval) which should hide the status view.
        pane.push_approval_request(exec_request());

        // Simulate pressing 'n' (deny) on the modal.
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;
        pane.handle_key_event(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE));

        // After denial, since the task is still running, the composer should
        // remain visible and the status overlay should be shown above it.
        assert!(
            !pane.status_view_active,
            "status view should not replace composer after denial"
        );
        assert!(
            pane.active_view.is_none(),
            "no active view should be present"
        );

        // Render and ensure the top row includes the Working header instead of the composer.
        // Give the animation thread a moment to tick.
        std::thread::sleep(std::time::Duration::from_millis(120));
        let area = Rect::new(0, 0, 40, 3);
        let mut buf = Buffer::empty(area);
        (&pane).render_ref(area, &mut buf);
        let mut row0 = String::new();
        for x in 0..area.width {
            row0.push(buf[(x, 0)].symbol().chars().next().unwrap_or(' '));
        }
        assert!(
            row0.contains("Working"),
            "expected Working header after denial: {row0:?}"
        );

        // Drain the channel to avoid unused warnings.
        drop(rx);
    }

    #[test]
    fn status_indicator_visible_during_command_execution() {
        let (tx_raw, _rx) = channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut pane = BottomPane::new(BottomPaneParams {
            app_event_tx: tx,
            has_input_focus: true,
            enhanced_keys_supported: false,
            approval_policy: codex_core::protocol::AskForApproval::OnRequest,
            sandbox_policy: codex_core::protocol::SandboxPolicy::new_workspace_write_policy(),
        });

        // Begin a task: show initial status.
        pane.set_task_running(true);
        pane.update_status_text("waiting for model".to_string());

        // As a long-running command begins (post-approval), ensure the status
        // indicator is visible while we wait for the command to run.
        pane.update_status_text("running command".to_string());

        // Allow some frames so the animation thread ticks.
        std::thread::sleep(std::time::Duration::from_millis(120));

        // Render and confirm the line contains the "Working" header.
        let area = Rect::new(0, 0, 40, 3);
        let mut buf = Buffer::empty(area);
        (&pane).render_ref(area, &mut buf);

        let mut row0 = String::new();
        for x in 0..area.width {
            row0.push(buf[(x, 0)].symbol().chars().next().unwrap_or(' '));
        }
        assert!(
            row0.contains("Working"),
            "expected Working header: {row0:?}"
        );
    }

    #[test]
    fn bottom_padding_present_for_status_view() {
        let (tx_raw, _rx) = channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut pane = BottomPane::new(BottomPaneParams {
            app_event_tx: tx,
            has_input_focus: true,
            enhanced_keys_supported: false,
            approval_policy: codex_core::protocol::AskForApproval::OnRequest,
            sandbox_policy: codex_core::protocol::SandboxPolicy::new_workspace_write_policy(),
        });

        // Activate spinner (status view replaces composer) with no live ring.
        pane.set_task_running(true);
        pane.update_status_text("waiting for model".to_string());

        // Use height == desired_height; expect 1 status row at top and 2 bottom padding rows.
        let height = pane.desired_height(30);
        assert!(
            height >= 3,
            "expected at least 3 rows with bottom padding; got {height}"
        );
        let area = Rect::new(0, 0, 30, height);
        let mut buf = Buffer::empty(area);
        (&pane).render_ref(area, &mut buf);

        // Top row contains the status header
        let mut top = String::new();
        for x in 0..area.width {
            top.push(buf[(x, 0)].symbol().chars().next().unwrap_or(' '));
        }
        assert_eq!(buf[(0, 0)].symbol().chars().next().unwrap_or(' '), '▌');
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
        let (tx_raw, _rx) = channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut pane = BottomPane::new(BottomPaneParams {
            app_event_tx: tx,
            has_input_focus: true,
            enhanced_keys_supported: false,
            approval_policy: codex_core::protocol::AskForApproval::OnRequest,
            sandbox_policy: codex_core::protocol::SandboxPolicy::new_workspace_write_policy(),
        });

        pane.set_task_running(true);
        pane.update_status_text("waiting for model".to_string());

        // Height=2 → overlay occupies row 0; composer uses row 1 (no padding remains).
        let area2 = Rect::new(0, 0, 20, 2);
        let mut buf2 = Buffer::empty(area2);
        (&pane).render_ref(area2, &mut buf2);
        let mut row0 = String::new();
        let mut row1 = String::new();
        for x in 0..area2.width {
            row0.push(buf2[(x, 0)].symbol().chars().next().unwrap_or(' '));
            row1.push(buf2[(x, 1)].symbol().chars().next().unwrap_or(' '));
        }
        assert!(
            row0.contains("Working"),
            "expected Working header on row 0: {row0:?}"
        );
        // The composer should occupy the bottom row (not blank).
        assert!(
            !row1.trim().is_empty(),
            "expected composer visible on bottom row: {row1:?}"
        );

        // Height=1 → single row is the status overlay.
        let area1 = Rect::new(0, 0, 20, 1);
        let mut buf1 = Buffer::empty(area1);
        (&pane).render_ref(area1, &mut buf1);
        let mut only = String::new();
        for x in 0..area1.width {
            only.push(buf1[(x, 0)].symbol().chars().next().unwrap_or(' '));
        }
        assert!(
            only.contains("Working"),
            "expected Working header with no padding/composer: {only:?}"
        );
    }
}
