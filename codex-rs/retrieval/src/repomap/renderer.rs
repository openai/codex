//! Tree renderer for repo map output.
//!
//! Formats ranked symbols as a tree structure with file paths
//! and line numbers for LLM context.

use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;

use super::RankedSymbol;

/// Maximum line length before truncation (for minified JS, etc.)
const MAX_LINE_LENGTH: usize = 100;

/// Tree renderer for repo map output.
pub struct TreeRenderer {
    /// Include line numbers in output
    show_line_numbers: bool,
    /// Include signatures in output
    show_signatures: bool,
}

impl TreeRenderer {
    /// Create a new tree renderer with default settings.
    pub fn new() -> Self {
        Self {
            show_line_numbers: true,
            show_signatures: true,
        }
    }

    /// Create a renderer with custom settings.
    #[allow(dead_code)]
    pub fn with_options(show_line_numbers: bool, show_signatures: bool) -> Self {
        Self {
            show_line_numbers,
            show_signatures,
        }
    }

    /// Render ranked symbols as a tree.
    ///
    /// # Arguments
    /// * `symbols` - Ranked symbols sorted by rank descending
    /// * `chat_files` - Files in chat context (highlighted)
    /// * `count` - Number of symbols to include
    /// * `workspace_root` - Workspace root for relative path display
    ///
    /// # Returns
    /// A tuple of (rendered content, set of rendered file paths)
    pub fn render(
        &self,
        symbols: &[RankedSymbol],
        chat_files: &HashSet<String>,
        count: i32,
        _workspace_root: &Path,
    ) -> (String, HashSet<String>) {
        // Take top N symbols
        let symbols_to_render = &symbols[..symbols.len().min(count as usize)];

        // Group by file
        let mut file_symbols: HashMap<String, Vec<&RankedSymbol>> = HashMap::new();
        for sym in symbols_to_render {
            file_symbols
                .entry(sym.filepath.clone())
                .or_default()
                .push(sym);
        }

        // Sort files by their highest-ranked symbol
        let mut file_order: Vec<(String, f64)> = file_symbols
            .iter()
            .map(|(path, syms)| {
                let max_rank = syms.iter().map(|s| s.rank).fold(0.0_f64, f64::max);
                (path.clone(), max_rank)
            })
            .collect();
        file_order.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Track rendered files
        let mut rendered_files: HashSet<String> = HashSet::new();

        // Render output
        let mut output = String::new();

        for (filepath, _rank) in file_order {
            let syms = match file_symbols.get(&filepath) {
                Some(s) => s,
                None => continue,
            };

            // Track this file as rendered
            rendered_files.insert(filepath.clone());

            // File header (highlight chat files)
            let is_chat_file = chat_files.contains(&filepath);
            if is_chat_file {
                output.push_str(&format!("{}:  [chat]\n", filepath));
            } else {
                output.push_str(&format!("{}:\n", filepath));
            }

            // Sort symbols by line number within file
            let mut sorted_syms: Vec<&&RankedSymbol> = syms.iter().collect();
            sorted_syms.sort_by_key(|s| s.tag.start_line);

            // Render each symbol
            for sym in sorted_syms {
                self.render_symbol(&mut output, sym);
            }

            output.push('\n');
        }

        // Truncate long lines (e.g., minified JS)
        (Self::truncate_lines(output.trim_end()), rendered_files)
    }

    /// Render symbols without file context (for token counting).
    pub fn render_symbols(&self, symbols: &[RankedSymbol], count: i32) -> String {
        if symbols.is_empty() || count <= 0 {
            return String::new();
        }

        let mut output = String::new();
        let symbols_to_render = &symbols[..symbols.len().min(count as usize)];

        // Group by filepath
        let mut current_file: Option<String> = None;

        for sym in symbols_to_render {
            // Add file header if changed
            if current_file.as_ref() != Some(&sym.filepath) {
                if current_file.is_some() {
                    output.push('\n');
                }
                output.push_str(&format!("{}:\n", sym.filepath));
                current_file = Some(sym.filepath.clone());
            }

            self.render_symbol(&mut output, sym);
        }

        // Truncate long lines
        Self::truncate_lines(&output)
    }

    /// Render a single symbol.
    fn render_symbol(&self, output: &mut String, sym: &RankedSymbol) {
        let tag = &sym.tag;

        if self.show_line_numbers {
            output.push_str(&format!("│{:>4}: ", tag.start_line));
        } else {
            output.push_str("│  ");
        }

        if self.show_signatures && tag.signature.is_some() {
            output.push_str(tag.signature.as_ref().unwrap());
        } else {
            // Fallback to kind + name
            output.push_str(&format!("{:?} {}", tag.kind, tag.name));
        }

        output.push('\n');
    }

