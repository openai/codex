use super::*;
use codex_execpolicy::rule::PrefixRule;

#[test]
fn prompt_reviewer_is_deserialized_and_propagated_to_prefix_rule() {
    let parsed: RequirementsExecPolicyToml = toml::from_str(
        r#"
prefix_rules = [
    { pattern = [{ token = "git" }, { token = "push" }], decision = "prompt", reviewer = "auto_review" },
]
"#,
    )
    .expect("requirements rules should deserialize");

    assert_eq!(
        parsed.prefix_rules[0].reviewer,
        Some(RuleReviewer::AutoReview)
    );

    let policy = parsed.to_policy().expect("prompt reviewer should be valid");
    let rules = policy
        .rules()
        .get_vec("git")
        .expect("git prefix rule should exist");
    let rule = rules[0]
        .as_any()
        .downcast_ref::<PrefixRule>()
        .expect("rule should be a prefix rule");

    assert_eq!(rule.reviewer, Some(RuleReviewer::AutoReview));
}

#[test]
fn user_reviewer_deserializes_from_requirements_toml() {
    let parsed: RequirementsExecPolicyToml = toml::from_str(
        r#"
prefix_rules = [
    { pattern = [{ token = "git" }], decision = "prompt", reviewer = "user" },
]
"#,
    )
    .expect("requirements rules should deserialize");

    assert_eq!(parsed.prefix_rules[0].reviewer, Some(RuleReviewer::User));
}

#[test]
fn reviewer_is_rejected_for_non_prompt_decisions() {
    let parsed: RequirementsExecPolicyToml = toml::from_str(
        r#"
prefix_rules = [
    { pattern = [{ token = "rm" }], decision = "forbidden", reviewer = "user" },
]
"#,
    )
    .expect("requirements rules should deserialize");

    assert!(matches!(
        parsed.to_policy(),
        Err(RequirementsExecPolicyParseError::ReviewerRequiresPromptDecision { rule_index: 0 })
    ));
}
