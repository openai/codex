use codex_chatgpt::referrals::ReferralOffer;
use codex_chatgpt::referrals::ReferralRewardStatus;
use std::time::Duration;
use std::time::Instant;
use uuid::Uuid;

use crate::app_event::ReferralInviteResult;

use super::usage::USAGE_MENU_VIEW_ID;
use super::*;

const REFERRAL_VIEW_ID: &str = "referral-invite";
pub(super) const REFERRAL_OFFER_MAX_AGE: Duration = Duration::from_secs(10 * 60);

#[derive(Default)]
pub(super) struct ReferralState {
    offer: Option<ReferralOffer>,
    pub(super) offer_loaded_at: Option<Instant>,
    pending_request_id: Option<Uuid>,
    pending_email: Option<String>,
}

impl ChatWidget {
    pub(super) fn start_referral_offer_refresh(&mut self) {
        if !self.has_chatgpt_account {
            return;
        }
        let request_id = Uuid::new_v4();
        self.referral_state.pending_request_id = Some(request_id);
        self.app_event_tx
            .send(AppEvent::RefreshReferralOffer(request_id));
    }

    pub(crate) fn finish_referral_offer_refresh(
        &mut self,
        request_id: Uuid,
        offer: Option<ReferralOffer>,
    ) {
        if self.referral_state.pending_request_id != Some(request_id) {
            return;
        }
        self.referral_state.pending_request_id = None;
        self.referral_state.offer_loaded_at = offer.as_ref().map(|_| Instant::now());
        self.referral_state.offer = offer;

        let selected = self
            .bottom_pane
            .selected_index_for_active_view(USAGE_MENU_VIEW_ID);
        let mut params = self.usage_menu_params();
        params.initial_selected_idx = selected;
        if self
            .bottom_pane
            .replace_selection_view_if_present(USAGE_MENU_VIEW_ID, params)
        {
            self.request_redraw();
        }
    }

    pub(super) fn referral_menu_item(&self) -> Option<SelectionItem> {
        let offer = self.current_referral_offer()?;
        Some(SelectionItem {
            name: "Invite someone to Codex".to_string(),
            description: Some(offer.description.clone()),
            actions: vec![Box::new(|tx| {
                tx.send(AppEvent::OpenReferralEmailPrompt);
            })],
            dismiss_on_select: true,
            ..Default::default()
        })
    }

    pub(crate) fn show_referral_email_prompt(&mut self) {
        let tx = self.app_event_tx.clone();
        let view = CustomPromptView::new(
            "Invite someone to Codex".to_string(),
            "name@example.com".to_string(),
            String::new(),
            Some("Enter one email address, then press Enter.".to_string()),
            Box::new(move |email: String| {
                let email = email.trim().to_string();
                if !email.is_empty() {
                    tx.send(AppEvent::OpenReferralConfirmation(email));
                }
            }),
        )
        .with_view_id(REFERRAL_VIEW_ID);
        self.bottom_pane.show_view(Box::new(view));
        self.request_redraw();
    }

