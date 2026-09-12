//! Server RPCs own lifecycle outcomes; successful actions invalidate dashboard group caches.
//! A modal keeps rendering during RPCs while deferring navigation and task notifications.
//! Removing the current root leaves an unattached dashboard, even when it is empty.

use super::App;
use super::agents_overview_view::AgentsOverviewFocus;
use crate::app_event::AgentsOverviewAction;
use crate::app_event::AppEvent;
use crate::app_server_session::AppServerSession;
use crate::bottom_pane::SelectionItem;
use crate::bottom_pane::SelectionViewParams;
use crate::chatwidget::ChatWidget;
use crate::render::renderable::Renderable;
use crate::tui;
use crate::wrapping::word_wrap_lines;
use codex_app_server_protocol::SessionSource;
use codex_app_server_protocol::ThreadLoadedListParams;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SubAgentSource;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use std::collections::HashSet;
use std::future::Future;
use tokio_stream::Stream;
use tokio_stream::StreamExt;

struct LifecycleHeader(Vec<Line<'static>>);

impl Renderable for LifecycleHeader {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        Renderable::render(
            &Paragraph::new(word_wrap_lines(&self.0, usize::from(area.width))),
            area,
            buf,
        );
    }

    fn desired_height(&self, width: u16) -> u16 {
        word_wrap_lines(&self.0, usize::from(width)).len() as u16
    }
}

struct LifecycleProgress(AgentsOverviewAction);

impl LifecycleProgress {
    fn header(&self) -> LifecycleHeader {
        let title = match self.0 {
            AgentsOverviewAction::Archive => "Archiving task…",
            AgentsOverviewAction::Delete => "Deleting task…",
        };
        LifecycleHeader(vec![
            title.bold().into(),
            "Please wait. Task switching is unavailable until this finishes."
                .dim()
                .into(),
        ])
    }
}

impl Renderable for LifecycleProgress {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.header().render(area, buf);
    }

    fn desired_height(&self, width: u16) -> u16 {
        self.header().desired_height(width)
    }
}

/// Keep the terminal responsive without dispatching navigation or queued task events.
async fn run_lifecycle_modal<T>(
    tui: &mut tui::Tui,
    action: AgentsOverviewAction,
    operation: impl Future<Output = T>,
    events: impl Stream<Item = tui::TuiEvent> + Unpin,
) -> color_eyre::Result<T> {
    let progress = LifecycleProgress(action);
    tui.draw(u16::MAX, |frame| {
        progress.render(frame.area(), frame.buffer_mut());
    })?;
    let mut events = events.fuse();
    tokio::pin!(operation);
    let result = loop {
        tokio::select! {
            biased;
            result = &mut operation => break result,
            Some(event) = events.next() => {
                tui.screen_size_for_event(&event)?;
                match event {
                    tui::TuiEvent::Key(_) | tui::TuiEvent::Paste(_) | tui::TuiEvent::FocusLost => {}
                    tui::TuiEvent::Draw | tui::TuiEvent::Resize(_) | tui::TuiEvent::Resume | tui::TuiEvent::FocusGained => {
                        tui.draw(u16::MAX, |frame| {
                            progress.render(frame.area(), frame.buffer_mut());
                        })?;
                    }
                }
            }
        }
    };
    tui.frame_requester().schedule_frame();
    Ok(result)
}

impl App {
    pub(super) fn confirm_agents_overview_action(
        &mut self,
        thread_id: ThreadId,
        action: AgentsOverviewAction,
    ) {
        let Some(thread) = self
            .agents_overview
            .threads
            .get(&thread_id)
            .and_then(Option::as_ref)
        else {
            return;
        };
        let name = thread.name.as_deref().unwrap_or(&thread.preview);
        let name = name
            .trim()
            .lines()
            .next()
            .filter(|name| !name.is_empty())
            .unwrap_or("Untitled task");
        let (title, description, label) = match action {
            AgentsOverviewAction::Archive => (
                format!("Archive “{name}”?"),
                "This stops any running work in this task and its child agents, then archives them. Their history can be restored from the resume picker.",
                "Archive task and child agents",
            ),
            AgentsOverviewAction::Delete => (
                format!("Permanently delete “{name}”?"),
                "This stops any running work in this task and its child agents, then permanently deletes their history. This cannot be undone.",
                "Permanently delete task and child agents",
            ),
        };
        self.chat_widget.show_selection_view(SelectionViewParams {
            header: Box::new(LifecycleHeader(vec![
                title.bold().into(),
                description.dim().into(),
            ])),
            items: vec![
                SelectionItem {
                    name: "Cancel".to_string(),
                    dismiss_on_select: true,
                    ..Default::default()
                },
                SelectionItem {
                    name: label.to_string(),
                    actions: vec![Box::new(move |tx| {
                        tx.send(AppEvent::RunAgentsOverviewAction { thread_id, action })
                    })],
                    dismiss_on_select: true,
                    require_explicit_confirmation: action == AgentsOverviewAction::Delete,
                    ..Default::default()
                },
            ],
            ..Default::default()
        });
    }

