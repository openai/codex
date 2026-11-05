//! Composer and side-conversation focus shortcuts. Thread selection replaces the widget
//! immediately, but Park and Finish transfer transcript views through the app event queue.

use super::sidebar::PaneFocus;
use super::sidebar::SidebarContent;
use super::*;
use crate::activation_key_guard::ActivationKeySuppression;
use crate::transcript_view::TranscriptView;

impl App {
    pub(super) fn sidebar_chooser_available(&self) -> bool {
        !self.reconnect.offline
            && !self.conversation_frame.has_caller_inset()
            && !self.chat_widget.has_active_modal()
            && self.chat_widget.sidebar_chooser_available()
    }

    async fn open_sidebar_tabs(
        &mut self,
        tui: &mut tui::Tui,
        server: &mut AppServerSession,
    ) -> Result<()> {
        if let Some(parent) = self.active_side_parent_thread_id() {
            self.select_agent_thread(tui, server, parent).await?;
            if self.current_displayed_thread_id() != Some(parent) {
                return Ok(());
            }
            self.focus_returned_parent_composer(parent).await?;
        }
        self.zoomed_thread = None;
        self.sidebar_mut()
            .show(SidebarContent::Chooser, PaneFocus::Sidebar);
        tui.frame_requester().schedule_frame();
        Ok(())
    }

