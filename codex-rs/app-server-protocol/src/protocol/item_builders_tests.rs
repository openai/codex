use super::*;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;

#[test]
fn foreign_read_retains_target_native_path_without_dropping_other_command_actions() {
    #[cfg(windows)]
    let cwd = PathUri::parse("file:///usr/local/src").expect("valid foreign POSIX cwd");
    #[cfg(not(windows))]
    let cwd = PathUri::parse("file:///C:/src").expect("valid foreign Windows cwd");
    let command = ["bash", "-lc", "cd subdir && cat file.txt"].map(str::to_string);
    let mut parsed_cmd = parse_command(&command);
    parsed_cmd.extend([
        ParsedCommand::ListFiles {
            cmd: "ls".to_string(),
            path: Some("subdir".to_string()),
        },
        ParsedCommand::Search {
            cmd: "rg needle".to_string(),
            query: Some("needle".to_string()),
            path: Some("src".to_string()),
        },
    ]);
    let read_path = cwd
        .join("subdir/file.txt")
        .expect("relative read path should resolve against foreign cwd");
    #[cfg(windows)]
    let expected_native_path = "/usr/local/src/subdir/file.txt";
    #[cfg(not(windows))]
    let expected_native_path = r"C:\src\subdir\file.txt";
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

#[test]
fn home_relative_read_resolves_against_the_native_home_directory() {
    let cwd = AbsolutePathBuf::current_dir().expect("current directory should be absolute");
    let parsed_cmd = vec![ParsedCommand::Read {
        cmd: "cat ~/README.md".to_string(),
        name: "README.md".to_string(),
        path: PathBuf::from("~/README.md").into(),
    }];

    assert_eq!(
        command_actions_for_path_uri(&parsed_cmd, &cwd.clone().into()),
        vec![CommandAction::Read {
            command: "cat ~/README.md".to_string(),
            name: "README.md".to_string(),
            path: cwd.join("~/README.md").into(),
        }]
    );
}
