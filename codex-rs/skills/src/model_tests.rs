use super::*;

#[test]
fn missing_policy_allows_implicit_invocation_and_all_products() {
    let skill = EnvironmentSkillMetadata {
        path_to_skills_md: PathUri::parse("file:///skills/demo/SKILL.md").expect("valid skill URI"),
        name: "demo".to_string(),
        description: "Demo skill".to_string(),
        short_description: None,
        dependencies: None,
        policy: None,
    };

    assert!(skill.allows_implicit_invocation());
    assert!(skill.matches_product_restriction(/*restriction_product*/ None));
}

#[test]
fn policy_restricts_implicit_invocation_and_products() {
    let policy = SkillPolicy {
        allow_implicit_invocation: Some(false),
        products: vec![Product::Codex],
    };
    let skill = EnvironmentSkillMetadata {
        path_to_skills_md: PathUri::parse("file:///skills/demo/SKILL.md").expect("valid skill URI"),
        name: "demo".to_string(),
        description: "Demo skill".to_string(),
        short_description: None,
        dependencies: None,
        policy: Some(policy),
    };

    assert!(!skill.allows_implicit_invocation());
    assert!(skill.matches_product_restriction(Some(Product::Codex)));
    assert!(!skill.matches_product_restriction(/*restriction_product*/ None));
}
