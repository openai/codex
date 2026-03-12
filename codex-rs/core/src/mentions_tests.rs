use std::collections::HashSet;

use codex_protocol::user_input::UserInput;
use pretty_assertions::assert_eq;

use super::collect_explicit_app_ids;
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
        description: None,
        has_skills: true,
        mcp_server_names: Vec::new(),
        app_connector_ids: Vec::new(),
    }
}

#[test]
fn collect_explicit_app_ids_from_linked_text_mentions() {
    let input = vec![text_input("use [$calendar](app://calendar)")];

    let app_ids = collect_explicit_app_ids(&input);

    assert_eq!(app_ids, HashSet::from(["calendar".to_string()]));
}

#[test]
fn collect_explicit_app_ids_dedupes_structured_and_linked_mentions() {
    let input = vec![
        text_input("use [$calendar](app://calendar)"),
        UserInput::Mention {
            name: "calendar".to_string(),
            path: "app://calendar".to_string(),
        },
    ];

    let app_ids = collect_explicit_app_ids(&input);

    assert_eq!(app_ids, HashSet::from(["calendar".to_string()]));
}

#[test]
fn collect_explicit_app_ids_ignores_non_app_paths() {
    let input = vec![
        text_input(
            "use [$docs](mcp://docs) and [$skill](skill://team/skill) and [$file](/tmp/file.txt)",
        ),
        UserInput::Mention {
            name: "docs".to_string(),
            path: "mcp://docs".to_string(),
        },
        UserInput::Mention {
            name: "skill".to_string(),
            path: "skill://team/skill".to_string(),
        },
        UserInput::Mention {
            name: "file".to_string(),
            path: "/tmp/file.txt".to_string(),
        },
    ];

    let app_ids = collect_explicit_app_ids(&input);

    assert_eq!(app_ids, HashSet::<String>::new());
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
            "use [$app](app://calendar) and [$skill](skill://team/skill) and [$file](/tmp/file.txt)",
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
