use super::*;
use crate::JsonSchema;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

#[test]
fn create_apply_patch_freeform_tool_matches_expected_spec() {
    assert_eq!(
        create_apply_patch_freeform_tool(ApplyPatchToolOptions {
            multi_environment_tools: false,
        }),
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
        create_apply_patch_json_tool(ApplyPatchToolOptions {
            multi_environment_tools: false,
        }),
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

#[test]
fn create_apply_patch_freeform_tool_with_environment_matches_expected_spec() {
    let ToolSpec::Freeform(tool) = create_apply_patch_freeform_tool(ApplyPatchToolOptions {
        multi_environment_tools: true,
    }) else {
        panic!("apply_patch should be a freeform tool");
    };
    assert!(
        tool.description
            .contains("*** Environment: <environment_id>")
    );
    assert_eq!(tool.format.definition, APPLY_PATCH_ENVIRONMENT_LARK_GRAMMAR);
}

#[test]
fn create_apply_patch_json_tool_with_environment_includes_environment_id() {
    let ToolSpec::Function(tool) = create_apply_patch_json_tool(ApplyPatchToolOptions {
        multi_environment_tools: true,
    }) else {
        panic!("apply_patch should be a function tool");
    };
    let properties = tool
        .parameters
        .properties
        .expect("apply_patch parameters should include properties");
    assert!(properties.contains_key("environment_id"));
}
