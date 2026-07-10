use super::*;
use pretty_assertions::assert_eq;

#[test]
fn foreign_read_is_retained_with_other_command_actions() {
    #[cfg(windows)]
    let cwd = PathUri::parse("file:///usr/local/src").expect("valid foreign POSIX cwd");
    #[cfg(not(windows))]
    let cwd = PathUri::parse("file:///C:/src").expect("valid foreign Windows cwd");
    #[cfg(windows)]
    let home_relative_path = "~/secret";
    #[cfg(not(windows))]
    let home_relative_path = r"~\secret";
    let parsed_cmd = vec![
        ParsedCommand::Read {
            cmd: "cat file.txt".to_string(),
            name: "file.txt".to_string(),
            path: PathBuf::from("file.txt"),
        },
        ParsedCommand::Read {
            cmd: format!("cat {home_relative_path}"),
            name: "secret".to_string(),
            path: PathBuf::from(home_relative_path),
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
        .expect("relative path should resolve against foreign cwd");

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
