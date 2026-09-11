//! Check window boundaries, shared item rendering, and stable context normalization.

use super::*;
use pretty_assertions::assert_eq;
use serde_json::json;

fn render_test_items(items: &[Value], options: &ContextSnapshotOptions) -> String {
    render_items(
        items,
        /*start_index*/ 0,
        options,
        &mut Normalizer::default(),
    )
}

#[test]
fn lite_tool_catalog_and_code_calls_are_visible() {
    let rendered = render_test_items(
        &[
            json!({ "type": "additional_tools", "role": "developer", "tools": [
                { "type": "namespace", "name": "functions", "tools": [
                    { "type": "custom", "name": "exec", "description": "Run JavaScript" },
                    { "type": "function", "name": "update_plan" }
                ] }
            ] }),
            json!({ "type": "custom_tool_call", "name": "exec", "input": "text('ready');" }),
            json!({ "type": "custom_tool_call_output", "output": [
                { "type": "output_text", "text": "Script completed\nWall time 0.1 seconds\nOutput:\n" },
                { "type": "output_text", "text": "ready" }
            ] }),
            json!({ "type": "custom_tool_call_output", "output": {
                "content": "plan updated", "success": true
            } }),
        ],
        &ContextSnapshotOptions::default(),
    );
    assert!(rendered.contains("00:additional_tools/developer (1; hash="));
    assert!(rendered.contains("- custom/exec"));
    assert!(rendered.contains("- namespace/functions"));
    assert!(rendered.contains("- function/update_plan"));
    assert!(rendered.contains("01:custom_tool_call/exec:text('ready');"));
    assert!(rendered.contains(
        "02:custom_tool_call_output:Script completed\n    Wall time <DURATION> seconds\n    Output:"
    ));
    assert!(rendered.contains("     | ready"));
    assert!(rendered.contains("03:custom_tool_call_output:success=true:plan updated"));
}

#[test]
fn encrypted_compaction_payload_changes_are_visible_without_exposing_contents() {
    let render = |encrypted_content| {
        render_test_items(
            &[json!({ "type": "compaction", "encrypted_content": encrypted_content })],
            &ContextSnapshotOptions::default(),
        )
    };
    let before = render("checkpoint A");
    assert!(before.contains("compaction:encrypted=true; chars=12; hash="));
    assert!(!before.contains("checkpoint A"));
    assert_ne!(before, render("checkpoint B"));
    assert_eq!(render(""), "00:compaction:encrypted=false");
}

#[test]
fn grouping_only_continues_when_input_extends_and_settings_match() {
    let request = |number, input: Vec<Value>, effort| CapturedRequest {
        number,
        kind: "turn".to_string(),
        label: None,
        input,
        settings: Some(json!({ "reasoning": { "effort": effort } })),
    };
    let one = json!({ "type": "message", "role": "user", "content": [] });
    let two = json!({ "type": "message", "role": "assistant", "content": [] });
    let windows = group_requests([
        request(1, vec![one.clone()], "medium"),
        request(2, vec![one.clone(), two.clone()], "medium"),
        request(3, vec![one.clone(), two], "high"),
        request(4, vec![one.clone()], "high"),
        request(5, vec![one], "high"),
    ]);
    assert_eq!(windows.len(), 4);
    assert_eq!(windows[0].requests[1].suffix_start, 1);
    assert_eq!(windows[1].requests[0].suffix_start, 0);
    assert!(matches!(
        windows[2].boundary,
        Some(InputBoundary::Truncated(1))
    ));
    assert!(matches!(windows[3].boundary, Some(InputBoundary::Repeated)));
}

