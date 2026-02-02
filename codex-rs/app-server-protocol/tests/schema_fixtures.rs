use anyhow::Context;
use anyhow::Result;
use codex_app_server_protocol::read_schema_fixture_tree;
use codex_app_server_protocol::write_schema_fixtures;
use similar::TextDiff;
use std::path::Path;

#[test]
fn schema_fixtures_match_generated() -> Result<()> {
    let schema_root = schema_root()?;
    let fixture_tree = read_tree(&schema_root)?;

    let temp_dir = tempfile::tempdir().context("create temp dir")?;
    write_schema_fixtures(temp_dir.path(), None).context("generate schema fixtures")?;
    let generated_tree = read_tree(temp_dir.path())?;

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
            "Vendored app-server schema fixture file set doesn't match freshly generated output. \
Run `just write-app-server-schema` to overwrite with your changes.\n\n{diff}"
        );
    }

    // If the file sets match, diff contents for each file for a nicer error.
    for (path, expected) in &fixture_tree {
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
            "Vendored app-server schema fixture {} differs from generated output. \
Run `just write-app-server-schema` to overwrite with your changes.\n\n{diff}",
            path.display()
        );
    }

    Ok(())
}

fn schema_root() -> Result<std::path::PathBuf> {
    // In Bazel runfiles (especially manifest-only mode), resolving directories is not
    // reliable. Resolve a known file, then walk up to the schema root.
    let typescript_index = codex_utils_cargo_bin::find_resource!("schema/typescript/index.ts")
        .context("resolve TypeScript schema index.ts")?;
    let schema_root = typescript_index
        .parent()
        .and_then(|p| p.parent())
        .context("derive schema root from schema/typescript/index.ts")?
        .to_path_buf();

    // Sanity check that the JSON fixtures resolve to the same schema root.
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

fn read_tree(root: &Path) -> Result<std::collections::BTreeMap<std::path::PathBuf, Vec<u8>>> {
    read_schema_fixture_tree(root).context("read schema fixture tree")
}
