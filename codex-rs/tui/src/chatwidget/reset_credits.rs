use crate::status::RateLimitSnapshotDisplay;
use chrono::DateTime;
use chrono::Local;
use chrono::Utc;
use codex_app_server_protocol::RateLimitResetCreditStatus;
use codex_app_server_protocol::RateLimitResetCreditsSummary;
use codex_app_server_protocol::RateLimitResetType;
use codex_protocol::account::PlanType;
use std::collections::BTreeMap;

use super::rate_limits::get_limits_duration;

pub(super) enum RateLimitResetScope {
    Monthly,
    WeeklyAndFiveHour,
    Unknown,
}

impl RateLimitResetScope {
    pub(super) fn picker_label(&self) -> &'static str {
        match self {
            Self::Monthly => "Full reset (Monthly)",
            Self::WeeklyAndFiveHour => "Full reset (Weekly + 5h)",
            Self::Unknown => "Full reset",
        }
    }

    pub(super) fn usage_description(&self) -> &'static str {
        match self {
            Self::Monthly => "Reset your current monthly usage limit.",
            Self::WeeklyAndFiveHour => "Reset your current 5-hour and weekly usage limits.",
            Self::Unknown => "Reset your current usage limits.",
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct ResetCreditOption {
    pub(super) credit_id: Option<String>,
    pub(super) name: String,
    pub(super) description: String,
}

pub(super) fn rate_limit_reset_scope(
    rate_limits: &BTreeMap<String, RateLimitSnapshotDisplay>,
    plan_type: Option<PlanType>,
) -> RateLimitResetScope {
    let window_labels = rate_limits
        .iter()
        .find(|(limit_id, _)| limit_id.eq_ignore_ascii_case("codex"))
        .into_iter()
        .flat_map(|(_, snapshot)| [snapshot.primary.as_ref(), snapshot.secondary.as_ref()])
        .flatten()
        .filter_map(|window| window.window_minutes.and_then(get_limits_duration))
        .collect::<Vec<_>>();

    if window_labels.iter().any(|label| label == "monthly")
        || matches!(plan_type, Some(PlanType::Free | PlanType::Go))
    {
        RateLimitResetScope::Monthly
    } else if window_labels
        .iter()
        .any(|label| label == "5h" || label == "weekly")
    {
        RateLimitResetScope::WeeklyAndFiveHour
    } else {
        RateLimitResetScope::Unknown
    }
}

pub(super) fn reset_credit_options(
    summary: &RateLimitResetCreditsSummary,
    scope: RateLimitResetScope,
) -> Vec<ResetCreditOption> {
    let available_count = summary.available_count.max(0);
    let detail_limit = usize::try_from(available_count).unwrap_or(usize::MAX);
    let mut available_credits = summary
        .credits
        .as_deref()
        .unwrap_or_default()
        .iter()
        .filter(|credit| credit.status == RateLimitResetCreditStatus::Available)
        .collect::<Vec<_>>();
    available_credits.sort_by_key(|credit| credit.expires_at.unwrap_or(i64::MAX));

    let mut options = available_credits
        .into_iter()
        .take(detail_limit)
        .map(|credit| {
            let expiration = match credit.expires_at {
                Some(expires_at) => DateTime::<Utc>::from_timestamp(expires_at, 0)
                    .map(|expires_at| {
                        format!(
                            "Expires {}",
                            expires_at
                                .with_timezone(&Local)
                                .format("%H:%M on %-d %b %Y")
                        )
                    })
                    .unwrap_or_else(|| "Expiration unavailable".to_string()),
                None => "Does not expire".to_string(),
            };
            let reset_label = credit
                .title
                .as_deref()
                .filter(|title| !title.trim().is_empty())
                .unwrap_or_else(|| match credit.reset_type {
                    RateLimitResetType::CodexRateLimits | RateLimitResetType::Unknown => {
                        scope.picker_label()
                    }
                });
            ResetCreditOption {
                credit_id: Some(credit.id.clone()),
                name: reset_label.to_string(),
                description: format!("{expiration}."),
            }
        })
        .collect::<Vec<_>>();

    if options.is_empty() {
        options.push(ResetCreditOption {
            credit_id: None,
            name: "Use a reset".to_string(),
            description: scope.usage_description().to_string(),
        });
    }

    options
}
