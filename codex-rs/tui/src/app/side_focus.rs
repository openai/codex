//! Composer and side-conversation focus shortcuts. Thread selection replaces the widget
//! immediately, but Park and Finish transfer transcript views through the app event queue.

use super::sidebar::PaneFocus;
use super::sidebar::SidebarContent;
use super::sidebar_close::Action::CloseTab;
use super::*;
use crate::activation_key_guard::ActivationKeySuppression;
use crate::sidebar_chooser::Discovery;
use crate::sidebar_tab_strip::SidebarTab;
use crate::transcript_view::TranscriptView;

pub(super) use super::sidebar_input::SidebarPress;
pub(super) use super::sidebar_input::SidebarTarget;

impl App {
    pub(super) fn hovered_sidebar_target(
        &self,
        point: Option<ratatui::layout::Position>,
    ) -> Option<SidebarTarget> {
        if self.pane_resize.is_some() {
            return None;
        }
        let point = point?;
        self.conversation_frame
            .sidebar_targets
            .iter()
            .find_map(|(target, area)| area.contains(point).then_some(*target))
    }

    pub(super) fn sidebar_chooser_available(&self) -> bool {
        !self.reconnect.offline
            && !self.conversation_frame.has_caller_inset()
            && !self.chat_widget.has_active_modal()
            && self.chat_widget.sidebar_chooser_available()
    }

    pub(super) async fn open_sidebar_tabs(
        &mut self,
        tui: &mut tui::Tui,
        server: &mut AppServerSession,
    ) -> Result<()> {
        if let Some(parent) = self
            .sidebar_parent()
            .filter(|parent| Some(*parent) != self.current_displayed_thread_id())
        {
            self.select_agent_thread(tui, server, parent).await?;
            if self.current_displayed_thread_id() != Some(parent) {
                return Ok(());
            }
            self.focus_returned_parent_composer(parent).await?;
        }
        self.zoomed_thread = None;
        let parent = self.current_displayed_thread_id();
        let sidebar_parent = self.sidebar_parent();
        let pane = self.sidebars.entry(sidebar_parent).or_default();
        pane.closing_chats.retain(|_, task| !task.is_finished());
        pane.show(SidebarContent::Chooser, PaneFocus::Sidebar);
        if pane.creation.is_none()
            && let Some(parent) = parent
        {
            if self.primary_thread_id == Some(parent) {
                pane.sync_agent_rows(&self.agent_navigation);
                if pane.agents.rows.iter().any(|agent| !agent.selectable) {
                    self.refresh_agent_picker_threads(server, parent);
                }
            }
            self.refresh_saved_chats(server, parent);
            self.refresh_sidebar_files(server, parent).await;
        }
        tui.frame_requester().schedule_frame();
        Ok(())
    }

    pub(super) fn refresh_saved_chats(&mut self, server: &AppServerSession, parent: ThreadId) {
        let pane = self.sidebar_mut();
        if pane.discovery.request().is_some() || !server.can_list_saved_side_chats(parent) {
            return;
        }
        let request = uuid::Uuid::new_v4();
        pane.set_discovery(Discovery::Loading {
            request,
            started: Instant::now(),
            retry: matches!(pane.discovery, Discovery::Failed(_)),
        });
        let codex_home = self.config.codex_home.clone();
        let mut view = server.request_view();
        let tx = self.app_event_tx.clone();
        tokio::spawn(async move {
            let result = view
                .list_saved_side_chats(parent, codex_home.as_path())
                .await
                .map(crate::app_event::SavedSideChatData::Choices)
                .map_err(|err| err.to_string());
            tx.send(AppEvent::SavedSideChatsLoaded {
                backend_id: view.backend_id(),
                parent,
                request,
                result,
            });
        });
    }

