use codex_app_server_protocol::ConsumeAccountRateLimitResetCreditCode;
use codex_app_server_protocol::ConsumeAccountRateLimitResetCreditResponse;
use codex_app_server_protocol::GetAccountRateLimitResetCreditsResponse;
use uuid::Uuid;

use super::*;

const USAGE_MENU_VIEW_ID: &str = "usage-menu";
const RATE_LIMIT_RESET_VIEW_ID: &str = "rate-limit-reset";

impl ChatWidget {
    pub(super) fn open_usage_menu(&mut self) {
        let show_rate_limit_resets = !self.plan_type.is_some_and(PlanType::is_workspace_account);
        let subtitle = if show_rate_limit_resets {
            "View account usage or redeem an earned reset."
        } else {
            "View account usage."
        };
        let mut items = vec![SelectionItem {
            name: "Token activity".to_string(),
            description: Some("View recent account token usage.".to_string()),
            actions: vec![Box::new(|tx| {
                tx.send(AppEvent::OpenTokenActivity);
            })],
            dismiss_on_select: true,
            ..Default::default()
        }];
        if show_rate_limit_resets {
            items.push(SelectionItem {
                name: "Rate-limit resets".to_string(),
                description: Some("View and redeem earned rate-limit resets.".to_string()),
                actions: vec![Box::new(|tx| {
                    tx.send(AppEvent::OpenRateLimitResetCredits);
                })],
                dismiss_on_select: true,
                ..Default::default()
            });
        }
        self.bottom_pane.show_selection_view(SelectionViewParams {
            view_id: Some(USAGE_MENU_VIEW_ID),
            title: Some("Usage".to_string()),
            subtitle: Some(subtitle.to_string()),
            footer_hint: Some(standard_popup_hint_line()),
            items,
            ..Default::default()
        });
        self.request_redraw();
    }

