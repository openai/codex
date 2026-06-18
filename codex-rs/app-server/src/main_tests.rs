use super::AppServerArgs;
use clap::Parser;
use pretty_assertions::assert_eq;
use toml::Value as TomlValue;

#[test]
fn app_server_accepts_cli_config_overrides() {
    let args = AppServerArgs::try_parse_from([
        "codex-app-server",
        "-c",
        "model=\"gpt-5-codex\"",
        "--config",
        "sandbox_mode=\"read-only\"",
        "--listen",
        "off",
    ])
    .expect("parse app-server args");

    let parsed_overrides = args
        .config_overrides
        .parse_overrides()
        .expect("parse config overrides");

    assert_eq!(
        parsed_overrides,
        vec![
            (
                "model".to_string(),
                TomlValue::String("gpt-5-codex".to_string()),
            ),
            (
                "sandbox_mode".to_string(),
                TomlValue::String("read-only".to_string()),
            ),
        ]
    );
}
