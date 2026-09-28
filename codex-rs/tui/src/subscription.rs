//! Subscription labels for TUI surfaces, preserving status-specific plan detail.

use codex_protocol::account::PlanType;

pub(crate) enum SubscriptionDisplay {
    Status,
    Analytics,
}

impl SubscriptionDisplay {
    pub(crate) fn label(&self, plan: PlanType) -> &'static str {
        match (plan, self) {
            (PlanType::Free, _) => "Free",
            (PlanType::Go, _) => "Go",
            (PlanType::Plus, _) => "Plus",
            (PlanType::Pro, _) => "Pro 200",
            (PlanType::ProLite, _) => "Pro 100",
            (PlanType::ProMax, _) => "Pro 500",
            (PlanType::Team | PlanType::SelfServeBusinessUsageBased, _) => "Business",
            (PlanType::Business, Self::Status) => "Enterprise",
            (PlanType::SelfServeBusinessProLite, Self::Status) => "Business Premium",
            (PlanType::Business | PlanType::SelfServeBusinessProLite, Self::Analytics) => {
                "Business"
            }
            (PlanType::EnterpriseCbpAutomation, Self::Status) => "Enterprise (Automation)",
            (PlanType::EnterpriseCbpAutomation, Self::Analytics)
            | (PlanType::Enterprise | PlanType::Ent26 | PlanType::EnterpriseCbpUsageBased, _) => {
                "Enterprise"
            }
            (PlanType::Edu, Self::Status) => "Edu",
            (PlanType::EduPlus, Self::Status) => "Edu Plus",
            (PlanType::EduPro, Self::Status) => "Edu Pro",
            (PlanType::Edu | PlanType::EduPlus | PlanType::EduPro, Self::Analytics) => "Education",
            (PlanType::Unknown, Self::Status) => "Unknown",
            (PlanType::Unknown, Self::Analytics) => "Account",
        }
    }
}