    pub(crate) fn show_referral_confirmation(&mut self, email: String) {
        let Some(offer) = self.current_referral_offer().cloned() else {
            self.show_referral_message(
                "This referral offer is no longer available. Reopen /usage to check again.",
            );
            return;
        };

        let build_params = || {
            let mut header = vec![
                Line::from("Send referral invite?").bold(),
                Line::from(offer.description.clone()).dim(),
                Line::from(format!("Recipient: {email}")).dim(),
            ];
            header.extend(
                offer
                    .rules
                    .iter()
                    .map(|rule| Line::from(format!("• {rule}")).dim()),
            );
            if offer.requires_explicit_confirmation {
                header.push(
                    Line::from("By sending, you confirm that you have this person's consent.")
                        .dim(),
                );
            }
            let email_for_action = email.clone();
            let offer_for_action = offer.clone();
            SelectionViewParams {
                view_id: Some(REFERRAL_VIEW_ID),
                header: Box::new(Paragraph::new(header).wrap(Wrap { trim: false })),
                footer_hint: Some(standard_popup_hint_line()),
                items: vec![
                    SelectionItem {
                        name: "Send invite".to_string(),
                        actions: vec![Box::new(move |tx| {
                            tx.send(AppEvent::SendReferralInvite {
                                email: email_for_action.clone(),
                                offer: Box::new(offer_for_action.clone()),
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
                initial_selected_idx: Some(1),
                ..Default::default()
            }
        };
        if !self
            .bottom_pane
            .replace_selection_view_if_present(REFERRAL_VIEW_ID, build_params())
            && !self
                .bottom_pane
                .replace_active_views_with_selection_view(&[REFERRAL_VIEW_ID], build_params())
        {
            self.bottom_pane.show_selection_view(build_params());
        }
        self.request_redraw();
    }

    pub(crate) fn start_referral_send(
        &mut self,
        email: &str,
        confirmed_offer: ReferralOffer,
    ) -> Option<(Uuid, ReferralOffer)> {
        let Some(current_offer) = self.current_referral_offer() else {
            self.show_referral_message(
                "This referral offer is no longer available. Reopen /usage to check again.",
            );
            return None;
        };
        if current_offer != &confirmed_offer {
            self.show_referral_message(
                "This referral offer changed. Reopen /usage and confirm the current offer.",
            );
            return None;
        }
        let request_id = Uuid::new_v4();
        self.referral_state.pending_request_id = Some(request_id);
        self.referral_state.pending_email = Some(email.to_string());
        let params = SelectionViewParams {
            view_id: Some(REFERRAL_VIEW_ID),
            title: Some("Invite someone to Codex".to_string()),
            subtitle: Some(format!("Sending an invite to {email}...")),
            items: vec![SelectionItem {
                name: "Sending invite...".to_string(),
                is_disabled: true,
                ..Default::default()
            }],
            allow_cancel: false,
            ..Default::default()
        };
        if !self
            .bottom_pane
            .replace_active_views_with_selection_view(&[REFERRAL_VIEW_ID], params)
        {
            self.bottom_pane.show_selection_view(SelectionViewParams {
                view_id: Some(REFERRAL_VIEW_ID),
                title: Some("Invite someone to Codex".to_string()),
                subtitle: Some(format!("Sending an invite to {email}...")),
                items: vec![SelectionItem {
                    name: "Sending invite...".to_string(),
                    is_disabled: true,
                    ..Default::default()
                }],
                allow_cancel: false,
                ..Default::default()
            });
        }
        self.request_redraw();
        Some((request_id, confirmed_offer))
    }

    pub(crate) fn finish_referral_send(&mut self, request_id: Uuid, result: ReferralInviteResult) {
        if self.referral_state.pending_request_id != Some(request_id) {
            return;
        }
        self.referral_state.pending_request_id = None;
        let email = self.referral_state.pending_email.take().unwrap_or_default();
        let message = match result {
            ReferralInviteResult::Sent(ReferralRewardStatus::Included) => {
                format!("Invite sent to {email}.")
            }
            ReferralInviteResult::Sent(ReferralRewardStatus::NotIncluded) => {
                format!("Invite sent to {email}, but it did not include a reward.")
            }
            ReferralInviteResult::Sent(ReferralRewardStatus::Unknown) => {
                format!("Invite sent to {email}; reward status wasn't confirmed.")
            }
            ReferralInviteResult::Rejected => {
                "We couldn't send that invite. Check the email address and try again.".to_string()
            }
            ReferralInviteResult::Unavailable => {
                "This referral offer changed. Reopen /usage and confirm the current offer."
                    .to_string()
            }
            ReferralInviteResult::Unknown => {
                "We couldn't confirm the invite. Check with the recipient before trying again."
                    .to_string()
            }
        };
        if matches!(
            result,
            ReferralInviteResult::Sent(_) | ReferralInviteResult::Unavailable
        ) {
            self.referral_state.offer = None;
            self.referral_state.offer_loaded_at = None;
        }
        self.show_referral_message(&message);
    }

    pub(crate) fn clear_referral_state(&mut self) {
        self.referral_state = ReferralState::default();
        self.bottom_pane.dismiss_view_by_id(USAGE_MENU_VIEW_ID);
        self.bottom_pane.dismiss_view_by_id(REFERRAL_VIEW_ID);
    }

    fn show_referral_message(&mut self, message: &str) {
        let build_params = || SelectionViewParams {
            view_id: Some(REFERRAL_VIEW_ID),
            title: Some("Invite someone to Codex".to_string()),
            subtitle: Some(message.to_string()),
            footer_hint: Some(standard_popup_hint_line()),
            items: vec![SelectionItem {
                name: "Close".to_string(),
                dismiss_on_select: true,
                ..Default::default()
            }],
            ..Default::default()
        };
        if !self
            .bottom_pane
            .replace_selection_view_if_present(REFERRAL_VIEW_ID, build_params())
            && !self
                .bottom_pane
                .replace_active_views_with_selection_view(&[REFERRAL_VIEW_ID], build_params())
        {
            self.bottom_pane.show_selection_view(build_params());
        }
        self.request_redraw();
    }

    fn current_referral_offer(&self) -> Option<&ReferralOffer> {
        let loaded_at = self.referral_state.offer_loaded_at?;
        if loaded_at.elapsed() > REFERRAL_OFFER_MAX_AGE {
            return None;
        }
        self.referral_state.offer.as_ref()
    }
}