    pub(crate) fn show_rate_limit_reset_loading_popup(&mut self) -> u64 {
        let request_id = self.take_next_rate_limit_reset_request_id();
        self.pending_rate_limit_reset_request_id = Some(request_id);
        self.bottom_pane.show_selection_view(SelectionViewParams {
            view_id: Some(RATE_LIMIT_RESET_VIEW_ID),
            title: Some("Rate-limit resets".to_string()),
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
        result: Result<GetAccountRateLimitResetCreditsResponse, String>,
    ) -> bool {
        if self.pending_rate_limit_reset_request_id != Some(request_id) {
            return false;
        }
        self.pending_rate_limit_reset_request_id = None;

        let params = match result {
            Ok(response) if response.available_count > 0 => {
                Self::rate_limit_reset_confirmation_params(response.available_count)
            }
            Ok(_) => Self::rate_limit_reset_message_params(
                "You don't have any rate-limit resets available.",
            ),
            Err(_) => Self::rate_limit_reset_message_params(
                "Couldn't load rate-limit resets. Please try again.",
            ),
        };
        let replaced = self
            .bottom_pane
            .replace_selection_view_if_present(RATE_LIMIT_RESET_VIEW_ID, params);
        if replaced {
            self.request_redraw();
        }
        replaced
    }

    fn rate_limit_reset_confirmation_params(available_count: i64) -> SelectionViewParams {
        let redeem_request_id = Uuid::new_v4().to_string();
        SelectionViewParams {
            view_id: Some(RATE_LIMIT_RESET_VIEW_ID),
            title: Some("Rate-limit resets".to_string()),
            subtitle: Some(format!(
                "You have {available_count} {} available.",
                reset_label(available_count)
            )),
            footer_hint: Some(standard_popup_hint_line()),
            items: vec![
                SelectionItem {
                    name: "Use a reset".to_string(),
                    description: Some("Reset your current Codex usage windows.".to_string()),
                    is_default: true,
                    actions: vec![Box::new(move |tx| {
                        tx.send(AppEvent::ConsumeRateLimitResetCredit {
                            redeem_request_id: redeem_request_id.clone(),
                        });
                    })],
                    dismiss_on_select: true,
                    ..Default::default()
                },
                SelectionItem {
                    name: "Cancel".to_string(),
                    dismiss_on_select: true,
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
    }

    fn rate_limit_reset_message_params(message: &str) -> SelectionViewParams {
        SelectionViewParams {
            view_id: Some(RATE_LIMIT_RESET_VIEW_ID),
            title: Some("Rate-limit resets".to_string()),
            subtitle: Some(message.to_string()),
            items: vec![SelectionItem {
                name: "Close".to_string(),
                dismiss_on_select: true,
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    pub(crate) fn show_rate_limit_reset_consuming_popup(&mut self) -> u64 {
        let request_id = self.take_next_rate_limit_reset_request_id();
        self.pending_rate_limit_reset_request_id = Some(request_id);
        self.bottom_pane.show_selection_view(SelectionViewParams {
            view_id: Some(RATE_LIMIT_RESET_VIEW_ID),
            title: Some("Rate-limit resets".to_string()),
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
        redeem_request_id: String,
        result: Result<ConsumeAccountRateLimitResetCreditResponse, String>,
    ) -> bool {
        if self.pending_rate_limit_reset_request_id != Some(request_id) {
            return false;
        }

        match result {
            Ok(response)
                if matches!(
                    response.code,
                    ConsumeAccountRateLimitResetCreditCode::Reset
                        | ConsumeAccountRateLimitResetCreditCode::AlreadyRedeemed
                ) =>
            {
                self.replace_rate_limit_reset_popup(Self::rate_limit_reset_success_loading_params());
                true
            }
            Ok(response) => {
                self.pending_rate_limit_reset_request_id = None;
                let message = match response.code {
                    ConsumeAccountRateLimitResetCreditCode::NothingToReset => {
                        "Your usage does not need a reset right now."
                    }
                    ConsumeAccountRateLimitResetCreditCode::NoCredit => {
                        "No rate-limit resets are available."
                    }
                    ConsumeAccountRateLimitResetCreditCode::Reset
                    | ConsumeAccountRateLimitResetCreditCode::AlreadyRedeemed => unreachable!(),
                };
                self.replace_rate_limit_reset_popup(Self::rate_limit_reset_message_params(message));
                false
            }
            Err(_) => {
                self.pending_rate_limit_reset_request_id = None;
                self.replace_rate_limit_reset_popup(SelectionViewParams {
                    view_id: Some(RATE_LIMIT_RESET_VIEW_ID),
                    title: Some("Rate-limit resets".to_string()),
                    subtitle: Some("Couldn't reset usage. Please try again.".to_string()),
                    items: vec![
                        SelectionItem {
                            name: "Try again".to_string(),
                            actions: vec![Box::new(move |tx| {
                                tx.send(AppEvent::ConsumeRateLimitResetCredit {
                                    redeem_request_id: redeem_request_id.clone(),
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
        result: Result<GetAccountRateLimitResetCreditsResponse, String>,
    ) -> bool {
        if self.pending_rate_limit_reset_request_id != Some(request_id) {
            return false;
        }
        self.pending_rate_limit_reset_request_id = None;

        let message = match result {
            Ok(response) => format!(
                "Usage reset. You have {} {} left.",
                response.available_count,
                reset_label(response.available_count)
            ),
            Err(_) => "Usage reset.".to_string(),
        };
        self.replace_rate_limit_reset_popup(Self::rate_limit_reset_message_params(&message));
        true
    }

    fn rate_limit_reset_success_loading_params() -> SelectionViewParams {
        SelectionViewParams {
            view_id: Some(RATE_LIMIT_RESET_VIEW_ID),
            title: Some("Rate-limit resets".to_string()),
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

    pub(super) fn start_rate_limit_reset_hint_check(&mut self) {
        if !self.has_codex_backend_auth {
            return;
        }
        let request_id = self.take_next_rate_limit_reset_request_id();
        self.pending_rate_limit_reset_hint_request_id = Some(request_id);
        self.app_event_tx
            .send(AppEvent::CheckRateLimitResetCredits { request_id });
    }

    pub(crate) fn finish_rate_limit_reset_hint_refresh(
        &mut self,
        request_id: u64,
        result: Result<GetAccountRateLimitResetCreditsResponse, String>,
    ) -> bool {
        if self.pending_rate_limit_reset_hint_request_id != Some(request_id) {
            return false;
        }
        self.pending_rate_limit_reset_hint_request_id = None;
        if !self.has_codex_backend_auth {
            return false;
        }
        if let Ok(response) = result {
            self.show_rate_limit_reset_available_hint(response.available_count);
        }
        true
    }

    pub(super) fn clear_pending_rate_limit_reset_requests(&mut self) {
        self.pending_rate_limit_reset_request_id = None;
        self.pending_rate_limit_reset_hint_request_id = None;
        self.bottom_pane
            .dismiss_view_by_id(RATE_LIMIT_RESET_VIEW_ID);
        self.bottom_pane.dismiss_view_by_id(USAGE_MENU_VIEW_ID);
    }

    fn show_rate_limit_reset_available_hint(&mut self, available_count: i64) {
        if available_count <= 0 {
            return;
        }
        self.add_info_message(
            format!(
                "You have {available_count} {} available. Run /usage to use one.",
                reset_label(available_count)
            ),
            /*hint*/ None,
        );
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
        "rate-limit reset"
    } else {
        "rate-limit resets"
    }
}
