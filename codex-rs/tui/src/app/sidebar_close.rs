//! Closing a tab keeps saved chats in the picker. Archiving also removes the saved entry.
//! Confirmation belongs to the parent and does not select the target tab.

use super::*;
use crate::app_server_session::SideChatStorage;
use crate::footer_hint;
use ratatui::buffer::Buffer;
use ratatui::layout::Margin;
use ratatui::widgets::Widget;

pub(super) use crate::app_event::SideChatRemoval as Action;

pub(super) struct Confirmation {
    action: Action,
    pub(super) target: ThreadId,
    message: String,
    confirm: bool,
}

impl Confirmation {
    pub(super) fn paint(&self, area: Rect, buf: &mut Buffer) -> [Rect; 2] {
        ratatui::widgets::Clear.render(area, buf);
        if area.width < 3 || area.height < 3 {
            return Default::default();
        }
        let body = area.inner(Margin::new(1, 0));
        let lines = textwrap::wrap(&self.message, usize::from(body.width));
        let offset = lines.len() + 2;
        let accept = match self.action {
            Action::CloseTab => " Close ",
            Action::Archive => " Archive ",
        };
        if body.width < 10 + accept.len() as u16 || offset >= usize::from(area.height) {
            Line::from("Resize to review · Esc to cancel").render(body, buf);
            return Default::default();
        }
        for (line, row) in lines.into_iter().zip(body.rows().skip(1)) {
            Line::from(line).render(row, buf);
        }
        let y = area.y + offset as u16;
        let buttons = [
            Rect::new(body.x, y, 8, 1).intersection(area),
            Rect::new(body.x.saturating_add(10), y, accept.len() as u16, 1).intersection(area),
        ];
        for (index, (label, button)) in [" Cancel ", accept].into_iter().zip(buttons).enumerate() {
            let style = if self.confirm == (index == 1) {
                crate::style::selection_style()
            } else {
                crate::style::secondary_text_style()
            };
            Line::from(label).style(style).render(button, buf);
        }
        if let Some(footer) = body.rows().nth(offset + 2) {
            footer_hint::HintLine::new(
                [
                    footer_hint::shortcut("←/→", "choose"),
                    footer_hint::shortcut("Enter", "confirm"),
                    footer_hint::shortcut("Esc", "cancel"),
                ],
                footer.width,
            )
            .line
            .render(footer, buf);
        }
        buttons
    }
}

pub(super) async fn request(
    app: &mut App,
    tui: &mut tui::Tui,
    server: &mut AppServerSession,
    target: ThreadId,
    action: Action,
) -> Result<()> {
    let Some(side) = app.side_threads.get(&target) else {
        return Ok(());
    };
    if side.close_request.is_some() {
        return Ok(());
    }
    let displayed = app.current_displayed_thread_id() == Some(target);
    let mut losses = Vec::new();
    if app.active_turn_id_for_thread(target).await.is_some()
        || (displayed && app.chat_widget.is_agent_turn_running())
    {
        losses.push("stop the running turn");
    }
    let has_unsent_input = side.opening_message.is_some()
        || if displayed {
            app.chat_widget.has_unsent_input()
        } else if let Some(channel) = app.thread_event_channels.get(&target) {
            channel
                .store
                .lock()
                .await
                .input_state
                .as_ref()
                .is_some_and(crate::chatwidget::ThreadInputState::has_unsent_input)
        } else {
            false
        };
    if has_unsent_input {
        losses.push("discard unsent input");
    }
    if side.storage == SideChatStorage::Temporary {
        losses.push("lose this temporary chat’s history");
    }
    if action == Action::Archive {
        losses.push("remove it from Side chats");
    }
    if losses.is_empty() {
        return execute(app, tui, server, target, action).await;
    }
    let Some(pane) = app.sidebar() else {
        return Ok(());
    };
    let tabs = pane.chat_tabs();
    let Some(index) = tabs.choice_index(target) else {
        return Ok(());
    };
    let title = crate::sidebar_chooser::side_chat_label(&tabs.choices[index - 1])
        .map(str::to_owned)
        .unwrap_or_else(|| format!("Side chat {index}"));
    app.conversation_frame.close_buttons = Default::default();
    let verb = match action {
        Action::CloseTab => "Close",
        Action::Archive => "Archive",
    };
    app.sidebar_mut().close = Some(Confirmation {
        target,
        action,
        message: format!(
            "This will {}. {verb} “{}”?",
            losses.join(" and "),
            crate::line_truncation::truncate_line_with_ellipsis_if_overflow(
                title.into(),
                /*max_width*/ 24
            )
        ),
        confirm: false,
    });
    tui.frame_requester().schedule_frame();
    Ok(())
}

