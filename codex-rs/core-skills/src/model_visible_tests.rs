use super::MAX_MODEL_VISIBLE_SKILL_DESCRIPTION_CHARS;
use super::TRUNCATED_SKILL_DESCRIPTION_SUFFIX;
use super::render_model_visible_skill_contents;

#[test]
fn skill_contents_cap_only_model_visible_descriptions() {
    let description = "💡".repeat(MAX_MODEL_VISIBLE_SKILL_DESCRIPTION_CHARS + 1);
    let short_description = "s".repeat(MAX_MODEL_VISIBLE_SKILL_DESCRIPTION_CHARS + 1);
    let contents = format!(
        "---\nname: demo\ndescription: {description}\nmetadata:\n  short-description: {short_description}\n---\n\n# Demo\n\nUse the full body.\n"
    );
    let expected_description = "💡".repeat(
        MAX_MODEL_VISIBLE_SKILL_DESCRIPTION_CHARS
            - TRUNCATED_SKILL_DESCRIPTION_SUFFIX.chars().count(),
    ) + TRUNCATED_SKILL_DESCRIPTION_SUFFIX;
    let expected_short_description = "s".repeat(
        MAX_MODEL_VISIBLE_SKILL_DESCRIPTION_CHARS
            - TRUNCATED_SKILL_DESCRIPTION_SUFFIX.chars().count(),
    ) + TRUNCATED_SKILL_DESCRIPTION_SUFFIX;

    let rendered = render_model_visible_skill_contents(&contents);

    assert!(rendered.contains(&format!("description: {expected_description}")));
    assert!(rendered.contains(&format!("short-description: {expected_short_description}")));
    assert!(!rendered.contains(&description));
    assert!(!rendered.contains(&short_description));
    assert!(rendered.contains("Use the full body."));
    assert!(contents.contains(&description));
}

#[test]
fn skill_contents_without_overlong_description_are_borrowed() {
    let contents = "---\nname: demo\ndescription: short\n---\n\n# Demo\n";

    let rendered = render_model_visible_skill_contents(contents);

    assert!(matches!(rendered, std::borrow::Cow::Borrowed(_)));
    assert_eq!(rendered, contents);
}
