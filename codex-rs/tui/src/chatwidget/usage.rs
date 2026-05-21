use super::ChatWidget;
use crate::app_event::AppEvent;
use crate::bottom_pane::USAGE_VIEW_ID;
use crate::bottom_pane::UsageView;
use codex_app_server_protocol::UsageRange;
use codex_app_server_protocol::UsageReadResponse;

impl ChatWidget {
    pub(crate) fn open_usage(&mut self, range: UsageRange) {
        let request_id = self.next_usage_request_id;
        self.next_usage_request_id = self.next_usage_request_id.saturating_add(/*rhs*/ 1);
        self.active_usage_request_id = Some(request_id);
        self.bottom_pane.dismiss_active_view_if_id(USAGE_VIEW_ID);
        self.bottom_pane.show_view(Box::new(UsageView::loading(
            range,
            self.app_event_tx.clone(),
        )));
        self.app_event_tx
            .send(AppEvent::FetchUsage { request_id, range });
        self.request_redraw();
    }

    pub(crate) fn on_usage_loaded(
        &mut self,
        request_id: u64,
        range: UsageRange,
        result: Result<UsageReadResponse, String>,
    ) {
        if self.active_usage_request_id != Some(request_id)
            || !self.bottom_pane.dismiss_active_view_if_id(USAGE_VIEW_ID)
        {
            return;
        }
        let view = match result {
            Ok(response) => UsageView::loaded(response, self.app_event_tx.clone()),
            Err(err) => UsageView::error(range, err, self.app_event_tx.clone()),
        };
        self.bottom_pane.show_view(Box::new(view));
        self.request_redraw();
    }
}
