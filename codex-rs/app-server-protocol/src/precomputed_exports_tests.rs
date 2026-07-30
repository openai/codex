use crate::GenerateTsOptions;
use crate::generate_internal_json_schema;
use crate::generate_json_with_experimental;
use crate::generate_ts_with_options;
use crate::precomputed_exports::GENERATED_TS_HEADER;
use crate::precomputed_exports::decode_precomputed_exports;
use crate::schema_fixtures::collect_export_files_recursive;
use anyhow::Context;
use anyhow::Result;
use pretty_assertions::assert_eq;
use std::path::Path;

#[test]
fn precomputed_exports_are_written_to_disk() -> Result<()> {
    let output_dir = tempfile::tempdir().context("create export temp dir")?;
    let typescript_dir = output_dir.path().join("typescript");
    let json_dir = output_dir.path().join("json");
    let internal_json_dir = output_dir.path().join("internal-json");

    generate_ts_with_options(
        &typescript_dir,
        /*prettier*/ None,
        GenerateTsOptions::default(),
    )?;
    generate_json_with_experimental(&json_dir, /*experimental_api*/ false)?;
    generate_internal_json_schema(&internal_json_dir)?;

    let exports = decode_precomputed_exports(/*experimental_api*/ false)?;
    assert_eq!(
        collect_export_files_recursive(&typescript_dir)?,
        exports.typescript
    );
    assert_eq!(
        collect_export_files_recursive(&json_dir)?,
        exports.json_schema
    );
    assert_eq!(
        collect_export_files_recursive(&internal_json_dir)?,
        exports.internal_json_schema
    );
    Ok(())
}

#[test]
fn typescript_export_options_preserve_generator_behavior() -> Result<()> {
    let output_dir = tempfile::tempdir().context("create TypeScript export temp dir")?;
    generate_ts_with_options(
        output_dir.path(),
        /*prettier*/ None,
        GenerateTsOptions {
            generate_indices: false,
            ensure_headers: false,
            ..GenerateTsOptions::default()
        },
    )?;

    let files = collect_export_files_recursive(output_dir.path())?;
    assert!(!files.keys().any(|path| {
        Path::new(path)
            .file_name()
            .is_some_and(|name| name == "index.ts")
    }));
    assert!(
        files
            .values()
            .all(|contents| !contents.starts_with(GENERATED_TS_HEADER))
    );
    Ok(())
}
