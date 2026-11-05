//! Composer and side-conversation focus shortcuts. Thread selection replaces the widget
//! immediately, but Park and Finish transfer transcript views through the app event queue.

use super::*;
use crate::activation_key_guard::ActivationKeySuppression;
use crate::transcript_view::TranscriptView;

impl App {
    pub(super) async fn handle_side_focus_key(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        key: KeyEvent,
    ) -> bool {
        let bindings = &self.keymap.app;
        let left = bindings.focus_pane_left.is_pressed(key);
        let right = bindings.focus_pane_right.is_pressed(key);
        let main =
            bindings.focus_main_composer.is_pressed(key) && self.main_composer_shortcut_available();
        let sidebar = bindings.focus_sidebar_content.is_pressed(key);
        let hide = bindings.hide_pane.is_pressed(key);
        let width_change = if bindings.move_divider_left.is_pressed(key) {
            2
        } else if bindings.move_divider_right.is_pressed(key) {
            -2
        } else {
            0
        };
        let focused_percent = [
            (&bindings.equalize_pane_widths, 50),
            (&bindings.minimize_pane_width, 0),
            (&bindings.maximize_pane_width, 100),
        ]
        .into_iter()
        .find_map(|(binding, percent)| binding.is_pressed(key).then_some(percent))
        .filter(|_| self.open_pane_focus().is_some());
        let zoom_target = if bindings.toggle_pane_zoom.is_pressed(key) {
            self.open_pane_focus()
        } else {
            None
        };
        if width_change != 0 && !self.can_nudge_side_width(tui) {
            return false;
        }
        let resize = bindings.resize_panes.is_pressed(key) && self.open_pane_focus().is_some();
        let resizing = resize || width_change != 0 || focused_percent.is_some();
        if !(left || right || main || sidebar || hide || zoom_target.is_some() || resizing)
            || !self.conversation_focus_available(tui)
        {
            return false;
        }
        let child = self.switchable_side_child();
        if (sidebar || hide) && child.is_none() {
            return false;
        }
        if key.kind != KeyEventKind::Press {
            // A matching repeat still belongs to the shortcut. Keep it out of the focused editor.
            return true;
        }
        let activation = self
            .routed_key_activation
            .take()
            .unwrap_or_else(|| KeyBinding::from_event(key));
        let previous_thread = self.current_displayed_thread_id();
        let on_child = child.is_some_and(|id| previous_thread == Some(id));
        let leaves_transcript = main
            && !on_child
            && (self.transcript_view.has_active_interaction()
                || self.backtrack.overlay_preview_active);
        let result = if resize {
            self.begin_pane_resize(tui)
        } else if let Some(percent) = focused_percent {
            self.set_focused_pane_width(tui, percent).await
        } else if width_change != 0 {
            self.nudge_side_width(tui, width_change).await
        } else if let Some(target) = zoom_target {
            self.zoomed_thread = (self.zoomed_thread != Some(target)).then_some(target);
            tui.frame_requester().schedule_frame();
            Ok(())
        } else if main {
            self.focus_main_composer(tui, app_server).await
        } else if (left && on_child)
            || (right && !on_child && child.is_some_and(|id| self.side_threads[&id].pane_open))
        {
            // Switching threads parks and restores each pane's transcript interaction.
            self.toggle_side_conversation(tui, app_server).await
        } else if sidebar
            && !on_child
            && let Some(child) = child
        {
            let result = self.select_agent_thread(tui, app_server, child).await;
            if result.is_ok() && self.current_displayed_thread_id() == Some(child) {
                // The outgoing parent's view is still here until Park runs.
                self.focus_current_parent_view(tui);
            }
            result
        } else if hide || sidebar {
            self.hide_focused_side_pane(tui, app_server).await
        } else {
            // A direction with no reachable neighbor leaves the current interaction intact.
            Ok(())
        };
        if let Err(err) = result {
            self.chat_widget.add_error_message(err.to_string());
        }
        let view_changed =
            leaves_transcript || self.current_displayed_thread_id() != previous_thread;
        let suppression = if view_changed {
            ActivationKeySuppression::ViewTransition
        } else {
            // A no-op leaves the editor in place, so prefer accepting fresh input.
            ActivationKeySuppression::ReportedRepeats
        };
        self.chat_widget.suppress_current_view_key_until_release(
            activation,
            Instant::now(),
            suppression,
        );
        true
    }

    async fn focus_main_composer(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
    ) -> Result<()> {
        if let Some(parent) = self.active_side_parent_thread_id() {
            self.select_agent_thread(tui, app_server, parent).await?;
            if self.current_displayed_thread_id() == Some(parent) {
                self.focus_returned_parent_composer(parent).await?;
            }
        } else {
            self.focus_current_parent_view(tui);
        }
        Ok(())
    }

    fn focus_current_parent_view(&mut self, tui: &mut tui::Tui) {
        self.cancel_transcript_browsing(tui);
        clear_transcript_interaction(&mut self.transcript_view, &self.transcript_cells);
        self.cancel_pending_key_chord();
    }

    pub(super) async fn focus_returned_parent_composer(&mut self, parent: ThreadId) -> Result<()> {
        // Finish will restore this view. The live transcript still belongs to the child.
        if let Some(companion) = self.side_companions.get_mut(&parent) {
            companion.view.set_highlight(/*index*/ None);
            clear_transcript_interaction(&mut companion.view, &companion.cells);
        }
        self.cancel_pending_key_chord();
        self.surface_pending_inactive_thread_interactive_requests()
            .await
    }
}

fn clear_transcript_interaction(view: &mut TranscriptView, cells: &[Arc<dyn HistoryCell>]) {
    view.end_select_mode();
    view.end_selection(cells);
    view.cancel_search();
    view.clear_activity_focus();
}