pub(super) async fn handle(
    app: &mut App,
    tui: &mut tui::Tui,
    server: &mut AppServerSession,
    event: &TuiEvent,
) -> bool {
    let parent = app.sidebar_parent();
    let available = app.conversation_focus_available(tui);
    let Some(pane) = app.sidebars.get_mut(&parent) else {
        return false;
    };
    let Some(confirmation) = pane.close.as_mut() else {
        return false;
    };
    if !available {
        pane.close = None;
        tui.frame_requester().schedule_frame();
        return matches!(
            event,
            TuiEvent::Key(_) | TuiEvent::Paste(_) | TuiEvent::Mouse(_)
        );
    }
    let buttons = app.conversation_frame.close_buttons;
    let accept = match event {
        TuiEvent::Key(key) if key.kind == KeyEventKind::Press && key.modifiers.is_empty() => {
            match key.code {
                KeyCode::Tab | KeyCode::Left | KeyCode::Right => {
                    confirmation.confirm = !confirmation.confirm;
                    tui.frame_requester().schedule_frame();
                    return true;
                }
                KeyCode::Esc => false,
                KeyCode::Enter if buttons[1].is_empty() => return true,
                KeyCode::Enter => confirmation.confirm,
                _ => return true,
            }
        }
        TuiEvent::Mouse(mouse) if mouse.kind == MouseEventKind::Down(MouseButton::Left) => {
            let Some(index) = buttons
                .iter()
                .position(|button| button.contains((mouse.column, mouse.row).into()))
            else {
                return true;
            };
            index == 1
        }
        TuiEvent::Key(_) | TuiEvent::Paste(_) | TuiEvent::Mouse(_) => return true,
        _ => return false,
    };
    let target = confirmation.target;
    let action = confirmation.action;
    pane.close = None;
    tui.frame_requester().schedule_frame();
    if accept && let Err(err) = execute(app, tui, server, target, action).await {
        app.chat_widget.add_error_message(err.to_string());
    }
    if let TuiEvent::Key(key) = event {
        app.chat_widget.suppress_current_view_key_until_release(
            KeyBinding::from_event(*key),
            Instant::now(),
            crate::activation_key_guard::ActivationKeySuppression::ViewTransition,
        );
    }
    true
}

async fn execute(
    app: &mut App,
    tui: &mut tui::Tui,
    server: &mut AppServerSession,
    target: ThreadId,
    action: Action,
) -> Result<()> {
    let Some(side) = app.side_threads.get(&target) else {
        return Ok(());
    };
    if app.current_displayed_thread_id() == Some(target) {
        let next = match action {
            Action::CloseTab => app.sidebar().and_then(|pane| pane.previous_chat(target)),
            Action::Archive => None,
        }
        .unwrap_or(side.parent_thread_id);
        app.select_agent_thread(tui, server, next).await?;
        if app.current_displayed_thread_id() != Some(next) {
            return Ok(());
        }
        if action == Action::Archive {
            app.focus_returned_parent_composer(next).await?;
        }
    }
    let Some(side) = app.side_threads.get_mut(&target) else {
        return Ok(());
    };
    if side.close_request.is_some() {
        return Ok(());
    }
    let request = uuid::Uuid::new_v4();
    side.close_request = Some(request);
    let turn = app.active_turn_id_for_thread(target).await;
    let attachment = app
        .thread_event_channels
        .get(&target)
        .map(ThreadEventChannel::attachment);
    let mut view = server.request_view();
    let tx = app.app_event_tx.clone();
    tokio::spawn(async move {
        let result = match action {
            Action::Archive => view
                .thread_archive(target)
                .await
                .map_err(|err| format!("Failed to archive side conversation {target}: {err}")),
            Action::CloseTab => {
                async {
                    super::side::interrupt_for_close(&mut view, target, turn, attachment)
                        .await
                        .map_err(|err| {
                            format!("Failed to interrupt side conversation {target}: {err}")
                        })?;
                    view.thread_unsubscribe(target)
                        .await
                        .map_err(|err| format!("Failed to close side conversation {target}: {err}"))
                }
                .await
            }
        };
        tx.send(AppEvent::SideChatRemovalFinished {
            backend_id: view.backend_id(),
            target,
            request,
            action,
            result,
        });
    });
    tui.frame_requester().schedule_frame();
    Ok(())
}

/// Reconcile both locally requested archives and notifications from another client.
pub(super) async fn forget_archived(
    app: &mut App,
    tui: &mut tui::Tui,
    server: &mut AppServerSession,
    target: ThreadId,
) -> Result<()> {
    let Some(parent) = app
        .side_threads
        .get(&target)
        .map(|side| side.parent_thread_id)
        .or_else(|| {
            app.sidebars
                .iter()
                .find_map(|(parent, pane)| pane.chat_tabs().choice_index(target).and(*parent))
        })
    else {
        return Ok(());
    };
    if app.current_displayed_thread_id() == Some(target) {
        app.select_agent_thread(tui, server, parent).await?;
        if app.current_displayed_thread_id() == Some(parent) {
            app.focus_returned_parent_composer(parent).await?;
        }
    }
    let hide = app
        .sidebars
        .get(&Some(parent))
        .and_then(sidebar::SidebarPane::visible_content)
        == Some(sidebar::SidebarContent::Chat(target));
    app.abandoned_side_threads.insert(target);
    app.discard_thread_local_state(target).await;
    if let Some(pane) = app.sidebars.get_mut(&Some(parent)) {
        pane.set_discovery(crate::sidebar_chooser::Discovery::Idle);
        pane.remove_choice(target);
        if hide {
            pane.hide();
        }
    }
    tui.frame_requester().schedule_frame();
    Ok(())
}
