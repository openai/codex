use super::BreakdownDimension;
use super::format_credit_micros;
use super::format_model_display_name;
use super::grouped_usage;
use codex_app_server_protocol::ThreadUsageBreakdownGroup;
use pretty_assertions::assert_eq;

#[test]
fn thread_usage_breakdown_groups_aggregate_model_credit_percentages() {
    let group = ThreadUsageBreakdownGroup {
        model: Some("gpt-5.4".to_string()),
        reasoning_effort: Some("high".to_string()),
        speed: Some("fast".to_string()),
        estimated_usage_credits_micros: 30_000_000,
        net_new_input_tokens: Some(60),
        cached_input_tokens: Some(10),
        input_tokens: Some(70),
        output_tokens: Some(20),
        total_tokens: Some(90),
    };
    let groups = vec![
        group.clone(),
        ThreadUsageBreakdownGroup {
            reasoning_effort: Some("medium".to_string()),
            speed: Some("standard".to_string()),
            estimated_usage_credits_micros: 10_000_000,
            ..group.clone()
        },
        ThreadUsageBreakdownGroup {
            model: Some("gpt-5-mini".to_string()),
            reasoning_effort: Some("low".to_string()),
            speed: Some("standard".to_string()),
            estimated_usage_credits_micros: 10_000_000,
            ..group
        },
    ];

    assert_eq!(
        grouped_usage(&groups, BreakdownDimension::Model),
        Some("GPT-5.4 80%, GPT-5 Mini 20%".to_string())
    );
    assert_eq!(
        grouped_usage(&groups, BreakdownDimension::Reasoning),
        Some("Light 20%, Medium 20%, High 60%".to_string())
    );
    assert_eq!(
        grouped_usage(&groups, BreakdownDimension::Speed),
        Some("Fast mode 60%, Standard 40%".to_string())
    );
}

#[test]
fn thread_usage_credit_formatting_rounds_to_one_decimal_place() {
    assert_eq!(format_credit_micros(/*micros*/ 0), "0");
    assert_eq!(format_credit_micros(/*micros*/ 400_000), "0.4");
    assert_eq!(format_credit_micros(/*micros*/ 5_200_000), "5.2");
    assert_eq!(format_credit_micros(/*micros*/ 5_240_000), "5.2");
    assert_eq!(format_credit_micros(/*micros*/ 5_250_000), "5.3");
    assert_eq!(format_credit_micros(/*micros*/ 46_000_000), "46");
    assert_eq!(format_credit_micros(/*micros*/ 1_240_000_000), "1.2K");
    assert_eq!(format_credit_micros(/*micros*/ 12_400_000_000), "12.4K");
}

#[test]
fn thread_usage_model_names_match_desktop_display_names() {
    assert_eq!(format_model_display_name("gpt-5.4"), "GPT-5.4");
    assert_eq!(format_model_display_name("gpt-5-mini"), "GPT-5 Mini");
    assert_eq!(format_model_display_name("gpt-5.6-sol"), "GPT-5.6 Sol");
    assert_eq!(format_model_display_name("mewfour"), "mewfour");
}
