use super::*;
use pretty_assertions::assert_eq;

#[test]
fn foreign_read_retains_target_native_path_without_dropping_other_command_actions() {
    #[cfg(windows)]
    let cwd = PathUri::parse("file:///usr/local/src").expect("valid foreign POSIX cwd");
    #[cfg(not(windows))]
    let cwd = PathUri::parse("file:///C:/src").expect("valid foreign Windows cwd");
    let parsed_cmd = vec![
        ParsedCommand::Read {
            cmd: "cat file.txt".to_string(),
            name: "file.txt".to_string(),
            path: PathBuf::from("file.txt").into(),
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
    let read_path = cwd
        .join("file.txt")
        .expect("relative read path should resolve against foreign cwd");
    #[cfg(windows)]
    let expected_native_path = "/usr/local/src/file.txt";
    #[cfg(not(windows))]
    let expected_native_path = r"C:\src\file.txt";
    assert_eq!(
        read_path.inferred_native_path_string(),
        expected_native_path
    );

    assert_eq!(
        command_actions_for_path_uri(&parsed_cmd, &cwd),
        vec![
            CommandAction::Read {
                command: "cat file.txt".to_string(),
                name: "file.txt".to_string(),
                path: read_path.into(),
            },
            CommandAction::ListFiles {
                command: "ls".to_string(),
                path: Some("subdir".to_string()),
            },
            CommandAction::Search {
                command: "rg needle".to_string(),
                query: Some("needle".to_string()),
                path: Some("src".to_string()),
            },
        ]
    );
}
