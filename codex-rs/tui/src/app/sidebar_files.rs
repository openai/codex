//! Inventory files and links for the owning conversation without moving its history cursor.

use super::thread_events::ThreadBufferedEvent;
use super::*;
use crate::app_server_session::ThreadParamsMode;
use crate::markdown_render::FileLink;
use crate::terminal_hyperlinks::ReaderLink;
use codex_app_server_protocol::PatchApplyStatus;
use codex_app_server_protocol::PatchChangeKind;
use codex_app_server_protocol::UserInput;
use codex_utils_path_uri::LegacyAppPathString;
use codex_utils_path_uri::PathUri;
use tokio_util::task::AbortOnDropHandle;

const LINK_SCAN_WIDTH: u16 = 100;

pub(super) struct Files {
    pub(super) entries: Vec<FileLink>,
    by_destination: HashMap<String, usize>,
    // Cached paths and reader opens follow the portion confirmed by the current scan.
    confirmed: usize,
    pub(super) cwd: Option<PathUri>,
    inspect_local_paths: bool,
    pub(super) scan: Option<(uuid::Uuid, AbortOnDropHandle<()>)>,
    // Advance only after a successful page so reopening the chooser can retry a failed request.
    pub(super) next: Option<String>,
    needs_history_cursor: bool,
    seen_cursors: HashSet<String>,
    older_turn: Vec<codex_app_server_protocol::ThreadItemEntry>,
    review_mode: bool,
}

impl Files {
    fn new(cwd: Option<PathUri>, next: Option<String>, inspect_local_paths: bool) -> Self {
        Self {
            entries: Vec::new(),
            by_destination: HashMap::new(),
            confirmed: 0,
            cwd,
            inspect_local_paths,
            scan: None,
            next,
            needs_history_cursor: false,
            seen_cursors: HashSet::new(),
            older_turn: Vec::new(),
            review_mode: false,
        }
    }

    pub(super) fn connection_replaced(&mut self) {
        self.scan = None;
        self.needs_history_cursor = true;
        self.confirmed = 0;
        self.seen_cursors.clear();
        self.older_turn.clear();
        self.review_mode = false;
    }

    pub(super) fn apply_page(
        &mut self,
        page: codex_app_server_protocol::ThreadItemsListResponse,
    ) -> bool {
        let cursor = self.next.take();
        self.seen_cursors.extend(cursor);
        self.next = page
            .next_cursor
            .filter(|next| !self.seen_cursors.contains(next));
        let mut entries = Vec::new();
        for item in page.data {
            if self
                .older_turn
                .last()
                .is_some_and(|last| last.turn_id != item.turn_id)
            {
                self.finish_older_turn(&mut entries);
            }
            if matches!(
                item.item.payload,
                ThreadItemPayload::AgentMessage { .. }
                    | ThreadItemPayload::UserMessage { .. }
                    | ThreadItemPayload::Plan { .. }
                    | ThreadItemPayload::FileChange { .. }
                    | ThreadItemPayload::ImageGeneration(_)
                    | ThreadItemPayload::EnteredReviewMode { .. }
                    | ThreadItemPayload::ExitedReviewMode { .. }
            ) {
                self.older_turn.push(item);
            }
        }
        if self.next.is_none() {
            self.finish_older_turn(&mut entries);
        }
        self.extend_page(entries)
    }

    fn finish_older_turn(&mut self, entries: &mut Vec<FileLink>) {
        let handoff = self
            .older_turn
            .iter()
            .rposition(|entry| is_voice_handoff(&entry.item.payload));
        for (index, entry) in self.older_turn.drain(..).enumerate() {
            let item = &entry.item.payload;
            match item {
                ThreadItemPayload::ExitedReviewMode { .. } => self.review_mode = true,
                ThreadItemPayload::EnteredReviewMode { .. } => self.review_mode = false,
                ThreadItemPayload::UserMessage { .. } if self.review_mode => {}
                _ if handoff.is_some_and(|handoff| index < handoff)
                    && crate::chatwidget::is_private_realtime_agent_item(item) => {}
                _ => newest_item_entries(self.cwd.as_ref(), item, entries),
            }
        }
    }