#[test]
fn cache_key_changes_split_windows_and_render_distinct_stable_labels() {
    let items = [
        json!({ "type": "message", "role": "user", "content": [] }),
        json!({ "type": "message", "role": "assistant", "content": [] }),
        json!({ "type": "message", "role": "user", "content": [] }),
        json!({ "type": "message", "role": "assistant", "content": [] }),
    ];
    let body = |count, key, metadata| {
        json!({
            "input": items[..count],
            "model": "test",
            "prompt_cache_key": key,
            "client_metadata": metadata,
        })
    };
    let first_key = "11111111-1111-1111-1111-111111111111";
    let second_key = "22222222-2222-2222-2222-222222222222";
    let bodies = [
        body(1, first_key, "first"),
        body(2, second_key, "second"),
        body(3, second_key, "third"),
        body(4, first_key, "fourth"),
    ];
    let entries = bodies.iter().map(SnapshotEntry::body).collect::<Vec<_>>();
    let rendered = format_context_snapshot(
        "cache keys",
        &entries,
        &ContextSnapshotOptions::default().include_request_settings(),
    );
    assert_eq!(rendered.matches("## Window").count(), 3);
    assert!(rendered.contains("## Window 2 (after request 1)"));
    assert!(
        rendered.contains(
            "Settings: relative to window 1\n  prompt_cache_key: \"<PROMPT_CACHE_KEY 2>\""
        )
    );
    assert!(rendered.contains("-- request 3 (request) --\n02:message/user"));
    assert!(rendered.contains("Settings: same as window 1"));
    assert!(rendered.contains("prompt_cache_key: \"<PROMPT_CACHE_KEY 1>\""));
    assert!(!rendered.contains(first_key));
    assert!(!rendered.contains(second_key));
    let without_settings =
        format_context_snapshot("cache keys", &entries, &ContextSnapshotOptions::default());
    assert!(
        without_settings
            .contains("## Window 2 (after request 1: settings changed (prompt_cache_key))")
    );
}

#[test]
fn body_entries_group_while_item_only_entries_keep_unknown_settings_explicit() {
    let first =
        json!({ "input": [{ "type": "message", "role": "user", "content": [] }], "model": "test" });
    let second = json!({ "input": [first["input"][0], { "type": "message", "role": "assistant", "content": [] }], "model": "test" });
    let options = ContextSnapshotOptions::default();
    let bodies = format_context_snapshot(
        "raw request bodies",
        &[
            SnapshotEntry::body(&first).labeled("first"),
            SnapshotEntry::body(&second).labeled("second"),
        ],
        &options,
    );
    assert_eq!(bodies.matches("## Window").count(), 1);
    assert!(bodies.contains("-- request 2 (request; second) --\n01:message/assistant"));
    assert!(!bodies.contains("Settings:"));

    let changed = json!({ "input": second["input"], "model": "other" });
    let changed_settings = format_context_snapshot(
        "settings still split windows",
        &[SnapshotEntry::body(&first), SnapshotEntry::body(&changed)],
        &options,
    );
    assert!(changed_settings.contains("## Window 2 (after request 1: settings changed (model))"));
    assert!(!changed_settings.contains("Settings:"));

    let diverged = json!({ "input": [], "model": "other" });
    let input_and_settings_changed = format_context_snapshot(
        "both input and settings changed",
        &[SnapshotEntry::body(&first), SnapshotEntry::body(&diverged)],
        &options,
    );
    assert!(input_and_settings_changed.contains(
        "## Window 2 (after request 1: input truncated at item 00, settings changed (model))"
    ));

    let opt_in = format_context_snapshot(
        "settings rendered on request",
        &[SnapshotEntry::body(&first)],
        &options.clone().include_request_settings(),
    );
    assert!(opt_in.contains("Settings:\n  model: \"test\""));

    let first_input = first["input"].as_array().expect("input array");
    let second_input = second["input"].as_array().expect("input array");
    let items = format_context_snapshot(
        "items without settings",
        &[
            SnapshotEntry::items(first_input),
            SnapshotEntry::items(second_input),
        ],
        &options,
    );
    assert_eq!(items.matches("## Window").count(), 2);
    assert!(items.contains("settings unavailable"));
}

#[test]
fn changed_tools_show_only_the_inventory_delta() {
    let old = json!({ "tools": [
            { "type": "function", "function": { "name": "read", "description": "Read a file" } },
            { "type": "function", "function": { "name": "write", "description": "Write a file" } }
        ] });
    let new = json!({ "tools": [
            { "type": "function", "function": { "name": "read", "description": "Read a changed file" } },
            { "type": "function", "function": { "name": "search", "description": "Find a file" } }
        ] });
    let rendered = render_settings(&new, Some(&old), &mut Normalizer::default());
    assert!(rendered.contains("- removed function/write"));
    assert!(rendered.contains("~ function/read:"));
    assert!(rendered.contains("+ function/search:"));
    assert!(!rendered.contains("Write a file"));
}

#[test]
fn unnamed_tools_with_shared_type_do_not_get_misreported_as_order_changes() {
    let old = json!({ "tools": [
            { "type": "mcp", "server_label": "calendar" },
            { "type": "mcp", "server_label": "mail" }
        ] });
    let new = json!({ "tools": [{ "type": "mcp", "server_label": "calendar" }] });
    let rendered = render_settings(&new, Some(&old), &mut Normalizer::default());
    assert!(rendered.contains("- removed mcp/mail"));
    assert!(!rendered.contains("order changed"));
}

