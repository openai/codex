use super::ChatWidget;
use crate::app_event::AppEvent;
use crate::bottom_pane::HooksBrowserView;
use crate::hooks_rpc::HooksListTarget;
use codex_app_server_protocol::HooksListEntry;

impl ChatWidget {
    pub(crate) fn add_hooks_output(&mut self) {
        self.app_event_tx.send(AppEvent::FetchHooksList {
            target: self.hooks_list_target(),
        });
    }

    pub(crate) fn on_hooks_loaded(
        &mut self,
        target: HooksListTarget,
        result: Result<HooksListEntry, String>,
    ) {
        if self.hooks_list_target() != target {
            return;
        }

        match result {
            Ok(entry) => self.open_hooks_browser(entry),
            Err(err) => self.add_error_message(format!("Failed to load hooks: {err}")),
        }
    }

    fn hooks_list_target(&self) -> HooksListTarget {
        self.thread_id()
            .map(HooksListTarget::Thread)
            .unwrap_or_else(|| HooksListTarget::Cwd(self.config.cwd.to_path_buf()))
    }

    pub(crate) fn open_hooks_browser(&mut self, entry: HooksListEntry) {
        self.bottom_pane
            .show_view(Box::new(HooksBrowserView::from_entry(
                entry,
                self.app_event_tx.clone(),
                self.bottom_pane.list_keymap(),
            )));
        self.request_redraw();
    }
}
