//! Show model-requested document tabs when their source conversation is visible.

use super::side_pane::MIN_SPLIT_WIDTH;
use super::sidebar::PaneFocus;
use super::sidebar::SidebarContent;
use super::sidebar::SidebarPane;
use super::sidebar_document::Document;
use super::sidebar_document::DocumentOrigin;
use super::*;

impl App {
    pub(super) async fn start_queued_model_document(
        &mut self,
        tui: &tui::Tui,
        server: &AppServerSession,
        width: u16,
    ) -> Option<uuid::Uuid> {
        let source = self.current_displayed_thread_id()?;
        if self.chat_widget.thread_id() != Some(source)
            || self.zoomed_thread == Some(source)
            || width < MIN_SPLIT_WIDTH
            || !self.document_preview_available(tui, server)
            || self.chat_widget.has_active_view()
            || self.divider_drag.is_some()
            || self.pane_resize.is_some()
            || self.panel_focus() == Some(PaneFocus::Sidebar)
            || self.sidebar().is_some_and(|pane| {
                pane.close.is_some()
                    || pane.focus_document_request.is_some()
                    || pane.agents.pending_link.is_some()
                    || matches!(pane.visible_content(), Some(SidebarContent::Document(id))
                        if pane.documents.get(&id).is_some_and(Document::awaits_open))
            })
        {
            return None;
        }
        let document = self
            .sidebars
            .get_mut(&Some(source))?
            .pending_documents
            .get_mut(&server.backend_id())?
            .pop_front()?;
        // Another request may be ready even when this one doesn't start a read.
        tui.frame_requester().schedule_frame();
        let permissions = server.model_preview_permissions(source).await;
        let id = self.open_document_tab(
            server,
            source,
            document.original_path,
            document.path,
            document.location,
            DocumentOrigin::Model { permissions },
        )?;
        // Leave a live companion selected so its composer remains visible.
        if self.sidebar_parent() == Some(source)
            && !matches!(
                self.sidebar().and_then(SidebarPane::visible_content),
                Some(SidebarContent::Chat(_))
            )
            && self.docked_agent().is_none()
        {
            self.sidebar_mut()
                .show(SidebarContent::Document(id), PaneFocus::Main);
        }
        self.sidebar()
            .and_then(|pane| pane.documents.get(&id))
            .filter(|doc| doc.model_loading())
            .map(|_| id)
    }
}
