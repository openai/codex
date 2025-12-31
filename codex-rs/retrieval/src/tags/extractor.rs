//! Tag extraction using tree-sitter-tags.
//!
//! Extracts function, class, method, and other symbol definitions from source code.

use std::path::Path;

use tree_sitter_tags::TagsConfiguration;
use tree_sitter_tags::TagsContext;

use crate::error::Result;
use crate::error::RetrievalErr;

use super::languages::SupportedLanguage;

/// Kind of tag (symbol type).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TagKind {
    /// Function definition
    Function,
    /// Method definition (inside class/impl)
    Method,
    /// Class definition
    Class,
    /// Struct definition
    Struct,
    /// Interface/trait definition
    Interface,
    /// Type definition
    Type,
    /// Module/namespace
    Module,
    /// Constant/variable
    Constant,
    /// Other/unknown
    Other,
}

impl TagKind {
    /// Create from tree-sitter-tags syntax type string.
    pub fn from_syntax_type(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "function" | "func" | "fn" => Self::Function,
            "method" => Self::Method,
            "class" => Self::Class,
            "struct" => Self::Struct,
            "interface" | "trait" => Self::Interface,
            "type" | "typedef" => Self::Type,
            "module" | "namespace" | "mod" => Self::Module,
            "constant" | "const" | "variable" | "var" => Self::Constant,
            _ => Self::Other,
        }
    }

    /// Get string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Method => "method",
            Self::Class => "class",
            Self::Struct => "struct",
            Self::Interface => "interface",
            Self::Type => "type",
            Self::Module => "module",
            Self::Constant => "constant",
            Self::Other => "other",
        }
    }
}

impl std::fmt::Display for TagKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Extracted code tag (symbol).
#[derive(Debug, Clone)]
pub struct CodeTag {
    /// Symbol name
    pub name: String,
    /// Kind of symbol
    pub kind: TagKind,
    /// Start line (0-indexed)
    pub start_line: i32,
    /// End line (0-indexed)
    pub end_line: i32,
    /// Start byte offset
    pub start_byte: i32,
    /// End byte offset
    pub end_byte: i32,
    /// Optional signature (function parameters, etc.)
    pub signature: Option<String>,
    /// Optional documentation
    pub docs: Option<String>,
    /// Is this a definition (vs reference)
    pub is_definition: bool,
}

/// Tag extractor using tree-sitter-tags.
pub struct TagExtractor {
    /// Reusable tags context
    context: TagsContext,
}

impl Default for TagExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl TagExtractor {
    /// Create a new tag extractor.
    pub fn new() -> Self {
        Self {
            context: TagsContext::new(),
        }
    }

    /// Extract tags from source code.
    ///
    /// # Arguments
    /// * `source` - Source code content
    /// * `language` - Programming language
    ///
    /// # Returns
    /// Vector of extracted tags
    pub fn extract(&mut self, source: &str, language: SupportedLanguage) -> Result<Vec<CodeTag>> {
        let config = language.tags_configuration()?;
        self.extract_with_config(source, &config)
    }

    /// Extract tags from a file.
    pub fn extract_file(&mut self, path: &Path) -> Result<Vec<CodeTag>> {
        let source = std::fs::read_to_string(path).map_err(|e| RetrievalErr::FileReadFailed {
            path: path.to_path_buf(),
            cause: e.to_string(),
        })?;

        let language = SupportedLanguage::from_path(path).ok_or_else(|| {
            RetrievalErr::UnsupportedLanguage {
                extension: path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("unknown")
                    .to_string(),
            }
        })?;

        self.extract(&source, language)
    }