    pub(super) fn extend(&mut self, entries: impl IntoIterator<Item = FileLink>) -> bool {
        let mut changed = false;
        for entry in entries {
            if let Some(&index) = self.by_destination.get(&entry.path) {
                let old = &mut self.entries[index];
                if old.location.is_none() && entry.location.is_some() {
                    old.location = entry.location;
                    changed = true;
                }
            } else {
                let path = Path::new(&entry.path);
                // Keep unresolved links, including paths on another host.
                if self.inspect_local_paths && path.is_absolute() && path.is_dir() {
                    continue;
                }
                self.by_destination
                    .insert(entry.path.clone(), self.entries.len());
                self.entries.push(entry);
                changed = true;
            }
        }
        changed
    }

    fn promote(&mut self, entries: impl IntoIterator<Item = FileLink>) -> bool {
        self.merge_history(/*start*/ 0, entries)
    }

    pub(super) fn extend_page(&mut self, entries: Vec<FileLink>) -> bool {
        if self.confirmed == self.entries.len() {
            let changed = self.extend(entries);
            self.confirmed = self.entries.len();
            return changed;
        }
        self.merge_history(self.confirmed, entries)
    }

    fn merge_history(&mut self, start: usize, entries: impl IntoIterator<Item = FileLink>) -> bool {
        let previous = self.entries.split_off(start);
        let confirmed = self.confirmed - start;
        self.by_destination.retain(|_, index| *index < start);
        self.extend(entries);
        for (offset, old) in previous.iter().enumerate() {
            if offset == confirmed {
                self.confirmed = self.entries.len();
            }
            if let Some(&index) = self.by_destination.get(&old.path) {
                if self.entries[index].location.is_none() {
                    self.entries[index].location = old.location;
                }
            } else {
                self.by_destination
                    .insert(old.path.clone(), self.entries.len());
                self.entries.push(old.clone());
            }
        }
        if confirmed == previous.len() {
            self.confirmed = self.entries.len();
        }
        self.entries[start..] != previous
    }
}

fn promote_files(
    pane: &mut super::sidebar::SidebarPane,
    entries: impl IntoIterator<Item = FileLink>,
) -> bool {
    let selected = pane
        .chat_tabs()
        .file(pane.choice)
        .map(|file| file.path.clone());
    let Some(files) = pane.files.as_mut() else {
        return false;
    };
    if !files.promote(entries) {
        return false;
    }
    pane.restore_file_selection(selected.as_deref());
    pane.tabs_revision += 1;
    true
}

impl super::sidebar::SidebarPane {
    pub(super) fn restore_file_selection(&mut self, path: Option<&str>) {
        if let Some(index) = path.and_then(|path| self.files.as_ref()?.by_destination.get(path)) {
            self.choice = self.chat_tabs().file_start() + index;
        }
    }
}

fn resolve(cwd: Option<&PathUri>, mut link: FileLink) -> Option<FileLink> {
    if link.path.is_empty() || link.path.len() > 4096 || link.path.chars().any(char::is_control) {
        return None;
    }
    let Some(cwd) = cwd else {
        return Some(link);
    };
    let path = LegacyAppPathString::from_string(&link.path)
        .resolve_against(cwd, /*user_home_dir*/ None)
        .ok()?;
    if path.to_url().has_host() {
        return None;
    }
    link.path = path.inferred_native_path_string();
    (link.path.len() <= 4096).then_some(link)
}

fn is_voice_handoff(item: &ThreadItemPayload) -> bool {
    matches!(item, ThreadItemPayload::UserMessage { content, .. }
        if crate::chatwidget::realtime_delegation_input(content).is_some())
}

fn item_entries(cwd: Option<&PathUri>, item: &ThreadItemPayload, entries: &mut Vec<FileLink>) {
    let mut found = |entry| entries.extend(resolve(cwd, entry));
    let mut markdown = |text: &str| {
        for file in crate::markdown_render::file_links(text, /*cwd*/ None) {
            found(file);
        }
    };
    match item {
        ThreadItemPayload::AgentMessage { text, .. } => markdown(text),
        ThreadItemPayload::Plan { text, .. } => markdown(text),
        ThreadItemPayload::UserMessage { content, .. }
            if crate::chatwidget::realtime_delegation_input(content).is_none() =>
        {
            for input in content {
                if let UserInput::Text { text, .. } = input {
                    markdown(text);
                }
            }
        }
        ThreadItemPayload::FileChange {
            changes,
            status: PatchApplyStatus::Completed,
            ..
        } => {
            for change in changes {
                let path = match &change.kind {
                    PatchChangeKind::Delete => continue,
                    PatchChangeKind::Update {
                        move_path: Some(path),
                    } => path.to_str(),
                    PatchChangeKind::Add | PatchChangeKind::Update { move_path: None } => {
                        Some(change.path.as_str())
                    }
                };
                if let Some(path) = path {
                    found(FileLink {
                        path: path.to_owned(),
                        location: None,
                    });
                }
            }
        }
        ThreadItemPayload::ImageGeneration(image) if image.status == "completed" => {
            for path in image.saved_path.iter().filter_map(|path| path.to_str()) {
                found(FileLink {
                    path: path.to_owned(),
                    location: None,
                });
            }
        }
        _ => {}
    }
}

