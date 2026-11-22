//! Each parent owns one sidebar. Its content survives hiding so the same surface
//! can paint the closing frames and reverse direction if reopened mid-slide.

use super::App;
use super::agent_navigation::AgentNavigationState;
use super::pane_motion::PaneMotion;
use super::side_pane;
use super::side_pane::SidePaneLayout;
use crate::app_event::AppEvent;
use crate::app_event::SavedSideChatData;
use crate::app_server_session::AppServerSession;
use crate::app_server_session::SideChat;
use crate::app_server_session::SideChatStorage;
use crate::app_server_session::side_chat_prompt_preview;
use crate::sidebar_chooser::ChatTabs;
use crate::sidebar_chooser::ChooserScroll;
use crate::sidebar_chooser::Discovery;
use crate::sidebar_tab_strip::SidebarTab;
use crate::terminal_hyperlinks::ReaderLink;
use crate::tui::Tui;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadItemPayload;
use codex_protocol::ThreadId;
use ratatui::layout::Rect;
use std::collections::HashMap;
use tokio::task::JoinHandle;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum SidebarContent {
    #[default]
    Chooser,
    Chat(ThreadId),
    Document(uuid::Uuid),
    Subagents,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PaneFocus {
    Main,
    Sidebar,
}

pub(super) enum CreationState {
    Waiting {
        request: uuid::Uuid,
        started: std::time::Instant,
    },
    Failed {
        message: String,
        cleanup: Option<ThreadId>,
    },
}

pub(super) struct SideCreation {
    pub(super) owner: crate::backend_id::BackendId,
    pub(super) state: CreationState,
    pub(super) message: Option<crate::chatwidget::UserMessage>,
}

impl SideCreation {
    pub(super) fn request(&self) -> Option<uuid::Uuid> {
        match self.state {
            CreationState::Waiting { request, .. } => Some(request),
            CreationState::Failed { .. } => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ChooserSelection {
    Agent(ThreadId),
    New,
    Chat(ThreadId),
    Retry,
    File(usize),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct SidebarInputState {
    pub(super) revision: usize,
    pub(super) selected_tab: Option<SidebarTab>,
    pub(super) choice: ChooserSelection,
    scroll: ChooserScroll,
    content: SidebarContent,
    showing_file_path: bool,
    file_path_scroll: usize,
}

#[derive(Default)]
pub(super) struct SidebarPane {
    pub(super) creation: Option<SideCreation>,
    pub(super) close: Option<super::sidebar_close::Confirmation>,
    pub(super) files: Option<super::sidebar_files::Files>,
    pub(super) opened_files: Vec<codex_utils_path_uri::PathUri>,
    pub(super) documents: HashMap<uuid::Uuid, super::sidebar_document::Document>,
    pub(super) pending_documents: HashMap<
        crate::backend_id::BackendId,
        std::collections::VecDeque<crate::open_in_codex::DocumentRequest>,
    >,
    pub(super) focus_document_request: Option<uuid::Uuid>,
    content: SidebarContent,
    tabs: Vec<SidebarTab>,
    pub(super) agents: super::sidebar_subagents::Overview,
    // The chooser keeps the same order when an attached chat becomes a live tab.
    choices: Vec<SideChat>,
    // A snapshot of the visible rows at the last explicit chooser refresh.
    pub(super) latest_prompts: HashMap<ThreadId, String>,
    pub(super) opening: Option<crate::sidebar_chooser::Opening>,
    pub(super) discovery: Discovery,
    pub(super) closing_chats: HashMap<ThreadId, JoinHandle<()>>,
    // Visit order is independent of the order in which tabs appear.
    visits: Vec<SidebarTab>,
    pub(super) choice: usize,
    pub(super) chooser_scroll: ChooserScroll,
    pub(super) showing_file_path: bool,
    pub(super) file_path_scroll: usize,
    pub(super) file_preview_unavailable: Option<&'static str>,
    pub(super) tabs_revision: usize,
    focus: Option<PaneFocus>,
    motion: PaneMotion,
}

impl SidebarPane {
    pub(super) fn visible_link_at(&self, column: u16, row: u16) -> Option<ReaderLink> {
        let SidebarContent::Document(id) = self.visible_content()? else {
            return None;
        };
        self.documents.get(&id)?.link_at(column, row)
    }

    pub(super) fn finish_document_copy(
        &mut self,
        completion: &(u64, crate::clipboard_copy::worker::CopyResult),
        visible: bool,
    ) {
        let selected = self.visible_content();
        let visible = visible && self.close.is_none();
        for (&id, doc) in &mut self.documents {
            if let Some((view, cells)) = doc.reader()
                && view.finish_copy(
                    cells,
                    completion,
                    visible && selected == Some(SidebarContent::Document(id)),
                ) == Some(true)
            {
                view.jump_to_latest();
            }
        }
    }

    pub(super) fn cancel_hidden_document_primary(&mut self, visible: bool) {
        let selected = self.visible_content();
        let visible = visible && self.close.is_none();
        for (&id, doc) in &mut self.documents {
            if (!visible || selected != Some(SidebarContent::Document(id)))
                && let Some((view, _)) = doc.reader()
            {
                view.cancel_primary();
            }
        }
    }

    pub(super) fn show(&mut self, content: SidebarContent, focus: PaneFocus) {
        if self.content != content {
            self.cancel_hidden_document_primary(/*visible*/ false);
        }
        self.focus_document_request = None;
        self.agents.pending_link = None;
        if self.content != content {
            self.showing_file_path = false;
        }
        match content {
            SidebarContent::Chat(id) => {
                self.add_tab(id);
                self.visit(SidebarTab::Chat(id));
            }
            SidebarContent::Chooser
                if self.focus.is_none() || matches!(self.content, SidebarContent::Chat(_)) =>
            {
                self.reset_choice();
            }
            SidebarContent::Document(id) => {
                let tab = SidebarTab::Document(id);
                self.add_document_tab(id);
                self.visit(tab);
            }
            SidebarContent::Subagents => {
                if !self.tabs.contains(&SidebarTab::Subagents) {
                    self.tabs.push(SidebarTab::Subagents);
                    self.tabs_revision += 1;
                }
                self.visit(SidebarTab::Subagents);
            }
            SidebarContent::Chooser => {}
        }
        if content != SidebarContent::Chooser || focus == PaneFocus::Main {
            self.opening = None;
        }
        if content != SidebarContent::Chooser
            && let Some(files) = &mut self.files
        {
            files.scan = None;
        }
        self.content = content;
        self.focus = Some(focus);
    }

    pub(super) fn clear_files(&mut self) {
        if self.files.take().is_some() {
            self.showing_file_path = false;
            self.choice = self.choice.min(self.chat_tabs().choice_count() - 1);
            self.tabs_revision += 1;
        }
    }

    pub(super) fn chat_tabs(&self) -> ChatTabs<'_> {
        ChatTabs {
            label_style: crate::thread_label::ThreadLabelStyle::Plain,
            discovery: &self.discovery,
            scroll: self.chooser_scroll,
            opening: self.opening.as_ref(),
            ids: &self.tabs,
            choices: &self.choices,
            agents: &self.agents.rows,
            agent_title: self.agents.detail.and_then(|id| {
                self.agents
                    .rows
                    .iter()
                    .find(|agent| agent.id == id)
                    .map(|agent| agent.name.as_str())
            }),
            latest_prompts: &self.latest_prompts,
            selected: self.visits.last().copied(),
            files: self
                .files
                .as_ref()
                .map_or(&[], |files| files.entries.as_slice()),
            files_cwd: self.files.as_ref().and_then(|files| files.cwd.as_ref()),
            file_preview_unavailable: self.file_preview_unavailable,
            files_loading: self
                .files
                .as_ref()
                .is_some_and(|files| files.scan.is_some()),
        }
    }

    pub(super) fn sync_agent_rows(&mut self, nav: &AgentNavigationState) {
        let rows = self.agents.chooser_rows(nav);
        let reordered = self
            .agents
            .rows
            .iter()
            .map(|agent| agent.id)
            .ne(rows.iter().map(|agent| agent.id));
        let selection = self.input_state().choice;
        self.agents.rows = rows;
        if reordered {
            self.restore_choice(selection);
            self.tabs_revision += 1;
        }
    }

    fn restore_choice(&mut self, selection: ChooserSelection) {
        let tabs = self.chat_tabs();
        let index = match selection {
            ChooserSelection::New => Some(0),
            ChooserSelection::Chat(id) => tabs.choice_index(id),
            ChooserSelection::Agent(id) => tabs.agent_index(id),
            ChooserSelection::File(offset) => {
                Some(tabs.file_start() + offset).filter(|&index| tabs.file(index).is_some())
            }
            ChooserSelection::Retry => {
                Some(tabs.choices.len() + 1).filter(|&index| tabs.retry_row(index))
            }
        };
        if let Some(index) = index {
            self.choice = index;
        } else {
            self.reset_choice();
        }
    }

    pub(super) fn set_discovery(&mut self, discovery: Discovery) {
        let selection = self.input_state().choice;
        self.discovery = discovery;
        if self.choices.is_empty()
            && matches!(self.discovery, Discovery::Failed(_))
            && selection == ChooserSelection::New
        {
            self.choice = 1;
        } else {
            self.restore_choice(selection);
        }
        self.tabs_revision += 1;
    }

    pub(super) fn update_saved_chats(&mut self, mut chats: Vec<SideChat>) {
        let selection = self.input_state().choice;
        let was_empty = self.choices.is_empty();
        for chat in &mut self.choices {
            if !self.tabs.contains(&SidebarTab::Chat(chat.id)) {
                continue;
            }
            if let Some(saved) = chats.iter_mut().find(|saved| saved.id == chat.id) {
                std::mem::swap(saved, chat);
            } else {
                chats.push(std::mem::replace(chat, chat.id.into()));
            }
        }
        let reordered = self
            .choices
            .iter()
            .map(|chat| chat.id)
            .ne(chats.iter().map(|chat| chat.id));
        self.choices = chats;
        if reordered {
            self.tabs_revision += 1;
            if selection == ChooserSelection::New && was_empty {
                self.reset_choice();
            } else {
                self.restore_choice(selection);
            }
        }
    }

    fn reset_choice(&mut self) {
        self.showing_file_path = false;
        self.chooser_scroll = ChooserScroll::default();
        self.choice = self
            .selected_chat()
            .and_then(|id| self.chat_tabs().choice_index(id))
            .unwrap_or(usize::from(!self.choices.is_empty()));
    }

    fn visit(&mut self, tab: SidebarTab) {
        self.visits.retain(|visited| *visited != tab);
        self.visits.push(tab);
    }

    pub(super) fn recent_tab(&self, excluding: Option<SidebarTab>) -> Option<SidebarTab> {
        self.visits
            .iter()
            .rev()
            .copied()
            .find(|tab| Some(*tab) != excluding)
    }

    pub(super) fn adjacent_tab(&self, step: isize) -> Option<SidebarTab> {
        let Some(selected) = self.recent_tab(/*excluding*/ None) else {
            return if step < 0 {
                self.tabs.last().copied()
            } else {
                self.tabs.first().copied()
            };
        };
        let index = self.tabs.iter().position(|id| *id == selected)?;
        let next = (index as isize + step).rem_euclid(self.tabs.len() as isize);
        Some(self.tabs[next as usize])
    }

    pub(super) fn selected_chat(&self) -> Option<ThreadId> {
        self.visits.iter().rev().find_map(|tab| match *tab {
            SidebarTab::Chat(id) => Some(id),
            SidebarTab::Document(_) | SidebarTab::Subagents => None,
        })
    }

    pub(super) fn active_document(&mut self) -> Option<&mut super::sidebar_document::Document> {
        let SidebarContent::Document(id) = self.content else {
            return None;
        };
        self.documents.get_mut(&id)
    }

    pub(super) fn document_title(&self, id: uuid::Uuid, width: usize) -> String {
        let Some(document) = self.documents.get(&id) else {
            return String::new();
        };
        let path = document.path.inferred_native_path_string();
        let parts: Vec<_> = path.rsplit(['/', '\\']).collect();
        let depth = self
            .documents
            .values()
            .filter(|other| other.path != document.path)
            .map(|other| {
                parts
                    .iter()
                    .copied()
                    .zip(other.path.inferred_native_path_string().rsplit(['/', '\\']))
                    .take_while(|(left, right)| left == right)
                    .count()
                    + 1
            })
            .max()
            .unwrap_or(1)
            .min(parts.len());
        let label = crate::line_truncation::document_label(&parts[..depth], width);
        let mut index = 1;
        let mut collision = false;
        for (&other_id, other) in &self.documents {
            if other_id == id {
                continue;
            }
            let other_path = other.path.inferred_native_path_string();
            let other_parts: Vec<_> = other_path.rsplit(['/', '\\']).take(depth).collect();
            if crate::line_truncation::document_label(&other_parts, width) == label {
                collision = true;
                index += usize::from(other_path < path);
            }
        }
        if !collision {
            return label;
        }
        let prefix = format!("{index}:");
        if prefix.len() > width {
            return label;
        }
        format!(
            "{prefix}{}",
            crate::line_truncation::document_label(&parts[..depth], width - prefix.len())
        )
    }

    pub(super) fn add_document_tab(&mut self, id: uuid::Uuid) {
        let tab = SidebarTab::Document(id);
        if !self.tabs.contains(&tab) {
            self.tabs.push(tab);
            self.tabs_revision += 1;
        }
    }

    pub(super) fn remove_document(&mut self, id: uuid::Uuid) {
        self.documents.remove(&id);
        let tab = SidebarTab::Document(id);
        self.tabs.retain(|other| *other != tab);
        self.visits.retain(|other| *other != tab);
        self.tabs_revision += 1;
    }

    pub(super) fn close_agents(&mut self) -> bool {
        self.agents.detail = None;
        self.tabs.retain(|tab| *tab != SidebarTab::Subagents);
        self.visits.retain(|tab| *tab != SidebarTab::Subagents);
        self.tabs_revision += 1;
        self.visible_content() == Some(SidebarContent::Subagents)
    }

    pub(super) fn input_state(&self) -> SidebarInputState {
        let tabs = self.chat_tabs();
        let choice = if let Some(id) = tabs.choice(self.choice) {
            ChooserSelection::Chat(id)
        } else if tabs.retry_row(self.choice) {
            ChooserSelection::Retry
        } else if let Some(offset) = tabs.file_offset(self.choice) {
            ChooserSelection::File(offset)
        } else if let Some(agent) = tabs.agent(self.choice) {
            ChooserSelection::Agent(agent.id)
        } else {
            ChooserSelection::New
        };
        SidebarInputState {
            revision: self.tabs_revision,
            selected_tab: tabs.selected,
            choice,
            scroll: self.chooser_scroll,
            content: self.content,
            showing_file_path: self.showing_file_path,
            file_path_scroll: self.file_path_scroll,
        }
    }

    fn add_tab(&mut self, id: ThreadId) {
        if !self.tabs.contains(&SidebarTab::Chat(id)) {
            if self.chat_tabs().choice_index(id).is_none() {
                let selection = self.input_state().choice;
                self.choices.push(id.into());
                self.restore_choice(selection);
            }
            self.tabs.push(SidebarTab::Chat(id));
            self.tabs_revision += 1;
        }
    }

    pub(super) fn chat_ready(&mut self, child: ThreadId) {
        self.creation = None;
        if !self.tabs.contains(&SidebarTab::Chat(child)) {
            self.add_tab(child);
            // A completed background fork must not replace the user's selection.
            self.visits.insert(0, SidebarTab::Chat(child));
        }
        if self.content == SidebarContent::Chooser
            && let Some(selected) = self.selected_chat()
        {
            self.content = SidebarContent::Chat(selected);
        }
    }

    pub(super) fn hide(&mut self) {
        self.cancel_hidden_document_primary(/*visible*/ false);
        self.focus_document_request = None;
        self.agents.pending_link = None;
        self.opening = None;
        self.showing_file_path = false;
        if let Some(files) = &mut self.files {
            files.scan = None;
        }
        self.focus = None;
    }

    pub(super) fn remove_chat(&mut self, id: ThreadId, storage: SideChatStorage) {
        let Some(index) = self
            .tabs
            .iter()
            .position(|tab| *tab == SidebarTab::Chat(id))
        else {
            return;
        };
        self.close.take_if(|close| close.target == id);
        self.tabs.remove(index);
        self.tabs_revision += 1;
        self.visits
            .retain(|visited| *visited != SidebarTab::Chat(id));
        if storage == SideChatStorage::Temporary {
            self.remove_choice(id);
        }
        if self.content == SidebarContent::Chat(id) {
            self.content = match self.recent_tab(/*excluding*/ None) {
                Some(SidebarTab::Chat(child)) => SidebarContent::Chat(child),
                Some(SidebarTab::Subagents) => SidebarContent::Subagents,
                Some(SidebarTab::Document(doc)) => SidebarContent::Document(doc),
                None => {
                    self.hide();
                    self.motion = PaneMotion::default();
                    SidebarContent::Chooser
                }
            };
        }
    }

    pub(super) fn remove_choice(&mut self, id: ThreadId) {
        if self
            .opening
            .as_ref()
            .is_some_and(|opening| opening.target.child == id)
        {
            self.opening = None;
        }
        if let Some(index) = self.chat_tabs().choice_index(id) {
            let selection = self.input_state().choice;
            self.choices.remove(index - 1);
            if selection == ChooserSelection::Chat(id) {
                self.choice = index.min(self.chat_tabs().choice_count() - 1);
            } else {
                self.restore_choice(selection);
            }
            self.tabs_revision += 1;
        }
    }

    pub(super) fn visible_content(&self) -> Option<SidebarContent> {
        self.focus.map(|_| self.content)
    }
}

impl App {
    pub(super) fn chooser_area(&self, tui: &Tui) -> Option<Rect> {
        self.chooser_focus()?;
        let screen: Rect = tui.terminal.last_known_screen_size.into();
        self.conversation_frame.panel_area().or_else(|| {
            match side_pane::split_layout(self, tui, screen) {
                Some(SidebarLayout::Panel { sidebar, .. }) => Some(sidebar),
                _ if screen.width < side_pane::MIN_SPLIT_WIDTH
                    && self.chooser_focus() == Some(PaneFocus::Sidebar) =>
                {
                    Some(screen)
                }
                _ => None,
            }
        })
    }

    pub(super) fn refresh_visible_side_chat_prompts(
        &mut self,
        tui: &Tui,
        server: &AppServerSession,
        parent: ThreadId,
        request: uuid::Uuid,
    ) {
        let area = (self.sidebar_parent() == Some(parent))
            .then(|| self.chooser_area(tui))
            .flatten();
        let Some(pane) = self.sidebars.get_mut(&Some(parent)) else {
            return;
        };
        let ids = area.map_or_else(Vec::new, |area| {
            pane.chat_tabs().visible_chat_ids(area, pane.choice)
        });
        if ids.is_empty() {
            pane.latest_prompts.clear();
            pane.set_discovery(Discovery::Idle);
            return;
        }
        let view = server.request_view();
        let tx = self.app_event_tx.clone();
        tokio::spawn(async move {
            let prompts = view.latest_side_chat_prompts(ids).await;
            tx.send(AppEvent::SavedSideChatsLoaded {
                backend_id: view.backend_id(),
                parent,
                request,
                result: Ok(SavedSideChatData::LatestPrompts(prompts)),
            });
        });
    }

    pub(super) fn track_side_chat_title(
        &mut self,
        id: ThreadId,
        notification: &ServerNotification,
    ) {
        let (name, prompt) = match notification {
            ServerNotification::ThreadNameUpdated(update) => (Some(&update.thread_name), None),
            ServerNotification::ItemCompleted(update) => match &update.item.payload {
                ThreadItemPayload::UserMessage { content, .. } => (None, Some(content)),
                _ => return,
            },
            _ => return,
        };
        let Some(chat) = self
            .sidebars
            .values_mut()
            .find_map(|pane| pane.choices.iter_mut().find(|chat| chat.id == id))
        else {
            return;
        };

        if let Some(name) = name {
            chat.name.clone_from(name);
        } else if chat.preview.is_empty()
            && let Some(preview) = prompt.and_then(|content| side_chat_prompt_preview(content))
        {
            chat.preview = preview.to_owned();
        } else {
            return;
        }
        self.chat_widget.request_redraw();
    }

    fn side_creation_pending(&self, owner: crate::backend_id::BackendId) -> bool {
        self.sidebars.values().any(|pane| {
            pane.creation
                .as_ref()
                .is_some_and(|creation| creation.owner == owner && creation.request().is_some())
        })
    }

    /// Before the fork returns its ID, ownership routing would drop the child's MCP statuses.
    /// Keep these events with their backend, then route them normally after registration.
    pub(super) fn defer_side_startup_notification(
        &self,
        event: &crate::app_server_session::AppServerSessionEvent,
        displayed: crate::backend_id::BackendId,
    ) -> bool {
        use codex_app_server_client::AppServerEvent;
        use codex_app_server_protocol::ServerNotification;
        let AppServerEvent::ServerNotification(notification) = &event.event else {
            return false;
        };
        let id = match notification.as_ref() {
            ServerNotification::ThreadStarted(started) if started.thread.ephemeral => {
                Some(started.thread.id.as_str())
            }
            ServerNotification::McpServerStatusUpdated(status) => status.thread_id.as_deref(),
            _ => None,
        };
        id.and_then(|id| ThreadId::from_string(id).ok())
            .is_some_and(|id| {
                (event.backend_id != displayed || !self.owns_thread_for_routing(id))
                    && self.side_creation_pending(event.backend_id)
            })
    }

    pub(super) fn sidebar_parent(&self) -> Option<ThreadId> {
        self.active_side_parent_thread_id()
            .or_else(|| {
                self.docked_agent()
                    .filter(|(_, agent)| Some(*agent) == self.current_displayed_thread_id())
                    .map(|(parent, _)| parent)
            })
            .or_else(|| self.current_displayed_thread_id())
    }

    pub(super) fn sidebar(&self) -> Option<&SidebarPane> {
        self.sidebars.get(&self.sidebar_parent())
    }

    pub(super) fn sidebar_mut(&mut self) -> &mut SidebarPane {
        self.sidebars.entry(self.sidebar_parent()).or_default()
    }

    pub(super) fn chooser_focus(&self) -> Option<PaneFocus> {
        let pane = self.sidebar()?;
        pane.focus
            .filter(|_| pane.content == SidebarContent::Chooser)
    }

    pub(super) fn panel_focus(&self) -> Option<PaneFocus> {
        let pane = self.sidebar()?;
        pane.focus.filter(|_| {
            !(matches!(pane.content, SidebarContent::Chat(_))
                || pane.content == SidebarContent::Subagents && pane.agents.detail.is_some())
        })
    }

    pub(super) fn sidebar_has_focus(&self) -> bool {
        self.active_side_parent_thread_id().is_some()
            || self
                .docked_agent()
                .is_some_and(|(_, agent)| self.current_displayed_thread_id() == Some(agent))
            || self.panel_focus() == Some(PaneFocus::Sidebar)
    }
}

#[derive(PartialEq)]
pub(super) enum SidebarLayout {
    Panel {
        main: Rect,
        sidebar: Rect,
        focus: PaneFocus,
    },
    Chat(SidePaneLayout),
}

impl SidebarLayout {
    pub(super) fn divider_area(&self) -> Option<Rect> {
        let (x, pane) = match self {
            Self::Panel { main, .. } => (main.right(), *main),
            Self::Chat(panes) => (panes.divider, panes.focused.union(panes.tab_strip)),
        };
        (!pane.is_empty()).then(|| Rect::new(x, pane.y, side_pane::GAP, pane.height))
    }
}

/// Step motion once per frame. Input and saved width calculations use the settled split.
pub(super) fn paint_layout(app: &mut App, tui: &Tui, area: Rect) -> Option<SidebarLayout> {
    let zoomed =
        app.zoomed_thread.is_some() && app.zoomed_thread == app.current_displayed_thread_id();
    let zoomed_document = zoomed
        && app.panel_focus() == Some(PaneFocus::Sidebar)
        && matches!(
            app.sidebar().and_then(SidebarPane::visible_content),
            Some(SidebarContent::Document(_))
        );
    let allowed = (!zoomed || zoomed_document)
        && app.side_retention_enabled(tui)
        && !app.conversation_frame.has_caller_inset();
    let animate = allowed
        && area.width >= side_pane::MIN_SPLIT_WIDTH
        && app.local_settings.tui.animations
        && app.pane_resize.is_none()
        && app.divider_drag.is_none();
    let parent = app.sidebar_parent();
    let pane = app.sidebars.get_mut(&parent)?;
    let open = pane.focus.is_some();
    let fraction = pane
        .motion
        .advance(open && allowed, animate.then(|| tui.frame_requester()));
    if !allowed || (!open && fraction == 0.0) {
        return None;
    }
    let content = pane.content;
    let focus = pane.focus.unwrap_or(PaneFocus::Main);
    let [main, sidebar] = if zoomed_document || area.width < side_pane::MIN_SPLIT_WIDTH {
        if focus == PaneFocus::Main {
            return None;
        }
        [Rect::default(), area]
    } else {
        side_pane::sidebar_areas(app, tui, area, fraction)?
    };
    content_layout(app, content, focus, [main, sidebar])
}

pub(super) fn content_layout(
    app: &App,
    content: SidebarContent,
    focus: PaneFocus,
    [main, sidebar]: [Rect; 2],
) -> Option<SidebarLayout> {
    match content {
        SidebarContent::Subagents if app.docked_agent().is_some() => {
            let (_, agent) = app.docked_agent()?;
            side_pane::pair_layout(app, agent, [main, sidebar]).map(SidebarLayout::Chat)
        }
        SidebarContent::Chooser | SidebarContent::Document(_) | SidebarContent::Subagents => (app
            .sidebar_chooser_available()
            && (focus == PaneFocus::Main || app.chat_widget.completion_can_yield_focus()))
        .then_some(SidebarLayout::Panel {
            main,
            sidebar,
            focus,
        }),
        SidebarContent::Chat(child) => {
            side_pane::pair_layout(app, child, [main, sidebar]).map(SidebarLayout::Chat)
        }
    }
}
