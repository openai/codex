use std::collections::HashMap;
use std::sync::Arc;

use codex_exec_server::CapabilityRootDiscovery;
use codex_exec_server::CapabilityTextFile;
use codex_exec_server::DiscoveredPluginFiles;
use codex_exec_server::ExecutorCapabilityDiscoverySnapshot;
use codex_plugin::ExecutorPluginHookSource;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use serde_json::json;

use super::*;

fn snapshot_for_manifest(
    plugin_id: &str,
    environment_id: &str,
    manifest_path: &str,
    manifest: serde_json::Value,
) -> ExecutorCapabilityDiscoverySnapshot {
    let plugin_root = PathUri::parse("file:///plugins/computer-use").expect("plugin root");
    let manifest_path = PathUri::parse(manifest_path).expect("manifest path");
    let selected_root = SelectedCapabilityRoot {
        id: plugin_id.to_string(),
        location: CapabilityRootLocation::Environment {
            environment_id: environment_id.to_string(),
            path: plugin_root.clone(),
        },
    };
    let discovery = CapabilityRootDiscovery {
        id: plugin_id.to_string(),
        path: plugin_root,
        plugin: Some(DiscoveredPluginFiles {
            manifest: CapabilityTextFile {
                path: manifest_path,
                contents: manifest.to_string(),
            },
            mcp_config: None,
            apps_config: None,
        }),
        skills: Vec::new(),
        namespace_manifests: Vec::new(),
        warnings: Vec::new(),
        error: None,
    };
    ExecutorCapabilityDiscoverySnapshot::new(
        &[selected_root],
        vec![Ok(Arc::new(discovery))],
        HashMap::new(),
    )
}

fn cleanup_hook_manifest() -> serde_json::Value {
    json!({
        "name": "computer-use",
        "hooks": {
            "hooks": {
                "Stop": [{
                    "hooks": [{
                        "type": "mcp_tool",
                        "server": "node_repl",
                        "tool": "turn_ended",
                        "input": {
                            "hook_event_name": "${hook_event_name}",
                            "session_id": "${session_id}",
                            "turn_id": "${turn_id}"
                        }
                    }]
                }]
            }
        }
    })
}

fn expected_source(index: usize) -> ExecutorPluginHookSource {
    ExecutorPluginHookSource {
        plugin_id: PluginId::parse("computer-use@openai-bundled").expect("plugin id"),
        environment_id: "executor-a".to_string(),
        plugin_root: PathUri::parse("file:///plugins/computer-use").expect("plugin root"),
        manifest_path: PathUri::parse("file:///plugins/computer-use/.codex-plugin/plugin.json")
            .expect("manifest path"),
        source_relative_path: format!(".codex-plugin/plugin.json#hooks[{index}]"),
        hooks: HookEventsToml {
            stop: vec![MatcherGroup {
                matcher: None,
                hooks: vec![HookHandlerConfig::McpTool {
                    server: "node_repl".to_string(),
                    tool: "turn_ended".to_string(),
                    input: serde_json::from_value(json!({
                        "hook_event_name": "${hook_event_name}",
                        "session_id": "${session_id}",
                        "turn_id": "${turn_id}",
                    }))
                    .expect("executor hook input"),
                    timeout_sec: None,
                    status_message: None,
                }],
            }],
            ..Default::default()
        },
    }
}

#[test]
fn discovers_allowlisted_executor_plugin_hook_sources() {
    let mut manifest = cleanup_hook_manifest();
    manifest["hooks"]["hooks"]["Stop"][0]["hooks"]
        .as_array_mut()
        .expect("stop hooks")
        .push(json!({
            "type": "command",
            "command": "echo ignored"
        }));
    manifest["hooks"]["hooks"]["UserPromptSubmit"] = json!([{
        "hooks": [{
            "type": "mcp_tool",
            "server": "other",
            "tool": "ignored"
        }]
    }]);
    let second_hook_file = cleanup_hook_manifest()["hooks"].clone();
    let first_hook_file = manifest["hooks"].take();
    manifest["hooks"] = json!([first_hook_file, second_hook_file]);
    let snapshot = snapshot_for_manifest(
        "computer-use@openai-bundled",
        "executor-a",
        "file:///plugins/computer-use/.codex-plugin/plugin.json",
        manifest,
    );

    let sources = executor_plugin_hook_sources(&snapshot);

    assert_eq!(
        sources,
        vec![expected_source(/*index*/ 0), expected_source(/*index*/ 1)]
    );
}

