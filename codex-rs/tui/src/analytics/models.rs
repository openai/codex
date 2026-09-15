//! Private analytics display types, independent of the app-server wire protocol.

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
