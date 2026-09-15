use super::*;
use pretty_assertions::assert_eq;

#[test]
fn typed_usage_preserves_missing_counts_and_supplied_zero() {
    let input = backend::DailyProductSurfaceUsageResponse {
        data: vec![backend::DailyProductSurfaceUsage {
            date: "2026-01-09".into(),
            models: Some(vec![backend::ModelUsage {
                model: "example".into(),
                credits: 1.5,
                cached_text_input_tokens: Some(0),
                text_output_tokens: Some(42),
                ..Default::default()
            }]),
            ..Default::default()
        }],
        ..Default::default()
    };
    let actual = AnalyticsData::from(AnalyticsResponse::Usage(input));
    let model = &actual.data[0].models.as_ref().unwrap()[0];
    assert_eq!(
        (
            model.credits,
            model.uncached_text_input_tokens,
            model.cached_text_input_tokens,
            model.text_output_tokens
        ),
        (Some(1.5), None, Some(0.0), Some(42.0))
    );
}

#[test]
fn typed_message_records_keep_independent_daily_remainders() {
    use super::super::models::*;
    let date = "2026-01-09".parse().unwrap();
    let row = backend::DailyWorkspaceUsageCount {
        date: "2026-01-09".into(),
        totals: backend::WorkspaceUsageCount {
            turns: 10,
            ..Default::default()
        },
        clients: vec![backend::ClientWorkspaceUsageCount {
            client_id: "CODEX_CLI".into(),
            turns: 8,
            ..Default::default()
        }],
        ..Default::default()
    };
    let response = AnalyticsResponse::Messages(backend::DailyWorkspaceUsageCountResponse {
        data: vec![row.clone(), row],
        ..Default::default()
    });
    assert_eq!(
        super::super::normalize::history(
            response.into(),
            AccountAnalyticsReport::Messages,
            AccountAnalyticsGrouping::Surface,
            date,
            date
        )
        .unwrap(),
        Some(AccountAnalyticsHistory {
            unit: AccountAnalyticsUnit::Count,
            updated_at: None,
            data: vec![AccountAnalyticsDay {
                date,
                total: 20.0,
                values: vec![
                    AccountAnalyticsValue {
                        key: "cli".into(),
                        label: "CLI".into(),
                        value: 16.0
                    },
                    AccountAnalyticsValue {
                        key: "other".into(),
                        label: "Other".into(),
                        value: 4.0
                    },
                ]
            }],
        })
    );
}
