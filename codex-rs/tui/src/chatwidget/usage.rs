use codex_app_server_protocol::ConsumeAccountRateLimitResetCreditOutcome;
use codex_app_server_protocol::ConsumeAccountRateLimitResetCreditResponse;
use codex_app_server_protocol::RateLimitResetCreditsSummary;
use uuid::Uuid;

use super::reset_credits::rate_limit_reset_scope;
use super::reset_credits::reset_credit_options;
use super::*;

const USAGE_MENU_VIEW_ID: &str = "usage-menu";
const RATE_LIMIT_RESET_VIEW_ID: &str = "rate-limit-reset";

impl ChatWidget {
    pub(super) fn open_usage_menu(&mut self) {
        self.clear_pending_rate_limit_reset_hint();
        let should_refresh_reset_availability = self.available_rate_limit_reset_credits == Some(0);
        self.bottom_pane
            .show_selection_view(self.usage_menu_params());
        if should_refresh_reset_availability {
            let request_id = self.take_next_rate_limit_reset_request_id();
            self.pending_usage_menu_rate_limit_request_id = Some(request_id);
            self.app_event_tx.send(AppEvent::RefreshRateLimits {
                origin: RateLimitRefreshOrigin::UsageMenu { request_id },
            });
        }
        self.request_redraw();
    }

