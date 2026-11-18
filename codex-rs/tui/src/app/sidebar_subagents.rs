//! Sidebar subagents follow the primary conversation's spawn tree and retain observed outcomes.
use super::sidebar::PaneFocus;
use super::sidebar::SidebarContent;
use super::*;
use crate::keymap::ListAction;
use crate::sidebar_chooser::AgentRow;
use codex_app_server_protocol::CollabAgentStatus;
use codex_app_server_protocol::CollabAgentTool;
use codex_app_server_protocol::ThreadStatus;
use std::collections::HashSet;

#[path = "sidebar_agent_status.rs"]
mod status;
use status::State;

pub(super) enum OutcomeReplay {
    History,
    Reconnect,
}

#[derive(Default)]
pub(super) struct Overview {
    members: HashSet<ThreadId>,
    states: HashMap<ThreadId, State>,
    restored_outcomes: HashSet<ThreadId>,
    pub(super) rows: Vec<AgentRow>,
    pub(super) detail: Option<ThreadId>,
}

impl Overview {
    pub(super) fn chooser_rows(&self, nav: &AgentNavigationState) -> Vec<AgentRow> {
        let mut rows: Vec<_> = nav
            .visible_threads()
            .into_iter()
            .filter(|(id, _)| self.members.contains(id))
            .map(|(id, entry)| {
                let state = self.states.get(&id).copied().unwrap_or(if entry.is_closed {
                    State::Unavailable
                } else if entry.is_running {
                    State::Running
                } else {
                    State::Idle
                });
                (id, entry, state)
            })
            .collect();
        rows.sort_by_key(|(_, _, state)| (!state.active(), state.done()));
        rows.into_iter()
            .map(|(id, entry, state)| {
                let name = entry
                    .agent_path
                    .as_deref()
                    .map(|path| path.strip_prefix("/root/").unwrap_or(path).to_owned())
                    .unwrap_or_else(|| {
                        if entry.agent_nickname.is_none() && entry.agent_role.is_none() {
                            id.to_string()
                        } else {
                            format_agent_picker_item_name(
                                entry.agent_nickname.as_deref(),
                                entry.agent_role.as_deref(),
                                /*is_primary*/ false,
                            )
                        }
                    });
                let (symbol, status) = state.label();
                AgentRow {
                    id,
                    name,
                    symbol,
                    status,
                }
            })
            .collect()
    }
}

impl App {
    pub(super) async fn handle_docked_agent_input(
        &mut self,
        tui: &mut tui::Tui,
        server: &mut AppServerSession,
        event: &TuiEvent,
    ) -> bool {
        let Some((root, agent)) = self.docked_agent() else {
            return false;
        };
        let TuiEvent::Key(key) = event else {
            return false;
        };
        if self.current_displayed_thread_id() != Some(agent)
            || !self.chat_widget.no_modal_or_popup_active()
            || self.transcript_view.has_active_interaction()
        {
            return false;
        }
        if self.agent_navigation.is_parent_owned(agent)
            && self
                .transcript_view
                .navigate_pager(*key, &self.transcript_cells, &self.keymap.pager)
        {
            tui.frame_requester().schedule_frame();
            return true;
        }
        if key.kind != KeyEventKind::Press
            || self.keymap.list.action_for(*key) != Some(ListAction::Cancel)
        {
            return false;
        }
        if let Err(error) = Box::pin(self.select_agent_thread(tui, server, root)).await {
            self.chat_widget.add_error_message(error.to_string());
        }
        if self.current_displayed_thread_id() == Some(root) {
            self.side_companions.remove(&agent);
            let pane = self.sidebar_mut();
            if let Some(index) = pane.chat_tabs().agent_index(agent) {
                pane.choice = index;
                pane.chooser_scroll = Default::default();
            }
            pane.close_agents();
            pane.show(SidebarContent::Chooser, PaneFocus::Sidebar);
        }
        tui.frame_requester().schedule_frame();
        true
    }