#[test]
fn portable_tool_schema_keeps_non_platform_changes_visible() {
    let mut unix = json!({
        "type": "function",
        "name": "exec_command",
        "description": "Runs a command in a PTY, returning output or a session ID for ongoing interaction.",
        "parameters": { "properties": {
            "cmd": { "description": "Shell command to execute." },
            "yield_time_ms": { "description": "Wait before yielding output. Defaults to 10000 ms; effective range is 250-30000 ms." }
        } }
    });
    let mut windows = unix.clone();
    windows["parameters"]["properties"]["yield_time_ms"]["description"] = json!(
        "Maximum time to wait before returning a session ID for a still-running command. Commands that finish sooner return immediately. For ordinary commands, omit this parameter to use the 10000 ms default. Effective range on Windows is 10000-30000 ms."
    );
    assert_eq!(portable_tool_schema(&unix), portable_tool_schema(&windows));

    windows["description"] = json!(format!(
        "{}\n\nWindows safety rules:\nNew guidance.",
        unix["description"].as_str().expect("shell description")
    ));
    assert_ne!(portable_tool_schema(&unix), portable_tool_schema(&windows));
    windows["description"] = unix["description"].clone();
    unix["parameters"]["properties"]["cmd"]["description"] =
        json!("Different shell command guidance.");
    assert_ne!(portable_tool_schema(&unix), portable_tool_schema(&windows));
}

#[test]
fn portable_tool_schema_normalizes_embedded_code_mode_shell_guidance() {
    let base = "Runs a command in a PTY, returning output or a session ID for ongoing interaction.";
    let windows_guidance = r#"Windows safety rules:
- Do not compose destructive filesystem commands across shells. Do not enumerate paths in PowerShell and then pass them to `cmd /c`, batch builtins, or another shell for deletion or moving. Use one shell end-to-end, prefer native PowerShell cmdlets such as `Remove-Item` / `Move-Item` with `-LiteralPath`, and avoid string-built shell commands for file operations.
- Before any recursive delete or move on Windows, verify the resolved absolute target paths stay within the intended workspace or explicitly named target directory. Never issue a recursive delete or move against a computed path if the final target has not been checked.
- When using `Start-Process` to launch a background helper or service, pass `-WindowStyle Hidden` unless the user explicitly asked for a visible interactive window. Use visible windows only for interactive tools the user needs to see or control."#;
    let description = |shell: String, wait: &str| {
        format!("### `exec_command`\n{shell}\n\nexec tool declaration:\n```ts\n  // {wait}\n```")
    };
    let nested = |description| {
        json!({ "type": "namespace", "name": "functions", "tools": [
            { "type": "custom", "name": "exec", "description": description }
        ] })
    };
    let unix = nested(description(
        base.to_string(),
        "Wait before yielding output. Defaults to 10000 ms; effective range is 250-30000 ms.",
    ));
    let windows = nested(description(
        format!("{base}\n\n{windows_guidance}"),
        "Maximum time to wait before returning a session ID for a still-running command. Commands that finish sooner return immediately. For ordinary commands, omit this parameter to use the 10000 ms default. Effective range on Windows is 10000-30000 ms.",
    ));
    assert_eq!(portable_tool_schema(&unix), portable_tool_schema(&windows));

    let changed = nested(description(
        format!("{base}\n\n{windows_guidance}\nA new restriction."),
        "Maximum time to wait before returning a session ID for a still-running command. Commands that finish sooner return immediately. For ordinary commands, omit this parameter to use the 10000 ms default. Effective range on Windows is 10000-30000 ms.",
    ));
    assert_ne!(portable_tool_schema(&unix), portable_tool_schema(&changed));
}

#[test]
fn one_renderer_handles_text_image_and_function_calls() {
    let items = vec![
        json!({ "type": "message", "role": "developer", "content": [{ "type": "input_text", "text": "<skills_instructions>\nbody\n</skills_instructions>" }] }),
        json!({ "type": "message", "role": "user", "content": [{ "type": "input_text", "text": "Look" }, { "type": "input_image", "image_url": "data:image/png;base64,AAAA" }] }),
        json!({ "type": "function_call", "namespace": "collaboration", "name": "lookup", "arguments": "{\"key\":\"value\"}" }),
    ];
    let rendered = render_test_items(
        &items,
        &ContextSnapshotOptions::default().rewrite_known_segments(),
    );
    assert_eq!(
        rendered,
        "00:message/developer:\n    <SKILLS_INSTRUCTIONS>\n01:message/user[2]:\n    [01] Look\n    [02] <input_image:image_url>\n02:function_call/collaboration.lookup:{\"key\":\"value\"}"
    );
}

