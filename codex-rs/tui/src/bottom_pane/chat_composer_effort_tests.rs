use super::tests::new_test_composer;
use super::tests::snapshot_composer_state_with_width;
use super::*;
use pretty_assertions::assert_eq;

#[test]
fn effort_composer_baseline_repeat_and_lowering_do_not_replay() {
    let (mut composer, _rx) = new_test_composer();
    composer.set_status_line_enabled(/*enabled*/ true);
    composer.set_status_line(Some(Line::from("gpt-5.4 high · main").into()));
    assert!(composer.set_active_reasoning_effort(
        Some(&ReasoningEffort::Ultra),
        /*animations_enabled*/ true,
    ));
    assert!(composer.effort_ignition.is_none());
    assert!(!composer.set_active_reasoning_effort(
        Some(&ReasoningEffort::Ultra),
        /*animations_enabled*/ true,
    ));

    assert!(composer.set_active_reasoning_effort(
        Some(&ReasoningEffort::Max),
        /*animations_enabled*/ true,
    ));
    assert!(composer.effort_ignition.is_some());
    assert!(composer.effort_animation_style.is_some());
    assert!(composer.effort_status_line_transition.is_some());
    assert!(composer.set_active_reasoning_effort(
        Some(&ReasoningEffort::Ultra),
        /*animations_enabled*/ true,
    ));
    assert!(composer.effort_ignition.is_some());
    assert!(composer.effort_status_line_transition.is_some());
    assert!(composer.set_active_reasoning_effort(
        Some(&ReasoningEffort::Medium),
        /*animations_enabled*/ true,
    ));
    assert!(composer.effort_ignition.is_none());
    assert!(composer.effort_status_line_transition.is_none());
}

#[test]
fn extension_content_skips_the_effort_transition_in_both_directions() {
    let (mut composer, _rx) = new_test_composer();
    composer.set_status_line_enabled(/*enabled*/ true);
    composer.set_status_line(Some("native".into()));
    composer.set_active_reasoning_effort_baseline(Some(&ReasoningEffort::High));
    composer.set_active_reasoning_effort(
        Some(&ReasoningEffort::Max),
        /*animations_enabled*/ true,
    );
    assert!(composer.effort_status_line_transition.is_some());

    let tree: crate::ui_tree::UiTree = r#""extension""#.parse().unwrap();
    composer.set_status_line(Some(tree.into()));
    assert!(composer.effort_status_line_transition.is_none());

    composer.set_active_reasoning_effort(
        Some(&ReasoningEffort::Ultra),
        /*animations_enabled*/ true,
    );
    composer.set_status_line(Some("native".into()));
    assert!(composer.effort_status_line_transition.is_none());
}

#[test]
fn effort_transition_does_not_queue_a_missing_outgoing_status_line() {
    let (mut composer, _rx) = new_test_composer();
    composer.set_status_line_enabled(/*enabled*/ true);
    composer.set_status_line(/*status_line*/ None);
    composer.set_task_running(/*running*/ true);
    composer.set_text_content("queued draft".to_string(), Vec::new(), Vec::new());
    composer.set_active_reasoning_effort(
        Some(&ReasoningEffort::High),
        /*animations_enabled*/ true,
    );

    assert!(composer.set_active_reasoning_effort(
        Some(&ReasoningEffort::Ultra),
        /*animations_enabled*/ true,
    ));
    assert!(composer.effort_ignition.is_some());
    assert!(composer.effort_status_line_transition.is_none());
}

