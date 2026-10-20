//! Cache the last server catalog and discard discovery replies after changing scope.

use super::slash_command_args;
use super::*;
use crate::permission_discovery::PermissionDiscovery;

pub(super) const VIEW_ID: &str = "permission-profiles";

pub(super) enum InlinePermissionProfileSelection {
    Discovering(String),
    Applying,
}

struct PermissionProfileArgs {
    profile_id: String,
    approval_policy: Option<AskForApproval>,
    reviewer: Option<ApprovalsReviewer>,
}

fn parse_approval_policy(value: &str) -> Result<AskForApproval, String> {
    match value {
        "untrusted" => Ok(AskForApproval::UnlessTrusted),
        "on-request" => Ok(AskForApproval::OnRequest),
        "never" => Ok(AskForApproval::Never),
        value => serde_json::from_str(value).map_err(|_| {
            format!(
                "Unknown approval policy `{value}`. Expected `untrusted`, `on-request`, `never`, or a granular policy."
            )
        }),
    }
}

fn parse_reviewer(value: &str) -> Result<ApprovalsReviewer, String> {
    match value {
        "user" => Ok(ApprovalsReviewer::User),
        "auto_review" => Ok(ApprovalsReviewer::AutoReview),
        value => Err(format!(
            "Unknown permission reviewer `{value}`. Expected `user` or `auto_review`."
        )),
    }
}

fn parse_permission_profile_args(args: &str) -> Result<PermissionProfileArgs, String> {
    let parsed = slash_command_args::parse(args, &["--approval-policy", "--reviewer"], &[], &[])?;
    let profile_id = parsed.positionals.first().cloned().ok_or_else(|| {
        "Usage: /permissions <profile-id> [--approval-policy=<policy>] [--reviewer=user|auto_review]"
            .to_string()
    })?;
    if let Some(argument) = parsed.positionals.get(1) {
        return Err(format!("Unexpected permission argument `{argument}`."));
    }
    Ok(PermissionProfileArgs {
        profile_id,
        approval_policy: parsed
            .value("--approval-policy")
            .map(parse_approval_policy)
            .transpose()?,
        reviewer: parsed.value("--reviewer").map(parse_reviewer).transpose()?,
    })
}

impl ChatWidget {
    pub(crate) fn select_permission_profile(&mut self, args: &str) {
        if let Err(message) = parse_permission_profile_args(args) {
            self.add_error_message(message);
            self.retain_input_after_failed_permission_selection();
            self.app_event_tx.send(AppEvent::SettingsSelectionClosed);
            return;
        }
        self.set_queue_autosend_suppressed(/*suppressed*/ true);
        self.inline_permission_profile_selection = Some(
            InlinePermissionProfileSelection::Discovering(args.to_string()),
        );
        if !self.apply_pending_permission_profile() {
            self.request_permission_profiles();
        }
    }

    pub(crate) fn retain_input_after_failed_permission_selection(&mut self) {
        self.input_queue.recovered_queue |= self.input_queue.has_queued_follow_up_messages()
            || !self.input_queue.pending_steers.is_empty();
        if let Some(message) = self.initial_user_message.take() {
            self.restore_user_message_to_composer(message);
        }
    }

    pub(super) fn apply_pending_permission_profile(&mut self) -> bool {
        let Some(discovery) = self.permission_discovery.clone() else {
            return false;
        };
        let Some(args) = self.pending_permission_profile_args().map(str::to_owned) else {
            return false;
        };
        let Ok(args) = parse_permission_profile_args(&args) else {
            self.recover_inline_permission_profile_selection();
            return true;
        };
        self.inline_permission_profile_selection = Some(InlinePermissionProfileSelection::Applying);
        if !self.try_select_permission_profile(
            &discovery,
            &args.profile_id,
            args.approval_policy,
            args.reviewer,
        ) {
            self.recover_inline_permission_profile_selection();
        }
        self.app_event_tx.send(AppEvent::SettingsSelectionClosed);
        true
    }

