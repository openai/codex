use super::AppServerArgs;
use clap::Parser;
use pretty_assertions::assert_eq;

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

    assert_eq!(
        args.config_overrides.raw_overrides,
        vec![
            "model=\"gpt-5-codex\"".to_string(),
            "sandbox_mode=\"read-only\"".to_string(),
        ]
    );
}
