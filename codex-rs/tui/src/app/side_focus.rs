//! Composer and side-conversation focus shortcuts. Thread selection replaces the widget
//! immediately, but Park and Finish transfer transcript views through the app event queue.

use super::*;
use crate::transcript_view::TranscriptView;

impl App {
    pub(super) async fn handle_side_focus_key(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        key: KeyEvent,
    ) -> bool {
        let bindings = &self.keymap.app;
        let main = bindings.focus_main_composer.is_pressed(key);
        let sidebar = bindings.focus_sidebar_content.is_pressed(key);
        if !(main || sidebar || bindings.hide_pane.is_pressed(key))
            || !self.main_composer_shortcut_available(tui)
            || (!main && !self.side_pane_switch_available())
        {
            return false;
        }
        if key.kind != KeyEventKind::Press {
            return true;
        }
        let activation = self
            .routed_key_activation
            .take()
            .unwrap_or_else(|| KeyBinding::from_event(key));
        let result = if main {
            self.focus_main_composer(tui, app_server).await
        } else if sidebar
            && let Some(parent) = self.current_displayed_thread_id()
            && let Some(child) = self.side_pair_key(parent)
            && child != parent
        {
            let result = self.toggle_side_conversation(tui, app_server).await;
            if result.is_ok() && self.current_displayed_thread_id() == Some(child) {
                // The outgoing parent's view is still here until Park runs.
                self.focus_current_parent_view(tui);
            }
            result
        } else {
            self.hide_focused_side_pane(tui, app_server).await
        };
        if let Err(err) = result {
            self.chat_widget.add_error_message(err.to_string());
        }
        self.chat_widget
            .suppress_current_view_key_until_release(activation, Instant::now());
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
