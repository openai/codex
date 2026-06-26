use codex_protocol::user_input::UserInput;
use pretty_assertions::assert_eq;

use super::collect_explicit_plugin_mentions;
use crate::plugins::PluginCapabilitySummary;

fn text_input(text: &str) -> UserInput {
    UserInput::Text {
        text: text.to_string(),
        text_elements: Vec::new(),
    }
}

fn plugin(config_name: &str, display_name: &str) -> PluginCapabilitySummary {
    PluginCapabilitySummary {
        config_name: config_name.to_string(),
        display_name: display_name.to_string(),
        has_skills: true,
        ..PluginCapabilitySummary::default()
    }
}

#[test]
fn collect_explicit_plugin_mentions_from_structured_paths() {
    let plugins = vec![
        plugin("sample@test", "sample"),
        plugin("other@test", "other"),
    ];

    let mentioned = collect_explicit_plugin_mentions(
        &[UserInput::Mention {
            name: "sample".to_string(),
            path: "plugin://sample@test".to_string(),
        }],
        &plugins,
    );

    assert_eq!(mentioned, vec![plugin("sample@test", "sample")]);
}

#[test]
fn collect_explicit_plugin_mentions_from_linked_text_mentions() {
    let plugins = vec![
        plugin("sample@test", "sample"),
        plugin("other@test", "other"),
    ];

    let mentioned = collect_explicit_plugin_mentions(
        &[text_input("use [@sample](plugin://sample@test)")],
        &plugins,
    );

    assert_eq!(mentioned, vec![plugin("sample@test", "sample")]);
}

#[test]
fn collect_explicit_plugin_mentions_dedupes_structured_and_linked_mentions() {
    let plugins = vec![
        plugin("sample@test", "sample"),
        plugin("other@test", "other"),
    ];

    let mentioned = collect_explicit_plugin_mentions(
        &[
            text_input("use [@sample](plugin://sample@test)"),
            UserInput::Mention {
                name: "sample".to_string(),
                path: "plugin://sample@test".to_string(),
            },
        ],
        &plugins,
    );

    assert_eq!(mentioned, vec![plugin("sample@test", "sample")]);
}

#[test]
fn collect_explicit_plugin_mentions_ignores_non_plugin_paths() {
    let plugins = vec![plugin("sample@test", "sample")];

    let mentioned = collect_explicit_plugin_mentions(
        &[text_input(
            "use [$server](mcp://calendar) and [$skill](skill://team/skill) and [$file](/tmp/file.txt)",
        )],
        &plugins,
    );

    assert_eq!(mentioned, Vec::<PluginCapabilitySummary>::new());
}

#[test]
fn collect_explicit_plugin_mentions_ignores_dollar_linked_plugin_mentions() {
    let plugins = vec![plugin("sample@test", "sample")];

    let mentioned = collect_explicit_plugin_mentions(
        &[text_input("use [$sample](plugin://sample@test)")],
        &plugins,
    );

    assert_eq!(mentioned, Vec::<PluginCapabilitySummary>::new());
}
