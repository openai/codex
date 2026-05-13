mod backend;
mod extension;
mod local;
mod schema;
mod tools;

pub use extension::install;

pub(crate) const DEFAULT_LIST_MAX_RESULTS: usize = 2_000;
pub(crate) const MAX_LIST_RESULTS: usize = 2_000;
pub(crate) const DEFAULT_SEARCH_MAX_RESULTS: usize = 200;
pub(crate) const MAX_SEARCH_RESULTS: usize = 200;
pub(crate) const DEFAULT_READ_MAX_TOKENS: usize = 20_000;

pub(crate) const LIST_TOOL_NAME: &str = "memory_list";
pub(crate) const READ_TOOL_NAME: &str = "memory_read";
pub(crate) const SEARCH_TOOL_NAME: &str = "memory_search";

#[cfg(test)]
mod tests;
