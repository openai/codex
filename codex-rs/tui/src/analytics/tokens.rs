//! Enterprise text token history, independent of credit and allowance accounting.

use super::models::*;
use super::report_data::AnalyticsData;
use chrono::NaiveDate;
use std::collections::BTreeMap;

pub(super) fn history(
    response: AnalyticsData,
    grouping: AccountAnalyticsGrouping,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<AccountAnalyticsHistory, String> {
    filtered_history(response, grouping, start, end, /*model_filter*/ None)
}

pub(super) fn filtered_history(
    response: AnalyticsData,
    grouping: AccountAnalyticsGrouping,
    start: NaiveDate,
    end: NaiveDate,
    model_filter: Option<&str>,
) -> Result<AccountAnalyticsHistory, String> {
    let mut days: BTreeMap<NaiveDate, BTreeMap<String, f64>> = BTreeMap::new();
    for record in response.data {
        let date = record
            .date
            .parse::<NaiveDate>()
            .map_err(|_| "Invalid token report date.")?;
        if date < start || date > end {
            continue;
        }
        let groups = if let Some(groups) = record.groups {
            groups
                .into_iter()
                .map(|group| {
                    (
                        if group.is_other {
                            "Other".into()
                        } else {
                            group
                                .dimensions
                                .get("model")
                                .cloned()
                                .unwrap_or_else(|| "Unknown".into())
                        },
                        [
                            group.uncached_text_input_tokens,
                            group.cached_text_input_tokens,
                            group.text_output_tokens,
                        ],
                        None,
                    )
                })
                .collect::<Vec<_>>()
        } else if let Some(models) = record.models {
            models
                .into_iter()
                .map(|model| {
                    (
                        model.model,
                        [
                            model.uncached_text_input_tokens,
                            model.cached_text_input_tokens,
                            model.text_output_tokens,
                        ],
                        model.text_total_tokens,
                    )
                })
                .collect()
        } else {
            return Err("Token breakdown is not available in this report.".into());
        };
        let values = days.entry(date).or_default();
        for (model, counts, total) in groups {
            if model_filter.is_some_and(|selected| selected != model) {
                continue;
            }
            let missing_components = counts.iter().all(Option::is_none);
            if grouping == AccountAnalyticsGrouping::Model
                && let Some(total) = total.filter(|total| *total != 0.0 || missing_components)
            {
                if !total.is_finite() || total < 0.0 || total.fract() != 0.0 {
                    return Err("Invalid token count.".into());
                }
                *values.entry(model).or_default() += total;
                continue;
            }
            if missing_components {
                return Err("Token counts were not reported.".into());
            }
            for (label, count) in ["Uncached input", "Cached input", "Output"]
                .into_iter()
                .zip(counts)
            {
                let count = count.unwrap_or(/*default*/ 0.0);
                if !count.is_finite() || count < 0.0 || count.fract() != 0.0 {
                    return Err("Invalid token count.".into());
                }
                let key = if grouping == AccountAnalyticsGrouping::Model {
                    &model
                } else {
                    label
                };
                *values.entry(key.to_string()).or_default() += count;
            }
        }
    }
    Ok(AccountAnalyticsHistory {
        unit: AccountAnalyticsUnit::Tokens,
        updated_at: response
            .data_freshness_ts
            .and_then(|time| chrono::DateTime::parse_from_rfc3339(&time).ok())
            .map(|time| time.timestamp()),
        data: days
            .into_iter()
            .map(|(date, values)| AccountAnalyticsDay {
                date,
                total: values.values().sum(),
                values: values
                    .into_iter()
                    .map(|(key, value)| AccountAnalyticsValue {
                        label: key.clone(),
                        key,
                        value,
                    })
                    .collect(),
            })
            .collect(),
    })
}

#[cfg(test)]
#[path = "tokens_tests.rs"]
mod tests;
