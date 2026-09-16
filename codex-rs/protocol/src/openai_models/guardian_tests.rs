use super::super::tests::test_model;
use super::*;
use pretty_assertions::assert_eq;

#[test]
fn guardian_policy_is_optional_and_tolerates_future_config() -> anyhow::Result<()> {
    let model = test_model(/*spec*/ None);
    let mut wire = serde_json::to_value(&model)?;
    assert!(
        !wire
            .as_object()
            .expect("model object")
            .contains_key("guardian")
    );
    assert_eq!(serde_json::from_value::<ModelInfo>(wire.clone())?, model);

    wire["guardian"] = serde_json::json!({
        "computer_use": "adaptive", "shell": "future_mode", "future_scope": {"version": 2},
        "mcp": null, "permissions": "adaptive",
    });
    let model: ModelInfo = serde_json::from_value(wire)?;
    assert_eq!(
        model.guardian,
        Some(GuardianModelPolicy {
            computer_use: Some(GuardianReviewMode::Adaptive),
            shell: Some(GuardianReviewMode::Unknown),
            permissions: Some(GuardianReviewMode::Adaptive),
            ..Default::default()
        })
    );
    assert!(model.computer_use_review_required());
    assert_eq!(
        model.guardian_review_mode(GuardianScope::Mcp),
        Some(GuardianReviewMode::Disabled)
    );
    Ok(())
}

#[test]
fn guardian_policy_overrides_the_legacy_cua_bit_only_when_present() {
    let mut model = test_model(/*spec*/ None);
    model.node_repl_auto_review_required = true;
    assert!(model.computer_use_review_required());
    model.guardian = Some(GuardianModelPolicy::default());
    assert!(!model.computer_use_review_required());
    model.guardian.as_mut().expect("policy").computer_use = Some(GuardianReviewMode::Synchronous);
    assert!(model.computer_use_review_required());
}
