//! A saved-child subscription is prepared on its originating connection. Only the
//! latest chooser intent may install it. Release is serialized with reopening that child.

use super::thread_attachment;
use super::thread_attachment::CloseState;
use super::thread_attachment::Incoming;
use super::thread_attachment::Prepared;
use super::*;
use crate::app_server_session::SideChatStorage;
use crate::backend_id::BackendId;
use crate::bottom_pane::TranscriptFooter;
use crate::footer_hint::Hint;
use crate::footer_hint::HintFocus;
use crate::sidebar_chooser::Discovery;
use crate::sidebar_chooser::OpenState;
use crate::sidebar_chooser::Opening;
use ratatui::style::Styled;
use uuid::Uuid;

pub(crate) use super::thread_attachment::Target;

struct Flight {
    request: Uuid,
    started: Instant,
    retired: bool,
    // None while releasing an unclaimed subscription.
    incoming: Option<Incoming>,
}

pub(super) struct Openings {
    active: HashMap<(BackendId, ThreadId), Flight>,
    // Background completions survive app event channel rotation on reconnect.
    pub(super) tx: AppEventSender,
    completions: mpsc::UnboundedReceiver<AppEvent>,
}

impl Default for Openings {
    fn default() -> Self {
        let (tx, completions) = mpsc::unbounded_channel();
        Self {
            active: HashMap::new(),
            tx: AppEventSender::new(tx),
            completions,
        }
    }
}

impl Openings {
    pub(super) async fn next_event(
        &mut self,
        events: &mut mpsc::UnboundedReceiver<AppEvent>,
    ) -> Option<AppEvent> {
        tokio::select! {
            event = events.recv() => event,
            event = self.completions.recv() => event,
        }
    }

    pub(super) fn preparing(&self, child: ThreadId, channel: Option<&ThreadEventChannel>) -> bool {
        channel.is_some_and(|channel| {
            channel
                .backend_id
                .and_then(|owner| self.active.get(&(owner, child)))
                .and_then(|flight| flight.incoming.as_ref())
                .is_some_and(|incoming| incoming.uses_channel(channel))
        })
    }

    pub(super) fn abandon(&mut self) {
        for flight in self.active.values_mut() {
            flight.retired = true;
        }
    }

    /// A provisional close is applied only after checking which runtime the resume attached.
    pub(super) fn observe(
        &mut self,
        owner: BackendId,
        child: ThreadId,
        channel: Option<&ThreadEventChannel>,
        notification: &ServerNotification,
    ) -> bool {
        let Some(updates) = self
            .active
            .get_mut(&(owner, child))
            .filter(|flight| !flight.retired)
            .and_then(|flight| flight.incoming.as_mut())
            .filter(|incoming| incoming.owns_channel(channel, owner))
        else {
            return false;
        };
        let updates = &mut updates.received;
        match notification {
            ServerNotification::ThreadNameUpdated(n) => {
                updates.name = Some(n.thread_name.clone());
            }
            ServerNotification::ThreadSettingsUpdated(n) => {
                updates.settings = Some(n.thread_settings.clone())
            }
            ServerNotification::ThreadReadStateChanged(_) => updates.read_state_changed = true,
            ServerNotification::TurnStarted(_) | ServerNotification::TurnCompleted(_) => {
                updates.turn_changed = true
            }
            ServerNotification::ThreadClosed(_) => {
                updates.close = CloseState::Reported;
                return true;
            }
            ServerNotification::ThreadStarted(_) => updates.close = CloseState::Open,
            _ => {}
        }
        false
    }
}

impl App {
    pub(super) fn can_cycle_scrollback_chats(&self) -> bool {
        self.scrollback_side_chats_enabled()
            && self.overlay.is_none()
            && self.chat_widget.completion_can_yield_focus()
            && self.sidebar_chooser_available()
    }

    pub(super) fn scrollback_cycle_pending(&self) -> bool {
        self.can_cycle_scrollback_chats()
            && self.sidebar().is_some_and(|pane| {
                matches!(pane.discovery, Discovery::Loading { cycle: true, .. })
                    || pane
                        .opening
                        .as_ref()
                        .is_some_and(|opening| opening.scrollback_origin.is_some())
            })
    }

