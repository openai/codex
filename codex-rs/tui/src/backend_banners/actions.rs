//! Map backend CTA names to existing CLI actions and desktop-equivalent browser destinations.

use super::BackendBanner;
use codex_app_server_protocol::AddCreditsNudgeCreditType;
use codex_protocol::account::PlanType;

const USAGE_URL: &str = "https://chatgpt.com/settings/usage";
const WORKSPACE_USAGE_URL: &str = "https://chatgpt.com/admin/usage-limits/workspace";

pub(super) enum BannerAction {
    OpenUrl(String),
    NotifyOwner(AddCreditsNudgeCreditType),
    ResetUsage,
}

impl BackendBanner {
    pub(super) fn resolve_action(&self, action: &str) -> Option<BannerAction> {
        match action {
            "notify_owner" | "contact_owner" => {
                return Some(BannerAction::NotifyOwner(
                    AddCreditsNudgeCreditType::Credits,
                ));
            }
            "request_increase" => {
                let Some(destination) = self.request_url.as_deref() else {
                    return Some(BannerAction::NotifyOwner(
                        AddCreditsNudgeCreditType::UsageLimit,
                    ));
                };
                let url = url::Url::parse(destination).ok()?;
                if !matches!(url.scheme(), "https" | "http")
                    || url.host_str().is_none()
                    || !url.username().is_empty()
                    || url.password().is_some()
                {
                    return None;
                }
                return Some(BannerAction::OpenUrl(url.to_string()));
            }
            // Keep the existing picker and explicit confirmation before consuming a reset.
            "reset_usage" => return Some(BannerAction::ResetUsage),
            _ => {}
        }
        let destination = match action {
            "add_credits" | "buy_credits"
                if self.plan_type.is_some_and(PlanType::is_workspace_account) =>
            {
                "https://chatgpt.com/admin/billing?codex_credit_action=add_credits".to_string()
            }
            "add_credits" | "buy_credits" => format!("{USAGE_URL}?credits_modal=true"),
            "buy_reset" => "https://chatgpt.com/codex/purchase/reset".to_string(),
            "view_usage" | "request_increase_usage_settings" => USAGE_URL.to_string(),
            "view_workspace_usage" | "increase_spend_cap" => WORKSPACE_USAGE_URL.to_string(),
            "open_plus_pricing_web" => "https://chatgpt.com/explore/plus".to_string(),
            "open_pro_pricing_web" => "https://chatgpt.com/explore/pro".to_string(),
            "open_pricing_dialog" => {
                let mut url = url::Url::parse("https://chatgpt.com/").ok()?;
                let target = if matches!(self.plan_type, Some(PlanType::Plus | PlanType::ProLite)) {
                    "pro"
                } else {
                    "plus"
                };
                url.query_pairs_mut()
                    .append_pair("cta_tab", "personal")
                    .append_pair("highlight_plan", target);
                if self.plan_type == Some(PlanType::ProLite) {
                    url.query_pairs_mut().append_pair("pro_variant", "2x");
                }
                url.set_fragment(Some("pricing"));
                url.to_string()
            }
            // Desktop-only referral and Premium dialogs need their own CLI flow.
            _ => return None,
        };
        let mut destination = url::Url::parse(&destination).ok()?;
        if destination.path().starts_with("/admin/") {
            if self.account_id.is_empty() {
                return None;
            }
            // The existing admin route selects the requested workspace before opening the modal.
            destination
                .query_pairs_mut()
                .append_pair("account_id", &self.account_id);
        }
        Some(BannerAction::OpenUrl(destination.to_string()))
    }
}
