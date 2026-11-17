//! Lazy, request-scoped matching of visible conversation snippets in rollout files.

use std::io;
use std::path::Path;
use std::sync::Arc;

use regex::Regex;

use super::case_insensitive_literal_regex;
use super::compression;
use super::content_match_snippet;
use super::json_escaped_search_term;

/// Finds snippets across rollouts for one search term, compiling only when a file is read.
pub struct RolloutSnippetMatcher<'a> {
    search_term: &'a str,
    patterns: Option<Arc<(Regex, Regex)>>,
}

impl<'a> RolloutSnippetMatcher<'a> {
    pub fn new(search_term: &'a str) -> Self {
        Self {
            search_term,
            patterns: None,
        }
    }

    /// Compiles the expressions now when a caller needs to validate a query before discovery.
    pub fn compile(&mut self) -> io::Result<()> {
        self.compiled_patterns().map(|_| ())
    }

    fn compiled_patterns(&mut self) -> io::Result<&Arc<(Regex, Regex)>> {
        Ok(match &mut self.patterns {
            Some(patterns) => patterns,
            slot @ None => slot.insert(Arc::new((
                case_insensitive_literal_regex(json_escaped_search_term(self.search_term)?)?,
                case_insensitive_literal_regex(self.search_term)?,
            ))),
        })
    }

    /// Opens the file before compiling so per-file I/O errors retain their priority.
    pub async fn first_match(&mut self, path: &Path) -> io::Result<Option<String>> {
        let lines = compression::open_rollout_line_reader(path).await?;
        // Share the regexes across file scans to reuse their scratch caches.
        let patterns = Arc::clone(self.compiled_patterns()?);
        lines
            .find_map(move |line| {
                let (json_search_term, search_term) = patterns.as_ref();
                if json_search_term.is_match(line) {
                    content_match_snippet(line, search_term)
                } else {
                    None
                }
            })
            .await
    }
}
