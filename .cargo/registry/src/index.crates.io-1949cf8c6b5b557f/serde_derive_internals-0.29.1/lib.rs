#![doc(html_root_url = "https://docs.rs/serde_derive_internals/0.29.1")]
#![cfg_attr(not(check_cfg), allow(unexpected_cfgs))]
// Ignored clippy lints
#![allow(
    clippy::cognitive_complexity,
    // clippy bug: https://github.com/rust-lang/rust-clippy/issues/7575
    clippy::collapsible_match,
    clippy::derive_partial_eq_without_eq,
    // clippy bug: https://github.com/rust-lang/rust-clippy/issues/6797
    clippy::manual_map,
    clippy::missing_panics_doc,
    clippy::redundant_field_names,
    clippy::result_unit_err,
    clippy::should_implement_trait,
    clippy::trivially_copy_pass_by_ref,
    clippy::wildcard_in_or_patterns,
    // clippy bug: https://github.com/rust-lang/rust-clippy/issues/5704
    clippy::unnested_or_patterns,
)]
// Ignored clippy_pedantic lints
#![allow(
    clippy::doc_markdown,
    clippy::enum_glob_use,
    clippy::items_after_statements,
    clippy::let_underscore_untyped,
    clippy::manual_assert,
    clippy::match_same_arms,
    // clippy bug: https://github.com/rust-lang/rust-clippy/issues/6984
    clippy::match_wildcard_for_single_variants,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::return_self_not_must_use,
    clippy::similar_names,
    clippy::single_match_else,
    clippy::struct_excessive_bools,
    clippy::too_many_lines,
    clippy::unused_self,
    clippy::wildcard_imports
)]

extern crate proc_macro2;
extern crate quote;
extern crate syn;

#[cfg_attr(serde_build_from_git, path = "../serde_derive/src/internals/mod.rs")]
#[cfg_attr(not(serde_build_from_git), path = "src/mod.rs")]
mod internals;

pub use internals::*;
