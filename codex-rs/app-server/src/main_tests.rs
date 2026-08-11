use super::AppServerArgs;
use clap::Parser;
use codex_app_server::AppServerTransport;
use pretty_assertions::assert_eq;
use toml::Value as TomlValue;
use url::Url;

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

#[test]
fn app_server_accepts_process_scoped_code_mode_host() {
    let args = AppServerArgs::try_parse_from([
        "codex-app-server",
        "--code-mode-host",
        "wss://example.test/code-mode",
        "--listen",
        "off",
    ])
    .expect("parse app-server args");

    assert_eq!(
        args.code_mode_host.code_mode_host,
        Some(Url::parse("wss://example.test/code-mode").expect("test endpoint should parse"))
    );
    assert_eq!(args.listen, AppServerTransport::Off);
    assert_eq!(args.config_overrides.raw_overrides, Vec::<String>::new());
}

#[test]
fn app_server_rejects_invalid_code_mode_host() {
    for endpoint in [
        "http://127.0.0.1:8765",
        "ws://",
        "wss://example.test/code-mode#fragment",
    ] {
        let error =
            AppServerArgs::try_parse_from(["codex-app-server", "--code-mode-host", endpoint])
                .expect_err("invalid code-mode host endpoint should fail startup argument parsing");

        assert_eq!(error.kind(), clap::error::ErrorKind::ValueValidation);
    }
}
