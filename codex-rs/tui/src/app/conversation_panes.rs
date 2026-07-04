//! Conversation-local state for the application-owned conversation surface.
//!
//! This first stage places the current conversation behind a pane boundary. Later split-screen
//! layers can add another pane without returning these fields to application-global state.

use std::ops::Deref;
use std::ops::DerefMut;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use codex_protocol::ThreadId;
use tokio::sync::mpsc;

use super::InitialHistoryReplayBuffer;
use super::owned_screen::OwnedScreen;
use super::thread_events::ThreadBufferedEvent;
use crate::app_event::PaneSlot;
use crate::chatwidget::ChatWidget;
use crate::file_search::FileSearchManager;
use crate::history_cell::HistoryCell;
use crate::transcript_reflow::TranscriptReflowState;

pub(super) struct ConversationPaneInit {
    pub(super) chat_widget: ChatWidget,
    pub(super) file_search: FileSearchManager,
    pub(super) owned_screen: Option<OwnedScreen>,
}

pub(crate) struct ConversationPane {
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
    fn new(init: ConversationPaneInit) -> Result<Self, ConversationPaneInit> {
        if init
            .chat_widget
            .conversation_origin()
            .is_some_and(|origin| origin.pane != PaneSlot::Parent)
        {
            return Err(init);
        }

        Ok(Self {
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
}

impl ConversationPanes {
    pub(super) fn new_parent(init: ConversationPaneInit) -> Result<Self, ConversationPaneInit> {
        Ok(Self {
            parent: ConversationPane::new(init)?,
        })
    }

    pub(super) fn selected_mut(&mut self) -> &mut ConversationPane {
        &mut self.parent
    }
}

impl Deref for ConversationPanes {
    type Target = ConversationPane;

    fn deref(&self) -> &Self::Target {
        &self.parent
    }
}

impl DerefMut for ConversationPanes {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.parent
    }
}
