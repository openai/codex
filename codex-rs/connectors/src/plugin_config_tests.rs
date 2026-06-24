use codex_plugin::AppConnectorId;
use codex_plugin::AppDeclaration;
use pretty_assertions::assert_eq;

use super::parse_plugin_app_config;
use super::parse_plugin_app_config_value;

#[test]
fn parses_plugin_app_config_in_declaration_order() {
    let parsed = parse_plugin_app_config(
        r#"{
            "apps": {
                "calendar": {
                    "id": "connector_calendar",
                    "category": "  productivity  "
                },
                "drive": {
                    "id": "connector_drive",
                    "category": "  "
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
                connector_id: AppConnectorId("connector_drive".to_string()),
                category: None,
            },
        ]
    );
}

#[test]
fn value_parser_matches_text_parser() {
    let value = serde_json::json!({
        "apps": {
            "calendar": { "id": "connector_calendar" }
        }
    });

    assert_eq!(
        parse_plugin_app_config_value(value).expect("plugin app value should parse"),
        parse_plugin_app_config(r#"{"apps":{"calendar":{"id":"connector_calendar"}}}"#)
            .expect("plugin app text should parse")
    );
}

#[test]
fn parser_keeps_duplicate_and_blank_connector_ids_for_host_validation() {
    let parsed = parse_plugin_app_config(
        r#"{
            "apps": {
                "calendar": { "id": "connector_shared" },
                "drive": { "id": "connector_shared" },
                "blank": { "id": "  " }
            }
        }"#,
    )
    .expect("plugin app config should parse");

    assert_eq!(
        parsed,
        vec![
            AppDeclaration {
                name: "calendar".to_string(),
                connector_id: AppConnectorId("connector_shared".to_string()),
                category: None,
            },
            AppDeclaration {
                name: "drive".to_string(),
                connector_id: AppConnectorId("connector_shared".to_string()),
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
