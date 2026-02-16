//! The bottom pane is the interactive footer of the chat UI.
//!
//! The pane owns the [`ChatComposer`] (editable prompt input) and a stack of transient
//! [`BottomPaneView`]s (popups/modals) that temporarily replace the composer for focused
//! interactions like selection lists.
//!
//! Input routing is layered: `BottomPane` decides which local surface receives a key (view vs
//! composer), while higher-level intent such as "interrupt" or "quit" is decided by the parent
//! widget (`ChatWidget`). This split matters for Ctrl+C/Ctrl+D: the bottom pane gives the active
//! view the first chance to consume Ctrl+C (typically to dismiss itself), and `ChatWidget` may
//! treat an unhandled Ctrl+C as an interrupt or as the first press of a double-press quit
//! shortcut.
//!
//! Some UI is time-based rather than input-based, such as the transient "press again to quit"
//! hint. The pane schedules redraws so those hints can expire even when the UI is otherwise idle.
use std::path::PathBuf;

use crate::app_event::ConnectorsSnapshot;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::queued_user_messages::QueuedUserMessages;
use crate::bottom_pane::unified_exec_footer::UnifiedExecFooter;
use crate::key_hint;
use crate::key_hint::KeyBinding;
use crate::render::renderable::FlexRenderable;
use crate::render::renderable::Renderable;
use crate::render::renderable::RenderableItem;
use crate::tui::FrameRequester;
use bottom_pane_view::BottomPaneView;
use codex_core::features::Features;
use codex_core::skills::model::SkillMetadata;
use codex_file_search::FileMatch;
use codex_protocol::request_user_input::RequestUserInputEvent;
use codex_protocol::user_input::TextElement;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use std::time::Duration;

mod app_link_view;
mod approval_overlay;
mod multi_select_picker;
mod request_user_input;
mod status_line_setup;
pub(crate) use app_link_view::AppLinkView;
pub(crate) use app_link_view::AppLinkViewParams;
pub(crate) use approval_overlay::ApprovalOverlay;
pub(crate) use approval_overlay::ApprovalRequest;
pub(crate) use request_user_input::RequestUserInputOverlay;
mod bottom_pane_view;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LocalImageAttachment {
    pub(crate) placeholder: String,
    pub(crate) path: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MentionBinding {
    /// Mention token text without the leading `$`.
    pub(crate) mention: String,
    /// Canonical mention target (for example `app://...` or absolute SKILL.md path).
    pub(crate) path: String,
}
mod chat_composer;
mod chat_composer_history;
mod command_popup;
pub mod custom_prompt_view;
mod experimental_features_view;
mod file_search_popup;
mod footer;
mod list_selection_view;
mod prompt_args;
mod skill_popup;
mod skills_toggle_view;
mod slash_commands;
pub(crate) use footer::CollaborationModeIndicator;
pub(crate) use list_selection_view::ColumnWidthMode;
pub(crate) use list_selection_view::SelectionViewParams;
mod feedback_view;
pub(crate) use feedback_view::FeedbackAudience;
pub(crate) use feedback_view::feedback_disabled_params;
pub(crate) use feedback_view::feedback_selection_params;
pub(crate) use feedback_view::feedback_upload_consent_params;
pub(crate) use skills_toggle_view::SkillsToggleItem;
pub(crate) use skills_toggle_view::SkillsToggleView;
pub(crate) use status_line_setup::StatusLineItem;
pub(crate) use status_line_setup::StatusLineSetupView;
mod paste_burst;
pub mod popup_consts;
mod queued_user_messages;
mod scroll_state;
mod selection_popup_common;
mod textarea;
mod unified_exec_footer;
pub(crate) use feedback_view::FeedbackNoteView;

/// How long the "press again to quit" hint stays visible.
///
/// This is shared between:
/// - `ChatWidget`: arming the double-press quit shortcut.
/// - `BottomPane`/`ChatComposer`: rendering and expiring the footer hint.
///
/// Keeping a single value ensures Ctrl+C and Ctrl+D behave identically.
pub(crate) const QUIT_SHORTCUT_TIMEOUT: Duration = Duration::from_secs(1);

/// Whether Ctrl+C/Ctrl+D require a second press to quit.
///
/// This UX experiment was enabled by default, but requiring a double press to quit feels janky in
/// practice (especially for users accustomed to shells and other TUIs). Disable it for now while we
/// rethink a better quit/interrupt design.
pub(crate) const DOUBLE_PRESS_QUIT_SHORTCUT_ENABLED: bool = false;

/// The result of offering a cancellation key to a bottom-pane surface.
///
/// This is primarily used for Ctrl+C routing: active views can consume the key to dismiss
/// themselves, and the caller can decide what higher-level action (if any) to take when the key is
/// not handled locally.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CancellationEvent {
    Handled,
    NotHandled,
}

pub(crate) use chat_composer::ChatComposer;
pub(crate) use chat_composer::ChatComposerConfig;
pub(crate) use chat_composer::InputResult;
use codex_protocol::custom_prompts::CustomPrompt;

use crate::status_indicator_widget::StatusIndicatorWidget;
pub(crate) use experimental_features_view::ExperimentalFeatureItem;
pub(crate) use experimental_features_view::ExperimentalFeaturesView;
pub(crate) use list_selection_view::SelectionAction;
pub(crate) use list_selection_view::SelectionItem;

