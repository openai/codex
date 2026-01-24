//! Smart overview chunk generation.
//!
//! Generates structure overview chunks for classes/modules with method signatures
//! collapsed to `{ ... }`, similar to Continue's getSmartCollapsedChunks.

use crate::tags::CodeTag;
use crate::tags::TagKind;
use crate::types::ChunkSpan;

/// Configuration for overview chunk generation.
#[derive(Debug, Clone)]
pub struct OverviewConfig {
    /// Minimum number of methods for a class to generate an overview.
    pub min_methods: usize,
    /// Maximum overview size in characters.
    pub max_size: usize,
}

impl Default for OverviewConfig {
    fn default() -> Self {
        Self {
            min_methods: 2,
            max_size: 4096,
        }
    }
}

/// Generate overview chunks for classes/structs in the source.
///
/// For each class/struct with enough methods, generates a chunk showing:
/// - The class definition header
/// - All method signatures with bodies collapsed to `{ ... }`
///
/// This allows semantic search to find "what methods does UserService have"
/// without returning the full implementation of each method.
pub fn generate_overview_chunks(
    source: &str,
    tags: &[CodeTag],
    config: &OverviewConfig,
) -> Vec<ChunkSpan> {
    let mut overviews = Vec::new();
    let lines: Vec<&str> = source.lines().collect();

    // Find all container types (class, struct, interface, module)
    let containers: Vec<&CodeTag> = tags
        .iter()
        .filter(|t| {
            matches!(
                t.kind,
                TagKind::Class | TagKind::Struct | TagKind::Interface | TagKind::Module
            )
        })
        .collect();

    for container in containers {
        // Find all methods/functions that belong to this container
        let methods: Vec<&CodeTag> = tags
            .iter()
            .filter(|t| {
                matches!(t.kind, TagKind::Function | TagKind::Method)
                    && t.start_line > container.start_line
                    && t.end_line <= container.end_line
            })
            .collect();

        if methods.len() < config.min_methods {
            continue;
        }

        // Build overview content
        let mut content = String::new();

        // Add container header (lines from start to first method or first brace)
        let header_end = methods
            .first()
            .map(|m| m.start_line)
            .unwrap_or(container.end_line);

        for i in container.start_line..header_end {
            if let Some(line) = lines.get(i as usize) {
                content.push_str(line);
                content.push('\n');
            }
        }

        // Add each method signature with collapsed body
        for method in &methods {
            if let Some(collapsed) = collapse_method_to_signature(source, method, &lines) {
                content.push_str(&collapsed);
                content.push('\n');
            }
        }

        // Add closing brace if container has one
        if let Some(last_line) = lines.get(container.end_line as usize) {
            let trimmed = last_line.trim();
            if trimmed == "}" || trimmed == "};" {
                content.push_str(last_line);
                content.push('\n');
            }
        }

        // Truncate if too large
        if content.len() > config.max_size {
            content.truncate(config.max_size);
            content.push_str("\n// ... truncated");
        }

        overviews.push(ChunkSpan {
            content,
            start_line: container.start_line,
            end_line: container.end_line,
            is_overview: true,
        });
    }

    overviews
}

/// Collapse a method to its signature with `{ ... }` or `...` for Python.
fn collapse_method_to_signature(_source: &str, tag: &CodeTag, lines: &[&str]) -> Option<String> {
    // Extract the method's source text
    let start = tag.start_line as usize;
    let end = tag.end_line as usize;

    if start >= lines.len() {
        return None;
    }

    // Determine indentation from first line
    let first_line = lines.get(start).unwrap_or(&"");
    let indent = first_line.len() - first_line.trim_start().len();
    let indent_str: String = first_line.chars().take(indent).collect();

    // Find the opening brace position
    let mut method_text = String::new();
    for i in start..=end.min(lines.len() - 1) {
        method_text.push_str(lines[i]);
        method_text.push('\n');
    }

    // Try brace-based languages first (Rust, Go, Java, JS, etc.)
    if let Some(brace_pos) = method_text.find('{') {
        // Get signature (everything before the brace)
        let signature = method_text[..brace_pos].trim_end();
        return Some(format!("{}{} {{ ... }}", indent_str, signature.trim()));
    }

    // Python-style: look for `:` at end of first line (def foo(): or def foo(args):)
    let first_trimmed = first_line.trim();
    if (first_trimmed.starts_with("def ") || first_trimmed.starts_with("async def "))
        && first_trimmed.ends_with(':')
    {
        // Remove the colon and add collapsed body indicator
        let sig = &first_trimmed[..first_trimmed.len() - 1];
        return Some(format!("{}{}: ...", indent_str, sig.trim()));
    }

    // For other cases (abstract methods, declarations), return first line
    Some(first_line.to_string())
}