    pub(super) async fn handle_sidebar_input(
        &mut self,
        tui: &mut tui::Tui,
        server: &mut AppServerSession,
        event: &TuiEvent,
    ) -> bool {
        if !self.conversation_focus_available(tui) || !self.sidebar_chooser_available() {
            return false;
        }
        if let TuiEvent::Mouse(mouse) = event
            && mouse.kind == MouseEventKind::Down(MouseButton::Left)
            && let Some(strip) = self.conversation_frame.chat_strip
            && strip.contains((mouse.column, mouse.row).into())
        {
            let point = (mouse.column, mouse.row).into();
            let (tab, add) = crate::sidebar_chooser::strip_targets(strip);
            let result = if add.contains(point) {
                self.open_sidebar_tabs(tui, server).await
            } else if tab.contains(point)
                && let Some(id) = self.switchable_side_child()
                && self.current_displayed_thread_id() != Some(id)
            {
                self.select_agent_thread(tui, server, id).await
            } else {
                return true;
            };
            if let Err(err) = result {
                self.chat_widget.add_error_message(err.to_string());
            }
            return true;
        }
        if self.chooser_focus().is_none() {
            return false;
        }
        let child = self.switchable_side_child();
        let (open, activation) = match event {
            TuiEvent::Mouse(mouse) if mouse.kind == MouseEventKind::Down(MouseButton::Left) => {
                tui.frame_requester().schedule_frame();
                let Some(area) = self
                    .conversation_frame
                    .chooser_area()
                    .filter(|area| area.contains((mouse.column, mouse.row).into()))
                else {
                    self.sidebar_mut()
                        .show(SidebarContent::Chooser, PaneFocus::Main);
                    return false;
                };
                let pane = self.sidebar_mut();
                pane.show(SidebarContent::Chooser, PaneFocus::Sidebar);
                if pane.creation.is_some() {
                    return true;
                }
                let open = if crate::sidebar_chooser::choice_contains(area, mouse.row)
                    || (child.is_some()
                        && crate::sidebar_chooser::strip_targets(area)
                            .0
                            .contains((mouse.column, mouse.row).into()))
                {
                    true
                } else if mouse.row == area.bottom() - 1 && mouse.column > area.x {
                    let column = mouse.column - area.x - 1;
                    let hints = crate::sidebar_chooser::footer(&self.keymap, area);
                    match hints.items.iter().position(|range| range.contains(&column)) {
                        Some(0) => true,            // Open.
                        Some(1) => child.is_some(), // Back.
                        Some(3) => false,           // Hide.
                        _ => return true,
                    }
                } else {
                    return true;
                };
                (open, None)
            }
            _ if self.chooser_focus() != Some(PaneFocus::Sidebar) => return false,
            TuiEvent::Key(key) if key.kind == KeyEventKind::Press && key.modifiers.is_empty() => {
                let open = match key.code {
                    KeyCode::Enter => true,
                    KeyCode::Esc => child.is_some(),
                    KeyCode::Left | KeyCode::Right if child.is_some() => true,
                    _ => return true,
                };
                tui.frame_requester().schedule_frame();
                (open, Some(KeyBinding::from_event(*key)))
            }
            _ => return matches!(event, TuiEvent::Key(_) | TuiEvent::Paste(_)),
        };
        let pane = self.sidebar_mut();
        if let Some(creation) = &pane.creation {
            match (&creation.state, open) {
                (sidebar::CreationState::Waiting { .. }, true) => {}
                (sidebar::CreationState::Waiting { .. }, false) => pane.hide(),
                (sidebar::CreationState::Failed { cleanup, .. }, _) => {
                    if let Some(id) = *cleanup
                        && !self.discard_side_thread(server, id).await
                    {
                        return true;
                    }
                    let message = self
                        .sidebar_mut()
                        .creation
                        .take()
                        .and_then(|creation| creation.message);
                    if open && let Some(parent_thread_id) = self.current_displayed_thread_id() {
                        self.app_event_tx.send(AppEvent::StartSide {
                            parent_thread_id,
                            user_message: message,
                        });
                    } else {
                        self.restore_side_user_message(message);
                    }
                }
            }
        } else {
            if open && let Some(id) = child {
                if let Err(err) = self.select_agent_thread(tui, server, id).await {
                    self.chat_widget.add_error_message(err.to_string());
                }
            } else if open {
                self.chat_widget
                    .dispatch_command(crate::slash_command::SlashCommand::Side);
            }
            if !open || self.chat_widget.thread_id().is_none() {
                self.sidebar_mut().hide();
            }
        }
        // Selecting a chat replaces its widget. Guard the composer that will receive repeats.
        if let Some(activation) = activation {
            self.chat_widget.suppress_current_view_key_until_release(
                activation,
                Instant::now(),
                ActivationKeySuppression::ViewTransition,
            );
        }
        true
    }

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
        let tabs = bindings.focus_sidebar_tabs.is_pressed(key) && self.sidebar_chooser_available();
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
        if !(tabs || left || right || main || sidebar || hide || zoom_target.is_some() || resizing)
            || !self.conversation_focus_available(tui)
        {
            return false;
        }
        let child = self.switchable_side_child();
        let chooser = self.chooser_focus();
        if child.is_none()
            && ((hide && chooser != Some(PaneFocus::Sidebar))
                || (sidebar && !self.sidebar_chooser_available()))
        {
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
        let result = if tabs {
            self.open_sidebar_tabs(tui, app_server).await
        } else if resize {
            self.begin_pane_resize(tui)
        } else if let Some(percent) = focused_percent {
            self.set_focused_pane_width(tui, percent).await
        } else if width_change != 0 {
            self.nudge_side_width(tui, width_change).await
        } else if let Some(target) = zoom_target {
            self.zoomed_thread = (self.zoomed_thread != Some(target)).then_some(target);
            tui.frame_requester().schedule_frame();
            Ok(())
        } else if chooser.is_some() {
            if hide || (sidebar && chooser == Some(PaneFocus::Sidebar)) {
                self.sidebar_mut().hide();
            } else {
                if main {
                    self.focus_current_parent_view(tui);
                }
                let focus = if main || left {
                    PaneFocus::Main
                } else {
                    PaneFocus::Sidebar
                };
                self.sidebar_mut().show(SidebarContent::Chooser, focus);
            }
            tui.frame_requester().schedule_frame();
            Ok(())
        } else if main {
            self.focus_main_composer(tui, app_server).await
        } else if (left && on_child)
            || (right && !on_child && child.is_some() && self.open_pane_focus().is_some())
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
        } else if sidebar && child.is_none() {
            self.focus_current_parent_view(tui);
            self.sidebar_mut()
                .show(SidebarContent::Chooser, PaneFocus::Sidebar);
            tui.frame_requester().schedule_frame();
            Ok(())
        } else if hide || sidebar {
            self.hide_focused_side_pane(tui, app_server).await
        } else {
            // A direction with no reachable neighbor leaves the current interaction intact.
            Ok(())
        };
        if let Err(err) = result {
            self.chat_widget.add_error_message(err.to_string());
        }
        let view_changed = leaves_transcript
            || self.current_displayed_thread_id() != previous_thread
            || (main && chooser == Some(PaneFocus::Sidebar));
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
