use clap::Parser;

use super::McpCli;
use super::McpSubcommand;

#[test]
fn parses_callback_url_subcommand() {
    let cli = McpCli::try_parse_from(["codex", "callback-url", "outlookmcp"])
        .expect("callback-url command should parse");

    assert!(matches!(
        cli.subcommand,
        McpSubcommand::CallbackUrl(args) if args.name == "outlookmcp"
    ));
}