/// Check if an overview should be generated for a container.
pub fn should_generate_overview(tags: &[CodeTag], container: &CodeTag, min_methods: usize) -> bool {
    let method_count = tags
        .iter()
        .filter(|t| {
            matches!(t.kind, TagKind::Function | TagKind::Method)
                && t.start_line > container.start_line
                && t.end_line <= container.end_line
        })
        .count();

    method_count >= min_methods
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tag(kind: TagKind, name: &str, start: i32, end: i32) -> CodeTag {
        CodeTag {
            kind,
            name: name.to_string(),
            start_line: start,
            end_line: end,
            start_byte: 0,
            end_byte: 0,
            signature: None,
            docs: None,
            is_definition: true,
        }
    }

    #[test]
    fn test_generate_overview_for_class() {
        let source = r#"class UserService {
    fn get_user(id: i64) -> User {
        // implementation
        self.db.find(id)
    }

    fn create_user(name: &str) -> User {
        // implementation
        User::new(name)
    }

    fn delete_user(id: i64) {
        // implementation
        self.db.delete(id)
    }
}"#;

        let tags = vec![
            make_tag(TagKind::Class, "UserService", 0, 14),
            make_tag(TagKind::Method, "get_user", 1, 4),
            make_tag(TagKind::Method, "create_user", 6, 9),
            make_tag(TagKind::Method, "delete_user", 11, 14),
        ];

        let config = OverviewConfig {
            min_methods: 2,
            max_size: 4096,
        };

        let overviews = generate_overview_chunks(source, &tags, &config);

        assert_eq!(overviews.len(), 1);
        let overview = &overviews[0];

        // Should contain class header
        assert!(overview.content.contains("class UserService"));

        // Should contain collapsed method signatures
        assert!(
            overview
                .content
                .contains("fn get_user(id: i64) -> User { ... }")
        );
        assert!(
            overview
                .content
                .contains("fn create_user(name: &str) -> User { ... }")
        );
        assert!(overview.content.contains("fn delete_user(id: i64) { ... }"));

        // Should NOT contain implementation details
        assert!(!overview.content.contains("self.db.find"));
        assert!(!overview.content.contains("User::new"));
    }

    #[test]
    fn test_skip_class_with_few_methods() {
        let source = r#"class SmallClass {
    fn only_method() {}
}"#;

        let tags = vec![
            make_tag(TagKind::Class, "SmallClass", 0, 2),
            make_tag(TagKind::Method, "only_method", 1, 1),
        ];

        let config = OverviewConfig {
            min_methods: 2, // Requires at least 2 methods
            max_size: 4096,
        };

        let overviews = generate_overview_chunks(source, &tags, &config);
        assert!(overviews.is_empty());
    }

    #[test]
    fn test_generate_overview_for_struct_impl() {
        let source = r#"struct Repository {
    db: Database,
}

impl Repository {
    fn find(&self, id: i64) -> Option<Entity> {
        self.db.query(id)
    }

    fn save(&self, entity: &Entity) -> Result<()> {
        self.db.insert(entity)
    }
}"#;

        // Note: We only generate overview for the impl block which contains methods
        let tags = vec![
            make_tag(TagKind::Struct, "Repository", 0, 2),
            make_tag(TagKind::Module, "Repository_impl", 4, 12), // Treat impl as module
            make_tag(TagKind::Method, "find", 5, 7),
            make_tag(TagKind::Method, "save", 9, 11),
        ];

        let config = OverviewConfig::default();
        let overviews = generate_overview_chunks(source, &tags, &config);

        // Should generate overview for the impl block (treated as module)
        assert_eq!(overviews.len(), 1);
    }

    #[test]
    fn test_should_generate_overview() {
        let tags = vec![
            make_tag(TagKind::Class, "MyClass", 0, 20),
            make_tag(TagKind::Method, "method1", 1, 5),
            make_tag(TagKind::Method, "method2", 6, 10),
            make_tag(TagKind::Method, "method3", 11, 15),
        ];

        let container = &tags[0];

        assert!(should_generate_overview(&tags, container, 2));
        assert!(should_generate_overview(&tags, container, 3));
        assert!(!should_generate_overview(&tags, container, 4));
    }

    #[test]
    fn test_collapse_method_to_signature() {
        let source = "    fn process(data: &str) {\n        // do something\n    }";
        let lines: Vec<&str> = source.lines().collect();
        let tag = make_tag(TagKind::Method, "process", 0, 2);

        let result = collapse_method_to_signature(source, &tag, &lines);
        assert!(result.is_some());

        let collapsed = result.unwrap();
        assert!(collapsed.contains("fn process(data: &str) { ... }"));
        assert!(!collapsed.contains("do something"));
    }

    #[test]
    fn test_python_class_overview() {
        let source = r#"class UserManager:
    def __init__(self, db):
        self.db = db

    def get_user(self, user_id):
        return self.db.find(user_id)

    def create_user(self, name, email):
        user = User(name, email)
        self.db.save(user)
        return user
"#;

        let tags = vec![
            make_tag(TagKind::Class, "UserManager", 0, 10),
            make_tag(TagKind::Method, "__init__", 1, 2),
            make_tag(TagKind::Method, "get_user", 4, 5),
            make_tag(TagKind::Method, "create_user", 7, 10),
        ];

        let config = OverviewConfig::default();
        let overviews = generate_overview_chunks(source, &tags, &config);

        assert_eq!(overviews.len(), 1);
        // For Python, we don't have braces, but the function should still work
        // by showing the def lines
    }
}
