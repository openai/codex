//! Hide and restore the sidebar using the same focus and tab transitions as keyboard navigation.

use super::sidebar::PaneFocus;
use super::sidebar::SidebarContent;
use super::sidebar::{self};
use super::*;
use crate::sidebar_tab_strip::SidebarTab;

impl App {
    pub(super) fn sidebar_is_hidden(&self) -> bool {
        self.sidebar()
            .and_then(sidebar::SidebarPane::hidden_content)
            .is_some()
    }

    /// Hide the sidebar while returning input to its parent conversation.
    pub(super) async fn hide_sidebar(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
    ) -> Result<()> {
        if self.panel_focus().is_none() && self.sidebar_has_focus() {
            let previous = self.current_displayed_thread_id();
            let result = self.hide_focused_side_pane(tui, app_server).await;
            if self.current_displayed_thread_id() != previous {
                self.zoomed_thread = None;
            }
            return result;
        }
        if self
            .sidebar()
            .and_then(sidebar::SidebarPane::visible_content)
            .is_some()
        {
            self.zoomed_thread = None;
            if self.panel_focus() == Some(PaneFocus::Sidebar) {
                self.focus_current_parent_view(tui);
            }
            self.sidebar_mut().hide();
            tui.frame_requester().schedule_frame();
        }
        Ok(())
    }

    pub(super) async fn restore_sidebar(
        &mut self,
        tui: &mut tui::Tui,
        server: &mut AppServerSession,
    ) -> Result<()> {
        let content = self
            .sidebar()
            .and_then(|pane| pane.hidden_content().or_else(|| pane.visible_content()));
        let tab = match content {
            Some(SidebarContent::Chat(id)) => Some(SidebarTab::Chat(id)),
            Some(SidebarContent::Document(id)) => Some(SidebarTab::Document(id)),
            Some(SidebarContent::Subagents) => Some(SidebarTab::Subagents),
            Some(SidebarContent::Chooser) | None => None,
        };
        if let Some(tab) = tab.filter(|tab| {
            self.sidebar()
                .is_some_and(|pane| pane.chat_tabs().ids.contains(tab))
        }) {
            if tab == SidebarTab::Subagents
                && self
                    .sidebar()
                    .is_some_and(|pane| pane.agents.detail.is_none())
            {
                self.sidebar_mut()
                    .show(SidebarContent::Subagents, PaneFocus::Sidebar);
                tui.frame_requester().schedule_frame();
                return Ok(());
            }
            let hidden = self.sidebar_is_hidden();
            let agent = (tab == SidebarTab::Subagents)
                .then(|| self.sidebar().and_then(|pane| pane.agents.detail))
                .flatten();
            let result = self.activate_sidebar_tab(tui, server, tab).await;
            if let Some(agent) = agent
                && self.current_displayed_thread_id() != Some(agent)
            {
                let pane = self.sidebar_mut();
                pane.show(SidebarContent::Subagents, PaneFocus::Main);
                if hidden {
                    pane.hide();
                }
                return result;
            }
            result?;
            if self.sidebar_has_focus() {
                self.zoomed_thread = None;
                tui.frame_requester().schedule_frame();
            }
            return Ok(());
        }
        self.open_sidebar_tabs(tui, server).await
    }
}