/// Pane displayed in the lower half of the chat UI.
///
/// This is the owning container for the prompt input (`ChatComposer`) and the view stack
/// (`BottomPaneView`). It performs local input routing and renders time-based hints, while leaving
/// process-level decisions (quit, interrupt, shutdown) to `ChatWidget`.
pub(crate) struct BottomPane {
    /// Composer is retained even when a BottomPaneView is displayed so the
    /// input state is retained when the view is closed.
    composer: ChatComposer,

    /// Stack of views displayed instead of the composer (e.g. popups/modals).
    view_stack: Vec<Box<dyn BottomPaneView>>,

    app_event_tx: AppEventSender,
    frame_requester: FrameRequester,

    has_input_focus: bool,
    enhanced_keys_supported: bool,
    disable_paste_burst: bool,
    is_task_running: bool,
    esc_backtrack_hint: bool,
    animations_enabled: bool,

    /// Inline status indicator shown above the composer while a task is running.
    status: Option<StatusIndicatorWidget>,
    /// Unified exec session summary source.
    ///
    /// When a status row exists, this summary is mirrored inline in that row;
    /// when no status row exists, it renders as its own footer row.
    unified_exec_footer: UnifiedExecFooter,
    /// Queued user messages to show above the composer while a turn is running.
    queued_user_messages: QueuedUserMessages,
    context_window_percent: Option<i64>,
    context_window_used_tokens: Option<i64>,
}

pub(crate) struct BottomPaneParams {
    pub(crate) app_event_tx: AppEventSender,
    pub(crate) frame_requester: FrameRequester,
    pub(crate) has_input_focus: bool,
    pub(crate) enhanced_keys_supported: bool,
    pub(crate) placeholder_text: String,
    pub(crate) disable_paste_burst: bool,
    pub(crate) animations_enabled: bool,
    pub(crate) skills: Option<Vec<SkillMetadata>>,
}

impl BottomPane {
    pub fn new(params: BottomPaneParams) -> Self {
        let BottomPaneParams {
            app_event_tx,
            frame_requester,
            has_input_focus,
            enhanced_keys_supported,
            placeholder_text,
            disable_paste_burst,
            animations_enabled,
            skills,
        } = params;
        let mut composer = ChatComposer::new(
            has_input_focus,
            app_event_tx.clone(),
            enhanced_keys_supported,
            placeholder_text,
            disable_paste_burst,
        );
        composer.set_skill_mentions(skills);

        Self {
            composer,
            view_stack: Vec::new(),
            app_event_tx,
            frame_requester,
            has_input_focus,
            enhanced_keys_supported,
            disable_paste_burst,
            is_task_running: false,
            status: None,
            unified_exec_footer: UnifiedExecFooter::new(),
            queued_user_messages: QueuedUserMessages::new(),
            esc_backtrack_hint: false,
            animations_enabled,
            context_window_percent: None,
            context_window_used_tokens: None,
        }
    }

    pub fn set_skills(&mut self, skills: Option<Vec<SkillMetadata>>) {
        self.composer.set_skill_mentions(skills);
        self.request_redraw();
    }

    /// Update image-paste behavior for the active composer and repaint immediately.
    ///
    /// Callers use this to keep composer affordances aligned with model capabilities.
    pub fn set_image_paste_enabled(&mut self, enabled: bool) {
        self.composer.set_image_paste_enabled(enabled);
        self.request_redraw();
    }

    pub fn set_connectors_snapshot(&mut self, snapshot: Option<ConnectorsSnapshot>) {
        self.composer.set_connector_mentions(snapshot);
        self.request_redraw();
    }

    pub fn take_mention_bindings(&mut self) -> Vec<MentionBinding> {
        self.composer.take_mention_bindings()
    }

    pub fn take_recent_submission_mention_bindings(&mut self) -> Vec<MentionBinding> {
        self.composer.take_recent_submission_mention_bindings()
    }

    /// Clear pending attachments and mention bindings e.g. when a slash command doesn't submit text.
    pub(crate) fn drain_pending_submission_state(&mut self) {
        let _ = self.take_recent_submission_images_with_placeholders();
        let _ = self.take_remote_image_urls();
        let _ = self.take_recent_submission_mention_bindings();
        let _ = self.take_mention_bindings();
    }

    pub fn set_steer_enabled(&mut self, enabled: bool) {
        self.composer.set_steer_enabled(enabled);
    }

    pub fn set_collaboration_modes_enabled(&mut self, enabled: bool) {
        self.composer.set_collaboration_modes_enabled(enabled);
        self.request_redraw();
    }

    pub fn set_connectors_enabled(&mut self, enabled: bool) {
        self.composer.set_connectors_enabled(enabled);
    }

    #[cfg(target_os = "windows")]
    pub fn set_windows_degraded_sandbox_active(&mut self, enabled: bool) {
        self.composer.set_windows_degraded_sandbox_active(enabled);
        self.request_redraw();
    }

    pub fn set_collaboration_mode_indicator(
        &mut self,
        indicator: Option<CollaborationModeIndicator>,
    ) {
        self.composer.set_collaboration_mode_indicator(indicator);
        self.request_redraw();
    }

    pub fn set_personality_command_enabled(&mut self, enabled: bool) {
        self.composer.set_personality_command_enabled(enabled);
        self.request_redraw();
    }

    pub fn status_widget(&self) -> Option<&StatusIndicatorWidget> {
        self.status.as_ref()
    }

    pub fn skills(&self) -> Option<&Vec<SkillMetadata>> {
        self.composer.skills()
    }

    #[cfg(test)]
    pub(crate) fn context_window_percent(&self) -> Option<i64> {
        self.context_window_percent
    }

