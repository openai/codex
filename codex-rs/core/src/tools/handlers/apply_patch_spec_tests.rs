use super::*;
use pretty_assertions::assert_eq;

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
