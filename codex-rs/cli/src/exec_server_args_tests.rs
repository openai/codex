use clap::error::ErrorKind;
use pretty_assertions::assert_eq;

use super::*;
use crate::exec_server_command::ExecServerRemoteTransport;

fn exec_server_from_args(args: &[&str]) -> ExecServerCommand {
    let cli = MultitoolCli::try_parse_from(
        ["codex", "exec-server"]
            .into_iter()
            .chain(args.iter().copied()),
    )
    .expect("parse executor arguments");
    let Some(Subcommand::ExecServer(command)) = cli.subcommand else {
        panic!("expected executor command");
    };
    command
}

#[test]
fn exec_server_help_documents_remote_options() {
    let command = MultitoolCli::command()
        .term_width(80)
        .mut_subcommand("exec-server", |command| {
            command.mut_arg("exit_on_stdin_close", |arg| arg.hide_env_values(true))
        });
    let help = command
        .try_get_matches_from(["codex", "exec-server", "--help"])
        .expect_err("help should exit before running the executor");
    assert_eq!(help.kind(), ErrorKind::DisplayHelp);
    let help_text = help
        .to_string()
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(help_text);
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
        command
            .validate_remote_transport()
            .expect("valid existing mode");
    }
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
            vec!["--remote-transport", "unsupported"],
            ErrorKind::InvalidValue,
        ),
    ] {
        let error = MultitoolCli::try_parse_from(
            [
                "codex",
                "exec-server",
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
        vec![
            "--remote",
            "https://registry.example.com",
            "--remote-transport",
            "direct",
            "--aws-sigv4",
        ],
    ] {
        let error =
            MultitoolCli::try_parse_from(["codex", "exec-server"].into_iter().chain(options))
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
