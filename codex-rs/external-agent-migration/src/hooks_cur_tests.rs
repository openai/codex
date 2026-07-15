use super::*;
use pretty_assertions::assert_eq;

const TEST_REWRITE_PROFILE: RewriteProfile =
    RewriteProfile::new(".source-rules", &["source agent"]);

#[test]
fn imports_supported_cur_hooks_and_drops_failure_policy() {
    let root = tempfile::TempDir::new().expect("tempdir");
    let source_dir = root.path().join(".source");
    let source_hooks_dir = source_dir.join("hooks");
    let source_hooks = source_dir.join("hooks.json");
    let target_hooks = root.path().join(".codex/hooks.json");
    fs::create_dir_all(&source_hooks_dir).expect("source hooks directory");
    fs::write(source_hooks_dir.join("check.sh"), "echo check\n").expect("hook script");
    fs::write(
        &source_hooks,
        serde_json::json!({
            "hooks": {
                "preToolUse": [{
                    "type": "command",
                    "command": "sh .source/hooks/check.sh",
                    "matcher": "Shell",
                    "statusMessage": "Source agent check",
                    "timeoutSec": "7",
                    "failClosed": false
                }],
                "postToolUse": [{
                    "type": "prompt",
                    "command": "echo ignored"
                }],
                "subagentStart": [{
                    "command": "echo subagent",
                    "failClosed": true
                }],
                "beforeSubmitPrompt": [{
                    "command": "echo ready",
                    "matcher": "ignored"
                }],
                "preCompact": [{
                    "command": "echo compact",
                    "matcher": "auto"
                }]
            }
        })
        .to_string(),
    )
    .expect("hooks config");

    assert!(
        import_hooks_cur(
            &source_dir,
            &source_hooks,
            &target_hooks,
            TEST_REWRITE_PROFILE,
        )
        .expect("import hooks")
    );

    let target: JsonValue =
        serde_json::from_str(&fs::read_to_string(&target_hooks).expect("target hooks"))
            .expect("target hooks JSON");
    let rewritten_script = target_hooks
        .parent()
        .expect("target hooks parent")
        .join("hooks")
        .join("check.sh");
    assert_eq!(
        target,
        serde_json::json!({
            "hooks": {
                "PreToolUse": [{
                    "matcher": "Shell",
                    "hooks": [{
                        "type": "command",
                        "command": format!("sh '{}'", rewritten_script.display()),
                        "timeout": 7,
                        "statusMessage": "Codex check"
                    }]
                }],
                "UserPromptSubmit": [{
                    "hooks": [{
                        "type": "command",
                        "command": "echo ready"
                    }]
                }],
                "PreCompact": [{
                    "matcher": "auto",
                    "hooks": [{
                        "type": "command",
                        "command": "echo compact"
                    }]
                }],
                "SubagentStart": [{
                    "hooks": [{
                        "type": "command",
                        "command": "echo subagent"
                    }]
                }]
            }
        })
    );
    assert_eq!(
        fs::read_to_string(rewritten_script).expect("copied hook script"),
        "echo check\n"
    );
}
