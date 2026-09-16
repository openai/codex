//! Route session-only permission shortcuts through the shared selection flow.

use super::*;

impl App {
    pub(super) async fn apply_permission_shortcut(
        &mut self,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
        selection: PermissionProfileSelection,
    ) {
        if self.current_displayed_thread_id() != Some(thread_id)
            || self.chat_widget.thread_id() != Some(thread_id)
        {
            self.chat_widget.complete_permission_shortcut(thread_id);
            return;
        }
        self.select_permission_profile(app_server, selection).await;
        self.chat_widget.complete_permission_shortcut(thread_id);
    }
}
