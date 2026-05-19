use codex_protocol::models::VIEW_IMAGE_TOOL_NAME;
use codex_tools::JsonSchema;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolSpec;
use serde_json::Value;
use serde_json::json;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewImageToolOptions {
    pub can_request_original_image_detail: bool,
    pub include_environment_id: bool,
}

pub fn create_view_image_tool(options: ViewImageToolOptions) -> ToolSpec {
    let mut properties = BTreeMap::from([(
        "path".to_string(),
        JsonSchema::string(Some("Local filesystem path to an image file".to_string())),
    )]);
    if options.can_request_original_image_detail {
        properties.insert(
            "detail".to_string(),
            JsonSchema::string_enum(
                vec![json!("high"), json!("original")],
                Some(
                    "Optional detail override. Supported values are `high` and `original`; omit this field for default high resized behavior. Use `original` to preserve the file's original resolution instead of resizing to fit. This is important when high-fidelity image perception or precise localization is needed, especially for CUA agents.".to_string(),
                ),
            ),
        );
    }
    if options.include_environment_id {
        properties.insert(
            "environment_id".to_string(),
            JsonSchema::string(Some(
                "Optional selected environment id to target. Omit this to use the primary environment."
                    .to_string(),
            )),
        );
    }

    ToolSpec::Function(ResponsesApiTool {
        name: VIEW_IMAGE_TOOL_NAME.to_string(),
        description: "View a local image from the filesystem (only use if given a full filepath by the user, and the image isn't already attached to the thread context within <image ...> tags)."
            .to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(properties, Some(vec!["path".to_string()]), Some(false.into())),
        output_schema: Some(view_image_output_schema()),
    })
}

fn view_image_output_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "image_url": {
                "type": "string",
                "description": "Data URL for the loaded image."
            },
            "detail": {
                "type": "string",
                "enum": ["high", "original"],
                "description": "Image detail hint returned by view_image. Returns `high` for default resized behavior or `original` when original resolution is preserved."
            }
        },
        "required": ["image_url", "detail"],
        "additionalProperties": false
    })
}
