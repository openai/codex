#![cfg(test)]

use assert_matches::assert_matches;
use clap::error::ErrorKind;
use pretty_assertions::assert_eq;

use super::*;
use crate::ExecServerCli;

fn exec_server_from_args(args: &[&str]) -> ExecServerCommand {
    let cli = ExecServerCli::try_parse_from(
        ["codex-exec-server"]
            .into_iter()
            .chain(args.iter().copied()),
    )
    .expect("parse executor arguments");
    cli.command
}

#[test]
fn exec_server_defaults_preserve_local_and_noise_modes() {
    for args in [
        vec![],
        vec!["--listen", "stdio"],
        vec![
            "--remote",
            "https://registry.example.com",
            "--environment-id",
            "env-1",
        ],
        vec![
            "--remote",
            "https://registry.example.com",
            "--environment-id",
            "env-1",
            "--remote-transport",
            "noise",
            "--use-agent-identity-auth",
        ],
    ] {
        let command = exec_server_from_args(&args);
        assert_eq!(command.remote_transport, ExecServerRemoteTransport::Noise);
        assert!(!command.aws_sigv4);
        assert!(!command.executor_credential_stdin);
        command
            .validate_remote_transport()
            .expect("valid existing mode");
    }
}

#[test]
fn remote_forwarder_skips_cloud_config() {
    let command = exec_server_from_args(&[
        "forward",
        "--connect",
        "ws://localhost:8765",
        "--remote",
        "https://registry.example.com",
        "--environment-id",
        "env-1",
    ]);
    assert!(!command.should_load_cloud_config());
}

#[test]
fn exec_server_parses_direct_aws_options() {
    for (options, expected) in [
        (vec![], (None, None, "execute-api")),
        (
            vec![
                "--aws-profile",
                "development",
                "--aws-region",
                "us-west-2",
                "--aws-service",
                "bedrock-mantle",
            ],
            (Some("development"), Some("us-west-2"), "bedrock-mantle"),
        ),
    ] {
        let mut args = vec![
            "--remote",
            "https://registry.example.com",
            "--environment-id",
            "env-1",
            "--remote-transport",
            "direct",
            "--aws-sigv4",
        ];
        args.extend(options);
        let command = exec_server_from_args(&args);

        assert_eq!(
            command.remote.as_deref(),
            Some("https://registry.example.com")
        );
        assert_eq!(command.environment_id.as_deref(), Some("env-1"));
        assert_eq!(command.remote_transport, ExecServerRemoteTransport::Direct);
        assert!(!command.should_load_cloud_config());
        assert!(command.aws_sigv4);
        assert_eq!(
            (
                command.aws_profile.as_deref(),
                command.aws_region.as_deref(),
                command.aws_service.as_str()
            ),
            expected,
        );
        command
            .validate_remote_transport()
            .expect("valid Direct mode");
    }
}

#[test]
fn exec_server_parses_executor_credential_options() {
    let args = [
        "--remote",
        "https://registry.example.com",
        "--environment-id",
        "env-1",
    ];
    for options in [
        vec!["--executor-credential-stdin"],
        vec!["--executor-credential-stdin", "--exit-on-stdin-close"],
    ] {
        let options: Vec<_> = args.iter().copied().chain(options).collect();
        let command = exec_server_from_args(&options);
        assert!(command.executor_credential_stdin);
        command
            .validate_remote_transport()
            .expect("valid scoped mode");
    }
}

#[test]
fn exec_server_rejects_missing_or_conflicting_remote_options() {
    for (options, expected) in [
        (
            vec!["--remote-transport", "direct"],
            ErrorKind::MissingRequiredArgument,
        ),
        (
            vec!["--aws-profile", "development"],
            ErrorKind::MissingRequiredArgument,
        ),
        (
            vec!["--aws-region", "us-west-2"],
            ErrorKind::MissingRequiredArgument,
        ),
        (
            vec!["--aws-service", "execute-api"],
            ErrorKind::MissingRequiredArgument,
        ),
        (
            vec![
                "--remote-transport",
                "direct",
                "--aws-sigv4",
                "--use-agent-identity-auth",
            ],
            ErrorKind::ArgumentConflict,
        ),
        (
            vec!["--executor-credential-stdin", "--use-agent-identity-auth"],
            ErrorKind::ArgumentConflict,
        ),
        (
            vec![
                "--executor-credential-stdin",
                "--remote-transport",
                "direct",
                "--aws-sigv4",
            ],
            ErrorKind::ArgumentConflict,
        ),
        (
            vec!["--remote-transport", "unsupported"],
            ErrorKind::InvalidValue,
        ),
    ] {
        let error = ExecServerCli::try_parse_from(
            [
                "codex-exec-server",
                "--remote",
                "https://registry.example.com",
                "--environment-id",
                "env-1",
            ]
            .into_iter()
            .chain(options),
        )
        .expect_err("reject invalid remote arguments");
        assert_eq!(error.kind(), expected);
    }
}

