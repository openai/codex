//! Private analytics display types, independent of the app-server wire protocol.

use codex_protocol::account::PlanType;

/// Billing families match the App's consumer, business, and workspace scopes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AccountKind {
    Consumer,
    Business,
    Enterprise,
    Unknown,
}

impl From<Option<PlanType>> for AccountKind {
    fn from(plan: Option<PlanType>) -> Self {
        match plan {
            None | Some(PlanType::Unknown) => Self::Unknown,
            Some(plan) if plan.is_team_like() => Self::Business,
            Some(plan) if plan.is_workspace_account() => Self::Enterprise,
            Some(_) => Self::Consumer,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AccountAnalyticsReport {
    Usage,
    Credits,
    Messages,
    Plugins,
    Skills,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AccountAnalyticsGrouping {
    Feature,
    Model,
    Surface,
    TaskStart,
    Speed,
    Reasoning,
    TokenType,
}

impl AccountAnalyticsGrouping {
    /// Credit breakdowns supported by the account's billing report.
    pub(crate) fn credit_groupings(plan: Option<PlanType>) -> &'static [Self] {
        match AccountKind::from(plan) {
            AccountKind::Business => &[Self::Surface, Self::Model, Self::Speed],
            AccountKind::Enterprise => &[Self::Surface, Self::Model, Self::Speed, Self::Reasoning],
            AccountKind::Consumer | AccountKind::Unknown => &[Self::Surface],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AccountAnalyticsUnit {
    RelativeUsage,
    Credits,
    Count,
    Tokens,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AccountAnalyticsHistory {
    pub(crate) unit: AccountAnalyticsUnit,
    /// Source freshness in Unix seconds, when provided. Not a completeness watermark.
    pub(crate) updated_at: Option<i64>,
    pub(crate) data: Vec<AccountAnalyticsDay>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AccountAnalyticsDay {
    /// Source reporting date; display formatting belongs to the UI.
    pub(crate) date: chrono::NaiveDate,
    /// Full daily denominator, including non-task usage when grouped by task start.
    pub(crate) total: f64,
    pub(crate) values: Vec<AccountAnalyticsValue>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AccountAnalyticsValue {
    pub(crate) key: String,
    pub(crate) label: String,
    pub(crate) value: f64,
}