    pub(super) fn docked_agent(&self) -> Option<(ThreadId, ThreadId)> {
        let parent = self.primary_thread_id?;
        let pane = self.sidebars.get(&Some(parent))?;
        (pane.visible_content() == Some(SidebarContent::Subagents))
            .then_some((parent, pane.agents.detail?))
    }

    pub(super) async fn open_sidebar_agent(
        &mut self,
        tui: &mut tui::Tui,
        server: &mut AppServerSession,
        agent: ThreadId,
    ) -> Result<()> {
        let Some(parent) = self.primary_thread_id else {
            return Ok(());
        };
        if !self.is_sidebar_agent(agent) || self.sidebar_parent() != Some(parent) {
            return Ok(());
        }
        let pane = self.sidebars.entry(Some(parent)).or_default();
        let previous_content = pane.visible_content();
        let previous_agent = pane.agents.detail.replace(agent);
        pane.show(SidebarContent::Subagents, PaneFocus::Sidebar);
        let result = Box::pin(self.select_agent_thread(tui, server, agent)).await;
        if self.current_displayed_thread_id() != Some(agent) {
            let pane = self.sidebars.entry(Some(parent)).or_default();
            if previous_agent.is_none() {
                pane.close_agents();
            }
            pane.agents.detail = previous_agent;
            pane.show(
                previous_content.unwrap_or(SidebarContent::Chooser),
                PaneFocus::Sidebar,
            );
        } else if let Some(previous) = previous_agent.filter(|previous| *previous != agent) {
            self.side_companions.remove(&previous);
        }
        tui.frame_requester().schedule_frame();
        result
    }

    pub(super) fn is_sidebar_agent(&self, id: ThreadId) -> bool {
        self.primary_thread_id
            .and_then(|root| self.sidebars.get(&Some(root)))
            .is_some_and(|pane| pane.agents.members.contains(&id))
    }
    pub(in crate::app) fn restore_sidebar_agent_outcomes(
        &mut self,
        root: ThreadId,
        turns: &[codex_app_server_protocol::Turn],
        source: OutcomeReplay,
    ) {
        let mut restored = HashMap::new();
        for item in turns.iter().flat_map(|turn| &turn.items) {
            status::visit_item(&item.payload, |raw, state, start_hint| {
                if let Ok(id) = ThreadId::from_string(raw)
                    && state.supersedes(restored.get(&id).copied(), start_hint)
                {
                    restored.insert(id, state);
                }
            });
        }
        for (id, state) in restored {
            if state.done()
                && self.register_sidebar_agent(root, id)
                && let Some(tabs) = self.sidebars.get_mut(&Some(root))
                && tabs.agents.states.get(&id).is_none_or(|previous| {
                    matches!(source, OutcomeReplay::Reconnect) && previous.active()
                })
            {
                tabs.agents.states.insert(id, state);
                tabs.agents.restored_outcomes.insert(id);
            }
        }
    }

    // Active or failed discovery supersedes saved history. Live notifications can arrive during
    // discovery and take precedence over its response.
    pub(super) fn seed_discovered_sidebar_agent(
        &mut self,
        root: ThreadId,
        child: ThreadId,
        status: &ThreadStatus,
    ) {
        if let Some(pane) = self.sidebars.get_mut(&Some(root))
            && (pane.agents.restored_outcomes.remove(&child)
                || !pane.agents.states.contains_key(&child))
        {
            pane.agents
                .states
                .insert(child, status::thread_status(status));
        }
    }

    pub(in crate::app) fn register_sidebar_agent(
        &mut self,
        parent: ThreadId,
        child: ThreadId,
    ) -> bool {
        let Some(root) = self.primary_thread_id else {
            return false;
        };
        if !self.local_settings.workspace_enabled(&self.config) || root == child {
            return false;
        }
        let parents = &mut self.sidebars;
        if parent != root
            && !parents
                .get(&Some(root))
                .is_some_and(|t| t.agents.members.contains(&parent))
        {
            return false;
        }
        let tabs = parents.entry(Some(root)).or_default();
        if tabs.agents.members.insert(child) {
            self.chat_widget.request_redraw();
        }
        true
    }

