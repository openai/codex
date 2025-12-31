//! Smart Edit Tool Specification
//!
//! Extension module for Smart Edit tool registration.
//! Uses instruction-based matching with three-tier strategies.

use crate::client_common::tools::ResponsesApiTool;
use crate::client_common::tools::ToolSpec;
use crate::tools::spec::JsonSchema;
use std::collections::BTreeMap;

/// Create Smart Edit tool specification
///
/// Smart Edit uses instruction-based matching with:
/// - Three-tier strategies (Exact → Flexible → Regex)
/// - LLM correction fallback with semantic context
/// - Concurrent modification detection
pub fn create_smart_edit_tool() -> ToolSpec {
    let mut properties = BTreeMap::new();

    properties.insert(
        "file_path".to_string(),
        JsonSchema::String {
            description: Some("Absolute path to the file to edit".to_string()),
        },
    );

    properties.insert(
        "instruction".to_string(),
        JsonSchema::String {
            description: Some(
                "Semantic instruction describing WHY/WHERE/WHAT/OUTCOME of the edit. \
                 Used for LLM correction if matching fails. Example: 'Update the timeout \
                 value in the config from 30 to 60 seconds'."
                    .to_string(),
            ),
        },
    );

    properties.insert(
        "old_string".to_string(),
        JsonSchema::String {
            description: Some(
                "Exact text to search for (will try flexible and regex if exact fails). \
                 Empty string creates a new file."
                    .to_string(),
            ),
        },
    );

    properties.insert(
        "new_string".to_string(),
        JsonSchema::String {
            description: Some("Replacement text (preserves indentation)".to_string()),
        },
    );

    properties.insert(
        "expected_replacements".to_string(),
        JsonSchema::Number {
            description: Some(
                "Expected number of occurrences to replace (default: 1). \
                 Edit fails if actual count doesn't match."
                    .to_string(),
            ),
        },
    );

    ToolSpec::Function(ResponsesApiTool {
        name: "smart_edit".to_string(),
        description:
            "Instruction-based file editing with intelligent matching and LLM correction. \
             Supports three-tier matching (exact → flexible → regex), handles whitespace \
             variations, preserves indentation, and uses semantic instructions for fallback \
             correction. Detects concurrent file modifications."
                .to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec![
                "file_path".to_string(),
                "instruction".to_string(),
                "old_string".to_string(),
                "new_string".to_string(),
            ]),
            additional_properties: Some(false.into()),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::Feature;
    use crate::features::Features;
    use crate::models_manager::model_family::ModelFamily;
    use crate::tools::handlers::ext::smart_edit::SmartEditHandler;
    use crate::tools::registry::ToolRegistryBuilder;
    use std::sync::Arc;

    /// Register Smart Edit tool if enabled (test utility)
    ///
    /// Conditionally registers smart_edit tool based on:
    /// 1. Feature::SmartEdit enabled in features
    /// 2. model_family.smart_edit_enabled is true
    fn register_smart_edit_tool(
        builder: &mut ToolRegistryBuilder,
        model_family: &ModelFamily,
        features: &Features,
    ) {
        if !features.enabled(Feature::SmartEdit) {
            return;
        }

        if !model_family.smart_edit_enabled {
            return;
        }

        builder.push_spec(create_smart_edit_tool());
        builder.register_handler("smart_edit", Arc::new(SmartEditHandler));
    }

    #[test]
    fn test_create_smart_edit_tool_spec() {
        let spec = create_smart_edit_tool();

        let ToolSpec::Function(tool) = spec else {
            panic!("Expected function tool spec");
        };

        assert_eq!(tool.name, "smart_edit");
        assert!(!tool.strict);
        assert!(
            tool.description
                .to_lowercase()
                .contains("instruction-based")
        );
        assert!(tool.description.to_lowercase().contains("three-tier"));

        let JsonSchema::Object {
            properties,
            required,
            ..
        } = tool.parameters
        else {
            panic!("Expected object parameters");
        };

        // Check required fields
        let required = required.expect("Should have required fields");
        assert_eq!(required.len(), 4);
        assert!(required.contains(&"file_path".to_string()));
        assert!(required.contains(&"instruction".to_string()));
        assert!(required.contains(&"old_string".to_string()));
        assert!(required.contains(&"new_string".to_string()));

        // Check all properties exist
        assert!(properties.contains_key("file_path"));
        assert!(properties.contains_key("instruction"));
        assert!(properties.contains_key("old_string"));
        assert!(properties.contains_key("new_string"));
        assert!(properties.contains_key("expected_replacements"));

        // Check instruction has descriptive help
        let instruction_schema = &properties["instruction"];
        if let JsonSchema::String {
            description: Some(desc),
        } = instruction_schema
        {
            assert!(desc.contains("WHY"));
            assert!(desc.contains("WHERE"));
            assert!(desc.contains("WHAT"));
        } else {
            panic!("instruction should have description");
        }
    }

    #[test]
    fn test_register_smart_edit_tool_feature_disabled() {
        let mut builder = ToolRegistryBuilder::new();
        let mut model_family =
            crate::models_manager::model_family::find_family_for_model("test-model");
        model_family.smart_edit_enabled = true;
        let mut features = Features::with_defaults();
        features.disable(Feature::SmartEdit);

        register_smart_edit_tool(&mut builder, &model_family, &features);

        let (tools, _) = builder.build();
        assert!(
            !tools
                .iter()
                .any(|t| matches!(&t.spec, ToolSpec::Function(f) if f.name == "smart_edit")),
            "Should not register when feature disabled"
        );
    }

    #[test]
    fn test_register_smart_edit_tool_model_disabled() {
        let mut builder = ToolRegistryBuilder::new();
        let mut model_family =
            crate::models_manager::model_family::find_family_for_model("test-model");
        model_family.smart_edit_enabled = false;
        let mut features = Features::with_defaults();
        features.enable(Feature::SmartEdit);

        register_smart_edit_tool(&mut builder, &model_family, &features);

        let (tools, _) = builder.build();
        assert!(
            !tools
                .iter()
                .any(|t| matches!(&t.spec, ToolSpec::Function(f) if f.name == "smart_edit")),
            "Should not register when model family doesn't support it"
        );
    }

    #[test]
    fn test_register_smart_edit_tool_both_enabled() {
        let mut builder = ToolRegistryBuilder::new();
        let mut model_family =
            crate::models_manager::model_family::find_family_for_model("test-model");
        model_family.smart_edit_enabled = true;
        let mut features = Features::with_defaults();
        features.enable(Feature::SmartEdit);

        register_smart_edit_tool(&mut builder, &model_family, &features);

        let (tools, _) = builder.build();
        assert!(
            tools
                .iter()
                .any(|t| matches!(&t.spec, ToolSpec::Function(f) if f.name == "smart_edit")),
            "Should register when both feature and model family enabled"
        );
    }
}
