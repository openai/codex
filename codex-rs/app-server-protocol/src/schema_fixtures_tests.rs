use crate::export::GenerateTsOptions;
use crate::export::generate_internal_json_schema;
use crate::export::generate_json_with_experimental;
use crate::export::generate_ts_with_options;
use crate::precomputed_exports::decode_precomputed_exports;
use crate::schema_fixtures::SchemaFixtureOptions;
use crate::schema_fixtures::collect_export_files_recursive;
use crate::schema_fixtures::generate_typescript_schema_fixture_subtree_for_tests;
use crate::schema_fixtures::read_schema_fixture_subtree;
use crate::schema_fixtures::write_schema_fixtures_with_options;
use anyhow::Context;
use anyhow::Result;
use pretty_assertions::assert_eq;
use similar::TextDiff;
use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;

#[test]
fn typescript_schema_fixtures_match_generated() -> Result<()> {
    let schema_root = schema_root()?;
    let fixture_tree = read_tree(&schema_root, "typescript")?;
    let generated_tree = generate_typescript_schema_fixture_subtree_for_tests()
        .context("generate in-memory typescript schema fixtures")?;

    assert_schema_trees_match("typescript", &fixture_tree, &generated_tree)?;
    let config_requirements = generated_tree
        .get(Path::new("v2/ConfigRequirements.ts"))
        .context("generated ConfigRequirements.ts should exist")?;
    anyhow::ensure!(
        !String::from_utf8_lossy(config_requirements).contains("../PathUri")
            || generated_tree.contains_key(Path::new("PathUri.ts")),
        "stable ConfigRequirements.ts imports PathUri but PathUri.ts was not generated"
    );

    Ok(())
}

#[test]
fn json_schema_fixtures_match_generated() -> Result<()> {
    assert_schema_fixtures_match_generated("json", |output_dir| {
        generate_json_with_experimental(output_dir, /*experimental_api*/ false)
    })
}

#[test]
fn stable_precomputed_exports_match_schema_fixtures() -> Result<()> {
    let schema_root = schema_root()?;
    let exports = decode_precomputed_exports(/*experimental_api*/ false)?;

    assert_eq!(
        exports.typescript,
        collect_export_files_recursive(&schema_root.join("typescript"))?
    );
    assert_eq!(
        exports.json_schema,
        collect_export_files_recursive(&schema_root.join("json"))?
    );

    let internal_dir = tempfile::tempdir().context("create internal schema temp dir")?;
    generate_internal_json_schema(internal_dir.path())?;
    assert_json_export_trees_match(
        &exports.internal_json_schema,
        &collect_export_files_recursive(internal_dir.path())?,
    )?;
    Ok(())
}

#[test]
fn experimental_precomputed_exports_match_generated() -> Result<()> {
    let output_dir = tempfile::tempdir().context("create experimental schema temp dir")?;
    let typescript_dir = output_dir.path().join("typescript");
    let json_dir = output_dir.path().join("json");
    generate_ts_with_options(
        &typescript_dir,
        /*prettier*/ None,
        GenerateTsOptions {
            experimental_api: true,
            ..GenerateTsOptions::default()
        },
    )?;
    generate_json_with_experimental(&json_dir, /*experimental_api*/ true)?;

    let exports = decode_precomputed_exports(/*experimental_api*/ true)?;
    assert_eq!(
        exports.typescript,
        collect_export_files_recursive(&typescript_dir)?
    );
    assert_json_export_trees_match(
        &exports.json_schema,
        &collect_export_files_recursive(&json_dir)?,
    )?;
    assert_eq!(exports.internal_json_schema, BTreeMap::new());
    Ok(())
}

#[test]
#[ignore = "invoked by `just write-app-server-schema`"]
fn write_schema_fixtures_from_env() -> Result<()> {
    let schema_root = std::env::var_os("CODEX_APP_SERVER_SCHEMA_ROOT")
        .map(PathBuf::from)
        .context("CODEX_APP_SERVER_SCHEMA_ROOT must be set")?;
    let prettier = std::env::var_os("CODEX_APP_SERVER_SCHEMA_PRETTIER").map(PathBuf::from);
    let experimental = std::env::var("CODEX_APP_SERVER_SCHEMA_EXPERIMENTAL")
        .context("CODEX_APP_SERVER_SCHEMA_EXPERIMENTAL must be set")?;
    let experimental_api = match experimental.as_str() {
        "0" => false,
        "1" => true,
        value => {
            anyhow::bail!("CODEX_APP_SERVER_SCHEMA_EXPERIMENTAL must be 0 or 1, got {value:?}")
        }
    };

    write_schema_fixtures_with_options(
        &schema_root,
        prettier.as_deref(),
        SchemaFixtureOptions { experimental_api },
    )
}