#[test]
fn exec_server_transport_and_aws_options_require_registration_arguments() {
    for options in [
        vec!["--remote-transport", "noise"],
        vec!["--remote-transport", "direct", "--aws-sigv4"],
        vec!["--aws-sigv4"],
        vec!["--executor-credential-stdin"],
        vec![
            "--remote",
            "https://registry.example.com",
            "--remote-transport",
            "direct",
            "--aws-sigv4",
        ],
    ] {
        let error = ExecServerCli::try_parse_from(["codex-exec-server"].into_iter().chain(options))
            .expect_err("require remote URL and environment ID");
        assert_eq!(error.kind(), ErrorKind::MissingRequiredArgument);
    }
}

#[tokio::test]
async fn exec_server_sigv4_does_not_enable_aws_auth_for_noise() {
    for options in [vec![], vec!["--remote-transport", "noise"]] {
        let mut args = vec![
            "--remote",
            "https://registry.example.com",
            "--environment-id",
            "env-1",
            "--aws-sigv4",
        ];
        args.extend(options);
        let command = exec_server_from_args(&args);
        // Unset runtime paths prove validation runs before startup or credential loading.
        let error = command
            .run(
                &Arg0DispatchPaths::default(),
                &CliConfigOverrides::default(),
            )
            .await
            .expect_err("Noise auth is unchanged");
        assert_eq!(
            error.to_string(),
            "--aws-sigv4 requires --remote-transport direct"
        );
    }
}

#[tokio::test]
async fn exec_server_direct_forwarding_remains_rejected() {
    let command = exec_server_from_args(&[
        "forward",
        "--connect",
        "ws://127.0.0.1:8765",
        "--remote",
        "https://registry.example.com",
        "--environment-id",
        "env-1",
        "--remote-transport",
        "direct",
        "--aws-sigv4",
        "--aws-profile",
        "development",
        "--aws-region",
        "us-west-2",
        "--aws-service",
        "bedrock-mantle",
    ]);
    assert_eq!(
        (
            command.remote_transport,
            command.aws_sigv4,
            command.aws_profile.as_deref(),
            command.aws_region.as_deref(),
            command.aws_service.as_str()
        ),
        (
            ExecServerRemoteTransport::Direct,
            true,
            Some("development"),
            Some("us-west-2"),
            "bedrock-mantle"
        ),
    );
    let error = command
        .run(
            &Arg0DispatchPaths::default(),
            &CliConfigOverrides::default(),
        )
        .await
        .expect_err("Direct forwarding is unsupported before startup");
    assert_eq!(
        error.to_string(),
        "direct exec-server transport does not support forwarding"
    );
}

#[test]
fn exec_server_forward_parses_shared_remote_options() {
    let cli = ExecServerCli::try_parse_from([
        "codex-exec-server",
        "forward",
        "--connect",
        "ws://127.0.0.1:8765",
        "--remote",
        "https://example.openai.com",
        "--environment-id",
        "env-1",
        "--name",
        "forwarded",
        "--strict-config",
        "--use-agent-identity-auth",
    ])
    .expect("parse forward");
    assert_matches!(cli.command, ExecServerCommand {
        command: Some(ExecServerSubcommand::Forward { connect }),
        remote: Some(remote),
        environment_id: Some(environment_id),
        name: Some(name),
        strict_config: true,
        use_agent_identity_auth: true,
        ..
    } if connect == "ws://127.0.0.1:8765" && remote == "https://example.openai.com" && environment_id == "env-1" && name == "forwarded");
}

#[test]
fn standalone_help_documents_executor_options() {
    use clap::CommandFactory;
    let help = ExecServerCli::command()
        .term_width(80)
        .mut_arg("exit_on_stdin_close", |arg| arg.hide_env_values(true))
        .mut_arg("proxy_private_ips_via_upstream", |arg| {
            arg.hide_env_values(true)
        })
        .render_long_help()
        .to_string()
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(help);
}
