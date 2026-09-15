//! Shared report category aggregation for token model filtering.

use super::models::AccountAnalyticsHistory;
use super::models::AccountAnalyticsValue;
use std::collections::BTreeMap;

pub(super) fn categories(history: &AccountAnalyticsHistory) -> Vec<AccountAnalyticsValue> {
    let mut totals = BTreeMap::<String, AccountAnalyticsValue>::new();
    for value in history.data.iter().flat_map(|day| &day.values) {
        totals
            .entry(value.key.clone())
            .or_insert_with(|| AccountAnalyticsValue {
                value: 0.0,
                ..value.clone()
            })
            .value += value.value;
    }
    let mut values = totals.into_values().collect::<Vec<_>>();
    values.sort_by(|a, b| {
        b.value
            .abs()
            .total_cmp(&a.value.abs())
            .then_with(|| a.key.cmp(&b.key))
    });
    values
}
