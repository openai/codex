use super::*;
use pretty_assertions::assert_eq;
use rmcp::model::ElicitationSchema;
use rmcp::model::EnumSchema;
use rmcp::model::Meta;
use serde_json::Map;
use serde_json::Value;
use std::collections::BTreeMap;

#[test]
fn converts_rmcp_form_schema_without_json_round_trip() {
    let single_select = EnumSchema::builder(vec!["allow".to_string(), "deny".to_string()])
        .enum_titles(vec!["Allow".to_string(), "Deny".to_string()])
        .expect("titles should match values")
        .build();
    let multi_select = EnumSchema::builder(vec!["alpha".to_string(), "beta".to_string()])
        .multiselect()
        .enum_titles(vec!["Alpha".to_string(), "Beta".to_string()])
        .expect("titles should match values")
        .min_items(1)
        .expect("minimum should be valid")
        .max_items(2)
        .expect("maximum should be valid")
        .build();
    let requested_schema = ElicitationSchema::builder()
        .required_email("email")
        .required_integer_with("count", |schema| {
            schema.title("Count").range(1, 5).with_default(3)
        })
        .optional_bool("confirmed", true)
        .required_enum_schema("choice", single_select)
        .optional_enum_schema("tags", multi_select)
        .build()
        .expect("schema should build");
    let meta = Meta(Map::from_iter([(
        "source".to_string(),
        Value::String("test".to_string()),
    )]));

    let request =
        elicitation_request_from_rmcp(CreateElicitationRequestParams::FormElicitationParams {
            meta: Some(meta),
            message: "Provide details".to_string(),
            requested_schema,
        })
        .expect("RMCP schema should convert");

    assert_eq!(
        request,
        ElicitationRequest::Form {
            meta: Some(serde_json::json!({ "source": "test" })),
            message: "Provide details".to_string(),
            requested_schema: McpElicitationSchema {
                schema_uri: None,
                type_: McpElicitationObjectType::Object,
                properties: BTreeMap::from([
                    (
                        "choice".to_string(),
                        McpElicitationPrimitiveSchema::Enum(
                            McpElicitationEnumSchema::SingleSelect(
                                McpElicitationSingleSelectEnumSchema::Titled(
                                    McpElicitationTitledSingleSelectEnumSchema {
                                        type_: McpElicitationStringType::String,
                                        title: None,
                                        description: None,
                                        one_of: vec![
                                            McpElicitationConstOption {
                                                const_: "allow".to_string(),
                                                title: "Allow".to_string(),
                                            },
                                            McpElicitationConstOption {
                                                const_: "deny".to_string(),
                                                title: "Deny".to_string(),
                                            },
                                        ],
                                        default: None,
                                    },
                                ),
                            ),
                        ),
                    ),
                    (
                        "confirmed".to_string(),
                        McpElicitationPrimitiveSchema::Boolean(McpElicitationBooleanSchema {
                            type_: McpElicitationBooleanType::Boolean,
                            title: None,
                            description: None,
                            default: Some(true),
                        },),
                    ),
                    (
                        "count".to_string(),
                        McpElicitationPrimitiveSchema::Number(McpElicitationNumberSchema {
                            type_: McpElicitationNumberType::Integer,
                            title: Some("Count".to_string()),
                            description: None,
                            minimum: Some(1.0),
                            maximum: Some(5.0),
                            default: Some(3.0),
                        }),
                    ),
                    (
                        "email".to_string(),
                        McpElicitationPrimitiveSchema::String(McpElicitationStringSchema {
                            type_: McpElicitationStringType::String,
                            title: None,
                            description: None,
                            min_length: None,
                            max_length: None,
                            format: Some(McpElicitationStringFormat::Email),
                            default: None,
                        }),
                    ),
                    (
                        "tags".to_string(),
                        McpElicitationPrimitiveSchema::Enum(McpElicitationEnumSchema::MultiSelect(
                            McpElicitationMultiSelectEnumSchema::Titled(
                                McpElicitationTitledMultiSelectEnumSchema {
                                    type_: McpElicitationArrayType::Array,
                                    title: None,
                                    description: None,
                                    min_items: Some(1),
                                    max_items: Some(2),
                                    items: McpElicitationTitledEnumItems {
                                        any_of: vec![
                                            McpElicitationConstOption {
                                                const_: "alpha".to_string(),
                                                title: "Alpha".to_string(),
                                            },
                                            McpElicitationConstOption {
                                                const_: "beta".to_string(),
                                                title: "Beta".to_string(),
                                            },
                                        ],
                                    },
                                    default: None,
                                },
                            ),
                        ),),
                    ),
                ]),
                required: Some(vec![
                    "email".to_string(),
                    "count".to_string(),
                    "choice".to_string(),
                ]),
            },
        }
    );
}

#[test]
fn rejects_rmcp_top_level_schema_fields_missing_from_the_app_api() {
    let requested_schema = ElicitationSchema::builder()
        .title("Unsupported title")
        .build()
        .expect("schema should build");

    let result = elicitation_schema_from_rmcp(requested_schema);

    assert_eq!(
        result
            .expect_err("top-level title should be rejected")
            .to_string(),
        "top-level MCP elicitation schema title and description are not supported"
    );
}
