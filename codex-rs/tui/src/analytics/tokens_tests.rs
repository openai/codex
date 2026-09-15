//! Enterprise token normalization across supported groupings.

use super::*;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn tokens_use_text_components_and_keep_daily_totals_across_groupings() {
    let response: AnalyticsData = serde_json::from_value(json!({"units":"credits", "data":[{
        "date":"2026-09-01", "groups":[{"dimensions":{"model":"gpt-5"}, "uncached_text_input_tokens":10, "cached_text_input_tokens":80, "text_output_tokens":10}]
    }]})).unwrap();
    let date = "2026-09-01".parse().unwrap();
    let by_type = history(
        response.clone(),
        AccountAnalyticsGrouping::TokenType,
        date,
        date,
    )
    .unwrap();
    let by_model = history(response, AccountAnalyticsGrouping::Model, date, date).unwrap();
    assert_eq!(
        (by_type.unit, by_type.data[0].total, by_model.data[0].total),
        (AccountAnalyticsUnit::Tokens, 100.0, 100.0)
    );
    assert_eq!(
        by_type.data[0]
            .values
            .iter()
            .map(|v| (v.label.as_str(), v.value))
            .collect::<Vec<_>>(),
        vec![
            ("Cached input", 80.0),
            ("Output", 10.0),
            ("Uncached input", 10.0)
        ]
    );
}

#[test]
fn token_model_filter_retains_components_and_freshness() {
    let date = "2026-09-01".parse().unwrap();
    let response: AnalyticsData = serde_json::from_value(json!({"data_freshness_ts":"2026-09-02T00:00:00Z", "data":[{
        "date":"2026-09-01", "groups":[
            {"dimensions":{"model":"alpha"}, "uncached_text_input_tokens":10,"cached_text_input_tokens":20,"text_output_tokens":30},
            {"dimensions":{"model":"beta"}, "uncached_text_input_tokens":100,"cached_text_input_tokens":200,"text_output_tokens":300}
        ], "models":[{"model":"ignored", "text_total_tokens":1000}]
    }]})).unwrap();
    let selected = filtered_history(
        response.clone(),
        AccountAnalyticsGrouping::TokenType,
        date,
        date,
        Some("alpha"),
    )
    .unwrap();
    let expected = history(serde_json::from_value(json!({"data_freshness_ts":"2026-09-02T00:00:00Z", "data":[{
        "date":"2026-09-01", "models":[{"model":"alpha", "uncached_text_input_tokens":10,"cached_text_input_tokens":20,"text_output_tokens":30}]
    }]})).unwrap(), AccountAnalyticsGrouping::TokenType, date, date).unwrap();
    assert_eq!(selected, expected);
    assert_eq!(
        history(response, AccountAnalyticsGrouping::TokenType, date, date)
            .unwrap()
            .data[0]
            .total,
        660.0
    );
}

#[test]
fn model_totals_use_reported_total_and_missing_components_remain_unavailable() {
    let date = "2026-09-01".parse().unwrap();
    let response: AnalyticsData = serde_json::from_value(json!({"data":[{"date":"2026-09-01", "models":[{"model":"alpha", "text_total_tokens":123}]}]})).unwrap();
    assert_eq!(
        history(
            response.clone(),
            AccountAnalyticsGrouping::Model,
            date,
            date
        )
        .unwrap()
        .data,
        vec![AccountAnalyticsDay {
            date: "2026-09-01".parse().unwrap(),
            total: 123.0,
            values: vec![AccountAnalyticsValue {
                key: "alpha".into(),
                label: "alpha".into(),
                value: 123.0
            }]
        }]
    );
    assert_eq!(
        history(response, AccountAnalyticsGrouping::TokenType, date, date).unwrap_err(),
        "Token counts were not reported."
    );
}

#[test]
fn explicit_zero_model_total_is_reported_without_components() {
    let date = "2026-09-01".parse().unwrap();
    let response: AnalyticsData = serde_json::from_value(json!({
        "data": [{"date": "2026-09-01", "models": [
            {"model": "idle", "text_total_tokens": 0},
            {"model": "active", "text_total_tokens": 0, "text_output_tokens": 5}
        ]}]
    }))
    .unwrap();
    assert_eq!(
        history(response, AccountAnalyticsGrouping::Model, date, date).unwrap(),
        AccountAnalyticsHistory {
            unit: AccountAnalyticsUnit::Tokens,
            updated_at: None,
            data: vec![AccountAnalyticsDay {
                date: "2026-09-01".parse().unwrap(),
                total: 5.0,
                values: vec![
                    AccountAnalyticsValue {
                        key: "active".into(),
                        label: "active".into(),
                        value: 5.0,
                    },
                    AccountAnalyticsValue {
                        key: "idle".into(),
                        label: "idle".into(),
                        value: 0.0,
                    },
                ],
            }],
        }
    );
}
