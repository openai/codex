//! Build script for generating JSON Schema from Rust types.
//!
//! This generates `schema/sdk-protocol.schema.json` at build time.

use std::fs;
use std::path::Path;

// We need to import the types here, but since this runs at build time,
// we can't import from the crate itself. Instead, we'll generate schemas
// in a separate binary or use a feature flag approach.
//
// For now, this is a placeholder that documents the intended behavior.
// The actual schema generation will be done via a separate command.

fn main() {
    // Create schema directory if it doesn't exist
    let schema_dir = Path::new("schema");
    if !schema_dir.exists() {
        fs::create_dir_all(schema_dir).expect("Failed to create schema directory");
    }

    // Note: We cannot directly generate schemas in build.rs because
    // the types aren't compiled yet. Instead, we provide a generate-schema
    // binary or test that generates the schemas.
    //
    // To generate schemas, run:
    //   cargo test -p codex-sdk-protocol generate_schemas -- --ignored
    //
    // Or use the generate-schema binary:
    //   cargo run -p codex-sdk-protocol --bin generate-schema

    println!("cargo:rerun-if-changed=src/");
}
