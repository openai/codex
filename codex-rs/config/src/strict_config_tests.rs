use super::*;
use crate::config_toml::ConfigToml;
use crate::diagnostics::TextPosition;
use crate::diagnostics::TextRange;
use pretty_assertions::assert_eq;
use std::path::PathBuf;

#[test]
fn ignored_toml_field_errors_accept_non_file_source_names() {
    let source_name = "com.openai.codex:config_toml_base64";
    let contents = r#"
model = "gpt-5"
unknown_key = true"#;

    let value = toml::from_str::<TomlValue>(contents).expect("valid TOML");
    let error = config_error_from_ignored_toml_value_fields_for_source_name::<ConfigToml>(
        source_name,
        contents,
        value,
    )
    .expect("unknown field error");

    assert_eq!(
        error,
        ConfigError::new(
            PathBuf::from(source_name),
            TextRange {
                start: TextPosition { line: 3, column: 1 },
                end: TextPosition {
                    line: 3,
                    column: 11,
                },
            },
            "unknown configuration field `unknown_key`",
        )
    );
}

#[test]
fn type_errors_take_precedence_over_ignored_fields() {
    let path = Path::new("/tmp/config.toml");
    let contents = r#"
model_context_window = "wide"
unknown_key = true"#;

    let error =
        config_error_from_ignored_toml_fields::<ConfigToml>(path, contents).expect("type error");

    assert_eq!(
        error,
        ConfigError::new(
            path.to_path_buf(),
            TextRange {
                start: TextPosition {
                    line: 2,
                    column: 24,
                },
                end: TextPosition {
                    line: 2,
                    column: 29,
                },
            },
            "invalid type: string \"wide\", expected i64",
        )
    );
}

#[test]
fn strict_config_rejects_unknown_feature_key() {
    let path = Path::new("/tmp/config.toml");
    let contents = r#"
[features]
foo = true"#;

    let error = config_error_from_ignored_toml_fields::<ConfigToml>(path, contents)
        .expect("unknown feature error");

    assert_eq!(
        error,
        ConfigError::new(
            path.to_path_buf(),
            TextRange {
                start: TextPosition { line: 3, column: 1 },
                end: TextPosition { line: 3, column: 3 },
            },
            "unknown configuration field `features.foo`",
        )
    );
}

#[test]
fn strict_config_rejects_unknown_profile_feature_key() {
    let path = Path::new("/tmp/config.toml");
    let contents = r#"
[profiles.work.features]
foo = true"#;

    let error = config_error_from_ignored_toml_fields::<ConfigToml>(path, contents)
        .expect("unknown feature error");

    assert_eq!(
        error,
        ConfigError::new(
            path.to_path_buf(),
            TextRange {
                start: TextPosition { line: 3, column: 1 },
                end: TextPosition { line: 3, column: 3 },
            },
            "unknown configuration field `profiles.work.features.foo`",
        )
    );
}

#[test]
fn strict_config_accepts_opaque_desktop_keys() {
    let path = Path::new("/tmp/config.toml");
    let contents = r#"
[desktop]
appearanceTheme = "dark"

[desktop.workspace]
collapsed = true"#;

    let error = config_error_from_ignored_toml_fields::<ConfigToml>(path, contents);

    assert_eq!(error, None);
}