    fn usage_menu_params(&self) -> SelectionViewParams {
        let reset_eligible = self.has_chatgpt_account;
        let (reset_action_enabled, reset_description) =
            match (reset_eligible, self.available_rate_limit_reset_credits) {
                (true, Some(available_count)) if available_count > 0 => (
                    true,
                    format!(
                        "You have {available_count} {} available.",
                        reset_label(available_count)
                    ),
                ),
                (true, None) => (true, "Check reset availability.".to_string()),
                (true, Some(_)) | (false, _) => {
                    (false, "No usage limit resets available.".to_string())
                }
            };
        SelectionViewParams {
            view_id: Some(USAGE_MENU_VIEW_ID),
            title: Some("Usage".to_string()),
            subtitle: Some("View account usage or redeem an earned reset.".to_string()),
            footer_hint: Some(standard_popup_hint_line()),
            items: vec![
                SelectionItem {
                    name: "Show usage".to_string(),
                    description: Some("View recent account token usage.".to_string()),
                    actions: vec![Box::new(|tx| {
                        tx.send(AppEvent::OpenTokenActivity);
                    })],
                    dismiss_on_select: true,
                    ..Default::default()
                },
                SelectionItem {
                    name: "Redeem usage limit reset".to_string(),
                    description: Some(reset_description),
                    is_disabled: !reset_action_enabled,
                    actions: vec![Box::new(|tx| {
                        tx.send(AppEvent::OpenRateLimitResetCredits);
                    })],
                    dismiss_on_select: true,
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
    }

    pub(crate) fn finish_usage_menu_rate_limit_refresh(
        &mut self,
        request_id: u64,
        snapshots: Vec<RateLimitSnapshot>,
        result: Result<RateLimitResetCreditsSummary, String>,
    ) {
        if self.pending_usage_menu_rate_limit_request_id != Some(request_id) {
            return;
        }
        self.pending_usage_menu_rate_limit_request_id = None;
        for snapshot in snapshots {
            self.on_rate_limit_snapshot(Some(snapshot));
        }
        if let Ok(response) = result {
            self.available_rate_limit_reset_credits = Some(response.available_count);
        }
        let params = self.usage_menu_params();
        if self
            .bottom_pane
            .replace_selection_view_if_present(USAGE_MENU_VIEW_ID, params)
        {
            self.request_redraw();
        }
    }

    pub(crate) fn show_rate_limit_reset_loading_popup(&mut self) -> u64 {
        self.clear_pending_rate_limit_reset_hint();
        let request_id = self.take_next_rate_limit_reset_request_id();
        self.pending_rate_limit_reset_request_id = Some(request_id);
        self.bottom_pane.show_selection_view(SelectionViewParams {
            view_id: Some(RATE_LIMIT_RESET_VIEW_ID),
            title: Some("Usage limit resets".to_string()),
            subtitle: Some("Checking your available resets...".to_string()),
            items: vec![SelectionItem {
                name: "Loading...".to_string(),
                is_disabled: true,
                ..Default::default()
            }],
            ..Default::default()
        });
        self.request_redraw();
        request_id
    }

    pub(crate) fn finish_rate_limit_reset_credits_refresh(
        &mut self,
        request_id: u64,
        snapshots: Vec<RateLimitSnapshot>,
        result: Result<RateLimitResetCreditsSummary, String>,
    ) -> bool {
        if self.pending_rate_limit_reset_request_id != Some(request_id) {
            return false;
        }
        self.pending_rate_limit_reset_request_id = None;
        for snapshot in snapshots {
            self.on_rate_limit_snapshot(Some(snapshot));
        }

        let params = match result {
            Ok(response) => {
                let available_count = response.available_count;
                let params = if available_count > 0 {
                    self.rate_limit_reset_confirmation_params(&response)
                } else {
                    Self::rate_limit_reset_message_params(
                        "You don't have any usage limit resets available.",
                    )
                };
                self.available_rate_limit_reset_credits = Some(available_count);
                params
            }
            Err(_) => {
                Self::reset_refresh_params("Couldn't load usage limit resets. Please try again.")
            }
        };
        let replaced = self
            .bottom_pane
            .replace_selection_view_if_present(RATE_LIMIT_RESET_VIEW_ID, params);
        if replaced {
            self.request_redraw();
        }
        replaced
    }

    fn rate_limit_reset_confirmation_params(
        &self,
        reset_credits: &RateLimitResetCreditsSummary,
    ) -> SelectionViewParams {
        let scope = rate_limit_reset_scope(&self.rate_limit_snapshots_by_limit_id, self.plan_type);
        let mut items = vec![SelectionItem {
            name: "Cancel".to_string(),
            dismiss_on_select: true,
            ..Default::default()
        }];
        items.extend(
            reset_credit_options(reset_credits, scope)
                .into_iter()
                .map(|option| {
                    let idempotency_key = Uuid::new_v4().to_string();
                    let credit_id = option.credit_id;
                    SelectionItem {
                        name: option.name,
                        description: Some(option.description),
                        actions: vec![Box::new(move |tx| {
                            tx.send(AppEvent::ConsumeRateLimitResetCredit {
                                idempotency_key: idempotency_key.clone(),
                                credit_id: credit_id.clone(),
                            });
                        })],
                        dismiss_on_select: true,
                        ..Default::default()
                    }
                }),
        );
        SelectionViewParams {
            view_id: Some(RATE_LIMIT_RESET_VIEW_ID),
            title: Some("Usage limit resets".to_string()),
            subtitle: Some(format!(
                "{} {} available.",
                reset_credits.available_count,
                reset_label(reset_credits.available_count)
            )),
            footer_hint: Some(standard_popup_hint_line()),
            items,
            initial_selected_idx: Some(0),
            ..Default::default()
        }
    }

    fn rate_limit_reset_message_params(message: &str) -> SelectionViewParams {
        SelectionViewParams {
            view_id: Some(RATE_LIMIT_RESET_VIEW_ID),
            title: Some("Usage limit resets".to_string()),
            subtitle: Some(message.to_string()),
            items: vec![SelectionItem {
                name: "Close".to_string(),
                dismiss_on_select: true,
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    fn reset_refresh_params(message: &str) -> SelectionViewParams {
        SelectionViewParams {
            view_id: Some(RATE_LIMIT_RESET_VIEW_ID),
            title: Some("Usage limit resets".to_string()),
            subtitle: Some(message.to_string()),
            items: vec![
                SelectionItem {
                    name: "Try again".to_string(),
                    actions: vec![Box::new(|tx| {
                        tx.send(AppEvent::OpenRateLimitResetCredits);
                    })],
                    dismiss_on_select: true,
                    ..Default::default()
                },
                SelectionItem {
                    name: "Close".to_string(),
                    dismiss_on_select: true,
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
    }

    pub(crate) fn show_rate_limit_reset_consuming_popup(&mut self) -> u64 {
        self.clear_pending_rate_limit_reset_hint();
        let request_id = self.take_next_rate_limit_reset_request_id();
        self.pending_rate_limit_reset_request_id = Some(request_id);
        self.bottom_pane.show_selection_view(SelectionViewParams {
            view_id: Some(RATE_LIMIT_RESET_VIEW_ID),
            title: Some("Usage limit resets".to_string()),
            subtitle: Some("Resetting your usage...".to_string()),
            items: vec![SelectionItem {
                name: "Using a reset...".to_string(),
                is_disabled: true,
                ..Default::default()
            }],
            allow_cancel: false,
            ..Default::default()
        });
        self.request_redraw();
        request_id
    }

    pub(crate) fn finish_rate_limit_reset_consume(
        &mut self,
        request_id: u64,
        idempotency_key: String,
        credit_id: Option<String>,
        result: Result<ConsumeAccountRateLimitResetCreditResponse, String>,
    ) -> bool {
        if self.pending_rate_limit_reset_request_id != Some(request_id) {
            return false;
        }

        match result {
            Ok(response)
                if matches!(
                    response.outcome,
                    ConsumeAccountRateLimitResetCreditOutcome::Reset
                        | ConsumeAccountRateLimitResetCreditOutcome::AlreadyRedeemed
                ) =>
            {
                self.available_rate_limit_reset_credits = None;
                self.replace_rate_limit_reset_popup(Self::rate_limit_reset_success_loading_params());
                true
            }
            Ok(response) => {
                self.pending_rate_limit_reset_request_id = None;
                let message = match response.outcome {
                    ConsumeAccountRateLimitResetCreditOutcome::NothingToReset => {
                        "Your usage does not need a reset right now."
                    }
                    ConsumeAccountRateLimitResetCreditOutcome::NoCredit if credit_id.is_some() => {
                        self.available_rate_limit_reset_credits = None;
                        self.replace_rate_limit_reset_popup(Self::reset_refresh_params(
                            "That reset is no longer available. Refresh to see your current resets.",
                        ));
                        return false;
                    }
                    ConsumeAccountRateLimitResetCreditOutcome::NoCredit => {
                        self.available_rate_limit_reset_credits = Some(0);
                        "No usage limit resets are available."
                    }
                    ConsumeAccountRateLimitResetCreditOutcome::Reset
                    | ConsumeAccountRateLimitResetCreditOutcome::AlreadyRedeemed => unreachable!(),
                };
                self.replace_rate_limit_reset_popup(Self::rate_limit_reset_message_params(message));
                false
            }
            Err(_) => {
                self.pending_rate_limit_reset_request_id = None;
                self.replace_rate_limit_reset_popup(SelectionViewParams {
                    view_id: Some(RATE_LIMIT_RESET_VIEW_ID),
                    title: Some("Usage limit resets".to_string()),
                    subtitle: Some("Couldn't reset usage. Please try again.".to_string()),
                    items: vec![
                        SelectionItem {
                            name: "Try again".to_string(),
                            actions: vec![Box::new(move |tx| {
                                tx.send(AppEvent::ConsumeRateLimitResetCredit {
                                    idempotency_key: idempotency_key.clone(),
                                    credit_id: credit_id.clone(),
                                });
                            })],
                            dismiss_on_select: true,
                            ..Default::default()
                        },
                        SelectionItem {
                            name: "Close".to_string(),
                            dismiss_on_select: true,
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                });
                false
            }
        }
    }

    pub(crate) fn finish_post_consume_reset_credits_refresh(
        &mut self,
        request_id: u64,
        snapshots: Vec<RateLimitSnapshot>,
        result: Result<RateLimitResetCreditsSummary, String>,
    ) -> bool {
        if self.pending_rate_limit_reset_request_id != Some(request_id) {
            return false;
        }
        self.pending_rate_limit_reset_request_id = None;
        for snapshot in snapshots {
            self.on_rate_limit_snapshot(Some(snapshot));
        }

        let message = match result {
            Ok(response) => {
                let available_count = response.available_count;
                self.available_rate_limit_reset_credits = Some(available_count);
                format!(
                    "Usage reset. You have {available_count} {} left.",
                    reset_label(available_count)
                )
            }
            Err(_) => "Usage reset.".to_string(),
        };
        self.replace_rate_limit_reset_popup(Self::rate_limit_reset_message_params(&message));
        true
    }

    fn rate_limit_reset_success_loading_params() -> SelectionViewParams {
        SelectionViewParams {
            view_id: Some(RATE_LIMIT_RESET_VIEW_ID),
            title: Some("Usage limit resets".to_string()),
            subtitle: Some("Usage reset. Checking your remaining resets...".to_string()),
            items: vec![SelectionItem {
                name: "Refreshing...".to_string(),
                is_disabled: true,
                ..Default::default()
            }],
            allow_cancel: false,
            ..Default::default()
        }
    }

    fn replace_rate_limit_reset_popup(&mut self, params: SelectionViewParams) {
        if self
            .bottom_pane
            .replace_selection_view_if_present(RATE_LIMIT_RESET_VIEW_ID, params)
        {
            self.request_redraw();
        }
    }

    pub(crate) fn start_rate_limit_reset_startup_check(&mut self) -> u64 {
        self.clear_pending_rate_limit_reset_hint();
        let request_id = self.take_next_rate_limit_reset_request_id();
        self.pending_rate_limit_reset_hint_request_id = Some(request_id);
        request_id
    }

    pub(crate) fn finish_rate_limit_reset_hint_refresh(
        &mut self,
        request_id: u64,
        snapshots: Vec<RateLimitSnapshot>,
        result: Result<RateLimitResetCreditsSummary, String>,
    ) -> bool {
        if self.pending_rate_limit_reset_hint_request_id != Some(request_id) {
            return false;
        }
        self.pending_rate_limit_reset_hint_request_id = None;
        for snapshot in snapshots {
            self.on_rate_limit_snapshot(Some(snapshot));
        }
        if !self.has_codex_backend_auth {
            return false;
        }
        if let Ok(response) = result {
            let available_count = response.available_count;
            self.available_rate_limit_reset_credits = Some(available_count);
            self.set_rate_limit_reset_available_hint(available_count);
        }
        true
    }

    pub(crate) fn clear_pending_rate_limit_reset_requests(&mut self) {
        self.pending_rate_limit_reset_request_id = None;
        self.pending_usage_menu_rate_limit_request_id = None;
        self.available_rate_limit_reset_credits = None;
        self.rate_limit_snapshots_by_limit_id.clear();
        self.clear_pending_rate_limit_reset_hint();
        self.bottom_pane.dismiss_view_by_id(USAGE_MENU_VIEW_ID);
        self.bottom_pane
            .dismiss_view_by_id(RATE_LIMIT_RESET_VIEW_ID);
    }

    pub(crate) fn clear_pending_rate_limit_reset_hint(&mut self) {
        self.pending_rate_limit_reset_hint_request_id = None;
        let cleared_hint = self.pending_rate_limit_reset_hint.take().is_some();
        if cleared_hint {
            self.bump_active_cell_revision();
            self.request_redraw();
        }
    }

    pub(super) fn pending_rate_limit_reset_hint(&self) -> Option<&PlainHistoryCell> {
        self.pending_rate_limit_reset_hint.as_ref()
    }

    pub(crate) fn take_pending_rate_limit_reset_hint(&mut self) -> Option<PlainHistoryCell> {
        let hint = self.pending_rate_limit_reset_hint.take()?;
        self.bump_active_cell_revision();
        Some(hint)
    }

    fn set_rate_limit_reset_available_hint(&mut self, available_count: i64) {
        if available_count <= 0 {
            return;
        }
        self.pending_rate_limit_reset_hint = Some(history_cell::new_info_event(
            format!(
                "You have {available_count} {} available. Run /usage to use one.",
                reset_label(available_count)
            ),
            /*hint*/ None,
        ));
        self.bump_active_cell_revision();
        self.request_redraw();
    }

    fn take_next_rate_limit_reset_request_id(&mut self) -> u64 {
        let request_id = self.next_rate_limit_reset_request_id;
        self.next_rate_limit_reset_request_id = self
            .next_rate_limit_reset_request_id
            .wrapping_add(/*rhs*/ 1);
        request_id
    }
}

fn reset_label(count: i64) -> &'static str {
    if count == 1 {
        "usage limit reset"
    } else {
        "usage limit resets"
    }
}