    pub(super) async fn run_agents_overview_action(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
        action: AgentsOverviewAction,
    ) -> color_eyre::Result<()> {
        if self.windows_sandbox_blocks_thread_switch() {
            return Ok(());
        }
        // The overview may lack intermediate ancestors, or even the primary's metadata.
        let mut removes_primary = self.primary_thread_id == Some(thread_id);
        let mut attempted = false;
        let primary_thread_id = self.primary_thread_id;
        let events = tui.event_stream();
        let operation = async {
            let result = async {
                if let Some(primary) = primary_thread_id
                    && primary != thread_id
                {
                    removes_primary = app_server
                        .thread_read(primary, /*include_turns*/ false)
                        .await?
                        .session_id
                        == thread_id.to_string();
                }
                attempted = true;
                match action {
                    AgentsOverviewAction::Archive => app_server.thread_archive(thread_id).await,
                    AgentsOverviewAction::Delete => app_server.thread_delete(thread_id).await,
                }
            }
            .await;
            let mut detach_primary = removes_primary && attempted;
            if detach_primary
                && result.is_err()
                && let Some(primary) = primary_thread_id
            {
                // A rejected operation may have left the attachment alive. Ask the server directly.
                let mut cursor = None;
                while let Ok(page) = app_server
                    .thread_loaded_list(ThreadLoadedListParams {
                        cursor,
                        limit: None,
                    })
                    .await
                {
                    if page.data.contains(&primary.to_string()) {
                        detach_primary = false;
                        break;
                    }
                    let Some(next) = page.next_cursor else { break };
                    cursor = Some(next);
                }
            }
            (result, detach_primary)
        };
        let (result, detach_primary) = run_lifecycle_modal(tui, action, operation, events).await?;
        if attempted {
            // A read started before the RPC can contain stale task and approval state.
            if let Some(refresh) = self.agents_overview.refresh_task.take() {
                refresh.abort();
            }
            self.agents_overview.request_id = None;
            self.agents_overview.refresh_pending = false;
            self.agents_overview.refresh_notifications.clear();
        }
        if result.is_ok() {
            // Invalidate cached details for the displayed group, without inferring child outcomes.
            let mut removed = HashSet::from([thread_id]);
            loop {
                let previous_len = removed.len();
                for (id, thread) in &self.agents_overview.threads {
                    let Some(thread) = thread else { continue };
                    let parent = thread
                        .parent_thread_id
                        .as_deref()
                        .and_then(|id| ThreadId::from_string(id).ok())
                        .or(match &thread.source {
                            SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                                parent_thread_id,
                                ..
                            }) => Some(*parent_thread_id),
                            _ => None,
                        });
                    if thread.session_id == thread_id.to_string()
                        || parent.is_some_and(|parent| removed.contains(&parent))
                    {
                        removed.insert(*id);
                    }
                }
                if removed.len() == previous_len {
                    break;
                }
            }
            if let Some(primary) = self.primary_thread_id
                && removes_primary
            {
                removed.insert(primary);
            }
            for removed_id in removed {
                self.agents_overview.threads.remove(&removed_id);
                self.agents_overview.activity.remove(&removed_id);
                self.agents_overview.last_messages.remove(&removed_id);
                self.agents_overview.usage.remove(&removed_id);
                self.agents_overview.refresh_thread_ids.remove(&removed_id);
                self.agents_overview.input_states.remove(&removed_id);
                self.agents_overview.dispatched_requests.remove(&removed_id);
            }
        }
        if detach_primary {
            self.reset_for_thread_switch(tui)?;
            self.pending_thread_switch_resets += 1;
            self.app_event_tx
                .send(AppEvent::ResetTranscriptForThreadSwitch);
            self.reset_thread_event_state();
            let init = self.chatwidget_init_for_forked_or_resumed_thread(
                tui,
                self.config.clone(),
                /*initial_user_message*/ None,
            );
            self.replace_chat_widget(ChatWidget::new_with_app_event(init));
            self.open_agents_overview(app_server, AgentsOverviewFocus::List);
        } else {
            self.repaint_agents_overview();
            if attempted {
                self.refresh_agents_overview_threads(app_server);
            }
        }
        if let Err(error) = result {
            let verb = match action {
                AgentsOverviewAction::Archive => "archive",
                AgentsOverviewAction::Delete => "delete",
            };
            let mut header = vec![
                format!("Could not {verb} task").red().bold().into(),
                format!("{error:#}").red().into(),
            ];
            if attempted {
                header.push(
                    "Work may have stopped. Resume the task to continue, or retry the action."
                        .dim()
                        .into(),
                );
            }
            self.chat_widget.show_selection_view(SelectionViewParams {
                header: Box::new(LifecycleHeader(header)),
                items: vec![SelectionItem {
                    name: "Back to agents".to_string(),
                    dismiss_on_select: true,
                    ..Default::default()
                }],
                ..Default::default()
            });
        }

        Ok(())
    }
}

#[cfg(test)]
#[path = "agents_overview_action_progress_tests.rs"]
mod progress_tests;
