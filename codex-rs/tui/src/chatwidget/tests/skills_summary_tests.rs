#![cfg(test)]

//! Snapshots change-count wording and silence when skill state is unchanged.

use super::*;

#[tokio::test]
async fn manage_skills_summary_snapshot() {
    let (mut chat, mut rx, _ops) = make_chatwidget_manual(/*model_override*/ None).await;
    let initial = [false, false, true];
    chat.skills_all = initial
        .iter()
        .enumerate()
        .map(|(index, &enabled)| SkillMetadata {
            name: format!("skill-{index}"),
            description: "Summary fixture".to_string(),
            short_description: None,
            interface: None,
            dependencies: None,
            path: test_path_buf(&format!("/tmp/skills/skill-{index}/SKILL.md"))
                .abs()
                .into(),
            scope: crate::test_support::skill_scope_user(),
            enabled,
            plugin_id: None,
        })
        .collect();
    drain_insert_history(&mut rx);
    let mut summaries = Vec::new();
    for (name, current) in [
        ("enable", [true, true, true]),
        ("disable", [false, false, false]),
        ("mixed", [true, true, false]),
        ("no changes", initial),
    ] {
        chat.skills_initial_state = Some(
            chat.skills_all
                .iter()
                .zip(initial)
                .map(|(skill, enabled)| (skill.path.to_inferred_path_uri().unwrap(), enabled))
                .collect(),
        );
        for (skill, enabled) in chat.skills_all.iter_mut().zip(current) {
            skill.enabled = enabled;
        }
        chat.handle_manage_skills_closed();
        let lines = drain_insert_history(&mut rx).concat();
        summaries.push(format!(
            "{name}: {}",
            lines_to_single_string(&lines).trim_end()
        ));
    }
    assert_chatwidget_snapshot!("manage_skills_summary", summaries.join("\n"));
}
