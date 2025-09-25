use std::collections::HashSet;
use std::ops::Deref;
use std::ops::DerefMut;
use std::sync::Arc;

use super::session_id::SessionId;
use crate::chatwidget::ChatWidget;
use crate::history_cell::HistoryCell;
use codex_protocol::mcp_protocol::ConversationId;

#[derive(Clone, Debug)]
pub struct ThreadOrigin {
    pub parent_index: usize,
    pub parent_conversation_id: Option<ConversationId>,
    pub parent_snapshot_len: usize,
}

/// Metadata tracked per chat session. Additional fields (e.g., unread counts) can
/// be added as the orchestration layer evolves.
pub(crate) struct SessionHandle {
    pub widget: ChatWidget,
    pub transcript_cells: Vec<Arc<dyn HistoryCell>>,
    pub title: String,
    pub display_title: Option<String>,
    pub conversation_id: Option<ConversationId>,
    pub unread_count: usize,
    pub origin: Option<ThreadOrigin>,
    pub session_id: SessionId,
    pub auto_name_kind: Option<AutoNameKind>,
}

impl SessionHandle {
    fn new(widget: ChatWidget, title: String, session_id: SessionId) -> Self {
        Self {
            widget,
            transcript_cells: Vec::new(),
            title,
            conversation_id: None,
            unread_count: 0,
            origin: None,
            session_id,
            auto_name_kind: None,
            display_title: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AutoNameKind {
    TopLevel,
    Child,
}

/// Manages one or more chat sessions. Currently only the active session is
/// visible, but additional sessions can be stored for future thread support.
pub(crate) struct SessionManager {
    sessions: Vec<SessionHandle>,
    active: usize,
}

impl SessionManager {
    pub(crate) fn single(widget: ChatWidget, session_id: SessionId) -> Self {
        Self {
            sessions: vec![SessionHandle::new(widget, "main".to_string(), session_id)],
            active: 0,
        }
    }

    fn active_handle(&self) -> &SessionHandle {
        &self.sessions[self.active]
    }

    fn active_handle_mut(&mut self) -> &mut SessionHandle {
        &mut self.sessions[self.active]
    }

    pub(crate) fn active_transcript(&self) -> &Vec<Arc<dyn HistoryCell>> {
        &self.active_handle().transcript_cells
    }

    pub(crate) fn active_transcript_mut(&mut self) -> &mut Vec<Arc<dyn HistoryCell>> {
        &mut self.active_handle_mut().transcript_cells
    }

    pub(crate) fn conversation_id(&self) -> Option<ConversationId> {
        self.active_handle()
            .conversation_id
            .or_else(|| self.active_handle().widget.conversation_id())
    }

    pub(crate) fn set_active_conversation_id(&mut self, id: ConversationId) {
        self.active_handle_mut().conversation_id = Some(id);
    }

    pub(crate) fn set_active_title(&mut self, title: String) {
        self.active_handle_mut().title = title;
    }

    pub(crate) fn len(&self) -> usize {
        self.sessions.len()
    }

    pub(crate) fn active_index(&self) -> usize {
        self.active
    }

    pub(crate) fn sessions(&self) -> &[SessionHandle] {
        &self.sessions
    }

    pub(crate) fn index_for_conversation_id(
        &self,
        conversation_id: &ConversationId,
    ) -> Option<usize> {
        self.sessions
            .iter()
            .position(|session| session.conversation_id.as_ref() == Some(conversation_id))
    }

    pub(crate) fn index_for_session_id(&self, session_id: SessionId) -> Option<usize> {
        self.sessions
            .iter()
            .position(|session| session.session_id == session_id)
    }

    pub(crate) fn thread_path_of(&self, idx: usize) -> Vec<String> {
        if self.sessions.is_empty() {
            return vec!["main".to_string()];
        }
        let mut parts: Vec<String> = Vec::new();
        let mut visited = HashSet::new();
        let mut current = Some(idx);
        while let Some(i) = current {
            if i >= self.sessions.len() || !visited.insert(i) {
                break;
            }
            let handle = &self.sessions[i];
            parts.push(handle.title.clone());
            current = handle
                .origin
                .as_ref()
                .and_then(|origin| self.parent_index_for_origin(origin));
        }
        if parts.is_empty() {
            parts.push("main".to_string());
        } else {
            parts.reverse();
        }
        parts
    }

    pub(crate) fn set_active_thread_path(&mut self, parts: Vec<String>) {
        if let Some(handle) = self.sessions.get_mut(self.active) {
            handle.widget.set_thread_path(parts);
        }
    }

    pub(crate) fn display_thread_path_of(&self, idx: usize) -> Vec<String> {
        self.thread_path_of(idx)
            .into_iter()
            .map(|title| self.label_for_title(&title))
            .collect()
    }

    pub(crate) fn display_thread_path_string_active(&self) -> String {
        self.display_thread_path_of(self.active).join("/")
    }

    pub(crate) fn display_label_for_index(&self, idx: usize) -> String {
        let Some(handle) = self.sessions.get(idx) else {
            return String::new();
        };
        self.label_for_title(&handle.title)
    }

    pub(crate) fn display_summary_for_index(&self, idx: usize) -> Option<&str> {
        self.sessions
            .get(idx)
            .and_then(|handle| handle.display_title.as_deref())
            .and_then(|s| if s.trim().is_empty() { None } else { Some(s) })
    }

    pub(crate) fn display_summary_for_index_owned(&self, idx: usize) -> Option<String> {
        self.display_summary_for_index(idx).map(|s| s.to_string())
    }

    fn label_for_title(&self, title: &str) -> String {
        if title == "main" {
            "#main".to_string()
        } else {
            title.to_string()
        }
    }

    pub(crate) fn update_display_title(&mut self, idx: usize, title: Option<String>) -> bool {
        if idx >= self.sessions.len() {
            return false;
        }
        let cleaned = title.and_then(|t| {
            let trimmed = t.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });
        let Some(handle) = self.sessions.get_mut(idx) else {
            return false;
        };
        if handle.display_title == cleaned {
            handle.auto_name_kind = None;
            return false;
        }
        handle.display_title = cleaned;
        handle.auto_name_kind = None;
        self.refresh_thread_paths();
        true
    }

    pub(crate) fn add_session(
        &mut self,
        widget: ChatWidget,
        title: String,
        session_id: SessionId,
    ) -> usize {
        self.sessions
            .push(SessionHandle::new(widget, title, session_id));
        self.sessions.len() - 1
    }

    pub(crate) fn switch_active(&mut self, idx: usize) -> bool {
        if idx >= self.sessions.len() {
            return false;
        }
        self.active = idx;
        self.sessions[self.active].unread_count = 0;
        true
    }

    pub(crate) fn session_mut(&mut self, idx: usize) -> Option<&mut SessionHandle> {
        self.sessions.get_mut(idx)
    }

    pub(crate) fn remove_session(&mut self, idx: usize) -> bool {
        if idx >= self.sessions.len() || self.sessions.len() == 1 {
            return false;
        }

        self.sessions.remove(idx);

        if self.active >= self.sessions.len() {
            self.active = self.sessions.len().saturating_sub(1);
        } else if self.active > idx {
            self.active -= 1;
        }

        for handle in &mut self.sessions {
            if let Some(origin) = &mut handle.origin {
                if origin.parent_index == idx {
                    origin.parent_index = self.active;
                } else if origin.parent_index > idx {
                    origin.parent_index -= 1;
                }
            }
        }

        self.refresh_thread_paths();

        true
    }

    pub(crate) fn refresh_thread_paths(&mut self) {
        let len = self.sessions.len();
        for i in 0..len {
            let display_path = self.display_thread_path_of(i);
            if let Some(handle) = self.sessions.get_mut(i) {
                handle.widget.set_thread_path(display_path);
            }
        }
    }

    pub(crate) fn parent_index_for(&self, origin: &ThreadOrigin) -> Option<usize> {
        self.parent_index_for_origin(origin)
    }

    pub(crate) fn thread_origin(
        parent_index: usize,
        parent_conversation_id: Option<ConversationId>,
        parent_snapshot_len: usize,
    ) -> ThreadOrigin {
        ThreadOrigin {
            parent_index,
            parent_conversation_id,
            parent_snapshot_len,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn increment_unread(&mut self, conversation_id: &ConversationId) {
        if let Some((idx, handle)) = self
            .sessions
            .iter_mut()
            .enumerate()
            .find(|(_, s)| s.conversation_id.as_ref() == Some(conversation_id))
            && idx != self.active
        {
            handle.unread_count = handle.unread_count.saturating_add(1);
        }
    }

    fn parent_index_for_origin(&self, origin: &ThreadOrigin) -> Option<usize> {
        if let Some(id) = origin.parent_conversation_id.as_ref()
            && let Some(idx) = self
                .sessions
                .iter()
                .position(|s| s.conversation_id.as_ref() == Some(id))
        {
            return Some(idx);
        }
        if origin.parent_index < self.sessions.len() {
            Some(origin.parent_index)
        } else {
            None
        }
    }
}

impl Deref for SessionManager {
    type Target = ChatWidget;

    fn deref(&self) -> &Self::Target {
        &self.active_handle().widget
    }
}

impl DerefMut for SessionManager {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.active_handle_mut().widget
    }
}

#[cfg(test)]
mod tests {
    use super::session_id::SessionId;
    use super::*;
    use crate::chatwidget::tests::make_chatwidget_manual_with_sender;

    #[test]
    fn thread_path_includes_parent() {
        let (widget, _tx, _rx, _op_rx) = make_chatwidget_manual_with_sender();
        let mut manager = SessionManager::single(widget, SessionId::new(0));

        let (child_widget, _tx2, _rx2, _op_rx2) = make_chatwidget_manual_with_sender();
        let child_idx = manager.add_session(child_widget, "child".to_string(), SessionId::new(1));
        if let Some(handle) = manager.session_mut(child_idx) {
            handle.origin = Some(ThreadOrigin {
                parent_index: 0,
                parent_conversation_id: None,
                parent_snapshot_len: 0,
            });
        }

        manager.switch_active(child_idx);
        let path = manager.thread_path_of(manager.active_index());
        assert_eq!(path, vec!["main".to_string(), "child".to_string()]);
    }

    #[test]
    fn display_path_includes_summary() {
        let (widget, _tx, _rx, _op_rx) = make_chatwidget_manual_with_sender();
        let mut manager = SessionManager::single(widget, SessionId::new(0));

        let (child_widget, _tx2, _rx2, _op_rx2) = make_chatwidget_manual_with_sender();
        let child_idx =
            manager.add_session(child_widget, "thread-1".to_string(), SessionId::new(1));
        if let Some(handle) = manager.session_mut(child_idx) {
            handle.origin = Some(ThreadOrigin {
                parent_index: 0,
                parent_conversation_id: None,
                parent_snapshot_len: 0,
            });
            handle.display_title = Some("Summarize failing tests".to_string());
        }

        manager.refresh_thread_paths();

        let display_path = manager.display_thread_path_of(child_idx);
        assert_eq!(
            display_path,
            vec!["#main".to_string(), "thread-1".to_string()]
        );

        let display_label = manager.display_label_for_index(child_idx);
        assert_eq!(display_label, "thread-1");
        assert_eq!(
            manager.display_summary_for_index(child_idx),
            Some("Summarize failing tests")
        );
    }

    #[test]
    fn update_display_title_resets_auto_name() {
        let (widget, _tx, _rx, _op_rx) = make_chatwidget_manual_with_sender();
        let mut manager = SessionManager::single(widget, SessionId::new(0));

        let (child_widget, _tx2, _rx2, _op_rx2) = make_chatwidget_manual_with_sender();
        let child_idx =
            manager.add_session(child_widget, "thread-1".to_string(), SessionId::new(1));
        if let Some(handle) = manager.session_mut(child_idx) {
            handle.origin = Some(ThreadOrigin {
                parent_index: 0,
                parent_conversation_id: None,
                parent_snapshot_len: 0,
            });
            handle.auto_name_kind = Some(AutoNameKind::Child);
        }

        assert!(
            manager.update_display_title(child_idx, Some("Summarize failing tests".to_string()))
        );
        assert!(
            manager
                .sessions()
                .get(child_idx)
                .and_then(|h| h.auto_name_kind)
                .is_none()
        );
        assert!(
            !manager.update_display_title(child_idx, Some("Summarize failing tests".to_string()))
        );
    }
}
