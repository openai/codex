use super::*;
use crate::context::world_state::WorldState;
use codex_config::RequirementSource;
use codex_protocol::models::ResponseItem;
use pretty_assertions::assert_eq;

#[test]
fn managed_policy_updates_and_removal_are_emitted_once() {
    let mut history: Vec<ResponseItem> = Vec::new();
    let mut previous = None;

    for (instructions, expected) in [
        (None, None),
        (Some("initial policy"), Some("initial policy".to_string())),
        (
            Some("updated policy"),
            Some(format!("{REPLACEMENT_NOTICE}\n\nupdated policy")),
        ),
        (Some(""), Some(REMOVAL_NOTICE.to_string())),
        (None, None),
    ] {
        let requirement =
            instructions.map(|text| Sourced::new(text.to_string(), RequirementSource::Unknown));
        let mut world_state = WorldState::default();
        world_state.add_section(ManagedDeveloperInstructionsState::new(requirement.as_ref()));
        let updates = world_state
            .render_history_diff(previous.as_ref(), &history)
            .into_iter()
            .map(ContextualUserFragment::into_boxed_response_item)
            .collect::<Vec<_>>();
        assert_eq!(
            updates,
            expected
                .map(|instructions| {
                    ContextualUserFragment::into(ManagedDeveloperInstructions { instructions })
                })
                .into_iter()
                .collect::<Vec<_>>()
        );
        history.extend(updates);
        previous = Some(world_state.snapshot());
        assert!(
            world_state
                .render_history_diff(previous.as_ref(), &history)
                .is_empty()
        );
    }

    let legacy_policy = ContextualUserFragment::into(ManagedDeveloperInstructions {
        instructions: "legacy policy".to_string(),
    });
    for (instructions, expected) in [
        (
            Some("current policy"),
            format!("{REPLACEMENT_NOTICE}\n\ncurrent policy"),
        ),
        (None, REMOVAL_NOTICE.to_string()),
    ] {
        let requirement =
            instructions.map(|text| Sourced::new(text.to_string(), RequirementSource::Unknown));
        let mut world_state = WorldState::default();
        world_state.add_section(ManagedDeveloperInstructionsState::new(requirement.as_ref()));
        assert_eq!(
            world_state
                .render_history_diff(/*previous*/ None, std::slice::from_ref(&legacy_policy))
                .into_iter()
                .map(ContextualUserFragment::into_boxed_response_item)
                .collect::<Vec<_>>(),
            vec![ContextualUserFragment::into(ManagedDeveloperInstructions {
                instructions: expected,
            })]
        );
    }
}

#[test]
fn managed_policy_limit_includes_replacement_notice_and_markers() -> io::Result<()> {
    let overhead = ManagedDeveloperInstructions {
        instructions: format!("{REPLACEMENT_NOTICE}\n\n"),
    }
    .render()
    .len();
    let mut requirement = Sourced::new(
        "x".repeat(approx_bytes_for_tokens(MAX_MANAGED_DEVELOPER_INSTRUCTIONS_TOKENS) - overhead),
        RequirementSource::Unknown,
    );
    validate_managed_developer_instructions(Some(&requirement))?;
    requirement.value.push('x');
    let error = validate_managed_developer_instructions(Some(&requirement))
        .expect_err("oversized policy must not be silently truncated");
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    Ok(())
}