    #[cfg(test)]
    pub(crate) fn context_window_used_tokens(&self) -> Option<i64> {
        self.context_window_used_tokens
    }

    fn active_view(&self) -> Option<&dyn BottomPaneView> {
        self.view_stack.last().map(std::convert::AsRef::as_ref)
    }

    fn push_view(&mut self, view: Box<dyn BottomPaneView>) {
        self.view_stack.push(view);
        self.request_redraw();
    }

    /// Forward a key event to the active view or the composer.
    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> InputResult {
        // If a modal/view is active, handle it here; otherwise forward to composer.
        if !self.view_stack.is_empty() {
            // We need three pieces of information after routing the key:
            // whether Esc completed the view, whether the view finished for any
            // reason, and whether a paste-burst timer should be scheduled.
            let (ctrl_c_completed, view_complete, view_in_paste_burst) = {
                let last_index = self.view_stack.len() - 1;
                let view = &mut self.view_stack[last_index];
                let prefer_esc =
                    key_event.code == KeyCode::Esc && view.prefer_esc_to_handle_key_event();
                let ctrl_c_completed = key_event.code == KeyCode::Esc
                    && !prefer_esc
                    && matches!(view.on_ctrl_c(), CancellationEvent::Handled)
                    && view.is_complete();
                if ctrl_c_completed {
                    (true, true, false)
                } else {
                    view.handle_key_event(key_event);
                    (false, view.is_complete(), view.is_in_paste_burst())
                }
            };

            if ctrl_c_completed {
                self.view_stack.pop();
                self.on_active_view_complete();
                if let Some(next_view) = self.view_stack.last()
                    && next_view.is_in_paste_burst()
                {
                    self.request_redraw_in(ChatComposer::recommended_paste_flush_delay());
                }
            } else if view_complete {
                self.view_stack.clear();
                self.on_active_view_complete();
            } else if view_in_paste_burst {
                self.request_redraw_in(ChatComposer::recommended_paste_flush_delay());
            }
            self.request_redraw();
            InputResult::None
        } else {
            // If a task is running and a status line is visible, allow Esc to
            // send an interrupt even while the composer has focus.
            // When a popup is active, prefer dismissing it over interrupting the task.
            if key_event.code == KeyCode::Esc
                && self.is_task_running
                && !self.composer.popup_active()
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

    /// Handles a Ctrl+C press within the bottom pane.
    ///
    /// An active modal view is given the first chance to consume the key (typically to dismiss
    /// itself). If no view is active, Ctrl+C clears draft composer input.
    ///
    /// This method may show the quit shortcut hint as a user-visible acknowledgement that Ctrl+C
    /// was received, but it does not decide whether the process should exit; `ChatWidget` owns the
    /// quit/interrupt state machine and uses the result to decide what happens next.
    pub(crate) fn on_ctrl_c(&mut self) -> CancellationEvent {
        if let Some(view) = self.view_stack.last_mut() {
            let event = view.on_ctrl_c();
            if matches!(event, CancellationEvent::Handled) {
                if view.is_complete() {
                    self.view_stack.pop();
                    self.on_active_view_complete();
                }
                self.show_quit_shortcut_hint(key_hint::ctrl(KeyCode::Char('c')));
                self.request_redraw();
            }
            event
        } else if self.composer_is_empty() {
            CancellationEvent::NotHandled
        } else {
            self.view_stack.pop();
            self.clear_composer_for_ctrl_c();
            self.show_quit_shortcut_hint(key_hint::ctrl(KeyCode::Char('c')));
            self.request_redraw();
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
    ///
    /// This is intended for fresh input where mention linkage does not need to
    /// survive; it routes to `ChatComposer::set_text_content`, which resets
    /// mention bindings.
    pub(crate) fn set_composer_text(
        &mut self,
        text: String,
        text_elements: Vec<TextElement>,
        local_image_paths: Vec<PathBuf>,
    ) {
        self.composer
            .set_text_content(text, text_elements, local_image_paths);
        self.composer.move_cursor_to_end();
        self.request_redraw();
    }

    /// Replace the composer text while preserving mention link targets.
    ///
    /// Use this when rehydrating a draft after a local validation/gating
    /// failure (for example unsupported image submit) so previously selected
    /// mention targets remain stable across retry.
    pub(crate) fn set_composer_text_with_mention_bindings(
        &mut self,
        text: String,
        text_elements: Vec<TextElement>,
        local_image_paths: Vec<PathBuf>,
        mention_bindings: Vec<MentionBinding>,
    ) {
        self.composer.set_text_content_with_mention_bindings(
            text,
            text_elements,
            local_image_paths,
            mention_bindings,
        );
        self.request_redraw();
    }

    #[allow(dead_code)]
    pub(crate) fn set_composer_input_enabled(
        &mut self,
        enabled: bool,
        placeholder: Option<String>,
    ) {
        self.composer.set_input_enabled(enabled, placeholder);
        self.request_redraw();
    }

    pub(crate) fn clear_composer_for_ctrl_c(&mut self) {
        self.composer.clear_for_ctrl_c();
        self.request_redraw();
    }

    /// Get the current composer text (for tests and programmatic checks).
    pub(crate) fn composer_text(&self) -> String {
        self.composer.current_text()
    }

    pub(crate) fn composer_text_elements(&self) -> Vec<TextElement> {
        self.composer.text_elements()
    }

    pub(crate) fn composer_local_images(&self) -> Vec<LocalImageAttachment> {
        self.composer.local_images()
    }

    pub(crate) fn composer_mention_bindings(&self) -> Vec<MentionBinding> {
        self.composer.mention_bindings()
    }

    #[cfg(test)]
    pub(crate) fn composer_local_image_paths(&self) -> Vec<PathBuf> {
        self.composer.local_image_paths()
    }

    pub(crate) fn composer_text_with_pending(&self) -> String {
        self.composer.current_text_with_pending()
    }

    pub(crate) fn apply_external_edit(&mut self, text: String) {
        self.composer.apply_external_edit(text);
        self.request_redraw();
    }

    pub(crate) fn set_footer_hint_override(&mut self, items: Option<Vec<(String, String)>>) {
        self.composer.set_footer_hint_override(items);
        self.request_redraw();
    }

    pub(crate) fn set_remote_image_urls(&mut self, urls: Vec<String>) {
        self.composer.set_remote_image_urls(urls);
        self.request_redraw();
    }

    pub(crate) fn remote_image_urls(&self) -> Vec<String> {
        self.composer.remote_image_urls()
    }

    pub(crate) fn take_remote_image_urls(&mut self) -> Vec<String> {
        let urls = self.composer.take_remote_image_urls();
        self.request_redraw();
        urls
    }

    /// Update the status indicator header (defaults to "Working") and details below it.
    ///
    /// Passing `None` clears any existing details. No-ops if the status indicator is not active.
    pub(crate) fn update_status(&mut self, header: String, details: Option<String>) {
        if let Some(status) = self.status.as_mut() {
            status.update_header(header);
            status.update_details(details);
            self.request_redraw();
        }
    }

    /// Show the transient "press again to quit" hint for `key`.
    ///
    /// `ChatWidget` owns the quit shortcut state machine (it decides when quit is
    /// allowed), while the bottom pane owns rendering. We also schedule a redraw
    /// after [`QUIT_SHORTCUT_TIMEOUT`] so the hint disappears even if the user
    /// stops typing and no other events trigger a draw.
    pub(crate) fn show_quit_shortcut_hint(&mut self, key: KeyBinding) {
        if !DOUBLE_PRESS_QUIT_SHORTCUT_ENABLED {
            return;
        }

        self.composer
            .show_quit_shortcut_hint(key, self.has_input_focus);
        let frame_requester = self.frame_requester.clone();
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                tokio::time::sleep(QUIT_SHORTCUT_TIMEOUT).await;
                frame_requester.schedule_frame();
            });
        } else {
            // In tests (and other non-Tokio contexts), fall back to a thread so
            // the hint can still expire without requiring an explicit draw.
            std::thread::spawn(move || {
                std::thread::sleep(QUIT_SHORTCUT_TIMEOUT);
                frame_requester.schedule_frame();
            });
        }
        self.request_redraw();
    }

    /// Clear the "press again to quit" hint immediately.
    pub(crate) fn clear_quit_shortcut_hint(&mut self) {
        self.composer.clear_quit_shortcut_hint(self.has_input_focus);
        self.request_redraw();
    }

    #[cfg(test)]
    pub(crate) fn quit_shortcut_hint_visible(&self) -> bool {
        self.composer.quit_shortcut_hint_visible()
    }

    #[cfg(test)]
    pub(crate) fn status_indicator_visible(&self) -> bool {
        self.status.is_some()
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
        let was_running = self.is_task_running;
        self.is_task_running = running;
        self.composer.set_task_running(running);

        if running {
            if !was_running {
                if self.status.is_none() {
                    self.status = Some(StatusIndicatorWidget::new(
                        self.app_event_tx.clone(),
                        self.frame_requester.clone(),
                        self.animations_enabled,
                    ));
                }
                if let Some(status) = self.status.as_mut() {
                    status.set_interrupt_hint_visible(true);
                }
                self.sync_status_inline_message();
                self.request_redraw();
            }
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

    pub(crate) fn ensure_status_indicator(&mut self) {
        if self.status.is_none() {
            self.status = Some(StatusIndicatorWidget::new(
                self.app_event_tx.clone(),
                self.frame_requester.clone(),
                self.animations_enabled,
            ));
            self.sync_status_inline_message();
            self.request_redraw();
        }
    }

    pub(crate) fn set_interrupt_hint_visible(&mut self, visible: bool) {
        if let Some(status) = self.status.as_mut() {
            status.set_interrupt_hint_visible(visible);
            self.request_redraw();
        }
    }

    pub(crate) fn set_context_window(&mut self, percent: Option<i64>, used_tokens: Option<i64>) {
        if self.context_window_percent == percent && self.context_window_used_tokens == used_tokens
        {
            return;
        }

        self.context_window_percent = percent;
        self.context_window_used_tokens = used_tokens;
        self.composer
            .set_context_window(percent, self.context_window_used_tokens);
        self.request_redraw();
    }

    /// Show a generic list selection view with the provided items.
    pub(crate) fn show_selection_view(&mut self, params: list_selection_view::SelectionViewParams) {
        let view = list_selection_view::ListSelectionView::new(params, self.app_event_tx.clone());
        self.push_view(Box::new(view));
    }

    /// Replace the active selection view when it matches `view_id`.
    pub(crate) fn replace_selection_view_if_active(
        &mut self,
        view_id: &'static str,
        params: list_selection_view::SelectionViewParams,
    ) -> bool {
        let is_match = self
            .view_stack
            .last()
            .is_some_and(|view| view.view_id() == Some(view_id));
        if !is_match {
            return false;
        }

        self.view_stack.pop();
        let view = list_selection_view::ListSelectionView::new(params, self.app_event_tx.clone());
        self.push_view(Box::new(view));
        true
    }

    /// Update the queued messages preview shown above the composer.
    pub(crate) fn set_queued_user_messages(&mut self, queued: Vec<String>) {
        self.queued_user_messages.messages = queued;
        self.request_redraw();
    }

    /// Update the unified-exec process set and refresh whichever summary surface is active.
    ///
    /// The summary may be displayed inline in the status row or as a dedicated
    /// footer row depending on whether a status indicator is currently visible.
    pub(crate) fn set_unified_exec_processes(&mut self, processes: Vec<String>) {
        if self.unified_exec_footer.set_processes(processes) {
            self.sync_status_inline_message();
            self.request_redraw();
        }
    }

    /// Copy unified-exec summary text into the active status row, if any.
    ///
    /// This keeps status-line inline text synchronized without forcing the
    /// standalone unified-exec footer row to be visible.
    fn sync_status_inline_message(&mut self) {
        if let Some(status) = self.status.as_mut() {
            status.update_inline_message(self.unified_exec_footer.summary_text());
        }
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

    /// Return true when no popups or modal views are active, regardless of task state.
    pub(crate) fn can_launch_external_editor(&self) -> bool {
        self.view_stack.is_empty() && !self.composer.popup_active()
    }

    /// Returns true when the bottom pane has no active modal view and no active composer popup.
    ///
    /// This is the UI-level definition of "no modal/popup is active" for key routing decisions.
    /// It intentionally does not include task state, since some actions are safe while a task is
    /// running and some are not.
    pub(crate) fn no_modal_or_popup_active(&self) -> bool {
        self.can_launch_external_editor()
    }

    pub(crate) fn show_view(&mut self, view: Box<dyn BottomPaneView>) {
        self.push_view(view);
    }

    /// Called when the agent requests user approval.
    pub fn push_approval_request(&mut self, request: ApprovalRequest, features: &Features) {
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
        let modal = ApprovalOverlay::new(request, self.app_event_tx.clone(), features.clone());
        self.pause_status_timer_for_modal();
        self.push_view(Box::new(modal));
    }

    /// Called when the agent requests user input.
    pub fn push_user_input_request(&mut self, request: RequestUserInputEvent) {
        let request = if let Some(view) = self.view_stack.last_mut() {
            match view.try_consume_user_input_request(request) {
                Some(request) => request,
                None => {
                    self.request_redraw();
                    return;
                }
            }
        } else {
            request
        };

        let modal = RequestUserInputOverlay::new(
            request,
            self.app_event_tx.clone(),
            self.has_input_focus,
            self.enhanced_keys_supported,
            self.disable_paste_burst,
        );
        self.pause_status_timer_for_modal();
        self.set_composer_input_enabled(
            false,
            Some("Answer the questions to continue.".to_string()),
        );
        self.push_view(Box::new(modal));
    }

    fn on_active_view_complete(&mut self) {
        self.resume_status_timer_after_modal();
        self.set_composer_input_enabled(true, None);
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
        // Give the active view the first chance to flush paste-burst state so
        // overlays that reuse the composer behave consistently.
        if let Some(view) = self.view_stack.last_mut()
            && view.flush_paste_burst_if_due()
        {
            return true;
        }
        self.composer.flush_paste_burst_if_due()
    }

    pub(crate) fn is_in_paste_burst(&self) -> bool {
        // A view can hold paste-burst state independently of the primary
        // composer, so check it first.
        self.view_stack
            .last()
            .is_some_and(|view| view.is_in_paste_burst())
            || self.composer.is_in_paste_burst()
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

    pub(crate) fn attach_image(&mut self, path: PathBuf) {
        if self.view_stack.is_empty() {
            self.composer.attach_image(path);
            self.request_redraw();
        }
    }

    #[cfg(test)]
    pub(crate) fn take_recent_submission_images(&mut self) -> Vec<PathBuf> {
        self.composer.take_recent_submission_images()
    }

    pub(crate) fn take_recent_submission_images_with_placeholders(
        &mut self,
    ) -> Vec<LocalImageAttachment> {
        self.composer
            .take_recent_submission_images_with_placeholders()
    }

    pub(crate) fn prepare_inline_args_submission(
        &mut self,
        record_history: bool,
    ) -> Option<(String, Vec<TextElement>)> {
        self.composer.prepare_inline_args_submission(record_history)
    }

    fn as_renderable(&'_ self) -> RenderableItem<'_> {
        if let Some(view) = self.active_view() {
            RenderableItem::Borrowed(view)
        } else {
            let mut flex = FlexRenderable::new();
            if let Some(status) = &self.status {
                flex.push(0, RenderableItem::Borrowed(status));
            }
            // Avoid double-surfacing the same summary and avoid adding an extra
            // row while the status line is already visible.
            if self.status.is_none() && !self.unified_exec_footer.is_empty() {
                flex.push(0, RenderableItem::Borrowed(&self.unified_exec_footer));
            }
            let has_queued_messages = !self.queued_user_messages.messages.is_empty();
            let has_status_or_footer =
                self.status.is_some() || !self.unified_exec_footer.is_empty();
            if has_queued_messages && has_status_or_footer {
                flex.push(0, RenderableItem::Owned("".into()));
            }
            flex.push(1, RenderableItem::Borrowed(&self.queued_user_messages));
            if !has_queued_messages && has_status_or_footer {
                flex.push(0, RenderableItem::Owned("".into()));
            }
            let mut flex2 = FlexRenderable::new();
            flex2.push(1, RenderableItem::Owned(flex.into()));
            flex2.push(0, RenderableItem::Borrowed(&self.composer));
            RenderableItem::Owned(Box::new(flex2))
        }
    }

    pub(crate) fn set_status_line(&mut self, status_line: Option<Line<'static>>) {
        if self.composer.set_status_line(status_line) {
            self.request_redraw();
        }
    }

    pub(crate) fn set_status_line_enabled(&mut self, enabled: bool) {
        if self.composer.set_status_line_enabled(enabled) {
            self.request_redraw();
        }
    }
}

impl Renderable for BottomPane {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.as_renderable().render(area, buf);
    }
    fn desired_height(&self, width: u16) -> u16 {
        self.as_renderable().desired_height(width)
    }
    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        self.as_renderable().cursor_pos(area)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_event::AppEvent;
    use codex_core::protocol::Op;
    use codex_protocol::protocol::SkillScope;
    use crossterm::event::KeyModifiers;
    use insta::assert_snapshot;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use std::cell::Cell;
    use std::path::PathBuf;
    use std::rc::Rc;
    use tokio::sync::mpsc::unbounded_channel;

    fn snapshot_buffer(buf: &Buffer) -> String {
        let mut lines = Vec::new();
        for y in 0..buf.area().height {
            let mut row = String::new();
            for x in 0..buf.area().width {
                row.push(buf[(x, y)].symbol().chars().next().unwrap_or(' '));
            }
            lines.push(row);
        }
        lines.join("\n")
    }

    fn render_snapshot(pane: &BottomPane, area: Rect) -> String {
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        snapshot_buffer(&buf)
    }

    fn exec_request() -> ApprovalRequest {
        ApprovalRequest::Exec {
            id: "1".to_string(),
            command: vec!["echo".into(), "ok".into()],
            reason: None,
            network_approval_context: None,
            proposed_execpolicy_amendment: None,
            additional_permissions: None,
        }
    }

    #[test]
    fn ctrl_c_on_modal_consumes_without_showing_quit_hint() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let features = Features::with_defaults();
        let mut pane = BottomPane::new(BottomPaneParams {
            app_event_tx: tx,
            frame_requester: FrameRequester::test_dummy(),
            has_input_focus: true,
            enhanced_keys_supported: false,
            placeholder_text: "Ask Codex to do anything".to_string(),
            disable_paste_burst: false,
            animations_enabled: true,
            skills: Some(Vec::new()),
        });
        pane.push_approval_request(exec_request(), &features);
        assert_eq!(CancellationEvent::Handled, pane.on_ctrl_c());
        assert!(!pane.quit_shortcut_hint_visible());
        assert_eq!(CancellationEvent::NotHandled, pane.on_ctrl_c());
    }

    // live ring removed; related tests deleted.

    #[test]
    fn overlay_not_shown_above_approval_modal() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let features = Features::with_defaults();
        let mut pane = BottomPane::new(BottomPaneParams {
            app_event_tx: tx,
            frame_requester: FrameRequester::test_dummy(),
            has_input_focus: true,
            enhanced_keys_supported: false,
            placeholder_text: "Ask Codex to do anything".to_string(),
            disable_paste_burst: false,
            animations_enabled: true,
            skills: Some(Vec::new()),
        });

        // Create an approval modal (active view).
        pane.push_approval_request(exec_request(), &features);

        // Render and verify the top row does not include an overlay.
        let area = Rect::new(0, 0, 60, 6);
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);

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
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let features = Features::with_defaults();
        let mut pane = BottomPane::new(BottomPaneParams {
            app_event_tx: tx,
            frame_requester: FrameRequester::test_dummy(),
            has_input_focus: true,
            enhanced_keys_supported: false,
            placeholder_text: "Ask Codex to do anything".to_string(),
            disable_paste_burst: false,
            animations_enabled: true,
            skills: Some(Vec::new()),
        });

        // Start a running task so the status indicator is active above the composer.
        pane.set_task_running(true);

        // Push an approval modal (e.g., command approval) which should hide the status view.
        pane.push_approval_request(exec_request(), &features);

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
        pane.render(area, &mut buf);
        let mut row0 = String::new();
        for x in 0..area.width {
            row0.push(buf[(x, 0)].symbol().chars().next().unwrap_or(' '));
        }
        assert!(
            row0.contains("Working"),
            "expected Working header after denial on row 0: {row0:?}"
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
            animations_enabled: true,
            skills: Some(Vec::new()),
        });

        // Begin a task: show initial status.
        pane.set_task_running(true);

        // Use a height that allows the status line to be visible above the composer.
        let area = Rect::new(0, 0, 40, 6);
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);

        let bufs = snapshot_buffer(&buf);
        assert!(bufs.contains("• Working"), "expected Working header");
    }