#[test]
fn effort_composer_restored_baseline_and_reduced_motion_do_not_start() {
    let (mut composer, _rx) = new_test_composer();
    composer.set_status_line_enabled(/*enabled*/ true);
    composer.set_status_line(Some(Line::from("gpt-5.4 high · main").into()));
    composer.set_active_reasoning_effort(
        Some(&ReasoningEffort::High),
        /*animations_enabled*/ false,
    );
    assert!(composer.set_active_reasoning_effort(
        Some(&ReasoningEffort::Ultra),
        /*animations_enabled*/ false,
    ));
    assert_eq!(composer.effort_tier, Some(EffortTier::Ultra));
    assert!(composer.effort_ignition.is_none());
    assert_eq!(composer.effort_animation_style, None);
    assert!(composer.effort_status_line_transition.is_none());

    assert!(composer.set_active_reasoning_effort(
        Some(&ReasoningEffort::Max),
        /*animations_enabled*/ true,
    ));
    assert!(composer.effort_ignition.is_some());
    assert!(composer.effort_status_line_transition.is_some());
    composer.set_active_reasoning_effort_baseline(Some(&ReasoningEffort::Max));
    assert_eq!(composer.effort_tier, Some(EffortTier::Max));
    assert!(composer.effort_ignition.is_none());
    assert!(composer.effort_status_line_transition.is_none());
    assert!(!composer.set_active_reasoning_effort(
        Some(&ReasoningEffort::Max),
        /*animations_enabled*/ true,
    ));
    assert!(composer.effort_ignition.is_none());
    assert!(composer.effort_status_line_transition.is_none());
}

#[test]
fn effort_transition_never_replaces_a_footer_flash() {
    let (mut composer, _rx) = new_test_composer();
    composer.set_status_line_enabled(/*enabled*/ true);
    composer.set_status_line(Some(Line::from("gpt-5.4 high · main").into()));
    composer.set_active_reasoning_effort(
        Some(&ReasoningEffort::High),
        /*animations_enabled*/ true,
    );
    composer.set_active_reasoning_effort(
        Some(&ReasoningEffort::Ultra),
        /*animations_enabled*/ true,
    );
    composer.set_status_line(Some(Line::from("gpt-5.4 ultra · main").into()));
    composer
        .footer
        .show_flash(Line::from("saved"), Duration::from_secs(/*secs*/ 1));

    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 60, /*height*/ 6,
    );
    let mut buf = Buffer::empty(area);
    composer.render(area, &mut buf);
    let footer = (0..area.width)
        .map(|column| buf[(column, area.bottom() - 1)].symbol())
        .collect::<String>();

    assert!(footer.contains("saved"));
    assert!(!footer.contains("U L T R A"));
}

#[test]
fn effort_transition_keeps_the_full_footer_row() {
    let (mut composer, _rx) = new_test_composer();
    composer.set_status_line_enabled(/*enabled*/ true);
    composer.set_collaboration_modes_enabled(/*enabled*/ true);
    composer.set_collaboration_mode_indicator(Some(CollaborationModeIndicator::Plan));
    composer.set_status_line(Some(Line::from("gpt-5.4 high · feature-branch").into()));
    composer.set_active_reasoning_effort(
        Some(&ReasoningEffort::High),
        /*animations_enabled*/ true,
    );
    composer.set_active_reasoning_effort(
        Some(&ReasoningEffort::Ultra),
        /*animations_enabled*/ true,
    );

    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 36, /*height*/ 6,
    );
    let mut buf = Buffer::empty(area);
    composer.render(area, &mut buf);
    let rows = [area.bottom() - 2, area.bottom() - 1].map(|y| {
        (0..area.width)
            .map(|column| buf[(column, y)].symbol())
            .collect::<String>()
            .trim_end()
            .to_owned()
    });
    insta::assert_snapshot!(
        "effort_transition_keeps_the_full_footer_row",
        rows.join("\n")
    );
}

#[test]
fn ultra_accent_upgrades_prompt_glyph() {
    snapshot_composer_state_with_width(
        "ultra_accent_upgrades_prompt_glyph",
        /*width*/ 60,
        /*enhanced_keys_supported*/ false,
        |composer| {
            composer.set_active_reasoning_effort(
                Some(&ReasoningEffort::Ultra),
                /*animations_enabled*/ true,
            );
        },
    );
}
