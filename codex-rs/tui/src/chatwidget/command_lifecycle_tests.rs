use std::collections::HashMap;

use codex_app_server_protocol::CommandExecutionSource as ExecCommandSource;
use codex_protocol::parse_command::ParsedCommand;
use pretty_assertions::assert_eq;

use super::RunningCommand;
use super::next_running_command_tab_detail;

fn command(name: &str, start_order: u64) -> RunningCommand {
    RunningCommand {
        command: name.split_whitespace().map(ToString::to_string).collect(),
        parsed_cmd: vec![ParsedCommand::Unknown {
            cmd: name.to_string(),
        }],
        source: ExecCommandSource::Agent,
        start_order,
    }
}

#[test]
fn picks_the_latest_command_then_falls_back_to_the_survivor() {
    let mut running = HashMap::from([
        (
            "newer".to_string(),
            command("cargo test", /*start_order*/ 2),
        ),
        (
            "older".to_string(),
            command("cargo build", /*start_order*/ 1),
        ),
    ]);

    assert_eq!(
        next_running_command_tab_detail(&running),
        Some("Run cargo test".to_string())
    );
    running.remove("newer");

    assert_eq!(
        next_running_command_tab_detail(&running),
        Some("Run cargo build".to_string())
    );
}
