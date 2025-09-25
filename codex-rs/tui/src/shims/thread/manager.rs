use crate::chatwidget::ChatWidget;
use crate::history_cell::HistoryCell;
use codex_protocol::mcp_protocol::ConversationId;
use std::sync::Arc;

use super::session_id::SessionId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum AutoNameKind {
    TopLevel,
    Child,
}

#[derive(Clone, Debug)]
pub(crate) struct ThreadOrigin {
    pub parent_index: usize,
    #[allow(dead_code)]
    pub parent_conversation_id: Option<ConversationId>,
    #[allow(dead_code)]
    pub parent_snapshot_len: usize,
}

pub(crate) struct SessionHandle {
    pub widget: ChatWidget,
    #[allow(dead_code)]
    pub transcript_cells: Vec<Arc<dyn HistoryCell>>,
    pub title: String,
    pub display_title: Option<String>,
    pub conversation_id: Option<ConversationId>,
    #[allow(dead_code)]
    pub unread_count: usize,
    pub origin: Option<ThreadOrigin>,
    #[allow(dead_code)]
    pub session_id: SessionId,
    #[allow(dead_code)]
    pub auto_name_kind: Option<AutoNameKind>,
}

pub(crate) struct SessionManager {
    sessions: Vec<SessionHandle>,
    active_index: usize,
}

impl SessionManager {
    pub(crate) fn single(widget: ChatWidget, session_id: SessionId) -> Self {
        let handle = SessionHandle {
            // Leave empty by default; filled by tool call when provided.
            title: String::new(),
            display_title: Some("main".to_string()),
            conversation_id: None,
            session_id,
            auto_name_kind: None,
            unread_count: 0,
            origin: None,
            transcript_cells: Vec::new(),
            widget,
        };
        Self {
            sessions: vec![handle],
            active_index: 0,
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.sessions.len()
    }

    pub(crate) fn active_index(&self) -> usize {
        self.active_index
    }

    #[allow(dead_code)]
    pub(crate) fn index_for_session_id(&self, session_id: SessionId) -> Option<usize> {
        self.sessions
            .iter()
            .enumerate()
            .find_map(|(i, s)| (s.session_id == session_id).then_some(i))
    }

    #[allow(dead_code)]
    pub(crate) fn index_for_conversation_id(
        &self,
        conversation_id: &ConversationId,
    ) -> Option<usize> {
        self.sessions.iter().enumerate().find_map(|(i, s)| {
            s.conversation_id
                .as_ref()
                .is_some_and(|cid| cid == conversation_id)
                .then_some(i)
        })
    }

    pub(crate) fn session(&self, idx: usize) -> Option<&SessionHandle> {
        self.sessions.get(idx)
    }

    pub(crate) fn session_mut(&mut self, idx: usize) -> Option<&mut SessionHandle> {
        self.sessions.get_mut(idx)
    }

    pub(crate) fn parent_index_of(&self, idx: usize) -> Option<usize> {
        self.sessions
            .get(idx)
            .and_then(|h| h.origin.as_ref().map(|o| o.parent_index))
    }

    pub(crate) fn title_of(&self, idx: usize) -> Option<&str> {
        self.sessions
            .get(idx)
            .and_then(|h| (!h.title.is_empty()).then_some(h.title.as_str()))
    }

    #[allow(dead_code)]
    pub(crate) fn add_session(
        &mut self,
        widget: ChatWidget,
        _title: String,
        session_id: SessionId,
    ) -> usize {
        self.add_top_level_session(widget, session_id)
    }

    pub(crate) fn add_top_level_session(
        &mut self,
        widget: ChatWidget,
        session_id: SessionId,
    ) -> usize {
        // Root-level naming: main-1, main-2, ... (exclude index 0 which is "main")
        let roots = self.sessions.iter().filter(|h| h.origin.is_none()).count();
        // roots includes main; so next index label is roots (since main is first)
        let next_label = roots; // main-1 when roots==1 (main only)
        let display = format!("main-{next_label}");
        let handle = SessionHandle {
            // Leave empty by default; filled by tool call when provided.
            title: String::new(),
            display_title: Some(display),
            conversation_id: None,
            session_id,
            auto_name_kind: None,
            unread_count: 0,
            origin: None,
            transcript_cells: Vec::new(),
            widget,
        };
        self.sessions.push(handle);
        self.sessions.len() - 1
    }

    pub(crate) fn add_child_session(
        &mut self,
        parent_index: usize,
        widget: ChatWidget,
        session_id: SessionId,
    ) -> usize {
        // Child naming: {parent_display}.N (N = count of existing children + 1)
        let parent_display = self
            .sessions
            .get(parent_index)
            .and_then(|h| h.display_title.clone())
            .unwrap_or_else(|| "main".to_string());
        let child_count = self
            .sessions
            .iter()
            .filter(|h| h.origin.as_ref().map(|o| o.parent_index) == Some(parent_index))
            .count();
        let display = format!("{parent_display}.{}", child_count + 1);
        let origin = ThreadOrigin {
            parent_index,
            parent_conversation_id: None,
            parent_snapshot_len: 0,
        };
        // Inherit the parent's title for a forked thread; otherwise leave empty.
        let inherited_title = self
            .sessions
            .get(parent_index)
            .map(|h| h.title.clone())
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_default();

        let handle = SessionHandle {
            title: inherited_title,
            display_title: Some(display),
            conversation_id: None,
            session_id,
            auto_name_kind: None,
            unread_count: 0,
            origin: Some(origin),
            transcript_cells: Vec::new(),
            widget,
        };
        self.sessions.push(handle);
        self.sessions.len() - 1
    }

    #[allow(dead_code)]
    pub(crate) fn next_session_id(&self) -> SessionId {
        SessionId::new(self.sessions.len() as u64 + 1)
    }

    #[allow(dead_code)]
    pub(crate) fn remove_session(&mut self, idx: usize) -> Option<SessionHandle> {
        if idx < self.sessions.len() {
            let removed = self.sessions.remove(idx);
            if self.active_index >= self.sessions.len() {
                self.active_index = self.sessions.len().saturating_sub(1);
            }
            Some(removed)
        } else {
            None
        }
    }

    pub(crate) fn switch_active(&mut self, idx: usize) -> bool {
        if idx < self.sessions.len() && self.active_index != idx {
            self.active_index = idx;
            true
        } else {
            false
        }
    }

    pub(crate) fn conversation_id(&self) -> Option<ConversationId> {
        self.sessions
            .get(self.active_index)
            .and_then(|s| s.conversation_id)
    }

    #[allow(dead_code)]
    pub(crate) fn sessions(&self) -> &[SessionHandle] {
        &self.sessions
    }
}
