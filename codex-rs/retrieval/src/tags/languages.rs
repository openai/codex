//! Supported languages for tag extraction.
//!
//! Each language has a tree-sitter grammar and tags query.

use std::path::Path;

use tree_sitter_tags::TagsConfiguration;

use crate::error::Result;
use crate::error::RetrievalErr;

/// Supported programming languages for tag extraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SupportedLanguage {
    /// Rust
    Rust,
    /// Go
    Go,
    /// Python
    Python,
    /// Java
    Java,
}

impl SupportedLanguage {
    /// Detect language from file extension.
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "rs" => Some(Self::Rust),
            "go" => Some(Self::Go),
            "py" | "pyw" | "pyi" => Some(Self::Python),
            "java" => Some(Self::Java),
            _ => None,
        }
    }

    /// Detect language from file path.
    pub fn from_path(path: &Path) -> Option<Self> {
        path.extension()
            .and_then(|e| e.to_str())
            .and_then(Self::from_extension)
    }

    /// Get the language name.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Go => "go",
            Self::Python => "python",
            Self::Java => "java",
        }
    }

    /// Get tree-sitter language.
    pub fn tree_sitter_language(&self) -> tree_sitter::Language {
        match self {
            Self::Rust => tree_sitter_rust::LANGUAGE.into(),
            Self::Go => tree_sitter_go::LANGUAGE.into(),
            Self::Python => tree_sitter_python::LANGUAGE.into(),
            Self::Java => tree_sitter_java::LANGUAGE.into(),
        }
    }

    /// Get tags configuration for this language.
    pub fn tags_configuration(&self) -> Result<TagsConfiguration> {
        let language = self.tree_sitter_language();
        let query = self.tags_query();

        TagsConfiguration::new(language, query, "").map_err(|e| RetrievalErr::TagExtractionFailed {
            cause: format!(
                "Failed to create tags configuration for {}: {e}",
                self.name()
            ),
        })
    }

    /// Get the tags query for this language.
    ///
    /// These queries define what symbols to extract.
    fn tags_query(&self) -> &'static str {
        match self {
            Self::Rust => RUST_TAGS_QUERY,
            Self::Go => GO_TAGS_QUERY,
            Self::Python => PYTHON_TAGS_QUERY,
            Self::Java => JAVA_TAGS_QUERY,
        }
    }
}

impl std::fmt::Display for SupportedLanguage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Rust tags query.
///
/// Extracts: functions, methods, structs, enums, traits, impls, modules, constants.
const RUST_TAGS_QUERY: &str = r#"
(function_item
  name: (identifier) @name) @definition.function

(function_signature_item
  name: (identifier) @name) @definition.function

(struct_item
  name: (type_identifier) @name) @definition.class

(enum_item
  name: (type_identifier) @name) @definition.class

(trait_item
  name: (type_identifier) @name) @definition.interface

(impl_item
  trait: (type_identifier)? @name
  type: (type_identifier) @name) @definition.class

(mod_item
  name: (identifier) @name) @definition.module

(const_item
  name: (identifier) @name) @definition.constant

(static_item
  name: (identifier) @name) @definition.constant

(type_item
  name: (type_identifier) @name) @definition.type

(macro_definition
  name: (identifier) @name) @definition.function
"#;

/// Go tags query.
///
/// Extracts: functions, methods, structs, interfaces, types.
const GO_TAGS_QUERY: &str = r#"
(function_declaration
  name: (identifier) @name) @definition.function

(method_declaration
  name: (field_identifier) @name) @definition.method

(type_declaration
  (type_spec
    name: (type_identifier) @name
    type: (struct_type))) @definition.class

(type_declaration
  (type_spec
    name: (type_identifier) @name
    type: (interface_type))) @definition.interface

(type_declaration
  (type_spec
    name: (type_identifier) @name)) @definition.type

(const_declaration
  (const_spec
    name: (identifier) @name)) @definition.constant

(var_declaration
  (var_spec
    name: (identifier) @name)) @definition.constant
"#;

/// Python tags query.
///
/// Extracts: functions, methods, classes.
const PYTHON_TAGS_QUERY: &str = r#"
(function_definition
  name: (identifier) @name) @definition.function

(class_definition
  name: (identifier) @name) @definition.class

(decorated_definition
  definition: (function_definition
    name: (identifier) @name)) @definition.function

(decorated_definition
  definition: (class_definition
    name: (identifier) @name)) @definition.class
"#;

/// Java tags query.
///
/// Extracts: methods, classes, interfaces, enums, fields.
const JAVA_TAGS_QUERY: &str = r#"
(method_declaration
  name: (identifier) @name) @definition.method

(constructor_declaration
  name: (identifier) @name) @definition.method

(class_declaration
  name: (identifier) @name) @definition.class

(interface_declaration
  name: (identifier) @name) @definition.interface

(enum_declaration
  name: (identifier) @name) @definition.class

(field_declaration
  declarator: (variable_declarator
    name: (identifier) @name)) @definition.constant

(constant_declaration
  declarator: (variable_declarator
    name: (identifier) @name)) @definition.constant
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_extension() {
        assert_eq!(
            SupportedLanguage::from_extension("rs"),
            Some(SupportedLanguage::Rust)
        );
        assert_eq!(
            SupportedLanguage::from_extension("go"),
            Some(SupportedLanguage::Go)
        );
        assert_eq!(
            SupportedLanguage::from_extension("py"),
            Some(SupportedLanguage::Python)
        );
        assert_eq!(
            SupportedLanguage::from_extension("java"),
            Some(SupportedLanguage::Java)
        );
        assert_eq!(SupportedLanguage::from_extension("unknown"), None);
    }

    #[test]
    fn test_from_path() {
        assert_eq!(
            SupportedLanguage::from_path(Path::new("main.rs")),
            Some(SupportedLanguage::Rust)
        );
        assert_eq!(
            SupportedLanguage::from_path(Path::new("main.go")),
            Some(SupportedLanguage::Go)
        );
        assert_eq!(
            SupportedLanguage::from_path(Path::new("script.py")),
            Some(SupportedLanguage::Python)
        );
    }

    #[test]
    fn test_tags_configuration() {
        // Test that we can create tags configuration for each language
        for lang in [
            SupportedLanguage::Rust,
            SupportedLanguage::Go,
            SupportedLanguage::Python,
            SupportedLanguage::Java,
        ] {
            let result = lang.tags_configuration();
            assert!(
                result.is_ok(),
                "Failed to create config for {lang}: {result:?}"
            );
        }
    }
}
