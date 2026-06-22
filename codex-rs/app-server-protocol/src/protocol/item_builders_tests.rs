use super::*;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;

#[test]
fn foreign_read_is_omitted_without_dropping_other_command_actions() {
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

    assert_eq!(
        command_actions_for_legacy_cwd(&parsed_cmd, &cwd.into()),
        vec![
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
fn raw_file_uri_cwd_is_converted_for_command_actions() {
    #[cfg(windows)]
    let raw_uri = "file:///C:/src";
    #[cfg(not(windows))]
    let raw_uri = "file:///usr/local/src";
    let expected_path = PathUri::parse(raw_uri)
        .expect("raw file URI should parse")
        .to_abs_path()
        .expect("raw file URI should be native")
        .join("file.txt");
    let cwd = serde_json::from_value::<LegacyAppPathString>(serde_json::json!(raw_uri))
        .expect("raw file URI should deserialize as a legacy API path");
    let parsed_cmd = vec![ParsedCommand::Read {
        cmd: "cat file.txt".to_string(),
        name: "file.txt".to_string(),
        path: PathBuf::from("file.txt"),
    }];

    assert_eq!(
        command_actions_for_legacy_cwd(&parsed_cmd, &cwd),
        vec![CommandAction::Read {
            command: "cat file.txt".to_string(),
            name: "file.txt".to_string(),
            path: expected_path,
        }],
    );
}
