use super::AppServerArgs;
use super::app_server_control_socket_for_ssh_agent;
use clap::Parser;
use pretty_assertions::assert_eq;
use std::path::Path;
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

#[test]
fn unix_socket_transport_prepares_ssh_agent_forwarding() {
    let args = AppServerArgs::try_parse_from([
        "codex-app-server",
        "--listen",
        "unix:///tmp/codex-app-server.sock",
    ])
    .expect("parse app-server args");

    assert_eq!(
        app_server_control_socket_for_ssh_agent(&args),
        Some(Path::new("/tmp/codex-app-server.sock"))
    );
}

#[test]
fn non_unix_transport_does_not_prepare_ssh_agent_forwarding() {
    let args = AppServerArgs::try_parse_from(["codex-app-server", "--listen", "stdio://"])
        .expect("parse app-server args");

    assert_eq!(app_server_control_socket_for_ssh_agent(&args), None);
}