    pub(super) fn scrollback_cycle_footer(&self, width: u16) -> Option<TranscriptFooter> {
        self.scrollback_cycle_pending().then(|| {
            let status = "Switching chats…".set_style(crate::style::secondary_text_style());
            let esc = crate::key_hint::plain(KeyCode::Esc).display_label();
            let hints = [Hint::Status(status.into()), Hint::key(esc, "to cancel")];
            let width = crate::bottom_pane::inset_footer_hint_area(Rect {
                width,
                ..Rect::default()
            })
            .width;
            TranscriptFooter {
                text: HintFocus::Active
                    .render_lines(hints, width, /*max_rows*/ 2)
                    .into(),
                is_interactive: true,
                cursor_column: None,
            }
        })
    }

    pub(super) fn waiting_saved_chat(&self) -> Option<Target> {
        let opening = self.sidebar()?.opening.as_ref()?;
        matches!(opening.state, OpenState::Waiting).then_some(opening.target)
    }

    pub(super) fn wants_saved_chat(&self, target: Target) -> bool {
        let Some(opening) = self.sidebar().and_then(|pane| pane.opening.as_ref()) else {
            return false;
        };
        if opening.target != target
            || self
                .app_server_backend_id
                .is_some_and(|id| id != target.backend_id)
            || matches!(opening.state, OpenState::Failed(_))
        {
            return false;
        }
        let active = self.current_displayed_thread_id();
        match opening.scrollback_origin {
            Some(origin) => active == Some(origin) && self.can_cycle_scrollback_chats(),
            None => {
                !self.reconnect.offline
                    && active == Some(target.parent)
                    && self.chooser_focus() == Some(sidebar::PaneFocus::Sidebar)
            }
        }
    }

    pub(super) async fn open_saved_chat(
        &mut self,
        tui: &mut tui::Tui,
        server: &mut AppServerSession,
        target: Target,
    ) -> Result<()> {
        if !self.can_select_thread()
            || (self.scrollback_side_chats_enabled()
                && (self.overlay.is_some() || !self.sidebar_chooser_available()))
        {
            return Ok(());
        }
        let pane = self.sidebar_mut();
        pane.agents.pending_link = None;
        pane.focus_document_request = None;
        let child = target.child;
        if self
            .sidebar()
            .and_then(|pane| pane.closing_chats.get(&child))
            .is_some_and(|task| !task.is_finished())
            || self
                .side_threads
                .get(&child)
                .is_some_and(|side| side.close_request.is_some())
        {
            self.chat_widget.add_info_message(
                "Side chat is still closing. Try again in a moment.".to_string(),
                /*hint*/ None,
            );
            return Ok(());
        }
        if self
            .side_threads
            .get(&child)
            .is_some_and(|side| side.storage == SideChatStorage::Temporary)
        {
            return self.select_agent_thread(tui, server, child).await;
        }
        let scrollback_origin = self
            .current_displayed_thread_id()
            .filter(|_| self.scrollback_side_chats_enabled());
        let flight = self.saved_openings.active.get(&target.key());
        let state = if let Some(flight) = flight
            && flight.incoming.is_some()
            && !flight.retired
        {
            OpenState::Loading(flight.started)
        } else if flight.is_some() || self.saved_openings.active.len() >= 2 {
            OpenState::Waiting
        } else {
            let side = side::SideThreadState {
                storage: SideChatStorage::Attached,
                ..side::SideThreadState::new(target.parent)
            };
            self.abandoned_side_threads.remove(&child);
            if let Some(channel) = self.thread_event_channels.get(&child)
                && channel.backend_id.is_none_or(|id| id == target.backend_id)
                && (channel.archived || !self.should_attach_live_thread_for_selection(child))
                && channel.store.lock().await.session.is_some()
            {
                self.side_threads.entry(child).or_insert(side);
                return self.select_agent_thread(tui, server, child).await;
            }
            let request = Uuid::new_v4();
            let started = Instant::now();
            let incoming = Incoming::begin(self, server, target).await;
            self.saved_openings.active.insert(
                target.key(),
                Flight {
                    request,
                    started,
                    retired: false,
                    incoming: Some(incoming),
                },
            );
            let view = server.attachment_view(child);
            let config = self.config.clone();
            let settings = self.local_settings.clone();
            let tx = self.saved_openings.tx.clone();
            tokio::spawn(async move {
                let prepared =
                    thread_attachment::prepare_saved(view, &settings, &config, child).await;
                tx.send(AppEvent::SavedChatPrepared {
                    target,
                    request,
                    result: Box::new(prepared),
                });
            });
            OpenState::Loading(started)
        };
        self.sidebar_mut().opening = Some(Opening {
            target,
            scrollback_origin,
            state,
        });
        tui.frame_requester().schedule_frame();
        Ok(())
    }

