//! Render backend-owned copy and ordered CTA labels with the shared inline banner UI.

use super::BackendBanner;
use super::BannerPresentation;
use super::actions::BannerAction;
use crate::app_event::AppEvent;
use crate::bottom_pane::ActionableBanner;
use crate::bottom_pane::BannerDismissal;
use crate::bottom_pane::SelectionItem;
use crate::clock_format::ClockFormat;
use chrono::DateTime;
use chrono::Local;

impl BackendBanner {
    pub(crate) fn actionable_banner(&self, clock_format: ClockFormat) -> ActionableBanner {
        let reset_time = self
            .reset_at
            .and_then(|timestamp| DateTime::from_timestamp(timestamp, /*nsecs*/ 0))
            .map(|time| {
                crate::status::format_reset_timestamp(
                    time.with_timezone(&Local),
                    Local::now(),
                    clock_format,
                )
            });
        let copy = |text: &str| {
            let text: String = text
                .chars()
                .filter(|c| !c.is_control() || *c == '\n')
                .collect();
            match reset_time.as_deref() {
                Some(reset_time) => text.replace("{time}", reset_time),
                None => text,
            }
        };
        let actions = self
            .ctas
            .iter()
            .filter_map(|cta| {
                if cta.label.trim().is_empty()
                    || cta.label.len() > 256
                    || cta.label.chars().any(char::is_control)
                {
                    return None;
                }
                let action = self.resolve_action(&cta.action)?;
                Some(SelectionItem {
                    name: cta.label.clone(),
                    actions: vec![Box::new(move |tx| {
                        tx.send(match &action {
                            BannerAction::OpenUrl(url) => {
                                AppEvent::OpenUrlInBrowser { url: url.clone() }
                            }
                            BannerAction::NotifyOwner(credit_type) => {
                                AppEvent::SendAddCreditsNudgeEmail {
                                    credit_type: *credit_type,
                                }
                            }
                            BannerAction::ResetUsage => AppEvent::OpenRateLimitResetCredits,
                        })
                    })],
                    dismiss_on_select: false,
                    ..Default::default()
                })
            })
            .collect::<Vec<_>>();
        ActionableBanner {
            title: copy(&self.title),
            description: copy(&self.description),
            actions,
            dismissal: if self.presentation == BannerPresentation::Dismissible {
                BannerDismissal::Dismissible
            } else {
                BannerDismissal::Persistent
            },
            ..Default::default()
        }
    }
}
