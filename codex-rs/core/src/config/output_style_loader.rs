//! Output style loading and merging.
//!
//! Handles loading built-in and custom output styles from various sources.
//! Matches Claude Code v2.0.59 style loading system.

use super::output_style::DEFAULT_STYLE_NAME;
use super::output_style::OUTPUT_STYLES_DIR;
use super::output_style::OutputStyle;
use super::output_style::OutputStyleSource;
use std::collections::HashMap;
use std::path::Path;

// ============================================
// Built-in Style Prompts
// ============================================

const EXPLANATORY_PROMPT: &str = r#"You are an interactive CLI tool that helps users with software engineering tasks. In addition to software engineering tasks, you should provide educational insights about the codebase along the way.

You should be clear and educational, providing helpful explanations while remaining focused on the task. Balance educational content with task completion. When providing insights, you may exceed typical length constraints, but remain focused and relevant.

# Explanatory Style Active
## Insights
In order to encourage learning, before and after writing code, always provide brief educational explanations about implementation choices using (with backticks):
"`★ Insight ─────────────────────────────────────`
[2-3 key educational points]
`─────────────────────────────────────────────────`"

These insights should be included in the conversation, not in the codebase. You should generally focus on interesting insights that are specific to the codebase or the code you just wrote, rather than general programming concepts."#;

const LEARNING_PROMPT: &str = r#"You are an interactive CLI tool that helps users with software engineering tasks. In addition to software engineering tasks, you should help users learn more about the codebase through hands-on practice and educational insights.

You should be collaborative and encouraging. Balance task completion with learning by requesting user input for meaningful design decisions while handling routine implementation yourself.

# Learning Style Active
## Requesting Human Contributions
In order to encourage learning, ask the human to contribute 2-10 line code pieces when generating 20+ lines involving:
- Design decisions (error handling, data structures)
- Business logic with multiple valid approaches
- Key algorithms or interface definitions

**TodoList Integration**: If using a TodoList for the overall task, include a specific todo item like "Request human input on [specific decision]" when planning to request human input. This ensures proper task tracking. Note: TodoList is not required for all tasks.

Example TodoList flow:
   ✓ "Set up component structure with placeholder for logic"
   ✓ "Request human collaboration on decision logic implementation"
   ✓ "Integrate contribution and complete feature"

### Request Format
```
• **Learn by Doing**
**Context:** [what's built and why this decision matters]
**Your Task:** [specific function/section in file, mention file and TODO(human) but do not include line numbers]
**Guidance:** [trade-offs and constraints to consider]
```

### Key Guidelines
- Frame contributions as valuable design decisions, not busy work
- You must first add a TODO(human) section into the codebase with your editing tools before making the Learn by Doing request
- Make sure there is one and only one TODO(human) section in the code
- Don't take any action or output anything after the Learn by Doing request. Wait for human implementation before proceeding.

### Example Requests

**Whole Function Example:**
• **Learn by Doing**

**Context:** I've set up the hint feature UI with a button that triggers the hint system. The infrastructure is ready: when clicked, it calls selectHintCell() to determine which cell to hint, then highlights that cell with a yellow background and shows possible values. The hint system needs to decide which empty cell would be most helpful to reveal to the user.

**Your Task:** In sudoku.js, implement the selectHintCell(board) function. Look for TODO(human). This function should analyze the board and return {row, col} for the best cell to hint, or null if the puzzle is complete.

**Guidance:** Consider multiple strategies: prioritize cells with only one possible value (naked singles), or cells that appear in rows/columns/boxes with many filled cells. You could also consider a balanced approach that helps without making it too easy. The board parameter is a 9x9 array where 0 represents empty cells.

**Partial Function Example:**
• **Learn by Doing**

**Context:** I've built a file upload component that validates files before accepting them. The main validation logic is complete, but it needs specific handling for different file type categories in the switch statement.

**Your Task:** In upload.js, inside the validateFile() function's switch statement, implement the 'case "document":' branch. Look for TODO(human). This should validate document files (pdf, doc, docx).

**Guidance:** Consider checking file size limits (maybe 10MB for documents?), validating the file extension matches the MIME type, and returning {valid: boolean, error?: string}. The file object has properties: name, size, type.

**Debugging Example:**
• **Learn by Doing**

**Context:** The user reported that number inputs aren't working correctly in the calculator. I've identified the handleInput() function as the likely source, but need to understand what values are being processed.

**Your Task:** In calculator.js, inside the handleInput() function, add 2-3 console.log statements after the TODO(human) comment to help debug why number inputs fail.

**Guidance:** Consider logging: the raw input value, the parsed result, and any validation state. This will help us understand where the conversion breaks.

