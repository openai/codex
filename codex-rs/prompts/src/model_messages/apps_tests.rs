#![cfg(test)]

use super::ResolvedModelMessages;
use codex_protocol::openai_models::AppsMessages;
use codex_protocol::openai_models::ModelMessages;
use pretty_assertions::assert_eq;

#[test]
fn apps_resolution_preserves_literal_bytes_and_rejects_invalid_overrides() {
    let bundled = ResolvedModelMessages::bundled().apps_usage_instructions("fixture_apps");
    for apps in [
        None,
        Some(AppsMessages::default()),
        Some(AppsMessages {
            usage_instructions: Some(" \t\n".to_string()),
        }),
        Some(AppsMessages {
            usage_instructions: Some("é".repeat(401)),
        }),
        Some(AppsMessages {
            usage_instructions: Some("x".repeat(801)),
        }),
    ] {
        let messages = ModelMessages {
            apps,
            ..Default::default()
        };
        assert_eq!(
            ResolvedModelMessages {
                catalog_messages: Some(&messages)
            }
            .apps_usage_instructions("fixture_apps"),
            bundled
        );
    }
}

#[test]
fn apps_resolution_keeps_empty_boundary_and_placeholder_text_literal() {
    for text in [
        String::new(),
        "  {{connector_id}}\r\n{{ namespace }}  ".to_string(),
        "é".repeat(400),
        "\u{10ffff}".repeat(200),
    ] {
        let messages = ModelMessages {
            apps: Some(AppsMessages {
                usage_instructions: Some(text.clone()),
            }),
            ..Default::default()
        };
        let resolved = ResolvedModelMessages {
            catalog_messages: Some(&messages),
        }
        .apps_usage_instructions("fixture_apps");
        assert_eq!(
            (resolved.usage_instructions, resolved.catalog_override),
            (text, true)
        );
    }
}
