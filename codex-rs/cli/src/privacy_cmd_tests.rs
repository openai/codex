use std::path::PathBuf;

use clap::CommandFactory;
use clap::Parser;
use pretty_assertions::assert_eq;

use super::PrivacyCommand;
use super::PrivacyExportCommand;
use super::PrivacySubcommand;

#[test]
fn parses_export_output() {
    let PrivacyCommand {
        subcommand: PrivacySubcommand::Export(PrivacyExportCommand { output }),
    } = PrivacyCommand::try_parse_from(["codex privacy", "export", "out"]).expect("parse");

    assert_eq!(output, PathBuf::from("out"));
}

#[test]
fn privacy_help() {
    let mut command = PrivacyCommand::command();

    insta::assert_snapshot!(command.render_long_help().to_string());
}

#[test]
fn export_help() {
    let mut command = PrivacyCommand::command();
    let export = command
        .find_subcommand_mut("export")
        .expect("export subcommand");

    insta::assert_snapshot!(export.render_long_help().to_string());
}