    #[test]
    fn status_and_composer_fill_height_without_bottom_padding() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut pane = BottomPane::new(BottomPaneParams {
            app_event_tx: tx,
            frame_requester: FrameRequester::test_dummy(),
            has_input_focus: true,
            enhanced_keys_supported: false,
            placeholder_text: "Ask Codex to do anything".to_string(),
            disable_paste_burst: false,
            animations_enabled: true,
            skills: Some(Vec::new()),
        });

        // Activate spinner (status view replaces composer) with no live ring.
        pane.set_task_running(true);

        // Use height == desired_height; expect spacer + status + composer rows without trailing padding.
        let height = pane.desired_height(30);
        assert!(
            height >= 3,
            "expected at least 3 rows to render spacer, status, and composer; got {height}"
        );
        let area = Rect::new(0, 0, 30, height);
        assert_snapshot!(
            "status_and_composer_fill_height_without_bottom_padding",
            render_snapshot(&pane, area)
        );
    }

    #[test]
    fn status_only_snapshot() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut pane = BottomPane::new(BottomPaneParams {
            app_event_tx: tx,
            frame_requester: FrameRequester::test_dummy(),
            has_input_focus: true,
            enhanced_keys_supported: false,
            placeholder_text: "Ask Codex to do anything".to_string(),
            disable_paste_burst: false,
            animations_enabled: true,
            skills: Some(Vec::new()),
        });

        pane.set_task_running(true);

        let width = 48;
        let height = pane.desired_height(width);
        let area = Rect::new(0, 0, width, height);
        assert_snapshot!("status_only_snapshot", render_snapshot(&pane, area));
    }

    #[test]
    fn unified_exec_summary_does_not_increase_height_when_status_visible() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut pane = BottomPane::new(BottomPaneParams {
            app_event_tx: tx,
            frame_requester: FrameRequester::test_dummy(),
            has_input_focus: true,
            enhanced_keys_supported: false,
            placeholder_text: "Ask Codex to do anything".to_string(),
            disable_paste_burst: false,
            animations_enabled: true,
            skills: Some(Vec::new()),
        });

        pane.set_task_running(true);
        let width = 120;
        let before = pane.desired_height(width);

        pane.set_unified_exec_processes(vec!["sleep 5".to_string()]);
        let after = pane.desired_height(width);

        assert_eq!(after, before);

        let area = Rect::new(0, 0, width, after);
        let rendered = render_snapshot(&pane, area);
        assert!(rendered.contains("background terminal running · /ps to view"));
    }

    #[test]
    fn status_with_details_and_queued_messages_snapshot() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut pane = BottomPane::new(BottomPaneParams {
            app_event_tx: tx,
            frame_requester: FrameRequester::test_dummy(),
            has_input_focus: true,
            enhanced_keys_supported: false,
            placeholder_text: "Ask Codex to do anything".to_string(),
            disable_paste_burst: false,
            animations_enabled: true,
            skills: Some(Vec::new()),
        });

        pane.set_task_running(true);
        pane.update_status(
            "Working".to_string(),
            Some("First detail line\nSecond detail line".to_string()),
        );
        pane.set_queued_user_messages(vec!["Queued follow-up question".to_string()]);

        let width = 48;
        let height = pane.desired_height(width);
        let area = Rect::new(0, 0, width, height);
        assert_snapshot!(
            "status_with_details_and_queued_messages_snapshot",
            render_snapshot(&pane, area)
        );
    }

    #[test]
    fn queued_messages_visible_when_status_hidden_snapshot() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut pane = BottomPane::new(BottomPaneParams {
            app_event_tx: tx,
            frame_requester: FrameRequester::test_dummy(),
            has_input_focus: true,
            enhanced_keys_supported: false,
            placeholder_text: "Ask Codex to do anything".to_string(),
            disable_paste_burst: false,
            animations_enabled: true,
            skills: Some(Vec::new()),
        });

        pane.set_task_running(true);
        pane.set_queued_user_messages(vec!["Queued follow-up question".to_string()]);
        pane.hide_status_indicator();

        let width = 48;
        let height = pane.desired_height(width);
        let area = Rect::new(0, 0, width, height);
        assert_snapshot!(
            "queued_messages_visible_when_status_hidden_snapshot",
            render_snapshot(&pane, area)
        );
    }

    #[test]
    fn status_and_queued_messages_snapshot() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut pane = BottomPane::new(BottomPaneParams {
            app_event_tx: tx,
            frame_requester: FrameRequester::test_dummy(),
            has_input_focus: true,
            enhanced_keys_supported: false,
            placeholder_text: "Ask Codex to do anything".to_string(),
            disable_paste_burst: false,
            animations_enabled: true,
            skills: Some(Vec::new()),
        });

        pane.set_task_running(true);
        pane.set_queued_user_messages(vec!["Queued follow-up question".to_string()]);

        let width = 48;
        let height = pane.desired_height(width);
        let area = Rect::new(0, 0, width, height);
        assert_snapshot!(
            "status_and_queued_messages_snapshot",
            render_snapshot(&pane, area)
        );
    }

    #[test]
    fn remote_images_render_above_composer_text() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut pane = BottomPane::new(BottomPaneParams {
            app_event_tx: tx,
            frame_requester: FrameRequester::test_dummy(),
            has_input_focus: true,
            enhanced_keys_supported: false,
            placeholder_text: "Ask Codex to do anything".to_string(),
            disable_paste_burst: false,
            animations_enabled: true,
            skills: Some(Vec::new()),
        });

        pane.set_remote_image_urls(vec![
            "https://example.com/one.png".to_string(),
            "data:image/png;base64,aGVsbG8=".to_string(),
        ]);

        assert_eq!(pane.composer_text(), "");
        let width = 48;
        let height = pane.desired_height(width);
        let area = Rect::new(0, 0, width, height);
        let snapshot = render_snapshot(&pane, area);
        assert!(snapshot.contains("[Image #1]"));
        assert!(snapshot.contains("[Image #2]"));
    }

    #[test]
    fn drain_pending_submission_state_clears_remote_image_urls() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut pane = BottomPane::new(BottomPaneParams {
            app_event_tx: tx,
            frame_requester: FrameRequester::test_dummy(),
            has_input_focus: true,
            enhanced_keys_supported: false,
            placeholder_text: "Ask Codex to do anything".to_string(),
            disable_paste_burst: false,
            animations_enabled: true,
            skills: Some(Vec::new()),
        });

        pane.set_remote_image_urls(vec!["https://example.com/one.png".to_string()]);
        assert_eq!(pane.remote_image_urls().len(), 1);

        pane.drain_pending_submission_state();

        assert!(pane.remote_image_urls().is_empty());
    }

    #[test]
    fn esc_with_skill_popup_does_not_interrupt_task() {
        let (tx_raw, mut rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut pane = BottomPane::new(BottomPaneParams {
            app_event_tx: tx,
            frame_requester: FrameRequester::test_dummy(),
            has_input_focus: true,
            enhanced_keys_supported: false,
            placeholder_text: "Ask Codex to do anything".to_string(),
            disable_paste_burst: false,
            animations_enabled: true,
            skills: Some(vec![SkillMetadata {
                name: "test-skill".to_string(),
                description: "test skill".to_string(),
                short_description: None,
                interface: None,
                dependencies: None,
                policy: None,
                permissions: None,
                path: PathBuf::from("test-skill"),
                scope: SkillScope::User,
            }]),
        });

        pane.set_task_running(true);

        // Repro: a running task + skill popup + Esc should dismiss the popup, not interrupt.
        pane.insert_str("$");
        assert!(
            pane.composer.popup_active(),
            "expected skill popup after typing `$`"
        );

        pane.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        while let Ok(ev) = rx.try_recv() {
            assert!(
                !matches!(ev, AppEvent::CodexOp(Op::Interrupt)),
                "expected Esc to not send Op::Interrupt when dismissing skill popup"
            );
        }
        assert!(
            !pane.composer.popup_active(),
            "expected Esc to dismiss skill popup"
        );
    }

    #[test]
    fn esc_with_slash_command_popup_does_not_interrupt_task() {
        let (tx_raw, mut rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut pane = BottomPane::new(BottomPaneParams {
            app_event_tx: tx,
            frame_requester: FrameRequester::test_dummy(),
            has_input_focus: true,
            enhanced_keys_supported: false,
            placeholder_text: "Ask Codex to do anything".to_string(),
            disable_paste_burst: false,
            animations_enabled: true,
            skills: Some(Vec::new()),
        });

        pane.set_task_running(true);

        // Repro: a running task + slash-command popup + Esc should not interrupt the task.
        pane.insert_str("/");
        assert!(
            pane.composer.popup_active(),
            "expected command popup after typing `/`"
        );

        pane.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        while let Ok(ev) = rx.try_recv() {
            assert!(
                !matches!(ev, AppEvent::CodexOp(Op::Interrupt)),
                "expected Esc to not send Op::Interrupt while command popup is active"
            );
        }
        assert_eq!(pane.composer_text(), "/");
    }

    #[test]
    fn esc_interrupts_running_task_when_no_popup() {
        let (tx_raw, mut rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut pane = BottomPane::new(BottomPaneParams {
            app_event_tx: tx,
            frame_requester: FrameRequester::test_dummy(),
            has_input_focus: true,
            enhanced_keys_supported: false,
            placeholder_text: "Ask Codex to do anything".to_string(),
            disable_paste_burst: false,
            animations_enabled: true,
            skills: Some(Vec::new()),
        });

        pane.set_task_running(true);

        pane.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        assert!(
            matches!(rx.try_recv(), Ok(AppEvent::CodexOp(Op::Interrupt))),
            "expected Esc to send Op::Interrupt while a task is running"
        );
    }

    #[test]
    fn esc_routes_to_handle_key_event_when_requested() {
        #[derive(Default)]
        struct EscRoutingView {
            on_ctrl_c_calls: Rc<Cell<usize>>,
            handle_calls: Rc<Cell<usize>>,
        }

        impl Renderable for EscRoutingView {
            fn render(&self, _area: Rect, _buf: &mut Buffer) {}

            fn desired_height(&self, _width: u16) -> u16 {
                0
            }
        }

        impl BottomPaneView for EscRoutingView {
            fn handle_key_event(&mut self, _key_event: KeyEvent) {
                self.handle_calls
                    .set(self.handle_calls.get().saturating_add(1));
            }

            fn on_ctrl_c(&mut self) -> CancellationEvent {
                self.on_ctrl_c_calls
                    .set(self.on_ctrl_c_calls.get().saturating_add(1));
                CancellationEvent::Handled
            }

            fn prefer_esc_to_handle_key_event(&self) -> bool {
                true
            }
        }

        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut pane = BottomPane::new(BottomPaneParams {
            app_event_tx: tx,
            frame_requester: FrameRequester::test_dummy(),
            has_input_focus: true,
            enhanced_keys_supported: false,
            placeholder_text: "Ask Codex to do anything".to_string(),
            disable_paste_burst: false,
            animations_enabled: true,
            skills: Some(Vec::new()),
        });

        let on_ctrl_c_calls = Rc::new(Cell::new(0));
        let handle_calls = Rc::new(Cell::new(0));
        pane.push_view(Box::new(EscRoutingView {
            on_ctrl_c_calls: Rc::clone(&on_ctrl_c_calls),
            handle_calls: Rc::clone(&handle_calls),
        }));

        pane.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        assert_eq!(on_ctrl_c_calls.get(), 0);
        assert_eq!(handle_calls.get(), 1);
    }
}
