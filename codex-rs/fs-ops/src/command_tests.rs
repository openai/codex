use super::FsCommand;
use super::READ_FILE_OPERATION_ARG2;
use super::parse_command_from_args;
use pretty_assertions::assert_eq;

#[test]
fn parse_read_command() {
    let command = parse_command_from_args(
        [READ_FILE_OPERATION_ARG2, "/tmp/example.png"]
            .into_iter()
            .map(Into::into),
    )
    .expect("command should parse");

    assert_eq!(
        command,
        FsCommand::ReadFile {
            path: "/tmp/example.png".into(),
        }
    );
}

#[test]
fn parse_write_command() {
    let command =
        parse_command_from_args(["write", "/tmp/example.png"].into_iter().map(Into::into))
            .expect("command should parse");

    assert_eq!(
        command,
        FsCommand::WriteFile {
            path: "/tmp/example.png".into(),
        }
    );
}
