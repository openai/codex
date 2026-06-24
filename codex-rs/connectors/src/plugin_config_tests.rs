use codex_plugin::AppConnectorId;
use codex_plugin::AppDeclaration;
use pretty_assertions::assert_eq;

use super::parse_plugin_app_config;

#[test]
fn parses_plugin_app_config_in_order_without_validating_connector_ids() {
    let parsed = parse_plugin_app_config(
        r#"{
            "apps": {
                "calendar": {
                    "id": "connector_calendar",
                    "category": "  productivity  "
                },
                "drive": {
                    "id": "connector_calendar",
                    "category": "  "
                },
                "blank": {
                    "id": "  "
                }
            }
        }"#,
    )
    .expect("plugin app config should parse");

    assert_eq!(
        parsed,
        vec![
            AppDeclaration {
                name: "calendar".to_string(),
                connector_id: AppConnectorId("connector_calendar".to_string()),
                category: Some("productivity".to_string()),
            },
            AppDeclaration {
                name: "drive".to_string(),
                connector_id: AppConnectorId("connector_calendar".to_string()),
                category: None,
            },
            AppDeclaration {
                name: "blank".to_string(),
                connector_id: AppConnectorId("  ".to_string()),
                category: None,
            },
        ]
    );
}

#[test]
fn rejects_invalid_plugin_app_config() {
    assert!(parse_plugin_app_config("not json").is_err());
}
