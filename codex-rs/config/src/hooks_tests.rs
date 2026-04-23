use pretty_assertions::assert_eq;

use super::HookEventsToml;
use super::HookHandlerConfig;
use super::HooksFile;
use super::ManagedHooksRequirementsToml;
use super::MatcherGroup;

#[test]
fn hooks_file_deserializes_existing_json_shape() {
    let parsed: HooksFile = serde_json::from_str(
        r#"{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "^Bash$",
        "hooks": [
          {
            "type": "command",
            "command": "python3 /tmp/pre.py",
            "timeout": 10,
            "statusMessage": "checking"
          }
        ]
      }
    ]
  }
}"#,
    )
    .expect("hooks.json should deserialize");

    assert_eq!(
        parsed,
        HooksFile {
            hooks: HookEventsToml {
                pre_tool_use: vec![MatcherGroup {
                    matcher: Some("^Bash$".to_string()),
                    hooks: vec![HookHandlerConfig::Command {
                        command: "python3 /tmp/pre.py".to_string(),
                        timeout_sec: Some(10),
                        r#async: false,
                        status_message: Some("checking".to_string()),
                    }],
                }],
                ..Default::default()
            },
        }
    );
}

#[test]
fn hook_events_deserialize_from_toml_arrays_of_tables() {
    let parsed: HookEventsToml = toml::from_str(
        r#"
[[PreToolUse]]
matcher = "^Bash$"

[[PreToolUse.hooks]]
type = "command"
command = "python3 /tmp/pre.py"
timeout = 10
statusMessage = "checking"
"#,
    )
    .expect("hook events TOML should deserialize");

    assert_eq!(
        parsed,
        HookEventsToml {
            pre_tool_use: vec![MatcherGroup {
                matcher: Some("^Bash$".to_string()),
                hooks: vec![HookHandlerConfig::Command {
                    command: "python3 /tmp/pre.py".to_string(),
                    timeout_sec: Some(10),
                    r#async: false,
                    status_message: Some("checking".to_string()),
                }],
            }],
            ..Default::default()
        }
    );
}

#[test]
fn managed_hooks_requirements_flatten_hook_events() {
    let parsed: ManagedHooksRequirementsToml = toml::from_str(
        r#"
managed_dir = "/enterprise/place"

[[PreToolUse]]
matcher = "^Bash$"

[[PreToolUse.hooks]]
type = "command"
command = "python3 /enterprise/place/pre.py"
"#,
    )
    .expect("requirements hooks TOML should deserialize");

    assert_eq!(
        parsed,
        ManagedHooksRequirementsToml {
            managed_dir: Some(std::path::PathBuf::from("/enterprise/place")),
            windows_managed_dir: None,
            hooks: HookEventsToml {
                pre_tool_use: vec![MatcherGroup {
                    matcher: Some("^Bash$".to_string()),
                    hooks: vec![HookHandlerConfig::Command {
                        command: "python3 /enterprise/place/pre.py".to_string(),
                        timeout_sec: None,
                        r#async: false,
                        status_message: None,
                    }],
                }],
                ..Default::default()
            },
        }
    );
}