#[test]
fn uuid_labels_preserve_identity_across_message_and_function_content() {
    let first = "11111111-1111-1111-1111-111111111111";
    let second = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa";
    let items = [
        json!({ "type": "message", "role": "user", "content": [{ "text": first }] }),
        json!({ "type": "function_call", "name": "lookup", "arguments": json!({"id": second.to_uppercase()}).to_string() }),
        json!({ "type": "message", "role": "user", "content": [{ "text": format!("{first} {second}") }] }),
    ];
    let rendered = render_test_items(&items, &ContextSnapshotOptions::default());
    assert!(rendered.contains("00:message/user:\n    <UUID 1>"));
    assert!(rendered.contains("01:function_call/lookup:{\"id\":\"<UUID 2>\"}"));
    assert!(rendered.contains("02:message/user:\n    <UUID 1> <UUID 2>"));
}

#[test]
fn rewritten_model_and_personality_guidance_keep_their_intro() {
    let render = |text| {
        render_test_items(
            &[json!({ "type": "message", "role": "developer", "content": [{ "text": text }] })],
            &ContextSnapshotOptions::default().rewrite_known_segments(),
        )
    };
    assert_eq!(
        render("<model_switch>\nOriginal intro\n\nlong instructions\n</model_switch>"),
        "00:message/developer:\n    <MODEL_SWITCH>\n    Original intro"
    );
    assert_eq!(
        render("<personality_spec> Original intro \nlong instructions\n</personality_spec>"),
        "00:message/developer:\n    <PERSONALITY_SPEC>\n    Original intro"
    );
    assert_ne!(render("<model_switch>"), render("<model_switch>New intro"));
}

#[test]
fn normalization_labels_distinct_working_directories_and_retains_permissions() {
    let mut normalizer = Normalizer::default();
    let render = |normalizer: &mut Normalizer, cwd: &str| {
        normalizer.normalize_or_replace(
            &format!("<environment_context>\n<cwd>{cwd}</cwd>\n<shell>zsh</shell>\n<filesystem><root>{cwd}/src</root></filesystem>\n</environment_context>"),
            /*rewrite_known_segments*/ false,
        )
    };
    assert!(render(&mut normalizer, "/tmp/one").contains("<cwd><CWD></cwd>"));
    let second = render(&mut normalizer, "/tmp/two");
    assert!(second.contains("<cwd><CWD 2></cwd>"));
    assert!(second.contains("<root><CWD 2>/src</root>"));
    let nested = render(&mut normalizer, "/tmp/one/PRETURN_CONTEXT_DIFF_CWD");
    assert!(nested.contains("<cwd><CWD>/PRETURN_CONTEXT_DIFF_CWD</cwd>"));
    let permissions = normalizer.normalize_or_replace(
        "<permissions instructions>\nAsk approval\n</permissions instructions>",
        /*rewrite_known_segments*/ false,
    );
    assert!(permissions.contains("Ask approval"));
}

#[test]
fn skill_paths_are_stable_across_hosts_without_rewriting_uris() {
    let path =
        "<skill>\n<path>/tmp/fixture/plugins/cache/test/agenda/SKILL.md</path>\nAgenda\n</skill>";
    let windows =
        "<skill>\n<path>C:\\Temp\\plugins\\cache\\test\\agenda\\SKILL.md</path>\nAgenda\n</skill>";
    let render = |text| {
        render_test_items(
            &[
                json!({ "type": "message", "role": "user", "content": [{ "type": "input_text", "text": text }] }),
            ],
            &ContextSnapshotOptions::default(),
        )
    };
    assert_eq!(render(path), render(windows));
    assert!(
        render("<skill>\n<path>skill://agenda/SKILL.md</path>\n</skill>")
            .contains("<path>skill://agenda/SKILL.md</path>")
    );
}