    pub(super) async fn finish_saved_chat(
        &mut self,
        tui: &mut tui::Tui,
        server: &mut AppServerSession,
        target: Target,
        request: Uuid,
        mut prepared: Prepared,
    ) -> Result<()> {
        let child = target.child;
        let wants = server.backend_id() == target.backend_id
            && self.wants_saved_chat(target)
            && (self.scrollback_side_chats_enabled() || self.conversation_focus_available(tui))
            && self.sidebar_chooser_available()
            && self.can_select_thread();
        let Some(flight) = self
            .saved_openings
            .active
            .get_mut(&target.key())
            .filter(|f| f.request == request)
        else {
            return Ok(());
        };
        let Some(mut incoming) = flight.incoming.take() else {
            return Ok(());
        };
        let current = wants
            && !flight.retired
            && incoming.owns_channel(self.thread_event_channels.get(&child), target.backend_id);
        if current
            && incoming.received.close == CloseState::Reported
            && matches!(
                prepared.result,
                Ok(thread_attachment::PreparedAttachment::Live(_))
            )
        {
            incoming.received.close = CloseState::Checking;
            flight.incoming = Some(incoming);
            let mut view = server.request_view();
            let tx = self.saved_openings.tx.clone();
            tokio::spawn(async move {
                prepared.close_check = match view.thread_read(child, /*include_turns*/ false).await
                {
                    Ok(thread) => {
                        Ok(thread.status == codex_app_server_protocol::ThreadStatus::NotLoaded)
                    }
                    Err(err) if App::is_terminal_thread_read_error(&err) => Ok(true),
                    Err(err) => Err(err.to_string()),
                };
                tx.send(AppEvent::SavedChatPrepared {
                    target,
                    request,
                    result: Box::new(prepared),
                });
            });
            return Ok(());
        }
        if incoming.received.close == CloseState::Checking {
            match prepared.close_check {
                Ok(true) => incoming.received.close = CloseState::Closed,
                Ok(false) => incoming.received.close = CloseState::Open,
                Err(error) => prepared.result = Err(error),
            }
        }
        if current && let Ok(attachment) = prepared.result {
            self.finish_saved_release(target, request);
            return self
                .install_saved_chat(
                    tui,
                    server,
                    target,
                    attachment,
                    prepared.metadata,
                    &incoming.received,
                )
                .await;
        }
        let release = incoming
            .discard(self, target, prepared.metadata.subscribed)
            .await;
        if let Some(pane) = self.sidebars.get_mut(&Some(target.parent))
            && let Some(opening) = pane.opening.take_if(|opening| {
                opening.target == target && matches!(opening.state, OpenState::Loading(_))
            })
            && current
            && let Err(message) = prepared.result
        {
            if opening.scrollback_origin.is_some() {
                self.chat_widget
                    .add_error_message(format!("Failed to open side chat: {message}"));
            } else {
                pane.opening = Some(Opening {
                    state: OpenState::Failed(message),
                    ..opening
                });
            }
        }
        if let Some(release) = release {
            let tx = self.saved_openings.tx.clone();
            tokio::spawn(async move {
                if let Err(error) = release.await {
                    tracing::warn!(%child, %error, "failed to release saved chat subscription");
                }
                tx.send(AppEvent::SavedChatReleased { target, request });
            });
        } else {
            self.finish_saved_release(target, request);
        }
        tui.frame_requester().schedule_frame();
        Ok(())
    }

    pub(super) fn finish_saved_release(&mut self, target: Target, request: Uuid) {
        if self
            .saved_openings
            .active
            .get(&target.key())
            .is_some_and(|flight| flight.request == request)
        {
            self.saved_openings.active.remove(&target.key());
            if self.waiting_saved_chat().is_some() {
                self.saved_openings.tx.send(AppEvent::StartQueuedSavedChat);
            }
        }
    }
}