    pub(super) async fn activate_sidebar_tab(
        &mut self,
        tui: &mut tui::Tui,
        server: &mut AppServerSession,
        tab: SidebarTab,
    ) -> Result<()> {
        if tab == SidebarTab::Subagents {
            if let Some(agent) = self.sidebar().and_then(|pane| pane.agents.detail) {
                return Box::pin(self.open_sidebar_agent(tui, server, agent)).await;
            }
            return Ok(());
        }
        if let SidebarTab::Document(id) = tab {
            if let Some(parent) = self
                .sidebar_parent()
                .filter(|parent| Some(*parent) != self.current_displayed_thread_id())
            {
                self.select_agent_thread(tui, server, parent).await?;
                if self.current_displayed_thread_id() != Some(parent) {
                    return Ok(());
                }
                self.focus_returned_parent_composer(parent).await?;
            }
            if !self
                .sidebar()
                .and_then(|pane| pane.documents.get(&id))
                .is_some_and(|doc| doc.backend == server.backend_id())
            {
                return Ok(());
            }
            self.zoomed_thread = None;
            self.sidebar_mut()
                .show(SidebarContent::Document(id), PaneFocus::Sidebar);
            tui.frame_requester().schedule_frame();
            return Ok(());
        }
        let SidebarTab::Chat(id) = tab else {
            unreachable!()
        };
        let current = self.current_displayed_thread_id();
        if current == Some(id) {
            return Ok(());
        }
        let zoomed = current.is_some() && self.zoomed_thread == current;
        let result = self.select_agent_thread(tui, server, id).await;
        if zoomed && self.current_displayed_thread_id() == Some(id) {
            self.zoomed_thread = Some(id);
        }
        result
    }

    pub(super) async fn close_sidebar_tab(
        &mut self,
        tui: &mut tui::Tui,
        server: &mut AppServerSession,
        tab: SidebarTab,
    ) -> Result<()> {
        match tab {
            SidebarTab::Chat(id) => sidebar_close::request(self, tui, server, id, CloseTab).await,
            SidebarTab::Document(_) | SidebarTab::Subagents => {
                if tab == SidebarTab::Subagents
                    && let Some((parent, agent)) = self.docked_agent()
                    && self.current_displayed_thread_id() == Some(agent)
                {
                    self.select_agent_thread(tui, server, parent).await?;
                    if self.current_displayed_thread_id() != Some(parent) {
                        return Ok(());
                    }
                    self.focus_returned_parent_composer(parent).await?;
                }
                if tab == SidebarTab::Subagents
                    && let Some(agent) = self.sidebar().and_then(|pane| pane.agents.detail)
                {
                    self.side_companions.remove(&agent);
                }
                let pane = self.sidebar_mut();
                let active = match tab {
                    SidebarTab::Document(id) => {
                        let active = pane.visible_content() == Some(SidebarContent::Document(id));
                        pane.remove_document(id);
                        active
                    }
                    SidebarTab::Subagents => pane.close_agents(),
                    SidebarTab::Chat(_) => unreachable!(),
                };
                if active {
                    if let Some(previous) = pane.recent_tab(/*excluding*/ None) {
                        self.zoomed_thread = None;
                        self.sidebar_mut()
                            .show(SidebarContent::Chooser, PaneFocus::Sidebar);
                        self.activate_sidebar_tab(tui, server, previous).await?;
                    } else {
                        self.open_sidebar_tabs(tui, server).await?;
                    }
                }
                tui.frame_requester().schedule_frame();
                Ok(())
            }
        }
    }

    pub(super) async fn step_sidebar_tab(
        &mut self,
        tui: &mut tui::Tui,
        server: &mut AppServerSession,
        step: isize,
    ) -> Result<()> {
        if let Some(id) = self.sidebar().and_then(|pane| pane.adjacent_tab(step)) {
            self.activate_sidebar_tab(tui, server, id).await?;
        }
        Ok(())
    }

