//! Projects endpoint-specific backend models into inputs for display normalization.
//! This representation is never used to deserialize HTTP responses.
//! Optional fields select the inputs used by each report; missing backend values stay missing.

use codex_backend_client::AnalyticsResponse;
use codex_backend_client::analytics_models as backend;

/// Report values projected for normalization after endpoint-specific decoding.
#[derive(Clone, Debug, Default)]
#[cfg_attr(test, derive(serde::Deserialize))]
pub(super) struct AnalyticsData {
    pub(super) data: Vec<AnalyticsRecord>,
    pub(super) units: Option<String>,
    pub(super) data_freshness_ts: Option<String>,
    pub(super) series: Option<Vec<AnalyticsSeries>>,
}

#[derive(Clone, Debug, Default)]
#[cfg_attr(test, derive(serde::Deserialize))]
pub(super) struct AnalyticsSeries {
    pub(super) key: String,
    pub(super) label: String,
}

#[derive(Clone, Debug, Default)]
#[cfg_attr(test, derive(serde::Deserialize))]
pub(super) struct AnalyticsRecord {
    pub(super) date: String,
    pub(super) values: Option<std::collections::BTreeMap<String, f64>>,
    pub(super) product_surface_usage_values: Option<std::collections::BTreeMap<String, f64>>,
    pub(super) premium_usage_values: Option<PremiumUsage>,
    pub(super) attribution: Option<Vec<UsageAttribution>>,
    pub(super) credit_amount: Option<f64>,
    pub(super) product_surface: Option<String>,
    pub(super) totals: Option<AnalyticsCount>,
    pub(super) models: Option<Vec<AnalyticsModel>>,
    pub(super) groups: Option<Vec<AnalyticsTokenGroup>>,
    pub(super) clients: Option<Vec<AnalyticsClient>>,
    pub(super) plugin_usage_overviews: Option<Vec<AnalyticsTool>>,
    pub(super) skill_usage_overviews: Option<Vec<AnalyticsTool>>,
}

#[derive(Clone, Debug, Default)]
#[cfg_attr(test, derive(serde::Deserialize))]
pub(super) struct PremiumUsage {
    pub(super) credit_usage_credits: std::collections::BTreeMap<String, f64>,
}

#[derive(Clone, Debug, Default)]
#[cfg_attr(test, derive(serde::Deserialize))]
pub(super) struct UsageAttribution {
    pub(super) thread_source: Option<String>,
    pub(super) turn_trigger: Option<String>,
    pub(super) model: Option<String>,
    pub(super) surface: Option<String>,
    pub(super) value: f64,
}

#[derive(Clone, Debug, Default)]
#[cfg_attr(test, derive(serde::Deserialize))]
pub(super) struct AnalyticsCount {
    pub(super) turns: f64,
}

#[derive(Clone, Debug, Default)]
#[cfg_attr(test, derive(serde::Deserialize))]
pub(super) struct AnalyticsModel {
    pub(super) model: String,
    pub(super) credits: Option<f64>,
    pub(super) uncached_text_input_tokens: Option<f64>,
    pub(super) cached_text_input_tokens: Option<f64>,
    pub(super) text_output_tokens: Option<f64>,
    pub(super) text_total_tokens: Option<f64>,
    pub(super) turns: Option<f64>,
    pub(super) on_demand_credits: Option<f64>,
    pub(super) speed: Option<String>,
}

/// Enterprise model groups carry text token counts independently of billing amounts.
#[derive(Clone, Debug, Default)]
#[cfg_attr(test, derive(serde::Deserialize))]
pub(super) struct AnalyticsTokenGroup {
    pub(super) dimensions: std::collections::BTreeMap<String, String>,
    #[cfg_attr(test, serde(default))]
    pub(super) is_other: bool,
    pub(super) uncached_text_input_tokens: Option<f64>,
    pub(super) cached_text_input_tokens: Option<f64>,
    pub(super) text_output_tokens: Option<f64>,
}

#[derive(Clone, Debug, Default)]
#[cfg_attr(test, derive(serde::Deserialize))]
pub(super) struct AnalyticsClient {
    pub(super) client_id: String,
    pub(super) turns: f64,
}

#[derive(Clone, Debug, Default)]
#[cfg_attr(test, derive(serde::Deserialize))]
pub(super) struct AnalyticsTool {
    pub(super) display_name: String,
    pub(super) invocation_counts: f64,
}