    /// Truncate lines that exceed MAX_LINE_LENGTH.
    fn truncate_lines(output: &str) -> String {
        output
            .lines()
            .map(|line| {
                if line.len() > MAX_LINE_LENGTH {
                    // Use char boundary to avoid panic on multi-byte UTF-8 characters
                    let truncated: String = line.chars().take(MAX_LINE_LENGTH - 3).collect();
                    format!("{truncated}...")
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl Default for TreeRenderer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tags::extractor::CodeTag;
    use crate::tags::extractor::TagKind;

    fn make_symbol(name: &str, line: i32, signature: &str) -> RankedSymbol {
        RankedSymbol {
            tag: CodeTag {
                name: name.to_string(),
                kind: TagKind::Function,
                start_line: line,
                end_line: line + 10,
                start_byte: line * 100,
                end_byte: (line + 10) * 100,
                signature: Some(signature.to_string()),
                docs: None,
                is_definition: true,
            },
            rank: 1.0 / (line as f64),
            filepath: format!("src/file_{}.rs", line / 100),
        }
    }

    #[test]
    fn test_render_empty() {
        let renderer = TreeRenderer::new();
        let output = renderer.render_symbols(&[], 10);
        assert!(output.is_empty());
    }

    #[test]
    fn test_render_single_symbol() {
        let renderer = TreeRenderer::new();
        let symbols = vec![make_symbol("foo", 10, "fn foo() -> i32")];

        let output = renderer.render_symbols(&symbols, 1);

        assert!(output.contains("fn foo() -> i32"));
        assert!(output.contains("10:"));
    }

    #[test]
    fn test_render_multiple_symbols() {
        let renderer = TreeRenderer::new();
        let symbols = vec![
            make_symbol("foo", 10, "fn foo()"),
            make_symbol("bar", 20, "fn bar()"),
            make_symbol("baz", 30, "fn baz()"),
        ];

        let output = renderer.render_symbols(&symbols, 3);

        assert!(output.contains("fn foo()"));
        assert!(output.contains("fn bar()"));
        assert!(output.contains("fn baz()"));
    }

    #[test]
    fn test_render_with_count_limit() {
        let renderer = TreeRenderer::new();
        let symbols = vec![
            make_symbol("foo", 10, "fn foo()"),
            make_symbol("bar", 20, "fn bar()"),
            make_symbol("baz", 30, "fn baz()"),
        ];

        let output = renderer.render_symbols(&symbols, 2);

        assert!(output.contains("fn foo()"));
        assert!(output.contains("fn bar()"));
        assert!(!output.contains("fn baz()"));
    }

    #[test]
    fn test_render_without_line_numbers() {
        let renderer = TreeRenderer::with_options(false, true);
        let symbols = vec![make_symbol("foo", 10, "fn foo()")];

        let output = renderer.render_symbols(&symbols, 1);

        assert!(output.contains("fn foo()"));
        assert!(!output.contains("10:"));
    }

    #[test]
    fn test_render_full_tree() {
        let renderer = TreeRenderer::new();
        let symbols = vec![
            make_symbol("process", 100, "fn process(req: Request) -> Response"),
            make_symbol("handle", 150, "fn handle(data: &[u8])"),
        ];

        let chat_files: HashSet<String> = ["src/file_1.rs".to_string()].into_iter().collect();
        let (output, rendered_files) =
            renderer.render(&symbols, &chat_files, 2, Path::new("/project"));

        // Should have file headers and symbol lines
        assert!(output.contains(".rs:"));
        assert!(output.contains("fn process"));

        // Should return the set of rendered files
        assert!(rendered_files.contains("src/file_1.rs"));
    }

    #[test]
    fn test_line_truncation() {
        // Test that lines longer than MAX_LINE_LENGTH are truncated
        let long_line = "a".repeat(150);
        let truncated = TreeRenderer::truncate_lines(&long_line);

        // Should be MAX_LINE_LENGTH chars (97 + "...")
        assert_eq!(truncated.len(), 100);
        assert!(truncated.ends_with("..."));

        // Short lines should not be truncated
        let short_line = "short line";
        let not_truncated = TreeRenderer::truncate_lines(short_line);
        assert_eq!(not_truncated, short_line);

        // Test multiple lines
        let multi_line = format!("{}\n{}\nshort", "b".repeat(120), "c".repeat(80));
        let truncated_multi = TreeRenderer::truncate_lines(&multi_line);
        let lines: Vec<&str> = truncated_multi.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].ends_with("...")); // First line truncated
        assert_eq!(lines[1].len(), 80); // Second line not truncated (80 < 100)
        assert_eq!(lines[2], "short"); // Third line not truncated
    }
}