    pub(super) async fn handle_side_focus_key(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        key: KeyEvent,
    ) -> bool {
        let bindings = &self.keymap.app;
        let focus_center = bindings.open_agents.is_pressed(key)
            && self.local_settings.workspace_enabled(&self.config);
        let left_from_main = bindings.focus_pane_left.is_pressed(key)
            && self.agents_overview.workspace.is_open()
            && !self.sidebar_has_focus();
        if (focus_center || left_from_main) && self.conversation_focus_available(tui) {
            if key.kind == KeyEventKind::Press {
                if focus_center {
                    self.open_agents_overview(app_server);
                } else {
                    self.agents_overview.workspace.focus_center();
                }
                let activation = self
                    .routed_key_activation
                    .take()
                    .unwrap_or_else(|| KeyBinding::from_event(key));
                self.chat_widget.suppress_current_view_key_until_release(
                    activation,
                    Instant::now(),
                    ActivationKeySuppression::ViewTransition,
                );
                tui.frame_requester().schedule_frame();
            }
            return true;
        }
        let panel_focus = self.panel_focus();
        let document = if panel_focus == Some(PaneFocus::Sidebar) {
            self.sidebar()
                .and_then(sidebar::SidebarPane::visible_content)
                .and_then(|content| match content {
                    SidebarContent::Document(id) => Some(id),
                    _ => None,
                })
        } else {
            None
        };
        let document_shortcut = document.is_some_and(|id| {
            self.sidebar()
                .and_then(|pane| pane.documents.get(&id))
                .is_some_and(|doc| doc.allows_tab_shortcut(key, &self.keymap))
        });
        let close = if let Some(id) = document {
            (bindings.close_sidebar_tab.is_pressed(key)
                || (document_shortcut && crate::key_hint::plain(KeyCode::Esc).is_press(key)))
            .then_some(SidebarTab::Document(id))
        } else if bindings.close_sidebar_tab.is_pressed(key) {
            self.current_displayed_thread_id().and_then(|id| {
                if self.docked_agent().is_some_and(|(_, agent)| id == agent) {
                    Some(SidebarTab::Subagents)
                } else {
                    self.side_threads
                        .contains_key(&id)
                        .then_some(SidebarTab::Chat(id))
                }
            })
        } else {
            None
        };
        let left = bindings.focus_pane_left.is_pressed(key);
        let tab_step = if bindings.sidebar_up.is_pressed(key)
            || (document_shortcut && crate::key_hint::plain(KeyCode::Left).is_press(key))
        {
            Some(-1)
        } else if bindings.sidebar_down.is_pressed(key)
            || (document_shortcut && crate::key_hint::plain(KeyCode::Right).is_press(key))
        {
            Some(1)
        } else {
            None
        }
        .filter(|_| self.sidebar_has_focus() && self.sidebar_chooser_available());
        let right = bindings.focus_pane_right.is_pressed(key);
        let main =
            bindings.focus_main_composer.is_pressed(key) && self.main_composer_shortcut_available();
        let sidebar = bindings.focus_sidebar_content.is_pressed(key);
        let tabs = bindings.focus_sidebar_tabs.is_pressed(key) && self.sidebar_chooser_available();
        let hide = bindings.hide_pane.is_pressed(key)
            || (self.chooser_focus() == Some(PaneFocus::Sidebar)
                && crate::key_hint::ctrl(KeyCode::Char('c')).is_press(key));
        let width_change = if bindings.move_divider_left.is_pressed(key) {
            2
        } else if bindings.move_divider_right.is_pressed(key) {
            -2
        } else {
            0
        };
        let can_resize = self.can_resize_sidebar(tui);
        let focused_percent = [
            (&bindings.equalize_pane_widths, 50),
            (&bindings.minimize_pane_width, 0),
            (&bindings.maximize_pane_width, 100),
        ]
        .into_iter()
        .find_map(|(binding, percent)| binding.is_pressed(key).then_some(percent))
        .filter(|_| can_resize);
        let zoom_target = if bindings.toggle_pane_zoom.is_pressed(key) {
            self.open_pane_focus()
        } else {
            None
        };
        if width_change != 0 && !self.can_nudge_side_width(tui) {
            return false;
        }
        let resize = bindings.resize_panes.is_pressed(key) && can_resize;
        let resizing = resize || width_change != 0 || focused_percent.is_some();
        if !(close.is_some()
            || tab_step.is_some()
            || tabs
            || left
            || right
            || main
            || sidebar
            || hide
            || zoom_target.is_some()
            || resizing)
            || !self.conversation_focus_available(tui)
        {
            return false;
        }
        let child = self.switchable_side_child();
        if child.is_none()
            && ((hide && panel_focus != Some(PaneFocus::Sidebar))
                || (sidebar && !self.sidebar_chooser_available()))
        {
            return false;
        }
        if key.kind != KeyEventKind::Press {
            // A matching repeat still belongs to the shortcut. Keep it out of the focused editor.
            return true;
        }
        let direct_tab_switch = tab_step.is_some() && self.routed_key_activation.is_none();
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
        let result = if let Some(tab) = close {
            self.close_sidebar_tab(tui, app_server, tab).await
        } else if let Some(step) = tab_step {
            self.step_sidebar_tab(tui, app_server, step).await
        } else if tabs {
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
            self.refresh_owned_transcript(tui)
        } else if panel_focus.is_some()
            && (hide || (sidebar && panel_focus == Some(PaneFocus::Sidebar)))
        {
            self.hide_sidebar(tui, app_server).await
        } else if panel_focus.is_some() {
            if main {
                self.focus_current_parent_view(tui);
            }
            let focus = if main || left {
                PaneFocus::Main
            } else {
                PaneFocus::Sidebar
            };
            if focus == PaneFocus::Main
                && matches!(
                    self.sidebar()
                        .and_then(sidebar::SidebarPane::visible_content),
                    Some(SidebarContent::Document(_))
                )
            {
                self.zoomed_thread = None;
            }
            let pane = self.sidebar_mut();
            if let Some(content) = pane.visible_content() {
                pane.show(content, focus);
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
            self.open_sidebar_tabs(tui, app_server).await
        } else if (hide || sidebar) && on_child {
            self.hide_sidebar(tui, app_server).await
        } else {
            // A direction with no reachable neighbor leaves the current interaction intact.
            Ok(())
        };
        if let Err(err) = result {
            self.chat_widget.add_error_message(err.to_string());
        }
        let view_changed = leaves_transcript
            || matches!(close, Some(SidebarTab::Document(_)))
            || self.current_displayed_thread_id() != previous_thread
            || ((main || hide) && panel_focus == Some(PaneFocus::Sidebar));
        let suppression = if view_changed && !direct_tab_switch {
            ActivationKeySuppression::ViewTransition
        } else {
            // A direct tab shortcut can be pressed repeatedly to cycle through several tabs.
            ActivationKeySuppression::ReportedRepeats
        };
        self.chat_widget.suppress_current_view_key_until_release(
            activation,
            Instant::now(),
            suppression,
        );
        true
    }

    pub(super) async fn focus_main_composer(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
    ) -> Result<()> {
        if let Some(parent) = self
            .sidebar_parent()
            .filter(|parent| Some(*parent) != self.current_displayed_thread_id())
        {
            self.select_agent_thread(tui, app_server, parent).await?;
            if self.current_displayed_thread_id() == Some(parent) {
                self.focus_returned_parent_composer(parent).await?;
            }
        } else {
            self.focus_current_parent_view(tui);
        }
        Ok(())
    }

    pub(super) fn focus_current_parent_view(&mut self, tui: &mut tui::Tui) {
        if let Some(pane) = self.sidebars.get_mut(&self.sidebar_parent()) {
            pane.agents.pending_link = None;
        }
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