    pub(super) fn cancel_pending_permission_profile_selection(&mut self) -> bool {
        if self.pending_permission_profile_args().is_none() {
            return false;
        }
        self.recover_inline_permission_profile_selection();
        self.app_event_tx.send(AppEvent::SettingsSelectionClosed);
        true
    }

    pub(crate) fn complete_inline_permission_profile_selection(&mut self) -> bool {
        self.inline_permission_profile_selection.take().is_some()
    }

    pub(crate) fn recover_inline_permission_profile_selection(&mut self) -> bool {
        if !self.complete_inline_permission_profile_selection() {
            return false;
        }
        self.retain_input_after_failed_permission_selection();
        true
    }

    pub(crate) fn inline_permission_profile_selection_pending(&self) -> bool {
        self.inline_permission_profile_selection.is_some()
    }

    fn pending_permission_profile_args(&self) -> Option<&str> {
        match self.inline_permission_profile_selection.as_ref() {
            Some(InlinePermissionProfileSelection::Discovering(profile_id)) => Some(profile_id),
            Some(InlinePermissionProfileSelection::Applying) | None => None,
        }
    }

    pub(crate) fn request_permission_profiles(&mut self) {
        self.bottom_pane.dismiss_view_by_id(VIEW_ID);
        if let Some(discovery) = self.permission_discovery.clone() {
            self.permission_profiles_menu_opened = true;
            self.open_permission_profiles_popup(discovery);
            return;
        }
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
        if self.permission_popup_request_id.is_some() {
            return;
        }
        let request_id = uuid::Uuid::new_v4();
        self.permission_popup_request_id = Some(request_id);
        self.app_event_tx.send(AppEvent::FetchPermissionProfiles {
            request_id,
            thread_cwd: self.thread_id.and(self.current_cwd.clone()),
        });
    }

    pub(super) fn invalidate_permission_discovery(&mut self) {
        self.permission_popup_request_id = None;
        let dismissed = self.bottom_pane.dismiss_view_by_id(VIEW_ID);
        if self.recover_inline_permission_profile_selection()
            || (dismissed && self.input_queue.suppress_queue_autosend)
        {
            self.app_event_tx.send(AppEvent::SettingsSelectionClosed);
        }
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
        if let Ok(discovery) = &result {
            self.permission_discovery = Some(discovery.clone());
        }
        if !self.bottom_pane.dismiss_active_view_if_id(VIEW_ID) {
            let was_covered = self.bottom_pane.dismiss_view_by_id(VIEW_ID);
            if was_covered && result.is_ok() {
                return;
            }
            if let Err(message) = result
                && self.pending_permission_profile_args().is_some()
            {
                self.add_error_message(message);
            }
            self.cancel_pending_permission_profile_selection();
            return;
        }
        match result {
            Ok(discovery) => {
                if !self.apply_pending_permission_profile() {
                    self.permission_profiles_menu_opened = true;
                    self.open_permission_profiles_popup(discovery);
                }
            }
            Err(message) => {
                let retry_args = self.pending_permission_profile_args().map(str::to_owned);
                self.bottom_pane.show_selection_view(SelectionViewParams {
                    view_id: Some(VIEW_ID),
                    title: Some("Update Model Permissions".to_string()),
                    subtitle: Some(message),
                    items: vec![SelectionItem {
                        name: "Retry".to_string(),
                        actions: vec![Box::new(move |tx| {
                            if let Some(args) = retry_args.clone() {
                                tx.send(AppEvent::ResolvePermissionProfile { profile_id: args });
                            } else {
                                tx.send(AppEvent::OpenPermissionsPopup);
                            }
                        })],
                        dismiss_on_select: false,
                        ..Default::default()
                    }],
                    ..SelectionViewParams::picker()
                });
            }
        }
    }
}
