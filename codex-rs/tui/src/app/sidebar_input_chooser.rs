//! Interpret chooser input and execute validated chooser actions without changing event ownership.

use super::*;
use crate::activation_key_guard::ActivationKeySuppression;
use crate::app_event::SideStartMode;
use crate::keymap::ListAction;
use crate::sidebar_chooser;

#[derive(Clone, Copy)]
struct ChooserContext {
    new_chat: sidebar_chooser::NewChatState,
    area: Option<Rect>,
    selection_visible: bool,
}

enum ChooserInput {
    Consumed,
    Unhandled,
    Action {
        action: ChooserAction,
        activation: Option<KeyBinding>,
    },
}

/// A chooser row resolved after visibility and revision checks have passed.
enum OpenTarget {
    Agent(ThreadId),
    Retry,
    Document(crate::markdown_render::FileLink),
    Chat(ThreadId),
    NewChat,
}

impl App {
    pub(super) async fn handle_chooser_input(
        &mut self,
        tui: &mut tui::Tui,
        server: &mut AppServerSession,
        event: &TuiEvent,
        pointer_action: Option<ChooserAction>,
    ) -> bool {
        let new_chat = self.chat_widget.side_chat_creation_state();
        let chooser_area = self.chooser_area(tui);
        let selection_visible = self.sidebar().is_some_and(|pane| {
            chooser_area.is_none_or(|area| {
                !pane.chooser_scroll.wheel_scrolled
                    || sidebar_chooser::choice_is_visible(area, pane.chat_tabs(), pane.choice)
            })
        });
        let context = ChooserContext {
            new_chat,
            area: chooser_area,
            selection_visible,
        };
        match self.decode_chooser_input(tui, event, pointer_action, &context) {
            ChooserInput::Consumed => true,
            ChooserInput::Unhandled => false,
            ChooserInput::Action { action, activation } => {
                self.execute_chooser_action(tui, server, action, activation, &context)
                    .await
            }
        }
    }

    fn decode_chooser_input(
        &mut self,
        tui: &mut tui::Tui,
        event: &TuiEvent,
        pointer_action: Option<ChooserAction>,
        context: &ChooserContext,
    ) -> ChooserInput {
        let ChooserContext {
            new_chat,
            area: chooser_area,
            selection_visible,
        } = *context;
        match event {
            TuiEvent::Mouse(_) if let Some(action) = pointer_action => ChooserInput::Action {
                action,
                activation: None,
            },
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
                            sidebar_chooser::file_path_areas(area)[0],
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
                ChooserInput::Consumed
            }
            _ if self.chooser_focus() != Some(PaneFocus::Sidebar) => ChooserInput::Unhandled,
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
                    return ChooserInput::Consumed;
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
                            sidebar_chooser::file_path_areas(area)[0],
                            &file.path,
                            pane.file_path_scroll,
                            action,
                        );
                        tui.frame_requester().schedule_frame();
                    }
                    return ChooserInput::Consumed;
                }
                let Some(list_action) = list_action else {
                    return ChooserInput::Consumed;
                };
                let pane = self.sidebar_mut();
                if new_chat == sidebar_chooser::NewChatState::WaitingForMainMessage
                    && list_action != ListAction::Cancel
                    && pane.creation.is_none()
                    && pane.chat_tabs().files.is_empty()
                    && pane.chat_tabs().agents.is_empty()
                {
                    return ChooserInput::Consumed;
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
                        return ChooserInput::Consumed;
                    }
                };
                let activation = self
                    .routed_key_activation
                    .take()
                    .unwrap_or_else(|| KeyBinding::from_event(*key));
                ChooserInput::Action {
                    action,
                    activation: Some(activation),
                }
            }
            _ => {
                if matches!(event, TuiEvent::Paste(_)) {
                    ChooserInput::Consumed
                } else {
                    ChooserInput::Unhandled
                }
            }
        }
    }

    async fn execute_chooser_action(
        &mut self,
        tui: &mut tui::Tui,
        server: &mut AppServerSession,
        action: ChooserAction,
        activation: Option<KeyBinding>,
        context: &ChooserContext,
    ) -> bool {
        let ChooserContext {
            new_chat,
            area: chooser_area,
            selection_visible,
        } = *context;
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
        let target = match action {
            ChooserAction::Open(index) => self
                .sidebar()
                .filter(|pane| pane.creation.is_none())
                .map(|pane| {
                    let tabs = pane.chat_tabs();
                    if let Some(agent) = tabs.agent(index) {
                        OpenTarget::Agent(agent.id)
                    } else if tabs.retry_row(index) {
                        OpenTarget::Retry
                    } else if let Some(link) = tabs.file(index) {
                        OpenTarget::Document(link.clone())
                    } else if let Some(id) = tabs.choice(index) {
                        OpenTarget::Chat(id)
                    } else {
                        OpenTarget::NewChat
                    }
                }),
            ChooserAction::Back | ChooserAction::Hide | ChooserAction::FilePath => None,
        };
        match &target {
            Some(OpenTarget::Agent(agent)) => {
                if let ChooserAction::Open(index) = action {
                    let pane = self.sidebar_mut();
                    if pane.chat_tabs().agent_is_disabled(index) {
                        self.guard_chooser_key(activation);
                        return true;
                    }
                    pane.choice = index;
                }
                if self
                    .agent_navigation
                    .visible_threads()
                    .iter()
                    .any(|(id, _)| id == agent)
                    && let Err(error) = Box::pin(self.open_sidebar_agent(tui, server, *agent)).await
                {
                    self.chat_widget.add_error_message(error.to_string());
                }
                self.guard_chooser_key(activation);
                return true;
            }
            Some(OpenTarget::Retry) => {
                if let Some(parent) = self.current_displayed_thread_id() {
                    self.refresh_saved_chats(server, parent);
                }
                self.guard_chooser_key(activation);
                return true;
            }
            Some(OpenTarget::Document(_) | OpenTarget::Chat(_) | OpenTarget::NewChat) | None => {}
        }
        let pane = self.sidebar_mut();
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
        } else if let Some(OpenTarget::Document(mut link)) = target {
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
                ChooserAction::Open(_) => match target {
                    Some(OpenTarget::Chat(id)) => Some(id),
                    _ => None,
                },
                ChooserAction::Back => pane.selected_chat(),
                ChooserAction::Hide | ChooserAction::FilePath => None,
            };
            if let Some(id) = target {
                if let Some(parent) = self.current_displayed_thread_id() {
                    let target = super::sidebar_loading::Target {
                        backend_id: server.backend_id(),
                        parent,
                        child: id,
                    };
                    if let Err(err) = Box::pin(self.open_saved_chat(tui, server, target)).await {
                        self.chat_widget.add_error_message(err.to_string());
                    }
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
}
