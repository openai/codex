mod async_context_test;
mod corpus_test;
mod detect_language;
mod helpers;
mod highlight_test;
mod language_test;
mod node_test;
mod parser_hang_test;
mod parser_test;
mod pathological_test;
mod query_test;
mod tags_test;
mod test_highlight_test;
mod test_tags_test;
mod text_provider_test;
mod tree_test;

#[cfg(feature = "wasm")]
mod wasm_language_test;

use tree_sitter_generate::GenerateResult;

pub use crate::fuzz::{
    allocations,
    edits::{get_random_edit, invert_edit},
    random::Rand,
    ITERATION_COUNT,
};

/// This is a simple wrapper around [`tree_sitter_generate::generate_parser_for_grammar`], because
/// our tests do not need to pass in a version number, only the grammar JSON.
fn generate_parser(grammar_json: &str) -> GenerateResult<(String, String)> {
    tree_sitter_generate::generate_parser_for_grammar(grammar_json, Some((0, 0, 0)))
}
