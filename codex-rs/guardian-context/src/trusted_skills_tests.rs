use super::MAX_TRUSTED_SKILL_TOKENS;
use super::TRUSTED_SKILLS_PREFIX;
use super::TrustedSkills;
use codex_context_fragments::ContextualUserFragment;
use codex_protocol::protocol::TruncationPolicy;
use pretty_assertions::assert_eq;

fn rendered_paths(paths: Vec<String>) -> Vec<String> {
    let rendered = TrustedSkills { paths }.render();
    assert!(rendered.len() <= TruncationPolicy::Tokens(MAX_TRUSTED_SKILL_TOKENS).byte_budget());
    serde_json::from_str(
        rendered
            .strip_prefix(TRUSTED_SKILLS_PREFIX)
            .expect("trusted skill context should contain JSON evidence"),
    )
    .expect("trusted skill evidence should remain valid JSON")
}

#[test]
fn bounds_escaped_skill_paths_without_corrupting_json_or_utf8() {
    let paths = (0..16)
        .map(|index| {
            format!(
                "/home/user/.codex/skills/{index:03}/{}SKILL.md",
                "\u{0001}é".repeat(80)
            )
        })
        .collect::<Vec<_>>();
    let retained = rendered_paths(paths.clone());

    assert!(!retained.is_empty());
    assert!(retained.len() < paths.len());
    assert!(retained.iter().all(|path| paths.contains(path)));
}

#[test]
fn preserves_multiple_invoked_skill_paths() {
    assert_eq!(
        rendered_paths(vec![
            "/home/user/.codex/skills/first/SKILL.md".to_owned(),
            "/home/user/.codex/skills/second/SKILL.md".to_owned(),
        ]),
        vec![
            "/home/user/.codex/skills/first/SKILL.md",
            "/home/user/.codex/skills/second/SKILL.md",
        ],
    );
}
