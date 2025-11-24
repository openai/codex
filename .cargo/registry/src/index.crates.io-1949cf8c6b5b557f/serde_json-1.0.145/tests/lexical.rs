#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::comparison_chain,
    clippy::doc_markdown,
    clippy::excessive_precision,
    clippy::float_cmp,
    clippy::if_not_else,
    clippy::let_underscore_untyped,
    clippy::module_name_repetitions,
    clippy::needless_late_init,
    clippy::question_mark,
    clippy::shadow_unrelated,
    clippy::similar_names,
    clippy::single_match_else,
    clippy::too_many_lines,
    clippy::unreadable_literal,
    clippy::unseparated_literal_suffix,
    clippy::wildcard_imports
)]

extern crate alloc;

#[path = "../src/lexical/mod.rs"]
mod lexical;

#[path = "lexical/algorithm.rs"]
mod algorithm;

#[path = "lexical/exponent.rs"]
mod exponent;

#[path = "lexical/float.rs"]
mod float;

#[path = "lexical/math.rs"]
mod math;

#[path = "lexical/num.rs"]
mod num;

#[path = "lexical/parse.rs"]
mod parse;

#[path = "lexical/rounding.rs"]
mod rounding;
