//! LSP Tool Specification
//!
//! Provides AI-friendly LSP operations using symbol name + kind matching
//! instead of exact line/column positions.

use crate::client_common::tools::ResponsesApiTool;
use crate::client_common::tools::ToolSpec;
use crate::tools::spec::JsonSchema;
use std::collections::BTreeMap;

/// Create LSP tool specification
///
/// LSP tool provides:
/// - goToDefinition: Jump to symbol definition
/// - findReferences: Find all references to a symbol
/// - hover: Get type info and documentation
/// - documentSymbol: List all symbols in a file
/// - getDiagnostics: Get diagnostics (errors/warnings) for a file
/// - workspaceSymbol: Search for symbols across the entire workspace
/// - goToImplementation: Find trait/interface implementations
/// - getCallHierarchy: Get incoming or outgoing function calls
pub fn create_lsp_tool() -> ToolSpec {
    let mut properties = BTreeMap::new();

    // Required - operation
    properties.insert(
        "operation".to_string(),
        JsonSchema::String {
            description: Some(
                "The LSP operation to perform. Valid values: goToDefinition, findReferences, \
                 hover, documentSymbol, getDiagnostics, workspaceSymbol, goToImplementation, \
                 getCallHierarchy, goToTypeDefinition, goToDeclaration"
                    .to_string(),
            ),
        },
    );

    // Required - filePath
    properties.insert(
        "filePath".to_string(),
        JsonSchema::String {
            description: Some(
                "Path to the file (absolute or relative to cwd). For workspaceSymbol, \
                 use any file with the target language extension (e.g., 'src/lib.rs' for Rust) \
                 to specify which language server to query."
                    .to_string(),
            ),
        },
    );

    // Optional - symbolName (required for most operations)
    properties.insert(
        "symbolName".to_string(),
        JsonSchema::String {
            description: Some(
                "Name of the symbol to find. Required for goToDefinition, findReferences, hover, \
                 workspaceSymbol, goToImplementation, getCallHierarchy, goToTypeDefinition, \
                 and goToDeclaration. For workspaceSymbol, this is the search query. \
                 Case-insensitive matching is used."
                    .to_string(),
            ),
        },
    );

    // Optional - symbolKind (helps narrow matches)
    properties.insert(
        "symbolKind".to_string(),
        JsonSchema::String {
            description: Some(
                "Type of symbol to filter by. Valid values: function, method, class, struct, \
                 interface, enum, variable, constant, property, field, module, type. \
                 Helps narrow results when multiple symbols match."
                    .to_string(),
            ),
        },
    );

    // Optional - direction (required for getCallHierarchy)
    properties.insert(
        "direction".to_string(),
        JsonSchema::String {
            description: Some(
                "Direction for getCallHierarchy operation. Valid values: incoming, outgoing. \
                 'incoming' shows functions that call this symbol, 'outgoing' shows functions \
                 this symbol calls. Required when operation is getCallHierarchy."
                    .to_string(),
            ),
        },
    );

    ToolSpec::Function(ResponsesApiTool {
        name: "lsp".to_string(),
        description: "Query code intelligence using Language Server Protocol. \
            Supports Rust (rust-analyzer), Go (gopls), and Python (pyright). \
            Use symbol names instead of line numbers for AI-friendly queries."
            .to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["operation".to_string(), "filePath".to_string()]),
            additional_properties: Some(false.into()),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_lsp_tool_spec() {
        let spec = create_lsp_tool();

        let ToolSpec::Function(tool) = spec else {
            panic!("Expected function tool spec");
        };

        assert_eq!(tool.name, "lsp");
        assert!(!tool.strict);
        assert!(tool.description.contains("Language Server Protocol"));

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
        assert_eq!(required.len(), 2);
        assert!(required.contains(&"operation".to_string()));
        assert!(required.contains(&"filePath".to_string()));

        // Check all properties exist
        assert!(properties.contains_key("operation"));
        assert!(properties.contains_key("filePath"));
        assert!(properties.contains_key("symbolName"));
        assert!(properties.contains_key("symbolKind"));
        assert!(properties.contains_key("direction"));
    }

    #[test]
    fn test_operation_description_contains_values() {
        let spec = create_lsp_tool();
        let ToolSpec::Function(tool) = spec else {
            panic!("Expected function tool spec");
        };

        let JsonSchema::Object { properties, .. } = tool.parameters else {
            panic!("Expected object parameters");
        };

        let JsonSchema::String { description } = properties.get("operation").unwrap() else {
            panic!("Expected string for operation");
        };

        let desc = description.as_ref().unwrap();
        assert!(desc.contains("goToDefinition"));
        assert!(desc.contains("findReferences"));
        assert!(desc.contains("hover"));
        assert!(desc.contains("documentSymbol"));
        assert!(desc.contains("getDiagnostics"));
        assert!(desc.contains("workspaceSymbol"));
        assert!(desc.contains("goToImplementation"));
        assert!(desc.contains("getCallHierarchy"));
    }
}
