//! No-op schema derives for production app-server protocol builds.
//!
//! The real `ts-rs` and `schemars` derives are only needed when regenerating
//! the vendored protocol exports. Normal builds retain the annotations so the
//! protocol definitions stay readable, but use these derives to avoid
//! generating implementations that cannot be reached at runtime.

use proc_macro::TokenStream;

/// Accepts `#[schemars(...)]` helper attributes without generating an impl.
#[proc_macro_derive(JsonSchema, attributes(schemars))]
pub fn derive_json_schema(_input: TokenStream) -> TokenStream {
    TokenStream::new()
}

/// Accepts `#[ts(...)]` helper attributes without generating an impl.
#[proc_macro_derive(TS, attributes(ts))]
pub fn derive_ts(_input: TokenStream) -> TokenStream {
    TokenStream::new()
}
