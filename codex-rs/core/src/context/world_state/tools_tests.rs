use super::MAX_NAMESPACE_DESCRIPTION_CHARS;
use super::MAX_RENDERED_FRAGMENT_BYTES;
use super::ToolsState;
use crate::context::world_state::PreviousSectionState;
use crate::context::world_state::WorldStateSection;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

#[test]
fn renders_first_line_of_namespace_descriptions() {
    let tools = ToolsState::new([
        (
            "app".to_string(),
            "  control the Codex App  \nAdditional instructions.".to_string(),
        ),
        (
            "gmail".to_string(),
            "access your Google Gmail Account & labels".to_string(),
        ),
        ("hotline".to_string(), String::new()),
    ]);

    let rendered = tools
        .render_diff(PreviousSectionState::Absent)
        .expect("tools state should render")
        .render();

    assert_eq!(
        rendered,
        "<tools>\nDeferred tool namespaces:\n- app: control the Codex App\n- gmail: access your Google Gmail Account &amp; labels\n- hotline\n</tools>"
    );
}

#[test]
fn renders_added_removed_and_updated_namespace_descriptions() {
    let tools = ToolsState::new([
        ("app".to_string(), "control the Codex App".to_string()),
        (
            "gmail".to_string(),
            "access your Google Gmail Account".to_string(),
        ),
    ]);
    let previous = BTreeMap::from([
        ("gmail".to_string(), "old Gmail description".to_string()),
        (
            "hotline".to_string(),
            "access hotline information".to_string(),
        ),
    ]);

    let rendered = tools
        .render_diff(PreviousSectionState::Known(&previous))
        .expect("tools state delta should render")
        .render();

    assert_eq!(
        rendered,
        "<tools>\nAdded deferred tool namespaces:\n- app: control the Codex App\n- gmail: access your Google Gmail Account\nRemoved deferred tool namespaces:\n- hotline: access hotline information\n</tools>"
    );
}

#[test]
fn caps_namespace_descriptions_by_character_count() {
    let description = "🦀".repeat(MAX_NAMESPACE_DESCRIPTION_CHARS + 1);
    let tools = ToolsState::new([("app".to_string(), description)]);

    assert_eq!(
        tools.snapshot(),
        BTreeMap::from([(
            "app".to_string(),
            "🦀".repeat(MAX_NAMESPACE_DESCRIPTION_CHARS)
        )])
    );
}

#[test]
fn caps_rendered_tools_fragment_after_xml_escaping() {
    let tools = ToolsState::new((0..100).map(|index| {
        (
            format!("namespace_{index}"),
            "&".repeat(MAX_NAMESPACE_DESCRIPTION_CHARS),
        )
    }));

    let rendered = tools
        .render_diff(PreviousSectionState::Absent)
        .expect("tools state should render")
        .render();

    assert!(rendered.len() <= MAX_RENDERED_FRAGMENT_BYTES);
    assert!(rendered.contains(" additional namespaces omitted.\n"));
}
