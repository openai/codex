//! Checks that filtering omits only the known checklist sections.

use super::without_update_plan_instructions;
use pretty_assertions::assert_eq;

#[test]
fn disabled_omits_checklist_sections_and_preserves_other_planning() {
    let instructions = "Before.\n\n## Planning\nYou have access to an `update_plan` tool which tracks steps.\n\n### Examples\nKeep steps current.\n\n## Work\nImplement.\n\n## `update_plan`\nUpdate the checklist.\n\n# Next\n## Planning\nDiscuss architecture and inspect update_plan before editing.\n";
    assert_eq!(
        without_update_plan_instructions(instructions),
        "Before.\n\n## Work\nImplement.\n\n# Next\n## Planning\nDiscuss architecture and inspect update_plan before editing.\n",
    );
}

#[test]
fn disabled_omits_legacy_tool_and_plan_mode_cross_reference_sections() {
    let instructions = "# Plan Mode\nKeep planning.\n\n## Plan Mode vs update_plan tool\nThe tools are separate.\n\n## Execution\nDo not edit.\n\n## Plan tool\nWhen using the planning tool:\n- Update steps.\n";
    assert_eq!(
        without_update_plan_instructions(instructions),
        "# Plan Mode\nKeep planning.\n\n## Execution\nDo not edit.\n\n",
    );
}

#[test]
fn disabled_omits_only_checklist_list_items_and_goal_guidance() {
    let instructions = "Keep working.\n- Use the plan tool to explain the work\n    - Skip simple tasks.\n    - Keep steps current.\n- Explain discoveries.\n- If you create a checklist or task list, update its statuses.\n\nProgress visibility:\nIf update_plan is available, use it for complex work.\n\nCompletion:\nVerify the result.\n";
    assert_eq!(
        without_update_plan_instructions(instructions),
        "Keep working.\n- Explain discoveries.\n\nCompletion:\nVerify the result.\n",
    );
}