    pub(in crate::app) fn observe_sidebar_agent_event(&mut self, event: &ServerNotification) {
        if !self.local_settings.workspace_enabled(&self.config) {
            return;
        }
        match event {
            ServerNotification::ThreadStarted(e) => {
                if let Some(parent) =
                    super::loaded_threads::thread_spawn_parent_thread_id(&e.thread.source)
                    && let Ok(child) = ThreadId::from_string(&e.thread.id)
                    && self.register_sidebar_agent(parent, child)
                {
                    self.upsert_agent_picker_thread(
                        child,
                        e.thread.agent_nickname.clone(),
                        e.thread.agent_role.clone(),
                        /*is_closed*/ false,
                    );
                    self.agent_navigation.set_agent_path(
                        child,
                        crate::app_server_session::source_agent_path(&e.thread.source),
                    );
                    self.agent_navigation.set_running(
                        child,
                        matches!(e.thread.status, ThreadStatus::Active { .. }),
                    );
                    if crate::app_server_session::thread_blocks_direct_input(&e.thread) {
                        self.agent_navigation.mark_parent_owned(child);
                    }
                }
            }
            ServerNotification::ThreadStatusChanged(e) => {
                if let Ok(id) = ThreadId::from_string(&e.thread_id)
                    && self.is_sidebar_agent(id)
                {
                    self.agent_navigation
                        .set_running(id, matches!(e.status, ThreadStatus::Active { .. }));
                    self.chat_widget.request_redraw();
                }
            }
            ServerNotification::ItemStarted(_) | ServerNotification::ItemCompleted(_) => {
                let (sender, item) = match event {
                    ServerNotification::ItemStarted(e) => (&e.thread_id, &e.item.payload),
                    ServerNotification::ItemCompleted(e) => (&e.thread_id, &e.item.payload),
                    _ => return,
                };
                let Ok(parent) = ThreadId::from_string(sender) else {
                    return;
                };
                if let Some(activity) = sub_agent_activity_display(item) {
                    if self.register_sidebar_agent(parent, activity.thread_id) {
                        self.agent_navigation.record_sub_agent_activity(activity);
                        self.chat_widget.request_redraw();
                    }
                } else if let ThreadItemPayload::CollabAgentToolCall {
                    tool,
                    receiver_thread_ids,
                    agents_states,
                    ..
                } = item
                {
                    for id in receiver_thread_ids {
                        let Ok(child) = ThreadId::from_string(id) else {
                            continue;
                        };
                        let state = agents_states.get(id);
                        if state.is_some_and(|s| s.status == CollabAgentStatus::NotFound) {
                            continue;
                        }
                        if *tool == CollabAgentTool::SpawnAgent
                            && self.register_sidebar_agent(parent, child)
                            && self.agent_navigation.get(&child).is_none()
                        {
                            self.upsert_agent_picker_thread(
                                child, /*agent_nickname*/ None, /*agent_role*/ None,
                                /*is_closed*/ false,
                            );
                        }
                        if let Some(state) = state
                            && self.is_sidebar_agent(child)
                        {
                            let running = matches!(
                                state.status,
                                CollabAgentStatus::PendingInit | CollabAgentStatus::Running
                            );
                            self.agent_navigation.set_running(child, running);
                            self.chat_widget.request_redraw();
                        }
                    }
                }
            }
            _ => {}
        }
        let mut changed = false;
        for tabs in self.sidebars.values_mut() {
            changed |= status::observe(event, &mut tabs.agents);
        }
        if changed {
            self.chat_widget.request_redraw();
        }
    }
}
