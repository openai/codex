use super::*;
use pretty_assertions::assert_eq;

#[test]
fn automation_update_omits_absent_clearable_fields() {
    let params = AutomationUpdateParams {
        automation_id: "automation-1".to_string(),
        name: None,
        prompt: None,
        rrule: None,
        model: None,
        reasoning_effort: None,
        status: None,
        target: None,
    };

    let serialized = serde_json::to_value(params).expect("serialize automation update");

    assert_eq!(
        serialized["automationId"],
        serde_json::json!("automation-1")
    );
    assert!(serialized.get("rrule").is_none());
    assert!(serialized.get("model").is_none());
    assert!(serialized.get("reasoningEffort").is_none());
}
