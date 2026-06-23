use super::WorldStateSection;
use crate::agents_md::EnvironmentInstructions;
use crate::agents_md::InstructionEntry;
use crate::agents_md::LoadedAgentsMd;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;

fn environment(cwd: &str, contents: &str) -> EnvironmentInstructions {
    let cwd = PathUri::parse(cwd).expect("valid cwd URI");
    EnvironmentInstructions {
        entries: vec![InstructionEntry {
            contents: contents.to_string(),
            source_path: cwd.join("AGENTS.md").expect("valid AGENTS.md URI"),
        }],
        cwd,
    }
}

fn state(
    environments: impl IntoIterator<Item = (&'static str, EnvironmentInstructions)>,
) -> LoadedAgentsMd {
    LoadedAgentsMd {
        user_instructions: None,
        internal_instructions: vec!["global instructions".to_string()],
        environments: environments
            .into_iter()
            .map(|(id, instructions)| (id.to_string(), instructions))
            .collect(),
    }
}

fn render_diff(current: &LoadedAgentsMd, previous: &LoadedAgentsMd) -> Option<String> {
    WorldStateSection::render_diff(current, Some(previous)).map(|fragment| fragment.render())
}

#[test]
fn added_environment_renders_only_its_instructions() {
    let primary = environment("file:///primary", "primary instructions");
    let previous = state([("primary", primary.clone())]);
    let current = state([
        ("primary", primary),
        (
            "secondary",
            environment("file:///secondary", "secondary instructions"),
        ),
    ]);

    assert_eq!(
        render_diff(&current, &previous),
        Some(
            r#"# AGENTS.md instructions

<INSTRUCTIONS>
for `secondary` with root /secondary

secondary instructions
</INSTRUCTIONS>"#
                .to_string()
        )
    );
}

#[test]
fn removed_environment_renders_no_diff() {
    let primary = environment("file:///primary", "primary instructions");
    let secondary = environment("file:///secondary", "secondary instructions");
    let previous = state([("primary", primary.clone()), ("secondary", secondary)]);
    let current = state([("primary", primary)]);

    assert_eq!(render_diff(&current, &previous), None);
}

#[test]
fn rerooted_environment_renders_no_diff() {
    let previous = state([(
        "local",
        environment("file:///previous", "previous instructions"),
    )]);
    let current = state([(
        "local",
        environment("file:///current", "current instructions"),
    )]);

    assert_eq!(render_diff(&current, &previous), None);
}

#[test]
fn unchanged_environment_renders_no_diff() {
    let state = state([(
        "local",
        environment("file:///workspace", "project instructions"),
    )]);

    assert_eq!(render_diff(&state, &state), None);
}
