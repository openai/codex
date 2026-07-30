use super::*;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn read_command_actions_preserve_native_and_foreign_paths() {
    for (cwd_uri, relative_path, expected_path) in [
        (
            "file:///home/alice/repo",
            "src/main.rs",
            "/home/alice/repo/src/main.rs",
        ),
        (
            "file:///C:/Users/Alice%20Smith/repo",
            r"src\main.rs",
            r"C:\Users\Alice Smith\repo\src\main.rs",
        ),
        (
            "file:///C:/Users/Alice%20Smith/repo",
            r"C:src\main.rs",
            r"C:\Users\Alice Smith\repo\src\main.rs",
        ),
        (
            "file://server/share/repo",
            r"src\main.rs",
            r"\\server\share\repo\src\main.rs",
        ),
    ] {
        let cwd = PathUri::parse(cwd_uri).expect("valid cross-platform cwd");
        let command = format!("cat {relative_path}");
        let parsed_cmd = vec![
            ParsedCommand::Read {
                cmd: command.clone(),
                name: "main.rs".to_string(),
                path: PathBuf::from(relative_path),
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
            serde_json::to_value(command_actions_for_path_uri(&parsed_cmd, &cwd))
                .expect("command actions should serialize"),
            json!([
                {
                    "type": "read",
                    "command": command,
                    "name": "main.rs",
                    "path": expected_path,
                },
                {
                    "type": "listFiles",
                    "command": "ls",
                    "path": "subdir",
                },
                {
                    "type": "search",
                    "command": "rg needle",
                    "query": "needle",
                    "path": "src",
                },
            ]),
            "resolving command actions against {cwd_uri}",
        );
    }
}