    /// Extract tags with a specific configuration.
    fn extract_with_config(
        &mut self,
        source: &str,
        config: &TagsConfiguration,
    ) -> Result<Vec<CodeTag>> {
        let source_bytes = source.as_bytes();

        let (tags, _errors) = self
            .context
            .generate_tags(config, source_bytes, None)
            .map_err(|e| RetrievalErr::TagExtractionFailed {
                cause: format!("Failed to generate tags: {e:?}"),
            })?;

        let mut result = Vec::new();

        for tag in tags {
            let tag = tag.map_err(|e| RetrievalErr::TagExtractionFailed {
                cause: format!("Tag error: {e:?}"),
            })?;

            // Get the tag name from source
            let name_range = tag.name_range;
            let name = std::str::from_utf8(&source_bytes[name_range.start..name_range.end])
                .unwrap_or("")
                .to_string();

            // Skip empty names
            if name.is_empty() {
                continue;
            }

            // Get syntax type from config
            let syntax_type = config.syntax_type_name(tag.syntax_type_id);

            // Calculate line numbers
            let start_line = source[..tag.range.start].lines().count() as i32;
            let end_line = source[..tag.range.end].lines().count() as i32;

            // Extract signature if it's a function/method
            let signature = if matches!(
                TagKind::from_syntax_type(syntax_type),
                TagKind::Function | TagKind::Method
            ) {
                extract_signature(source, tag.range.start, tag.range.end)
            } else {
                None
            };

            // Extract docs (look for comments before the tag)
            let docs = extract_docs(source, tag.range.start);

            result.push(CodeTag {
                name,
                kind: TagKind::from_syntax_type(syntax_type),
                start_line,
                end_line,
                start_byte: tag.range.start as i32,
                end_byte: tag.range.end as i32,
                signature,
                docs,
                is_definition: tag.is_definition,
            });
        }

        Ok(result)
    }
}

/// Extract function/method signature from source.
fn extract_signature(source: &str, start: usize, end: usize) -> Option<String> {
    let snippet = &source[start..end.min(source.len())];

    // Find the first line or up to opening brace
    let sig_end = snippet
        .find('{')
        .or_else(|| snippet.find('\n'))
        .unwrap_or(snippet.len().min(200));

    let signature = snippet[..sig_end].trim();

    if signature.is_empty() {
        None
    } else {
        Some(signature.to_string())
    }
}

/// Find parent symbol for a given line range.
///
/// Returns the parent symbol (class, struct, impl, module) that contains
/// the given line range. This is used to provide context for method chunks.
pub fn find_parent_symbol(tags: &[CodeTag], start_line: i32, end_line: i32) -> Option<String> {
    // Find the innermost container that fully contains this range
    let mut best_match: Option<&CodeTag> = None;
    let mut best_size = i32::MAX;

    for tag in tags {
        // Skip non-container types
        if !matches!(
            tag.kind,
            TagKind::Class | TagKind::Struct | TagKind::Interface | TagKind::Module
        ) {
            continue;
        }

        // Check if this tag contains the target range
        if tag.start_line <= start_line && tag.end_line >= end_line {
            // Prefer the smallest (innermost) container
            let size = tag.end_line - tag.start_line;
            if size < best_size {
                best_size = size;
                best_match = Some(tag);
            }
        }
    }

    best_match.map(|tag| {
        // Format the parent symbol string
        let kind_prefix = match tag.kind {
            TagKind::Class => "class",
            TagKind::Struct => "struct",
            TagKind::Interface => "trait", // Rust: trait, Python/Java: interface
            TagKind::Module => "mod",
            _ => "",
        };

        if kind_prefix.is_empty() {
            tag.name.clone()
        } else {
            format!("{kind_prefix} {}", tag.name)
        }
    })
}

/// Find parent symbol using impl blocks for Rust.
///
/// For Rust, we also need to check for impl blocks which aren't
/// captured as tags but contain method definitions.
///
/// Uses brace counting to verify the target line is actually inside the impl block.
pub fn find_parent_impl(source: &str, start_line: i32) -> Option<String> {
    let lines: Vec<&str> = source.lines().collect();

    // Search backwards from start_line for an impl block
    for i in (0..start_line as usize).rev() {
        if i >= lines.len() {
            continue;
        }

        let line = lines[i].trim();

        // Look for impl blocks
        if line.starts_with("impl") {
            // Verify that start_line is inside this impl block by counting braces
            let mut brace_depth = 0;
            for j in i..lines.len().min(start_line as usize + 1) {
                for c in lines[j].chars() {
                    match c {
                        '{' => brace_depth += 1,
                        '}' => brace_depth -= 1,
                        _ => {}
                    }
                }
                // If we closed more braces than opened before reaching start_line,
                // this impl block doesn't contain our target
                if brace_depth <= 0 && j < start_line as usize {
                    break;
                }
            }

            // Only return if we're still inside the impl block
            if brace_depth > 0 {
                // Extract the impl header
                let impl_line = if line.contains('{') {
                    &line[..line.find('{').unwrap_or(line.len())]
                } else {
                    line
                };
                return Some(impl_line.trim().to_string());
            }
        }

        // Stop if we hit a top-level closing brace (end of previous block)
        // Only stop if it's at the start of the line (not indented)
        if line == "}" && lines[i].starts_with('}') {
            break;
        }
    }

    None
}

