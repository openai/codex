//! Conversation-local state for the application-owned conversation surface.
//!
//! The owned screen supports a fixed parent pane and an optional side pane. Focus controls where
//! user input goes, while a temporary dispatch slot lets asynchronous events update their origin
//! pane without changing focus.

use std::ops::Deref;
use std::ops::DerefMut;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use codex_protocol::ThreadId;
use crossterm::event::KeyCode;
use tokio::sync::mpsc;

use super::InitialHistoryReplayBuffer;
use super::owned_screen::OwnedScreen;
use super::thread_events::ThreadBufferedEvent;
use crate::app_event::ConversationOrigin;
use crate::app_event::PaneSlot;
use crate::chatwidget::ChatWidget;
use crate::file_search::FileSearchManager;
use crate::history_cell::HistoryCell;
use crate::key_hint;
use crate::key_hint::KeyBinding;
use crate::transcript_reflow::TranscriptReflowState;

pub(super) fn parent_pane_shortcut() -> KeyBinding {
    key_hint::alt(KeyCode::Char('1'))
}

pub(super) fn side_pane_shortcut() -> KeyBinding {
    key_hint::alt(KeyCode::Char('2'))
}

pub(super) struct ConversationPaneInit {
    pub(super) chat_widget: ChatWidget,
    pub(super) file_search: FileSearchManager,
    pub(super) owned_screen: Option<OwnedScreen>,
}

pub(crate) struct ConversationPane {
    slot: PaneSlot,
    pub(super) chat_widget: ChatWidget,
    pub(super) file_search: FileSearchManager,
    pub(crate) transcript_cells: Vec<Arc<dyn HistoryCell>>,
    pub(super) owned_screen: Option<OwnedScreen>,
    pub(super) transcript_reflow: TranscriptReflowState,
    pub(super) initial_history_replay_buffer: Option<InitialHistoryReplayBuffer>,
    pub(super) commit_anim_running: Arc<AtomicBool>,
    pub(super) active_thread_id: Option<ThreadId>,
    pub(super) active_thread_rx: Option<mpsc::Receiver<ThreadBufferedEvent>>,
}

impl ConversationPane {
    fn new(init: ConversationPaneInit, slot: PaneSlot) -> Result<Self, ConversationPaneInit> {
        if init
            .chat_widget
            .conversation_origin()
            .is_some_and(|origin| origin.pane != slot)
        {
            return Err(init);
        }

        Ok(Self {
            slot,
            chat_widget: init.chat_widget,
            file_search: init.file_search,
            transcript_cells: Vec::new(),
            owned_screen: init.owned_screen,
            transcript_reflow: TranscriptReflowState::default(),
            initial_history_replay_buffer: None,
            commit_anim_running: Arc::new(AtomicBool::new(/*v*/ false)),
            active_thread_id: None,
            active_thread_rx: None,
        })
    }

    pub(super) fn origin(&self) -> Option<ConversationOrigin> {
        self.chat_widget
            .conversation_origin()
            .filter(|origin| origin.pane == self.slot)
    }

    fn thread_id(&self) -> Option<ThreadId> {
        self.active_thread_id.or(self.chat_widget.thread_id())
    }

    pub(super) fn attach_thread(
        &mut self,
        thread_id: ThreadId,
        receiver: Option<mpsc::Receiver<ThreadBufferedEvent>>,
    ) {
        self.active_thread_id = Some(thread_id);
        self.active_thread_rx = receiver;
    }

    pub(super) fn take_thread_receiver(&mut self) -> Option<mpsc::Receiver<ThreadBufferedEvent>> {
        self.active_thread_rx.take()
    }

    pub(super) fn clear_thread(&mut self) {
        self.active_thread_id = None;
        self.active_thread_rx = None;
    }
}

impl Deref for ConversationPane {
    type Target = ChatWidget;

    fn deref(&self) -> &Self::Target {
        &self.chat_widget
    }
}

impl DerefMut for ConversationPane {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.chat_widget
    }
}

pub(crate) struct ConversationPanes {
    parent: ConversationPane,
    side: Option<ConversationPane>,
    focused: PaneSlot,
    dispatch: Option<PaneSlot>,
}

// Rendering and `/side` lifecycle consume the remaining APIs in the next stack layers.
#[allow(dead_code)]
impl ConversationPanes {
    pub(super) fn new_parent(init: ConversationPaneInit) -> Result<Self, ConversationPaneInit> {
        Ok(Self {
            parent: ConversationPane::new(init, PaneSlot::Parent)?,
            side: None,
            focused: PaneSlot::Parent,
            dispatch: None,
        })
    }

    pub(super) fn focused_slot(&self) -> PaneSlot {
        self.focused
    }

    pub(super) fn has_side(&self) -> bool {
        self.side.is_some()
    }

