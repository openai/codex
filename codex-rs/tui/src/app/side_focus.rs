//! Composer and side-conversation focus shortcuts. Thread selection replaces the widget
//! immediately, but Park and Finish transfer transcript views through the app event queue.

use super::sidebar::PaneFocus;
use super::sidebar::SidebarContent;
use super::sidebar_close::Action::CloseTab;
use super::*;
use crate::activation_key_guard::ActivationKeySuppression;
use crate::app_event::SideStartMode;
use crate::keymap::ListAction;
use crate::sidebar_chooser;
use crate::sidebar_chooser::ChooserAction;
use crate::sidebar_chooser::Discovery;
use crate::sidebar_tab_strip::SidebarTab;
use crate::sidebar_tab_strip::TabTarget;
use crate::transcript_view::TranscriptView;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SidebarTarget {
    Tab(TabTarget<SidebarTab>),
    Row(usize),
    Footer(ChooserAction),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct SidebarPress {
    parent: Option<ThreadId>,
    state: sidebar::SidebarInputState,
    target: SidebarTarget,
    area: Rect,
    button: MouseButton,
}

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

    async fn open_sidebar_tabs(
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
        let pane = self.sidebar_mut();
        pane.closing_chats.retain(|_, task| !task.is_finished());
        pane.show(SidebarContent::Chooser, PaneFocus::Sidebar);
        if pane.creation.is_none()
            && let Some(parent) = parent
        {
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

    pub(super) async fn handle_sidebar_input(
        &mut self,
        tui: &mut tui::Tui,
        server: &mut AppServerSession,
        event: &TuiEvent,
    ) -> bool {
        if !self.conversation_focus_available(tui) || !self.sidebar_chooser_available() {
            self.sidebar_press = None;
            return false;
        }
        let mut pointer_action = None;
        if let TuiEvent::Mouse(mouse) = event {
            let point = (mouse.column, mouse.row).into();
            let frame = &self.conversation_frame;
            let over_strip = frame
                .sidebar_tab_strip()
                .is_some_and(|area| area.contains(point));
            let over_chooser = self.chooser_focus().is_some()
                && frame.panel_area().is_some_and(|area| area.contains(point));
            if tui.shortcut_hud_contains(*mouse) {
                self.sidebar_press = None;
                return false;
            }
            if over_strip
                && matches!(
                    mouse.kind,
                    MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
                )
            {
                let step = if mouse.kind == MouseEventKind::ScrollUp {
                    -1
                } else {
                    1
                };
                if frame.sidebar_input == self.sidebar().map(sidebar::SidebarPane::input_state)
                    && frame
                        .sidebar_targets
                        .iter()
                        .any(|(target, _)| matches!(target, SidebarTarget::Tab(_)))
                    && let Err(err) = self.step_sidebar_tab(tui, server, step).await
                {
                    self.chat_widget.add_error_message(err.to_string());
                }
                return true;
            }
            if let MouseEventKind::Down(button @ (MouseButton::Left | MouseButton::Middle))
            | MouseEventKind::Up(button @ (MouseButton::Left | MouseButton::Middle)) = mouse.kind
            {
                let down = matches!(mouse.kind, MouseEventKind::Down(_));
                let pressed = self.sidebar_press.take();
                if over_strip || over_chooser || pressed.is_some() {
                    let current = self.sidebar().map(sidebar::SidebarPane::input_state);
                    let hit = frame
                        .sidebar_targets
                        .iter()
                        .find(|(_, area)| area.contains(point));
                    let candidate = if frame.sidebar_input == current && self.pane_resize.is_none()
                    {
                        hit.zip(current)
                            .map(|(&(target, area), state)| SidebarPress {
                                parent: self.sidebar_parent(),
                                state,
                                target,
                                area,
                                button,
                            })
                    } else {
                        None
                    };
                    tui.frame_requester().schedule_frame();
                    if down {
                        self.sidebar_press = candidate;
                        return true;
                    }
                    let Some(press) = candidate.filter(|candidate| Some(*candidate) == pressed)
                    else {
                        return true;
                    };
                    match press.target {
                        SidebarTarget::Tab(target) => {
                            let result = match (button, target) {
                                (MouseButton::Left, TabTarget::Close(id))
                                | (
                                    MouseButton::Middle,
                                    TabTarget::Tab(id) | TabTarget::Close(id),
                                ) => self.close_sidebar_tab(tui, server, id).await,
                                (MouseButton::Left, TabTarget::Add) => {
                                    self.open_sidebar_tabs(tui, server).await
                                }
                                (
                                    MouseButton::Left,
                                    TabTarget::Tab(id) | TabTarget::Overflow { tab: id, .. },
                                ) => self.activate_sidebar_tab(tui, server, id).await,
                                _ => return true,
                            };
                            if let Err(err) = result {
                                self.chat_widget.add_error_message(err.to_string());
                            }
                            return true;
                        }
                        SidebarTarget::Row(index) if button == MouseButton::Left => {
                            pointer_action = Some(ChooserAction::Open(index));
                        }
                        SidebarTarget::Footer(action) if button == MouseButton::Left => {
                            pointer_action = Some(action);
                        }
                        _ => return true,
                    }
                    self.sidebar_mut()
                        .show(SidebarContent::Chooser, PaneFocus::Sidebar);
                } else if down && button == MouseButton::Left && self.chooser_focus().is_some() {
                    self.sidebar_mut()
                        .show(SidebarContent::Chooser, PaneFocus::Main);
                }
            }
        }
        if matches!(
            self.sidebar()
                .and_then(sidebar::SidebarPane::visible_content),
            Some(SidebarContent::Document(_))
        ) {
            return self.handle_document_input(tui, server, event).await;
        }
        if self
            .sidebar()
            .and_then(sidebar::SidebarPane::visible_content)
            == Some(SidebarContent::Subagents)
        {
            return self.handle_docked_agent_input(tui, server, event).await;
        }
        if self.chooser_focus().is_none() {
            return false;
        }
        let new_chat = self.chat_widget.side_chat_creation_state();
        let chooser_area = self.chooser_area(tui);
        let selection_visible = self.sidebar().is_some_and(|pane| {
            chooser_area.is_none_or(|area| {
                !pane.chooser_scroll.wheel_scrolled
                    || sidebar_chooser::choice_is_visible(area, pane.chat_tabs(), pane.choice)
            })
        });
        let (action, activation) = match event {
            TuiEvent::Mouse(_) if let Some(action) = pointer_action => (action, None),
            TuiEvent::Mouse(mouse)
                if matches!(
                    mouse.kind,
                    MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
                ) && let Some(area) = self.conversation_frame.panel_area()
                    && area.contains((mouse.column, mouse.row).into()) =>
            {
                let pane = self.sidebar_mut();
                let delta = if mouse.kind == MouseEventKind::ScrollUp {
                    -3
                } else {
                    3
                };
                if pane.showing_file_path {
                    if let Some(file) = pane.chat_tabs().file(pane.choice) {
                        let action = if delta < 0 {
                            ListAction::MoveUp
                        } else {
                            ListAction::MoveDown
                        };
                        pane.file_path_scroll = sidebar_chooser::scroll_file_path(
                            area,
                            &file.path,
                            pane.file_path_scroll,
                            action,
                        );
                    }
                    tui.frame_requester().schedule_frame();
                } else if pane.creation.is_none() && mouse.row < area.bottom() - 1 {
                    pane.chooser_scroll =
                        sidebar_chooser::wheel_scroll(area, pane.chat_tabs(), pane.choice, delta);
                    tui.frame_requester().schedule_frame();
                }
                return true;
            }
            _ if self.chooser_focus() != Some(PaneFocus::Sidebar) => return false,
            TuiEvent::Key(key) => {
                let list_action = self.keymap.list.action_for(*key);
                if list_action.is_none()
                    && *key == KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE)
                    && self.sidebar().is_some_and(|pane| {
                        pane.showing_file_path
                            || (selection_visible && pane.chat_tabs().file(pane.choice).is_some())
                    })
                {
                    let pane = self.sidebar_mut();
                    pane.showing_file_path = !pane.showing_file_path;
                    pane.file_path_scroll = 0;
                    tui.frame_requester().schedule_frame();
                    return true;
                }
                if self.sidebar().is_some_and(|pane| pane.showing_file_path)
                    && list_action != Some(ListAction::Cancel)
                {
                    let pane = self.sidebar_mut();
                    if let Some(area) = chooser_area
                        && let Some(file) = pane.chat_tabs().file(pane.choice)
                        && let Some(action) = list_action
                    {
                        pane.file_path_scroll = sidebar_chooser::scroll_file_path(
                            area,
                            &file.path,
                            pane.file_path_scroll,
                            action,
                        );
                        tui.frame_requester().schedule_frame();
                    }
                    return true;
                }
                let Some(list_action) = list_action else {
                    return true;
                };
                let pane = self.sidebar_mut();
                if new_chat == sidebar_chooser::NewChatState::WaitingForMainMessage
                    && list_action != ListAction::Cancel
                    && pane.creation.is_none()
                    && pane.chat_tabs().files.is_empty()
                    && pane.chat_tabs().agents.is_empty()
                {
                    return true;
                }
                tui.frame_requester().schedule_frame();
                let action = match (list_action, key.kind) {
                    (ListAction::Accept, KeyEventKind::Press) => ChooserAction::Open(pane.choice),
                    (ListAction::Cancel, KeyEventKind::Press) => ChooserAction::Back,
                    (ListAction::MoveLeft | ListAction::MoveRight, KeyEventKind::Press)
                        if pane.selected_chat().is_some() =>
                    {
                        ChooserAction::Back
                    }
                    _ => {
                        let area = chooser_area
                            .unwrap_or_else(|| tui.terminal.last_known_screen_size.into());
                        (pane.choice, pane.chooser_scroll) = sidebar_chooser::navigate(
                            area,
                            pane.chat_tabs(),
                            pane.choice,
                            list_action,
                        );
                        return true;
                    }
                };
                let activation = self
                    .routed_key_activation
                    .take()
                    .unwrap_or_else(|| KeyBinding::from_event(*key));
                (action, Some(activation))
            }
            _ => return matches!(event, TuiEvent::Paste(_)),
        };

        if action == ChooserAction::FilePath
            || self.sidebar().is_some_and(|pane| pane.showing_file_path)
        {
            let pane = self.sidebar_mut();
            pane.showing_file_path = action == ChooserAction::FilePath
                && selection_visible
                && pane.chat_tabs().file(pane.choice).is_some();
            pane.file_path_scroll = 0;
            tui.frame_requester().schedule_frame();
            return true;
        }

        if let ChooserAction::Open(index) = action
            && new_chat == sidebar_chooser::NewChatState::WaitingForMainMessage
            && self.sidebar().is_some_and(|pane| {
                pane.creation.is_none()
                    && pane.chat_tabs().file(index).is_none()
                    && pane.chat_tabs().agent(index).is_none()
            })
        {
            return true;
        }
        if let ChooserAction::Open(index) = action
            && let Some(pane) = self.sidebar()
            && pane.creation.is_none()
            && (pane.chooser_scroll.wheel_scrolled
                && chooser_area.is_some_and(|area| {
                    !sidebar_chooser::choice_is_visible(area, pane.chat_tabs(), index)
                })
                || pane.chat_tabs().file(index).is_some()
                    && pane.file_preview_unavailable.is_some())
        {
            return true;
        }
        let open = matches!(action, ChooserAction::Open(_));
        let frame = &self.conversation_frame;
        // Fast keyboard navigation is safe until an asynchronous update changes its target.
        if activation.is_some()
            && open
            && frame.panel_area().is_some()
            && self.sidebar().is_some_and(|pane| {
                let current = pane.input_state();
                pane.creation.is_none()
                    && frame.sidebar_input.is_some_and(|painted| {
                        painted.revision != current.revision && painted.choice != current.choice
                    })
            })
        {
            self.guard_chooser_key(activation);
            return true;
        }
        let pane = self.sidebar_mut();
        if let ChooserAction::Open(index) = action
            && pane.creation.is_none()
            && let Some(agent) = pane.chat_tabs().agent(index).map(|agent| agent.id)
        {
            pane.choice = index;
            if self
                .agent_navigation
                .visible_threads()
                .iter()
                .any(|(id, _)| *id == agent)
                && let Err(error) = Box::pin(self.open_sidebar_agent(tui, server, agent)).await
            {
                self.chat_widget.add_error_message(error.to_string());
            }
            self.guard_chooser_key(activation);
            return true;
        }
        if let ChooserAction::Open(index) = action
            && pane.creation.is_none()
            && pane.chat_tabs().retry_row(index)
        {
            if let Some(parent) = self.current_displayed_thread_id() {
                self.refresh_saved_chats(server, parent);
            }
            self.guard_chooser_key(activation);
            return true;
        }
        if matches!(action, ChooserAction::Back)
            && pane
                .opening
                .take()
                .is_some_and(|o| !matches!(o.state, sidebar_chooser::OpenState::Failed(_)))
        {
            self.guard_chooser_key(activation);
            return true;
        }
        if let ChooserAction::Open(index) = action {
            pane.choice = index;
        }
        pane.opening = None;
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
                            mode: SideStartMode::New,
                        });
                    } else {
                        let pane = self.sidebar_mut();
                        pane.choice = 0;
                        pane.chooser_scroll = Default::default();
                        self.restore_side_user_message(message);
                    }
                }
            }
        } else if let ChooserAction::Open(index) = action
            && let Some(mut link) = pane.chat_tabs().file(index).cloned()
        {
            // The chooser opens the file. Transcript and reader links carry coordinates.
            link.location = None;
            if let Some(source) = self.current_displayed_thread_id() {
                self.app_event_tx.send(AppEvent::OpenSidebarDocument {
                    backend_id: server.backend_id(),
                    source,
                    link,
                });
            }
        } else if matches!(action, ChooserAction::Back)
            && let Some(tab @ (SidebarTab::Document(_) | SidebarTab::Subagents)) =
                pane.recent_tab(/*excluding*/ None)
        {
            if let Err(err) = self.activate_sidebar_tab(tui, server, tab).await {
                self.chat_widget.add_error_message(err.to_string());
            }
        } else {
            let target = match action {
                ChooserAction::Open(index) => pane.chat_tabs().choice(index),
                ChooserAction::Back => pane.selected_chat(),
                ChooserAction::Hide | ChooserAction::FilePath => None,
            };
            if let Some(id) = target {
                let pane = self.sidebar_mut();
                if pane
                    .closing_chats
                    .get(&id)
                    .is_some_and(|task| !task.is_finished())
                {
                    self.chat_widget.add_info_message(
                        "Side chat is still closing. Try again in a moment.".to_string(),
                        /*hint*/ None,
                    );
                    return true;
                }
                let saved = !pane.chat_tabs().ids.contains(&SidebarTab::Chat(id));
                if saved && let Some(parent) = self.current_displayed_thread_id() {
                    let target = super::sidebar_loading::Target {
                        backend_id: server.backend_id(),
                        parent,
                        child: id,
                    };
                    if let Err(err) = Box::pin(self.open_saved_chat(tui, server, target)).await {
                        self.chat_widget.add_error_message(err.to_string());
                    }
                } else if let Err(err) = self.select_agent_thread(tui, server, id).await {
                    self.chat_widget.add_error_message(err.to_string());
                }
            } else if open && let Some(parent_thread_id) = self.chat_widget.thread_id() {
                self.app_event_tx.send(AppEvent::StartSide {
                    parent_thread_id,
                    user_message: None,
                    mode: SideStartMode::New,
                });
            } else {
                if open {
                    self.chat_widget
                        .dispatch_command(crate::slash_command::SlashCommand::Side);
                }
                self.sidebar_mut().hide();
            }
        }
        // Selecting a chat replaces its widget. Guard the composer that will receive repeats.
        self.guard_chooser_key(activation);
        true
    }

    fn guard_chooser_key(&mut self, activation: Option<KeyBinding>) {
        if let Some(activation) = activation {
            self.chat_widget.suppress_current_view_key_until_release(
                activation,
                Instant::now(),
                ActivationKeySuppression::ViewTransition,
            );
        }
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
                .is_some_and(|doc| doc.allows_tab_shortcut(key))
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
        } else if panel_focus.is_some() {
            if hide || (sidebar && panel_focus == Some(PaneFocus::Sidebar)) {
                self.zoomed_thread = None;
                self.focus_current_parent_view(tui);
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

    fn focus_current_parent_view(&mut self, tui: &mut tui::Tui) {
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