fn newest_item_entries(
    cwd: Option<&PathUri>,
    item: &ThreadItemPayload,
    entries: &mut Vec<FileLink>,
) {
    let start = entries.len();
    item_entries(cwd, item, entries);
    entries[start..].reverse();
}

impl App {
    pub(super) async fn refresh_sidebar_files(
        &mut self,
        server: &AppServerSession,
        parent: ThreadId,
    ) {
        // Until replay finishes, the rendered cells can still belong to the previous thread.
        if server.thread_params_mode() == ThreadParamsMode::Cloud
            || self.pending_thread_switch_resets != 0
            || self.current_displayed_thread_id() != Some(parent)
        {
            return;
        }
        let Some(channel) = self.thread_event_channels.get(&parent) else {
            return;
        };
        let mut entries = Vec::new();
        let (cwd, inspect_local_paths, file_preview_unavailable) = {
            let store = channel.store.lock().await;
            let Some(session) = &store.session else {
                return;
            };
            let inspect_local_paths = server.thread_params_mode() == ThreadParamsMode::Embedded
                && session
                    .primary_environment_cwd
                    .as_ref()
                    .is_none_or(|environment| environment.is_server_local);
            let cwd = if inspect_local_paths {
                Some(session.cwd.clone())
            } else {
                session
                    .primary_environment_cwd
                    .as_ref()
                    .filter(|environment| environment.is_server_local)
                    .and_then(|environment| environment.path.to_inferred_path_uri())
            };
            for event in store.buffer.iter().rev() {
                if let ThreadBufferedEvent::Notification(event) = event
                    && let ServerNotification::ItemCompleted(item) = event.as_ref()
                    // User messages already contribute through their visible transcript cells.
                    && !matches!(item.item.payload, ThreadItemPayload::UserMessage { .. })
                    && (!store.is_realtime_delegated_turn(&item.turn_id)
                        || !crate::chatwidget::is_private_realtime_agent_item(&item.item.payload))
                {
                    newest_item_entries(cwd.as_ref(), &item.item.payload, &mut entries);
                }
            }
            let hidden = super::history_pagination::hidden_review_item_ids(&store.turns);
            for turn in store.turns.iter().rev() {
                let has_handoff = turn.item_payloads().any(is_voice_handoff);
                let mut turn_entries = Vec::new();
                let mut voice = store.is_realtime_delegated_turn(&turn.id) && !has_handoff;
                for item in turn.item_payloads() {
                    voice |= is_voice_handoff(item);
                    if !hidden.contains(&(turn.id.as_str(), item.id()))
                        && (!voice || !crate::chatwidget::is_private_realtime_agent_item(item))
                    {
                        item_entries(cwd.as_ref(), item, &mut turn_entries);
                    }
                }
                entries.extend(turn_entries.into_iter().rev());
            }
            let file_preview_unavailable =
                if session.windows_sandbox_host != WindowsSandboxHost::Local {
                    Some("Requires workspace on server host")
                } else if server.app_server_platform_family() == Some("windows")
                    || (cfg!(windows) && !server.uses_remote_workspace())
                    || cwd.as_ref().and_then(PathUri::infer_path_convention)
                        != Some(codex_utils_path_uri::PathConvention::Posix)
                {
                    Some("Preview unavailable on Windows")
                } else {
                    None
                };
            (cwd, inspect_local_paths, file_preview_unavailable)
        };
        let active = self
            .chat_widget
            .active_cell_transcript_hyperlink_lines(LINK_SCAN_WIDTH)
            .unwrap_or_default();
        let visible_links = self
            .transcript_cells
            .iter()
            // Skip tool output, which can require rendering and highlighting an entire patch.
            .filter(|cell| {
                let cell = cell.as_any();
                cell.is::<crate::history_cell::AgentMarkdownCell>()
                    || cell.is::<crate::history_cell::AgentMessageCell>()
                    || cell.is::<crate::history_cell::UserHistoryCell>()
                    || cell.is::<crate::history_cell::ProposedPlanCell>()
                    || cell.is::<crate::history_cell::ProposedPlanStreamCell>()
                    || cell.is::<crate::history_cell::DelegatedMessageCell>()
            })
            .flat_map(|cell| cell.transcript_hyperlink_lines(LINK_SCAN_WIDTH))
            .flat_map(|line| line.hyperlinks)
            .filter_map(|link| {
                let ReaderLink::Document(file) = link.sidebar_destination()? else {
                    return None;
                };
                resolve(cwd.as_ref(), file)
            });
        let active_links = active
            .into_iter()
            .flat_map(|line| line.hyperlinks)
            .filter_map(|link| {
                let ReaderLink::Document(file) = link.sidebar_destination()? else {
                    return None;
                };
                resolve(cwd.as_ref(), file)
            });
        let pane = self.sidebars.entry(Some(parent)).or_default();
        pane.file_preview_unavailable = file_preview_unavailable;
        if pane.files.as_ref().is_some_and(|files| files.cwd != cwd) {
            pane.clear_files();
        }
        let files = pane.files.get_or_insert_with(|| {
            Files::new(
                cwd.clone(),
                server.older_item_cursor(parent).map(str::to_owned),
                inspect_local_paths,
            )
        });
        if std::mem::take(&mut files.needs_history_cursor) {
            // Resume may have missed files between the cached and newly hydrated windows.
            files.next = server.older_item_cursor(parent).map(str::to_owned);
        }
        let opened = pane
            .opened_files
            .iter()
            .map(|path| FileLink {
                path: path.inferred_native_path_string(),
                location: None,
            })
            .collect::<Vec<_>>();
        promote_files(
            pane,
            active_links.rev().chain(entries).chain(visible_links.rev()),
        );
        let Some(files) = pane.files.as_mut() else {
            return;
        };
        if files.extend(opened) {
            pane.tabs_revision += 1;
        }
        self.start_sidebar_files_scan(server, parent);
    }

