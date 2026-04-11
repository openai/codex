use super::*;
use crate::JsonSchema;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

#[test]
fn create_apply_patch_freeform_tool_matches_expected_spec() {
    assert_eq!(
        create_apply_patch_freeform_tool(),
        ToolSpec::Freeform(FreeformTool {
            name: "apply_patch".to_string(),
            description:
                "Use the `apply_patch` tool to edit files. This is a FREEFORM tool, so do not wrap the patch in JSON."
                    .to_string(),
            format: FreeformToolFormat {
                r#type: "grammar".to_string(),
                syntax: "lark".to_string(),
                definition: APPLY_PATCH_LARK_GRAMMAR.to_string(),
            },
        })
    );
}

#[test]
fn create_apply_patch_json_tool_matches_expected_spec() {
    assert_eq!(
        create_apply_patch_json_tool(),
        ToolSpec::Function(ResponsesApiTool {
            name: "apply_patch".to_string(),
            description: APPLY_PATCH_JSON_TOOL_DESCRIPTION.to_string(),
            strict: false,
            defer_loading: None,
            parameters: JsonSchema::object(
                BTreeMap::from([(
                    "input".to_string(),
                    JsonSchema::string(Some(
                        "The entire contents of the apply_patch command".to_string(),
                    ),),
                )]),
                Some(vec!["input".to_string()]),
                Some(false.into())
            ),
            output_schema: None,
        })
    );
}
