//! Keep named-profile discovery responsive and discard replies after closing or changing scope.

use super::*;
use crate::permission_discovery::PermissionDiscovery;

pub(super) const VIEW_ID: &str = "permission-profiles";

impl ChatWidget {
    pub(crate) fn retain_input_after_failed_permission_selection(&mut self) {
        self.input_queue.recovered_queue |= self.input_queue.has_queued_follow_up_messages()
            || !self.input_queue.pending_steers.is_empty();
        if let Some(message) = self.initial_user_message.take() {
            self.restore_user_message_to_composer(message);
        }
    }
    pub(crate) fn request_permission_profiles(&mut self) {
        self.invalidate_permission_discovery();
        let request_id = uuid::Uuid::new_v4();
        self.permission_popup_request_id = Some(request_id);
        self.bottom_pane.show_selection_view(SelectionViewParams {
            view_id: Some(VIEW_ID),
            title: Some("Update Model Permissions".into()),
            items: vec![SelectionItem {
                name: "Loading permission profiles…".to_string(),
                is_disabled: true,
                ..Default::default()
            }],
            ..SelectionViewParams::picker()
        });
        self.app_event_tx.send(AppEvent::FetchPermissionProfiles {
            request_id,
            thread_cwd: self.thread_id.and(self.current_cwd.clone()),
        });
    }

    pub(super) fn invalidate_permission_discovery(&mut self) {
        self.permission_popup_request_id = None;
        self.bottom_pane.dismiss_view_by_id(VIEW_ID);
    }

    pub(crate) fn permission_popup_request_is_current(&self, request_id: uuid::Uuid) -> bool {
        self.permission_popup_request_id == Some(request_id)
    }

    pub(crate) fn on_permission_profiles_loaded(
        &mut self,
        request_id: uuid::Uuid,
        result: Result<PermissionDiscovery, String>,
    ) {
        if !self.permission_popup_request_is_current(request_id) {
            return;
        }
        self.permission_popup_request_id = None;
        if !self.bottom_pane.dismiss_active_view_if_id(VIEW_ID) {
            self.bottom_pane.dismiss_view_by_id(VIEW_ID);
            return;
        }
        match result {
            Ok(discovery) if !discovery.explicit_profile_mode => {
                self.open_legacy_permissions_popup()
            }
            Ok(discovery) => {
                self.permission_profiles_menu_opened = true;
                self.open_permission_profiles_popup(discovery);
            }
            Err(message) => self.bottom_pane.show_selection_view(SelectionViewParams {
                view_id: Some(VIEW_ID),
                title: Some("Update Model Permissions".to_string()),
                subtitle: Some(message),
                items: vec![SelectionItem {
                    name: "Retry".to_string(),
                    actions: vec![Box::new(|tx| tx.send(AppEvent::OpenPermissionsPopup))],
                    dismiss_on_select: true,
                    ..Default::default()
                }],
                ..SelectionViewParams::picker()
            }),
        }
    }
}