    pub(super) fn start_sidebar_files_scan(&mut self, server: &AppServerSession, parent: ThreadId) {
        // Check the owner again after every page before issuing the next one.
        if self.current_displayed_thread_id() != Some(parent) || self.chooser_focus().is_none() {
            return;
        }
        let Some(files) = self
            .sidebars
            .get_mut(&Some(parent))
            .and_then(|pane| pane.files.as_mut())
        else {
            return;
        };
        if files.scan.is_some() {
            return;
        }
        let Some(cursor) = files.next.clone() else {
            return;
        };
        let request = uuid::Uuid::new_v4();
        let tx = self.app_event_tx.clone();
        let mut view = server.request_view();
        let task = tokio::spawn(async move {
            let page = view
                .thread_items_page(
                    parent,
                    /*turn_id*/ None,
                    Some(cursor),
                    /*limit*/ 100,
                )
                .await
                .ok();
            tx.send(AppEvent::SidebarFilesLoaded {
                backend_id: view.backend_id(),
                parent,
                request,
                page,
            });
        });
        files.scan = Some((request, AbortOnDropHandle::new(task)));
    }

    pub(super) fn notice_sidebar_file_item(
        &mut self,
        thread: ThreadId,
        notification: &ServerNotification,
    ) {
        if let ServerNotification::ItemCompleted(item) = notification
            && matches!(
                item.item.payload,
                ThreadItemPayload::FileChange { .. } | ThreadItemPayload::ImageGeneration(_)
            )
            && let Some(pane) = self.sidebars.get_mut(&Some(thread))
            && let Some(files) = pane.files.as_ref()
        {
            let mut entries = Vec::new();
            newest_item_entries(files.cwd.as_ref(), &item.item.payload, &mut entries);
            if !entries.is_empty() && promote_files(pane, entries) {
                self.chat_widget.request_redraw();
            }
        }
    }
}

#[cfg(test)]
#[path = "sidebar_files_tests.rs"]
mod tests;