### After Contributions
Share one insight connecting their code to broader patterns or system effects. Avoid praise or repetition.

## Insights
In order to encourage learning, before and after writing code, always provide brief educational explanations about implementation choices using (with backticks):
"`★ Insight ─────────────────────────────────────`
[2-3 key educational points]
`─────────────────────────────────────────────────`"

These insights should be included in the conversation, not in the codebase. You should generally focus on interesting insights that are specific to the codebase or the code you just wrote, rather than general programming concepts."#;

// ============================================
// Built-in Styles
// ============================================

/// Returns the built-in output styles.
pub fn built_in_styles() -> Vec<OutputStyle> {
    vec![
        // 1. Default - no modifications
        OutputStyle::new(
            DEFAULT_STYLE_NAME,
            "Codex completes coding tasks efficiently and provides concise responses",
            None,
            false,
            OutputStyleSource::BuiltIn,
        ),
        // 2. Explanatory
        OutputStyle::new(
            "Explanatory",
            "Codex explains its implementation choices and codebase patterns",
            Some(EXPLANATORY_PROMPT.to_string()),
            true,
            OutputStyleSource::BuiltIn,
        ),
        // 3. Learning
        OutputStyle::new(
            "Learning",
            "Codex pauses and asks you to write small pieces of code for hands-on practice",
            Some(LEARNING_PROMPT.to_string()),
            true,
            OutputStyleSource::BuiltIn,
        ),
    ]
}

// ============================================
// Custom Style Loading
// ============================================

/// Parse YAML frontmatter from markdown content.
fn parse_frontmatter(content: &str) -> (HashMap<String, String>, String) {
    let mut frontmatter = HashMap::new();
    let body;

    if content.starts_with("---") {
        if let Some(end) = content[3..].find("---") {
            let yaml_str = &content[3..3 + end];
            body = content[3 + end + 3..].trim().to_string();

            // Simple YAML parsing (key: value pairs)
            for line in yaml_str.lines() {
                let line = line.trim();
                if let Some(colon_pos) = line.find(':') {
                    let key = line[..colon_pos].trim().to_string();
                    let value = line[colon_pos + 1..]
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'')
                        .to_string();
                    frontmatter.insert(key, value);
                }
            }
        } else {
            body = content.to_string();
        }
    } else {
        body = content.to_string();
    }

    (frontmatter, body)
}

/// Load custom styles from a directory.
fn load_styles_from_dir(dir: &Path, source: OutputStyleSource) -> Vec<OutputStyle> {
    let mut styles = Vec::new();

    if !dir.is_dir() {
        return styles;
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return styles,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let (frontmatter, body) = parse_frontmatter(&content);
        let filename = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("custom")
            .to_string();

        let name = frontmatter.get("name").cloned().unwrap_or(filename.clone());
        let description = frontmatter
            .get("description")
            .cloned()
            .unwrap_or_else(|| format!("Custom {} output style", filename));
        let keep_coding_instructions = frontmatter
            .get("keep-coding-instructions")
            .map(|v| v == "true")
            .unwrap_or(false);

        styles.push(OutputStyle::new(
            name,
            description,
            if body.is_empty() { None } else { Some(body) },
            keep_coding_instructions,
            source,
        ));
    }

    styles
}

/// Load all output styles with proper priority merging.
/// Priority: built-in < user settings < project settings
pub fn load_all_styles(cwd: &Path, codex_home: &Path) -> HashMap<String, OutputStyle> {
    let mut styles = HashMap::new();

    // 1. Built-in styles (lowest priority)
    for style in built_in_styles() {
        styles.insert(style.name.to_lowercase(), style);
    }

    // 2. User settings (~/.codex/output-styles/ or codex_home/output-styles/)
    let user_dir = codex_home.join(OUTPUT_STYLES_DIR);
    for style in load_styles_from_dir(&user_dir, OutputStyleSource::UserSettings) {
        styles.insert(style.name.to_lowercase(), style);
    }

    // 3. Project settings (.codex/output-styles/)
    let project_dir = cwd.join(".codex").join(OUTPUT_STYLES_DIR);
    for style in load_styles_from_dir(&project_dir, OutputStyleSource::ProjectSettings) {
        styles.insert(style.name.to_lowercase(), style);
    }

    styles
}

/// Find a style by name (case-insensitive).
pub fn find_style<'a>(
    styles: &'a HashMap<String, OutputStyle>,
    name: &str,
) -> Option<&'a OutputStyle> {
    styles.get(&name.to_lowercase())
}

/// Get the current output style based on config setting.
pub fn get_current_style(
    styles: &HashMap<String, OutputStyle>,
    current_style_name: &str,
) -> Option<OutputStyle> {
    find_style(styles, current_style_name).cloned()
}

