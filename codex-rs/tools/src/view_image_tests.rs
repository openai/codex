use super::*;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

#[test]
fn view_image_tool_omits_detail_without_original_detail_feature() {
    assert_eq!(
        create_view_image_tool(ViewImageToolOptions {
            can_request_original_image_detail: false,
        }),
        ToolSpec::Function(ResponsesApiTool {
            name: "view_image".to_string(),
            description: "View a local image from the filesystem (only use if given a full filepath by the user, and the image isn't already attached to the thread context within <image ...> tags)."
                .to_string(),
            strict: false,
            defer_loading: None,
            parameters: JsonSchema::Object {
                properties: BTreeMap::from([(
                    "path".to_string(),
                    JsonSchema::String {
                        description: Some("Local filesystem path to an image file".to_string()),
                    },
                )]),
                required: Some(vec!["path".to_string()]),
                additional_properties: Some(false.into()),
            },
            output_schema: Some(view_image_output_schema()),
        })
    );
}

#[test]
fn view_image_tool_includes_detail_with_original_detail_feature() {
    assert_eq!(
        create_view_image_tool(ViewImageToolOptions {
            can_request_original_image_detail: true,
        }),
        ToolSpec::Function(ResponsesApiTool {
            name: "view_image".to_string(),
            description: "View a local image from the filesystem (only use if given a full filepath by the user, and the image isn't already attached to the thread context within <image ...> tags)."
                .to_string(),
            strict: false,
            defer_loading: None,
            parameters: JsonSchema::Object {
                properties: BTreeMap::from([
                    (
                        "detail".to_string(),
                        JsonSchema::String {
                            description: Some(
                                "Optional detail override. The only supported value is `original`; omit this field for default resized behavior. Use `original` to preserve the file's original resolution instead of resizing to fit. This is important when high-fidelity image perception or precise localization is needed, especially for CUA agents.".to_string(),
                            ),
                        },
                    ),
                    (
                        "path".to_string(),
                        JsonSchema::String {
                            description: Some("Local filesystem path to an image file".to_string()),
                        },
                    ),
                ]),
                required: Some(vec!["path".to_string()]),
                additional_properties: Some(false.into()),
            },
            output_schema: Some(view_image_output_schema()),
        })
    );
}
