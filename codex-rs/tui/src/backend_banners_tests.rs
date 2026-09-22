use super::BackendBanner;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn parse_rejects_more_actions_than_the_inline_banner_can_display() {
    let labels: Vec<_> = (1..=8).map(|index| format!("Action {index}")).collect();
    let mut raw = json!({
        "banner_type": "usage_limit", "title": "Usage limit reached",
        "description": "Choose how to continue.",
        "ctas": labels.iter().map(|label| json!({
            "action": "view_usage", "label": label,
        })).collect::<Vec<_>>(),
    });
    let banner = BackendBanner::parse(&raw).expect("eight actions fit");
    assert_eq!(
        banner
            .actionable_banner(crate::clock_format::ClockFormat::TwentyFourHour)
            .actions
            .into_iter()
            .map(|action| action.name)
            .collect::<Vec<_>>(),
        labels
    );

    raw["ctas"].as_array_mut().unwrap().push(json!({
        "action": "view_usage", "label": "Hidden ninth action",
    }));
    assert_eq!(BackendBanner::parse(&raw), None);
}

#[test]
fn parse_rejects_unsupported_or_unrenderable_content() {
    let valid = json!({
        "banner_type": "usage_limit", "title": "Usage limit reached",
        "description": "Choose how to continue.", "ctas": [],
    });
    for invalid_fields in [
        json!({"presentation":"future_mode"}),
        json!({"presentation":null}),
        json!({"title":" "}),
        json!({"title":"x".repeat(1025)}),
        json!({"title":"line\n".repeat(4)}),
        json!({"description":"x".repeat(4097)}),
        json!({"description":"line\n".repeat(13)}),
        json!({"blocked_model_slug":""}),
        json!({"blocked_model_slug":"bad\nslug"}),
        json!({"fallback_model_slugs":["x".repeat(257)]}),
        json!({"fallback_model_slugs":vec!["model"; 17]}),
    ] {
        let mut raw = valid.clone();
        raw.as_object_mut()
            .unwrap()
            .extend(invalid_fields.as_object().unwrap().clone());
        assert_eq!(BackendBanner::parse(&raw), None, "{invalid_fields}");
    }
}