#[test]
fn cwd_aliases_match_path_components_only() {
    let mut normalizer = Normalizer::default();
    let text = normalizer.normalize_or_replace(
            "<environment_context><cwd>/tmp/repo</cwd><root>/tmp/repo/src</root><root>/tmp/repo-other</root></environment_context>",
            /*rewrite_known_segments*/ false,
        );
    assert!(text.contains("<root><CWD>/src</root>"));
    assert!(text.contains("<root><WORKSPACE_ROOT 1></root>"));

    let mut windows = Normalizer::default();
    let first = windows.normalize_or_replace(
            r"<environment_context><cwd>C:\tmp\repo</cwd><root>C:\tmp\repo\src</root></environment_context>",
            /*rewrite_known_segments*/ false,
        );
    assert!(first.contains("<root><CWD>/src</root>"));
    let nested = windows.normalize_or_replace(
            r"<environment_context><cwd>C:\tmp\repo\PRETURN_CONTEXT_DIFF_CWD</cwd></environment_context>",
            /*rewrite_known_segments*/ false,
        );
    assert!(nested.contains("<cwd><CWD>/PRETURN_CONTEXT_DIFF_CWD</cwd>"));
}

#[test]
fn indented_lines_distinguish_literal_escapes_from_newlines() {
    let render = |text| {
        render_test_items(
            &[
                json!({ "type": "message", "role": "user", "content": [{ "type": "input_text", "text": text }] }),
            ],
            &ContextSnapshotOptions::default(),
        )
    };
    assert_ne!(render("line one\\nline two"), render("line one\nline two"));
    assert!(render("line one\nline two").contains("00:message/user:\n    line one\n    line two"));
}

#[test]
fn hidden_changes_remain_visible_in_fingerprints() {
    let options = ContextSnapshotOptions::default();
    let render = |word| {
        render_test_items(
            &[
                json!({ "type": "message", "role": "developer", "content": [{ "type": "input_text", "text": format!("<permissions instructions>\n{} {word}\n</permissions instructions>", "long line ".repeat(40)) }] }),
            ],
            &options,
        )
    };
    let before = render("before");
    assert!(before.contains("[hash="));
    assert_ne!(before, render("after"));

    let ordinary = |word| {
        render_test_items(
            &[
                json!({ "type": "message", "role": "user", "content": [{ "type": "input_text", "text": format!("{} {word}", "long line ".repeat(40)) }] }),
            ],
            &options,
        )
    };
    assert_ne!(ordinary("before"), ordinary("after"));

    let guardian = |policy| {
        render_test_items(
            &[
                json!({ "type": "message", "role": "developer", "content": [{ "type": "input_text", "text": format!("You are judging one planned coding-agent action.\n{policy}") }] }),
            ],
            &options.clone().rewrite_known_segments(),
        )
    };
    let before = guardian("Original policy");
    assert!(before.contains("<GUARDIAN_INSTRUCTIONS:hash="));
    assert_ne!(before, guardian("Changed policy"));
}

#[test]
fn long_content_parts_keep_both_ends_and_fingerprint_only_the_omitted_middle() {
    let render = |middle: &str, first: &str| {
        let text = (0..80)
            .map(|index| match index {
                0 => first.to_string(),
                40 => middle.to_string(),
                _ => format!("line {index:02}"),
            })
            .collect::<Vec<_>>()
            .join("\n");
        render_test_items(
            &[json!({ "type": "message", "role": "user", "content": [
                { "type": "input_text", "text": text }
            ] })],
            &ContextSnapshotOptions::default(),
        )
    };
    let before = render("hidden before", "visible before");
    let hidden_change = render("hidden after", "visible before");
    let visible_change = render("hidden before", "visible after");
    assert!(before.contains("visible before"));
    assert!(before.contains("line 79"));
    assert!(!before.contains("hidden before"));
    assert!(before.contains("<OMITTED 48 LINES; ~97 TOKENS; hash="));
    assert!(render(&"é".repeat(13), "visible before").contains("~97 TOKENS; hash="));
    assert_ne!(before, hidden_change);
    assert_eq!(
        before.lines().find(|line| line.contains("<OMITTED")),
        visible_change
            .lines()
            .find(|line| line.contains("<OMITTED"))
    );
    let near_threshold = (0..48)
        .map(|index| format!("line {index:02}"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !render_test_items(
            &[json!({ "type": "message", "role": "user", "content": [
                { "type": "input_text", "text": near_threshold }
            ] })],
            &ContextSnapshotOptions::default(),
        )
        .contains("<OMITTED")
    );
}
