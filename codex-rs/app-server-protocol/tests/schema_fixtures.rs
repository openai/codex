use codex_app_server_protocol::read_schema_fixture_tree;
use codex_app_server_protocol::write_schema_fixtures;
use similar::TextDiff;
use std::path::Path;

fn schema_root() -> std::path::PathBuf {
    let fixture_path =
        codex_utils_cargo_bin::find_resource!("schema").expect("resolve schema fixture root");
    fixture_path
}

fn read_tree(root: &Path) -> std::collections::BTreeMap<std::path::PathBuf, Vec<u8>> {
    read_schema_fixture_tree(root).expect("read schema fixture tree")
}

#[test]
fn schema_fixtures_match_generated() {
    let schema_root = schema_root();
    let fixture_tree = read_tree(&schema_root);

    let temp_dir = tempfile::tempdir().expect("create temp dir");
    write_schema_fixtures(temp_dir.path(), None).expect("generate schema fixtures");
    let generated_tree = read_tree(temp_dir.path());

    if fixture_tree != generated_tree {
        let expected = fixture_tree
            .keys()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let actual = generated_tree
            .keys()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let diff = TextDiff::from_lines(&expected, &actual)
            .unified_diff()
            .header("fixture", "generated")
            .to_string();

        panic!(
            "Vendored app-server schema fixtures don't match freshly generated output. \
Run `just write-app-server-schema` to overwrite with your changes.\n\n{diff}"
        );
    }

    // If the file sets match, diff contents for each file for a nicer error.
    for (path, expected) in &fixture_tree {
        let actual = generated_tree
            .get(path)
            .unwrap_or_else(|| panic!("missing generated file: {}", path.display()));

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
}