#[test]
fn preserves_allowlisted_executor_plugin_hook_options() {
    let mut manifest = cleanup_hook_manifest();
    let handler = &mut manifest["hooks"]["hooks"]["Stop"][0]["hooks"][0];
    handler["input"] = json!({ "untrusted": "manifest-provided input" });
    handler["timeout"] = json!(30);
    handler["statusMessage"] = json!("Cleaning up Computer Use");
    let snapshot = snapshot_for_manifest(
        "computer-use@openai-bundled",
        "executor-a",
        "file:///plugins/computer-use/.codex-plugin/plugin.json",
        manifest,
    );

    let mut expected = expected_source(/*index*/ 0);
    expected.hooks.stop[0].hooks[0] = HookHandlerConfig::McpTool {
        server: "node_repl".to_string(),
        tool: "turn_ended".to_string(),
        input: serde_json::from_value(json!({ "untrusted": "manifest-provided input" }))
            .expect("manifest-provided executor hook input"),
        timeout_sec: Some(30),
        status_message: Some("Cleaning up Computer Use".to_string()),
    };

    assert_eq!(executor_plugin_hook_sources(&snapshot), vec![expected]);
}

#[test]
fn ignores_unallowlisted_executor_plugin_hooks() {
    let mut wrong_event = cleanup_hook_manifest();
    let hook_events = wrong_event["hooks"]["hooks"]
        .as_object_mut()
        .expect("hook events");
    let stop_groups = hook_events.remove("Stop").expect("stop groups");
    hook_events.insert("SessionStart".to_string(), stop_groups);
    let mut wrong_handler = cleanup_hook_manifest();
    wrong_handler["hooks"]["hooks"]["Stop"][0]["hooks"][0] = json!({
        "type": "command",
        "command": "echo cleanup"
    });
    let mut wrong_server = cleanup_hook_manifest();
    wrong_server["hooks"]["hooks"]["Stop"][0]["hooks"][0]["server"] = json!("other");
    let mut wrong_tool = cleanup_hook_manifest();
    wrong_tool["hooks"]["hooks"]["Stop"][0]["hooks"][0]["tool"] = json!("other");
    let mut wrong_matcher = cleanup_hook_manifest();
    wrong_matcher["hooks"]["hooks"]["Stop"][0]["matcher"] = json!("unexpected");
    for (name, plugin_id, manifest) in [
        (
            "wrong plugin id",
            "computer-use@other",
            cleanup_hook_manifest(),
        ),
        ("wrong event", "computer-use@openai-bundled", wrong_event),
        (
            "wrong handler",
            "computer-use@openai-bundled",
            wrong_handler,
        ),
        ("wrong server", "computer-use@openai-bundled", wrong_server),
        ("wrong tool", "computer-use@openai-bundled", wrong_tool),
        (
            "wrong matcher",
            "computer-use@openai-bundled",
            wrong_matcher,
        ),
    ] {
        let snapshot = snapshot_for_manifest(
            plugin_id,
            "executor-a",
            "file:///plugins/computer-use/.codex-plugin/plugin.json",
            manifest,
        );

        assert_eq!(
            executor_plugin_hook_sources(&snapshot),
            Vec::<ExecutorPluginHookSource>::new(),
            "{name}"
        );
    }
}

#[test]
fn ignores_file_backed_executor_plugin_hooks() {
    let file_backed = snapshot_for_manifest(
        "computer-use@openai-bundled",
        "executor-a",
        "file:///plugins/computer-use/.codex-plugin/plugin.json",
        json!({
            "name": "computer-use",
            "hooks": "./hooks/hooks.json"
        }),
    );

    assert_eq!(
        executor_plugin_hook_sources(&file_backed),
        Vec::<ExecutorPluginHookSource>::new()
    );
}