fn assert_schema_fixtures_match_generated(
    label: &'static str,
    generate: impl FnOnce(&Path) -> Result<()>,
) -> Result<()> {
    let schema_root = schema_root()?;
    let fixture_tree = read_tree(&schema_root, label)?;

    let temp_dir = tempfile::tempdir().context("create temp dir")?;
    let generated_root = temp_dir.path().join(label);
    generate(&generated_root).with_context(|| {
        format!(
            "generate {label} schema fixtures into {}",
            generated_root.display()
        )
    })?;

    let generated_tree = read_tree(temp_dir.path(), label)?;
    assert_schema_trees_match(label, &fixture_tree, &generated_tree)
}

fn assert_json_export_trees_match(
    expected: &BTreeMap<String, String>,
    actual: &BTreeMap<String, String>,
) -> Result<()> {
    assert_eq!(
        expected.keys().collect::<Vec<_>>(),
        actual.keys().collect::<Vec<_>>()
    );
    for (path, expected) in expected {
        let expected: serde_json::Value = serde_json::from_str(expected)
            .with_context(|| format!("parse precomputed JSON export {path}"))?;
        let actual: serde_json::Value = serde_json::from_str(&actual[path])
            .with_context(|| format!("parse freshly generated JSON export {path}"))?;
        assert_eq!(expected, actual, "JSON export differs: {path}");
    }
    Ok(())
}

fn assert_schema_trees_match(
    label: &str,
    fixture_tree: &BTreeMap<PathBuf, Vec<u8>>,
    generated_tree: &BTreeMap<PathBuf, Vec<u8>>,
) -> Result<()> {
    let fixture_paths = fixture_tree
        .keys()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>();
    let generated_paths = generated_tree
        .keys()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>();

    if fixture_paths != generated_paths {
        let expected = fixture_paths.join("\n");
        let actual = generated_paths.join("\n");
        let diff = TextDiff::from_lines(&expected, &actual)
            .unified_diff()
            .header("fixture", "generated")
            .to_string();

        panic!(
            "Vendored {label} app-server schema fixture file set doesn't match freshly generated output. \
Run `just write-app-server-schema` to overwrite with your changes.\n\n{diff}"
        );
    }

    for (path, expected) in fixture_tree {
        let actual = generated_tree
            .get(path)
            .ok_or_else(|| anyhow::anyhow!("missing generated file: {}", path.display()))?;

        if expected == actual {
            continue;
        }

        let expected_str = String::from_utf8_lossy(expected);
        let actual_str = String::from_utf8_lossy(actual);
        let diff = TextDiff::from_lines(&expected_str, &actual_str)
            .unified_diff()
            .header("fixture", "generated")
            .to_string();
        panic!(
            "Vendored {label} app-server schema fixture {} differs from generated output. \
Run `just write-app-server-schema` to overwrite with your changes.\n\n{diff}",
            path.display()
        );
    }

    Ok(())
}

fn schema_root() -> Result<PathBuf> {
    let typescript_index = codex_utils_cargo_bin::find_resource!("schema/typescript/index.ts")
        .context("resolve TypeScript schema index.ts")?;
    let schema_root = typescript_index
        .parent()
        .and_then(|p| p.parent())
        .context("derive schema root from schema/typescript/index.ts")?
        .to_path_buf();

    let json_bundle =
        codex_utils_cargo_bin::find_resource!("schema/json/codex_app_server_protocol.schemas.json")
            .context("resolve JSON schema bundle")?;
    let json_root = json_bundle
        .parent()
        .and_then(|p| p.parent())
        .context("derive schema root from schema/json/codex_app_server_protocol.schemas.json")?;
    anyhow::ensure!(
        schema_root == json_root,
        "schema roots disagree: typescript={} json={}",
        schema_root.display(),
        json_root.display()
    );

    Ok(schema_root)
}

fn read_tree(root: &Path, label: &str) -> Result<BTreeMap<PathBuf, Vec<u8>>> {
    read_schema_fixture_subtree(root, label).with_context(|| {
        format!(
            "read {label} schema fixture subtree from {}",
            root.display()
        )
    })
}