// ============================================
// Tests
// ============================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_built_in_styles() {
        let styles = built_in_styles();
        assert_eq!(styles.len(), 3);

        // Default
        let default = &styles[0];
        assert_eq!(default.name.to_lowercase(), "default");
        assert!(default.prompt.is_none());
        assert!(!default.keep_coding_instructions);

        // Explanatory
        let explanatory = &styles[1];
        assert_eq!(explanatory.name, "Explanatory");
        assert!(explanatory.prompt.is_some());
        assert!(explanatory.keep_coding_instructions);

        // Learning
        let learning = &styles[2];
        assert_eq!(learning.name, "Learning");
        assert!(learning.prompt.is_some());
        assert!(learning.keep_coding_instructions);
    }

    #[test]
    fn test_parse_frontmatter() {
        let content = r#"---
name: "My Style"
description: "A custom style"
keep-coding-instructions: true
---

Custom prompt content here."#;

        let (frontmatter, body) = parse_frontmatter(content);

        assert_eq!(frontmatter.get("name"), Some(&"My Style".to_string()));
        assert_eq!(
            frontmatter.get("description"),
            Some(&"A custom style".to_string())
        );
        assert_eq!(
            frontmatter.get("keep-coding-instructions"),
            Some(&"true".to_string())
        );
        assert_eq!(body, "Custom prompt content here.");
    }

    #[test]
    fn test_parse_frontmatter_no_frontmatter() {
        let content = "Just content, no frontmatter";
        let (frontmatter, body) = parse_frontmatter(content);

        assert!(frontmatter.is_empty());
        assert_eq!(body, "Just content, no frontmatter");
    }

    #[test]
    fn test_load_styles_from_dir() {
        let temp = tempdir().unwrap();
        let styles_dir = temp.path().join("output-styles");
        fs::create_dir_all(&styles_dir).unwrap();

        // Create a custom style file
        let style_content = r#"---
name: "Custom"
description: "A custom style"
---

Custom prompt."#;
        fs::write(styles_dir.join("custom.md"), style_content).unwrap();

        let styles = load_styles_from_dir(&styles_dir, OutputStyleSource::ProjectSettings);
        assert_eq!(styles.len(), 1);
        assert_eq!(styles[0].name, "Custom");
        assert_eq!(styles[0].description, "A custom style");
        assert_eq!(styles[0].prompt, Some("Custom prompt.".to_string()));
    }

    #[test]
    fn test_load_all_styles() {
        let temp = tempdir().unwrap();
        let cwd = temp.path();
        let codex_home = temp.path().join("codex-home");
        fs::create_dir_all(&codex_home).unwrap();

        let styles = load_all_styles(cwd, &codex_home);

        // Should have at least the 3 built-in styles
        assert!(styles.len() >= 3);
        assert!(styles.contains_key("default"));
        assert!(styles.contains_key("explanatory"));
        assert!(styles.contains_key("learning"));
    }

    #[test]
    fn test_project_overrides_builtin() {
        let temp = tempdir().unwrap();
        let cwd = temp.path();
        let codex_home = temp.path().join("codex-home");
        fs::create_dir_all(&codex_home).unwrap();

        // Create project override for Explanatory
        let project_styles_dir = cwd.join(".codex").join("output-styles");
        fs::create_dir_all(&project_styles_dir).unwrap();

        let override_content = r#"---
name: "Explanatory"
description: "Project-specific explanatory"
---

Project prompt."#;
        fs::write(project_styles_dir.join("explanatory.md"), override_content).unwrap();

        let styles = load_all_styles(cwd, &codex_home);

        let explanatory = styles.get("explanatory").unwrap();
        assert_eq!(explanatory.description, "Project-specific explanatory");
        assert_eq!(explanatory.source, OutputStyleSource::ProjectSettings);
    }

    #[test]
    fn test_find_style() {
        let temp = tempdir().unwrap();
        let styles = load_all_styles(temp.path(), temp.path());

        assert!(find_style(&styles, "default").is_some());
        assert!(find_style(&styles, "Default").is_some());
        assert!(find_style(&styles, "DEFAULT").is_some());
        assert!(find_style(&styles, "explanatory").is_some());
        assert!(find_style(&styles, "Explanatory").is_some());
        assert!(find_style(&styles, "nonexistent").is_none());
    }

    #[test]
    fn test_get_current_style() {
        let temp = tempdir().unwrap();
        let styles = load_all_styles(temp.path(), temp.path());

        let default = get_current_style(&styles, "default");
        assert!(default.is_some());
        assert!(default.unwrap().is_default());

        let explanatory = get_current_style(&styles, "Explanatory");
        assert!(explanatory.is_some());
        assert!(!explanatory.unwrap().is_default());
    }
}