/// Get parent context for a chunk.
///
/// Combines tag-based parent lookup with impl block detection.
pub fn get_parent_context(
    source: &str,
    tags: &[CodeTag],
    start_line: i32,
    end_line: i32,
) -> Option<String> {
    // First try to find a parent from extracted tags
    if let Some(parent) = find_parent_symbol(tags, start_line, end_line) {
        return Some(parent);
    }

    // Fall back to impl block detection for Rust
    find_parent_impl(source, start_line)
}

/// Extract documentation comments before a definition.
fn extract_docs(source: &str, start: usize) -> Option<String> {
    // Look backwards from start to find doc comments
    let before = &source[..start];
    let lines: Vec<&str> = before.lines().collect();

    if lines.is_empty() {
        return None;
    }

    let mut doc_lines = Vec::new();

    // Scan backwards from the end
    for line in lines.iter().rev() {
        let trimmed = line.trim();

        // Check for doc comments
        if trimmed.starts_with("///") || trimmed.starts_with("//!") {
            // Rust doc comments
            let doc = trimmed
                .trim_start_matches("///")
                .trim_start_matches("//!")
                .trim();
            doc_lines.push(doc);
        } else if trimmed.starts_with('#') && !trimmed.starts_with("#[") {
            // Python comments (but not decorators)
            let doc = trimmed.trim_start_matches('#').trim();
            doc_lines.push(doc);
        } else if trimmed.starts_with("//") {
            // Regular comments
            let doc = trimmed.trim_start_matches("//").trim();
            doc_lines.push(doc);
        } else if trimmed.is_empty() {
            // Allow blank lines between doc and definition
            continue;
        } else {
            // Non-comment, non-blank line - stop
            break;
        }
    }

    if doc_lines.is_empty() {
        return None;
    }

    // Reverse since we collected backwards
    doc_lines.reverse();
    Some(doc_lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tag_kind_from_syntax_type() {
        assert_eq!(TagKind::from_syntax_type("function"), TagKind::Function);
        assert_eq!(TagKind::from_syntax_type("method"), TagKind::Method);
        assert_eq!(TagKind::from_syntax_type("class"), TagKind::Class);
        assert_eq!(TagKind::from_syntax_type("struct"), TagKind::Struct);
        assert_eq!(TagKind::from_syntax_type("trait"), TagKind::Interface);
        assert_eq!(TagKind::from_syntax_type("unknown"), TagKind::Other);
    }

    #[test]
    fn test_extract_signature() {
        let source = "fn add(a: i32, b: i32) -> i32 {\n    a + b\n}";
        let sig = extract_signature(source, 0, source.len());
        assert_eq!(sig, Some("fn add(a: i32, b: i32) -> i32".to_string()));
    }

    #[test]
    fn test_extract_docs() {
        let source = "/// This is a doc comment\n/// Second line\nfn foo() {}";
        let start = source.find("fn").unwrap();
        let docs = extract_docs(source, start);
        assert!(docs.is_some());
        assert!(docs.unwrap().contains("This is a doc comment"));
    }

    #[test]
    fn test_extract_rust_tags() {
        let source = r#"
/// A simple struct
struct Point {
    x: i32,
    y: i32,
}

impl Point {
    fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

fn main() {
    let p = Point::new(1, 2);
}
"#;
        let mut extractor = TagExtractor::new();
        let tags = extractor.extract(source, SupportedLanguage::Rust).unwrap();

        // Should find: struct Point, fn new, fn main
        assert!(
            tags.len() >= 2,
            "Expected at least 2 tags, got {}",
            tags.len()
        );

        let names: Vec<&str> = tags.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"Point"), "Should contain Point struct");
        assert!(names.contains(&"main"), "Should contain main function");
    }

    #[test]
    fn test_extract_go_tags() {
        let source = r#"
package main

type User struct {
    Name string
    Age  int
}

func (u *User) Greet() string {
    return "Hello, " + u.Name
}

func main() {
    u := &User{Name: "Alice", Age: 30}
    fmt.Println(u.Greet())
}
"#;
        let mut extractor = TagExtractor::new();
        let tags = extractor.extract(source, SupportedLanguage::Go).unwrap();

        let names: Vec<&str> = tags.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"User"), "Should contain User struct");
        assert!(names.contains(&"main"), "Should contain main function");
    }

    #[test]
    fn test_extract_python_tags() {
        let source = r#"
class Calculator:
    def add(self, a, b):
        return a + b

    def subtract(self, a, b):
        return a - b

def main():
    calc = Calculator()
    print(calc.add(1, 2))
"#;
        let mut extractor = TagExtractor::new();
        let tags = extractor
            .extract(source, SupportedLanguage::Python)
            .unwrap();

        let names: Vec<&str> = tags.iter().map(|t| t.name.as_str()).collect();
        assert!(
            names.contains(&"Calculator"),
            "Should contain Calculator class"
        );
        assert!(names.contains(&"main"), "Should contain main function");
    }

    #[test]
    fn test_extract_java_tags() {
        let source = r#"
public class HelloWorld {
    private String message;

    public HelloWorld(String msg) {
        this.message = msg;
    }

    public void sayHello() {
        System.out.println(message);
    }

    public static void main(String[] args) {
        HelloWorld hw = new HelloWorld("Hello!");
        hw.sayHello();
    }
}
"#;
        let mut extractor = TagExtractor::new();
        let tags = extractor.extract(source, SupportedLanguage::Java).unwrap();

        let names: Vec<&str> = tags.iter().map(|t| t.name.as_str()).collect();
        assert!(
            names.contains(&"HelloWorld"),
            "Should contain HelloWorld class"
        );
        assert!(names.contains(&"main"), "Should contain main method");
    }

    #[test]
    fn test_find_parent_symbol_class() {
        // Simulate tags from a Python file
        let tags = vec![
            CodeTag {
                name: "Calculator".to_string(),
                kind: TagKind::Class,
                start_line: 1,
                end_line: 10,
                start_byte: 0,
                end_byte: 100,
                signature: None,
                docs: None,
                is_definition: true,
            },
            CodeTag {
                name: "add".to_string(),
                kind: TagKind::Method,
                start_line: 2,
                end_line: 4,
                start_byte: 20,
                end_byte: 50,
                signature: Some("def add(self, a, b)".to_string()),
                docs: None,
                is_definition: true,
            },
        ];

        // Method inside class
        let parent = find_parent_symbol(&tags, 2, 4);
        assert_eq!(parent, Some("class Calculator".to_string()));

        // Top-level function (outside class)
        let parent = find_parent_symbol(&tags, 15, 20);
        assert_eq!(parent, None);
    }

    #[test]
    fn test_find_parent_impl_rust() {
        let source = r#"
struct Point {
    x: i32,
    y: i32,
}

impl Point {
    fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    fn distance(&self) -> f64 {
        ((self.x * self.x + self.y * self.y) as f64).sqrt()
    }
}

fn main() {
    let p = Point::new(3, 4);
}
"#;
        // Line 8 is inside impl Point (fn new)
        let parent = find_parent_impl(source, 8);
        assert!(parent.is_some());
        assert!(parent.unwrap().contains("impl Point"));

        // Line 17 is main function (outside impl)
        let parent = find_parent_impl(source, 17);
        assert!(parent.is_none());
    }

    #[test]
    fn test_get_parent_context() {
        let source = r#"
class UserService:
    def get_user(self, user_id):
        return self.repo.find(user_id)

    def create_user(self, name):
        return self.repo.create(name)

def main():
    service = UserService()
"#;
        let tags = vec![CodeTag {
            name: "UserService".to_string(),
            kind: TagKind::Class,
            start_line: 1,
            end_line: 7,
            start_byte: 0,
            end_byte: 150,
            signature: None,
            docs: None,
            is_definition: true,
        }];

        // Inside class
        let parent = get_parent_context(source, &tags, 2, 4);
        assert_eq!(parent, Some("class UserService".to_string()));

        // Outside class
        let parent = get_parent_context(source, &tags, 9, 10);
        assert_eq!(parent, None);
    }
}
