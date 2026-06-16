use super::canonicalize;
use super::config_schema_json;
use super::write_config_schema;

use pretty_assertions::assert_eq;
use similar::TextDiff;
use tempfile::TempDir;

fn trim_single_trailing_newline(contents: &str) -> &str {
    contents.strip_suffix('\n').unwrap_or(contents)
}

#[test]
fn config_schema_matches_fixture() {
    let fixture_path = codex_utils_cargo_bin::find_resource!("config.schema.json")
        .expect("resolve config schema fixture path");
    let fixture = std::fs::read_to_string(fixture_path).expect("read config schema fixture");
    let fixture_value: serde_json::Value =
        serde_json::from_str(&fixture).expect("parse config schema fixture");
    let schema_json = config_schema_json().expect("serialize config schema");
    let schema_value: serde_json::Value =
        serde_json::from_slice(&schema_json).expect("decode schema json");
    let fixture_value = canonicalize(&fixture_value);
    let schema_value = canonicalize(&schema_value);
    if fixture_value != schema_value {
        let expected =
            serde_json::to_string_pretty(&fixture_value).expect("serialize fixture json");
        let actual = serde_json::to_string_pretty(&schema_value).expect("serialize schema json");
        let diff = TextDiff::from_lines(&expected, &actual)
            .unified_diff()
            .header("fixture", "generated")
            .to_string();
        panic!(
            "Current schema for `config.toml` doesn't match the fixture. \
Run `just write-config-schema` to overwrite with your changes.\n\n{diff}"
        );
    }

    // Make sure the version in the repo matches exactly: https://github.com/openai/codex/pull/10977.
    let tmp = TempDir::new().expect("create temp dir");
    let tmp_path = tmp.path().join("config.schema.json");
    write_config_schema(&tmp_path).expect("write config schema to temp path");
    let tmp_contents =
        std::fs::read_to_string(&tmp_path).expect("read back config schema from temp path");
    #[cfg(windows)]
    let fixture = fixture.replace("\r\n", "\n");
    #[cfg(windows)]
    let tmp_contents = tmp_contents.replace("\r\n", "\n");

    assert_eq!(
        trim_single_trailing_newline(&fixture),
        trim_single_trailing_newline(&tmp_contents),
        "fixture should match exactly with generated schema"
    );
}

#[test]
fn config_schema_hides_unsupported_inline_mcp_bearer_token() {
    let schema_json = config_schema_json().expect("serialize config schema");
    let schema_value: serde_json::Value =
        serde_json::from_slice(&schema_json).expect("decode schema json");
    let properties = schema_value
        .pointer("/definitions/RawMcpServerConfig/properties")
        .expect("RawMcpServerConfig properties should exist")
        .as_object()
        .expect("RawMcpServerConfig properties should be an object");

    assert_eq!(
        (
            properties.contains_key("bearer_token"),
            properties.contains_key("bearer_token_env_var"),
        ),
        (false, true),
    );
}