    pub(super) fn focus(&mut self, slot: PaneSlot) -> bool {
        if self.by_slot(slot).is_none() {
            return false;
        }
        self.focused = slot;
        true
    }

    pub(super) fn install_side(
        &mut self,
        init: ConversationPaneInit,
    ) -> Result<Option<ConversationPane>, ConversationPaneInit> {
        let pane = ConversationPane::new(init, PaneSlot::Side)?;
        Ok(self.side.replace(pane))
    }

    pub(super) fn take_side(&mut self) -> Option<ConversationPane> {
        if self.focused == PaneSlot::Side {
            self.focused = PaneSlot::Parent;
        }
        if self.dispatch == Some(PaneSlot::Side) {
            self.dispatch = None;
        }
        if let Some(side) = &self.side {
            side.commit_anim_running
                .store(/*val*/ false, Ordering::Release);
        }
        self.side.take()
    }

    pub(super) fn by_slot(&self, slot: PaneSlot) -> Option<&ConversationPane> {
        match slot {
            PaneSlot::Parent => Some(&self.parent),
            PaneSlot::Side => self.side.as_ref(),
        }
    }

    pub(super) fn by_slot_mut(&mut self, slot: PaneSlot) -> Option<&mut ConversationPane> {
        match slot {
            PaneSlot::Parent => Some(&mut self.parent),
            PaneSlot::Side => self.side.as_mut(),
        }
    }

    pub(super) fn by_origin(&self, origin: ConversationOrigin) -> Option<&ConversationPane> {
        self.by_slot(origin.pane)
            .filter(|pane| pane.origin() == Some(origin))
    }

    pub(super) fn by_thread_id(&self, thread_id: ThreadId) -> Option<&ConversationPane> {
        if self.parent.thread_id() == Some(thread_id) {
            return Some(&self.parent);
        }
        self.side
            .as_ref()
            .filter(|pane| pane.thread_id() == Some(thread_id))
    }

    pub(super) fn by_thread_id_mut(
        &mut self,
        thread_id: ThreadId,
    ) -> Option<&mut ConversationPane> {
        if self.parent.thread_id() == Some(thread_id) {
            return Some(&mut self.parent);
        }
        self.side
            .as_mut()
            .filter(|pane| pane.thread_id() == Some(thread_id))
    }

    pub(super) fn contains_thread(&self, thread_id: ThreadId) -> bool {
        self.by_thread_id(thread_id).is_some()
    }

    pub(super) fn dispatch_to(&mut self, origin: ConversationOrigin) -> bool {
        if self.by_origin(origin).is_none() {
            return false;
        }
        self.dispatch = Some(origin.pane);
        true
    }

    pub(super) fn clear_dispatch(&mut self) -> Option<PaneSlot> {
        self.dispatch.take()
    }

    pub(super) fn finish_dispatch(&mut self, origin: ConversationOrigin) -> bool {
        match self.clear_dispatch() {
            Some(slot) => slot == origin.pane,
            None => self.by_origin(origin).is_none(),
        }
    }

    pub(super) fn selected_mut(&mut self) -> &mut ConversationPane {
        let slot = self.dispatch.unwrap_or(self.focused);
        self.by_slot_mut(slot)
            .expect("focused or dispatch pane must be installed")
    }

    pub(super) fn has_thread_event_receiver(&self) -> bool {
        self.parent.active_thread_rx.is_some()
            || self
                .side
                .as_ref()
                .is_some_and(|pane| pane.active_thread_rx.is_some())
    }

    /// Waits fairly for an event from either installed pane.
    pub(super) async fn recv_thread_event(&mut self) -> (PaneSlot, Option<ThreadBufferedEvent>) {
        let parent_rx = self.parent.active_thread_rx.as_mut();
        let side_rx = self
            .side
            .as_mut()
            .and_then(|pane| pane.active_thread_rx.as_mut());

        match (parent_rx, side_rx) {
            (Some(parent_rx), Some(side_rx)) => {
                tokio::select! {
                    event = parent_rx.recv() => (PaneSlot::Parent, event),
                    event = side_rx.recv() => (PaneSlot::Side, event),
                }
            }
            (Some(parent_rx), None) => (PaneSlot::Parent, parent_rx.recv().await),
            (None, Some(side_rx)) => (PaneSlot::Side, side_rx.recv().await),
            (None, None) => (self.focused, None),
        }
    }

    fn selected(&self) -> &ConversationPane {
        let slot = self.dispatch.unwrap_or(self.focused);
        self.by_slot(slot)
            .expect("focused or dispatch pane must be installed")
    }
}

impl Deref for ConversationPanes {
    type Target = ConversationPane;

    fn deref(&self) -> &Self::Target {
        self.selected()
    }
}

impl DerefMut for ConversationPanes {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.selected_mut()
    }
}

#[cfg(test)]
#[path = "conversation_panes_tests.rs"]
mod tests;