impl From<AnalyticsResponse> for AnalyticsData {
    fn from(response: AnalyticsResponse) -> Self {
        match response {
            AnalyticsResponse::Usage(response) => Self {
                units: response.units,
                data_freshness_ts: response.data_freshness_ts,
                data: response
                    .data
                    .into_iter()
                    .map(|record| AnalyticsRecord {
                        date: record.date,
                        product_surface_usage_values: Some(record.product_surface_usage_values),
                        premium_usage_values: record.premium_usage_values.map(|value| {
                            PremiumUsage {
                                credit_usage_credits: value.credit_usage_credits,
                            }
                        }),
                        attribution: record.attribution.map(|values| {
                            values
                                .into_iter()
                                .map(|value| UsageAttribution {
                                    thread_source: Some(value.thread_source),
                                    turn_trigger: Some(value.turn_trigger),
                                    model: Some(value.model),
                                    surface: Some(value.surface),
                                    value: value.value,
                                })
                                .collect()
                        }),
                        models: record
                            .models
                            .map(|values| values.into_iter().map(AnalyticsModel::from).collect()),
                        groups: record.groups.map(|values| {
                            values
                                .into_iter()
                                .map(|value| AnalyticsTokenGroup {
                                    dimensions: value.dimensions,
                                    is_other: value.is_other.unwrap_or(/*default*/ false),
                                    uncached_text_input_tokens: value
                                        .uncached_text_input_tokens
                                        .map(|count| count as f64),
                                    cached_text_input_tokens: value
                                        .cached_text_input_tokens
                                        .map(|count| count as f64),
                                    text_output_tokens: value
                                        .text_output_tokens
                                        .map(|count| count as f64),
                                })
                                .collect()
                        }),
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            },
            AnalyticsResponse::Credits(response) => Self {
                data: response
                    .data
                    .into_iter()
                    .map(|record| AnalyticsRecord {
                        date: record.date,
                        credit_amount: Some(record.credit_amount),
                        product_surface: Some(record.product_surface),
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            },
            AnalyticsResponse::EnterpriseCredits(response) => Self {
                data_freshness_ts: response.data_freshness_ts,
                series: Some(
                    response
                        .series
                        .into_iter()
                        .map(|value| AnalyticsSeries {
                            key: value.key,
                            label: value.label,
                        })
                        .collect(),
                ),
                data: response
                    .data
                    .into_iter()
                    .map(|record| AnalyticsRecord {
                        date: record.date,
                        values: Some(record.values),
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            },
            AnalyticsResponse::Messages(response) => Self {
                data: response
                    .data
                    .into_iter()
                    .map(|record| AnalyticsRecord {
                        date: record.date,
                        totals: Some(AnalyticsCount {
                            turns: record.totals.turns as f64,
                        }),
                        clients: Some(
                            record
                                .clients
                                .into_iter()
                                .map(|value| AnalyticsClient {
                                    client_id: value.client_id,
                                    turns: value.turns as f64,
                                })
                                .collect(),
                        ),
                        models: record
                            .models
                            .map(|values| values.into_iter().map(AnalyticsModel::from).collect()),
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            },
            AnalyticsResponse::Plugins(response) => Self {
                data_freshness_ts: response.data_freshness_ts,
                data: response
                    .data
                    .into_iter()
                    .map(|record| AnalyticsRecord {
                        date: record.date,
                        plugin_usage_overviews: Some(
                            record
                                .plugin_usage_overviews
                                .into_iter()
                                .map(|value| AnalyticsTool {
                                    display_name: value.display_name,
                                    invocation_counts: value.invocation_counts as f64,
                                })
                                .collect(),
                        ),
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            },
            AnalyticsResponse::Skills(response) => Self {
                data_freshness_ts: response.data_freshness_ts,
                data: response
                    .data
                    .into_iter()
                    .map(|record| AnalyticsRecord {
                        date: record.date,
                        skill_usage_overviews: Some(
                            record
                                .skill_usage_overviews
                                .into_iter()
                                .map(|value| AnalyticsTool {
                                    display_name: value.display_name,
                                    invocation_counts: value.invocation_counts as f64,
                                })
                                .collect(),
                        ),
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            },
        }
    }
}

impl From<backend::ModelUsage> for AnalyticsModel {
    fn from(value: backend::ModelUsage) -> Self {
        Self {
            model: value.model,
            credits: Some(value.credits),
            uncached_text_input_tokens: value.uncached_text_input_tokens.map(|count| count as f64),
            cached_text_input_tokens: value.cached_text_input_tokens.map(|count| count as f64),
            text_output_tokens: value.text_output_tokens.map(|count| count as f64),
            text_total_tokens: value.text_total_tokens.map(|count| count as f64),
            turns: value.turns.map(|count| count as f64),
            on_demand_credits: value.on_demand_credits,
            speed: value.speed,
        }
    }
}

#[cfg(test)]
#[path = "report_data_tests.rs"]
mod tests;
