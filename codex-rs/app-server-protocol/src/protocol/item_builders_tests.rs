use super::*;
use pretty_assertions::assert_eq;

#[test]
fn foreign_read_fails_the_entire_command_action_conversion() {
    #[cfg(windows)]
    let cwd = PathUri::parse("file:///usr/local/src").expect("valid foreign POSIX cwd");
    #[cfg(not(windows))]
    let cwd = PathUri::parse("file:///C:/src").expect("valid foreign Windows cwd");
    let parsed_cmd = vec![
        ParsedCommand::Read {
            cmd: "cat file.txt".to_string(),
            name: "file.txt".to_string(),
            path: PathBuf::from("file.txt"),
        },
        ParsedCommand::ListFiles {
            cmd: "ls".to_string(),
            path: Some("subdir".to_string()),
        },
        ParsedCommand::Search {
            cmd: "rg needle".to_string(),
            query: Some("needle".to_string()),
            path: Some("src".to_string()),
        },
    ];

    let error = command_actions_for_path_uri(&parsed_cmd, &cwd)
        .expect_err("foreign read should fail the entire conversion");
    assert_eq!(
        (error.kind(), error.to_string()),
        (
            io::ErrorKind::InvalidInput,
            format!("cannot resolve command action path against foreign cwd `{cwd}`"),
        )
    );
}
