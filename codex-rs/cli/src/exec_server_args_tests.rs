#![cfg(test)]

use super::*;
use clap::error::ErrorKind;
use pretty_assertions::assert_eq;

#[test]
fn exec_server_help_documents_remote_options() {
    let command = MultitoolCli::command()
        .term_width(80)
        .mut_subcommand("exec-server", |command| {
            command
                .mut_arg("exit_on_stdin_close", |arg| arg.hide_env_values(true))
                .mut_arg("proxy_private_ips_via_upstream", |arg| {
                    arg.hide_env_values(true)
                })
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
