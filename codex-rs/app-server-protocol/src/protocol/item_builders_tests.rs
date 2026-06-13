use super::*;
use codex_protocol::parse_command::ParsedCommand;
use codex_protocol::protocol::ExecCommandBeginEvent;
use codex_protocol::protocol::ExecCommandSource;
use pretty_assertions::assert_eq;
use std::path::PathBuf;

fn begin_event(
    cwd: PathUri,
    path_convention: PathConvention,
    parsed_cmd: Vec<ParsedCommand>,
) -> ExecCommandBeginEvent {
    ExecCommandBeginEvent {
        call_id: "exec-1".to_string(),
        process_id: None,
        turn_id: "turn-1".to_string(),
        started_at_ms: 0,
        command: vec!["cat".to_string(), "notes.txt".to_string()],
        cwd,
        path_convention,
        parsed_cmd,
        source: ExecCommandSource::Agent,
        interaction_input: None,
    }
}

#[test]
fn windows_command_event_renders_windows_native_cwd() {
    let event = begin_event(
        PathUri::parse("file:///C:/Research/space%20%23%25").expect("Windows cwd URI"),
        PathConvention::Windows,
        vec![ParsedCommand::Unknown {
            cmd: "cat notes.txt".to_string(),
        }],
    );

    assert_eq!(
        build_command_execution_begin_item(&event),
        ThreadItem::CommandExecution {
            id: "exec-1".to_string(),
            command: "cat notes.txt".to_string(),
            cwd: ApiPathString::new(r"C:\Research\space #%"),
            process_id: None,
            source: CommandExecutionSource::Agent,
            status: CommandExecutionStatus::InProgress,
            command_actions: vec![CommandAction::Unknown {
                command: "cat notes.txt".to_string(),
            }],
            aggregated_output: None,
            exit_code: None,
            duration_ms: None,
        }
    );
}

#[test]
fn foreign_command_event_does_not_project_read_path_onto_host() {
    let (cwd, path_convention, native_cwd) = match PathConvention::native() {
        PathConvention::Posix => (
            PathUri::parse("file:///C:/workspace").expect("Windows cwd URI"),
            PathConvention::Windows,
            ApiPathString::new(r"C:\workspace"),
        ),
        PathConvention::Windows => (
            PathUri::parse("file:///workspace").expect("POSIX cwd URI"),
            PathConvention::Posix,
            ApiPathString::new("/workspace"),
        ),
    };
    let event = begin_event(
        cwd,
        path_convention,
        vec![ParsedCommand::Read {
            cmd: "cat notes.txt".to_string(),
            name: "notes.txt".to_string(),
            path: PathBuf::from("notes.txt"),
        }],
    );

    assert_eq!(
        build_command_execution_begin_item(&event),
        ThreadItem::CommandExecution {
            id: "exec-1".to_string(),
            command: "cat notes.txt".to_string(),
            cwd: native_cwd,
            process_id: None,
            source: CommandExecutionSource::Agent,
            status: CommandExecutionStatus::InProgress,
            command_actions: vec![CommandAction::Unknown {
                command: "cat notes.txt".to_string(),
            }],
            aggregated_output: None,
            exit_code: None,
            duration_ms: None,
        }
    );
}
