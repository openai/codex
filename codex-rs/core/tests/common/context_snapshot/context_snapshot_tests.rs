//! Check window boundaries, shared item rendering, and stable context normalization.

use super::*;
use pretty_assertions::assert_eq;
use serde_json::json;

fn render_test_items(items: &[Value], options: &ContextSnapshotOptions) -> String {
    render_items(
        items,
        /*start_index*/ 0,
        &[],
        options,
        &mut Normalizer::default(),
    )
}

fn message(role: &str, text: &str) -> Value {
    json!({"type": "message", "role": role, "content": [{"type": "input_text", "text": text}]})
}

fn tagged_message(role: &str, tag: &str, body: &str) -> Value {
    message(role, &format!("<{tag}>{body}</{tag}>"))
}

fn detailed(items: &[Value]) -> String {
    render_test_items(items, &ContextSnapshotOptions::default())
}

fn rewritten(items: &[Value]) -> String {
    render_test_items(
        items,
        &ContextSnapshotOptions::default().rewrite_known_segments(),
    )
}

fn captured(body: &Value) -> String {
    format_context_snapshot(
        "test",
        &[SnapshotEntry::body(body)],
        &ContextSnapshotOptions::default(),
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
fn tool_outputs_show_only_their_own_names_and_namespaces() {
    let items = [
        json!({ "type": "function_call", "call_id": "lookup", "namespace": "collaboration", "name": "lookup", "arguments": "{}" }),
        json!({ "type": "custom_tool_call", "call_id": "custom", "namespace": "functions", "name": "exec", "input": "" }),
        json!({ "type": "function_call_output", "call_id": "lookup", "output": "ordinary function result" }),
        json!({ "type": "custom_tool_call_output", "call_id": "custom", "output": "ordinary custom result" }),
        json!({ "type": "custom_tool_call_output", "call_id": "custom", "name": "exec", "output": "Code Mode notification" }),
        json!({ "type": "function_call_output", "call_id": "lookup", "name": "lookup", "output": "explicit name only" }),
        json!({ "type": "function_call_output", "name": "notifications", "namespace": "slack", "output": "standalone" }),
        json!({ "type": "function_call_output", "call_id": "lookup", "namespace": "slack", "output": "explicit namespace only" }),
    ];
    let rendered = render_test_items(&items, &ContextSnapshotOptions::default());
    let outputs = rendered
        .lines()
        .filter(|line| line.contains("call_output"))
        .collect::<Vec<_>>();
    assert_eq!(
        outputs,
        [
            "02:function_call_output:ordinary function result",
            "03:custom_tool_call_output:ordinary custom result",
            "04:custom_tool_call_output/exec:Code Mode notification",
            "05:function_call_output/lookup:explicit name only",
            "06:function_call_output/slack.notifications:standalone",
            "07:function_call_output[namespace=slack]:explicit namespace only",
        ]
    );
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
        model_instruction_parts: Vec::new(),
        settings: Some(json!({ "reasoning": { "effort": effort } })),
    };
    let one = json!({ "type": "message", "role": "user", "content": [] });
    let two = json!({ "type": "message", "role": "assistant", "content": [] });
    let requests = [
        request(1, vec![one.clone()], "medium"),
        request(2, vec![one.clone(), two.clone()], "medium"),
        request(3, vec![one.clone(), two], "high"),
        request(4, vec![one.clone()], "high"),
        request(5, vec![one], "high"),
    ];
    let windows = group_requests(&requests);
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
    let rendered = render_settings(
        &new,
        Some(&old),
        &ContextSnapshotOptions::default(),
        &mut Normalizer::default(),
    );
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
    let rendered = render_settings(
        &new,
        Some(&old),
        &ContextSnapshotOptions::default(),
        &mut Normalizer::default(),
    );
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
fn rewritten_model_and_personality_use_plain_tags() {
    let render = |text| {
        render_test_items(
            &[json!({ "type": "message", "role": "developer", "content": [{ "text": text }] })],
            &ContextSnapshotOptions::default().rewrite_known_segments(),
        )
    };
    assert_eq!(
        render("<model_switch>\nOriginal intro\n\nlong instructions\n</model_switch>"),
        "00:message/developer:\n    <MODEL_SWITCH>"
    );
    assert_eq!(
        render("<personality_spec> Original intro \nlong instructions\n</personality_spec>"),
        "00:message/developer:\n    <PERSONALITY_SPEC>"
    );
    assert_ne!(render("<model_switch>"), render("<model_switch>New intro"));
}

#[test]
fn base_instructions_use_the_same_mode_in_settings_and_annotated_developer_content() {
    let prompt = "My model instructions\nA rule below the prefix";
    let developer = |metadata: Value| {
        json!({
            "type": "message", "role": "developer",
            "content": [{ "type": "input_text", "text": "Neighbor stays literal" },
                        { "type": "input_text", "text": prompt }],
            "internal_chat_message_metadata_passthrough": metadata
        })
    };
    let annotated = developer(json!({"content_item_kinds": [null, "model.base_instructions"]}));
    let plain = developer(Value::Null);
    let first = json!({"instructions": prompt, "input": [annotated]});
    let second = json!({"instructions": prompt, "input": [plain,
        {"type": "message", "role": "user", "content": [{"type": "input_text", "text": "Next turn"}]}]});
    let render = |options: ContextSnapshotOptions| {
        format_context_snapshot(
            "transport",
            &[SnapshotEntry::body(&first), SnapshotEntry::body(&second)],
            &options.include_request_settings(),
        )
    };
    let detailed = render(ContextSnapshotOptions::default());
    let shape = render(ContextSnapshotOptions::default().rewrite_known_segments());
    for rendered in [&detailed, &shape] {
        assert!(rendered.contains("Neighbor stays literal"));
        assert_eq!(rendered.matches("## Window").count(), 1);
    }
    assert_eq!(detailed.matches("My model instructions").count(), 2);
    assert_eq!(detailed.matches("A rule below the prefix").count(), 2);
    assert_eq!(shape.matches("<MODEL_INSTRUCTIONS>").count(), 2);
    assert!(!shape.contains("A rule below the prefix"));
    let unmarked = captured(&json!({"input": [plain]}));
    assert!(unmarked.contains("A rule below the prefix"));

    let catalog_prompt = codex_models_manager::model_info::BASE_INSTRUCTIONS;
    let lite = |prompt: &str| {
        json!({"input": [
            {"type": "additional_tools", "role": "developer", "tools": []},
            {"type": "message", "role": "developer", "content": [{"type": "input_text", "text": prompt}]},
            {"type": "message", "role": "developer", "content": [{"type": "input_text", "text": catalog_prompt}]}
        ]})
    };
    let rewrite = |body: &Value| {
        format_context_snapshot(
            "test",
            &[SnapshotEntry::body(body)],
            &ContextSnapshotOptions::default().rewrite_known_segments(),
        )
    };
    let known = rewrite(&lite(catalog_prompt));
    assert_eq!(known.matches("<MODEL_INSTRUCTIONS>").count(), 1);
    let custom = rewrite(&lite(&format!("{catalog_prompt}\nKeep this line")));
    assert!(!custom.contains("<MODEL_INSTRUCTIONS>"));
    assert!(custom.contains("Keep this line"));
    let mut annotated_other = lite(catalog_prompt);
    annotated_other["input"][1]["internal_chat_message_metadata_passthrough"] =
        json!({"content_item_kinds": ["developer_instructions"]});
    assert!(!rewrite(&annotated_other).contains("<MODEL_INSTRUCTIONS>"));
    let mut multipart = lite(catalog_prompt);
    multipart["input"][1]["content"]
        .as_array_mut()
        .unwrap()
        .push(json!({"type": "input_text", "text": "Keep neighboring guidance"}));
    let multipart = rewrite(&multipart);
    assert!(!multipart.contains("<MODEL_INSTRUCTIONS>"));
    assert!(multipart.contains("Keep neighboring guidance"));
}

#[test]
fn known_harness_text_stays_literal_unless_rewriting_is_enabled() {
    let developer = |tag, body| tagged_message("developer", tag, body);
    let items = [
        developer(
            "collaboration_mode",
            "# Collaboration Mode: Plan\nHidden rule",
        ),
        developer("multi_agent_role", "You are `/root`\nHidden rule"),
        developer("multi_agent_mode", "Delegation is enabled.\nHidden rule"),
        developer(
            "permissions instructions",
            "Sandbox policy\nApproval policy is currently never.\nHidden rule",
        ),
        tagged_message(
            "user",
            "environment_context",
            "\n<cwd>/tmp/fixture</cwd>\n<shell>zsh</shell>\n<subagents>\n- running\n</subagents>\n",
        ),
        developer("skills_instructions", "Plugin-specific skill inventory"),
        tagged_message("user", "skill", "Scenario-specific skill"),
        message(
            "developer",
            "<collaboration_mode>unclosed developer instruction",
        ),
        tagged_message("user", "collaboration_mode", "user-supplied tags"),
        json!({"type": "function_call_output", "output": "<collaboration_mode>tool-supplied tags</collaboration_mode>"}),
        message(
            "developer",
            "<collaboration_mode>One</collaboration_mode>\nIndependent instruction\n<collaboration_mode>Two</collaboration_mode>",
        ),
        tagged_message(
            "user",
            "permissions instructions",
            "\n The writable root is `/tmp/fixture/user`.\n",
        ),
        json!({"type": "function_call_output", "output": "<permissions instructions>\n The writable root is `/tmp/fixture/tool`.\n</permissions instructions>"}),
    ];
    let rendered = detailed(&items);
    for visible in [
        "# Collaboration Mode: Plan",
        "You are `/root`",
        "Delegation is enabled.",
        "Sandbox policy",
        "Approval policy is currently never.",
        "<environment_context>",
        "<cwd><CWD></cwd>",
        "<shell><HOST_SHELL></shell>",
        "- running",
        "Hidden rule",
        "Plugin-specific skill inventory",
        "Scenario-specific skill",
        "unclosed developer instruction",
        "user-supplied tags",
        "tool-supplied tags",
        "Independent instruction",
        "`/tmp/fixture/user`",
        "`/tmp/fixture/tool`",
    ] {
        assert!(rendered.contains(visible), "{visible}: {rendered}");
    }
    assert!(!rendered.contains("[hash="));
    let collaboration = |body| detailed(&[developer("collaboration_mode", body)]);
    assert!(collaboration("# Collaboration Mode: Plan\nChanged rule").contains("Changed rule"));
    let nested = collaboration(
        "# Collaboration Mode: Plan\nQuote `<collaboration_mode>...</collaboration_mode>`.",
    );
    assert!(nested.contains("Quote `<collaboration_mode>...</collaboration_mode>`."));
    let environment = |path: &str, shell: &str, agent: &str| {
        detailed(&[tagged_message(
            "user",
            "environment_context",
            &format!(
                "\n<cwd>{path}</cwd>\n<shell>{shell}</shell>\n<subagents>\n- {agent}\n</subagents>\n"
            ),
        )])
    };
    let first_environment = environment("/tmp/one", "zsh", "one");
    assert_eq!(first_environment, environment("/tmp/two", "bash", "one"));
    assert_ne!(first_environment, environment("/tmp/two", "bash", "two"));
    let inputs = [
        message("user", "<environment_context><cwd>/fake</cwd>"),
        tagged_message("user", "environment_context", "<cwd>/real</cwd>"),
        json!({"type": "function_call_output", "output": "<environment_context><cwd>/real/tool</cwd></environment_context>"}),
    ];
    let malformed = captured(&json!({"input": inputs}));
    assert!(malformed.contains("<cwd>/fake</cwd>"));
    assert!(malformed.contains("<environment_context><cwd><CWD></cwd></environment_context>"));
    assert!(malformed.contains("<cwd>/real/tool</cwd>"));

    let shape = rewritten(&items);
    assert!(shape.contains("<COLLABORATION_MODE>"));
    assert!(shape.contains("<MULTI_AGENT_ROLE>"));
    assert!(shape.contains("<MULTI_AGENT_MODE>"));
    assert!(shape.contains("<PERMISSIONS_INSTRUCTIONS>"));
    assert!(shape.contains("<SKILLS_INSTRUCTIONS>"));
    assert!(shape.contains("<ENVIRONMENT_CONTEXT>"));
    assert!(!shape.contains("Sandbox policy"));
    assert!(!shape.contains("Hidden rule"));
    for lookalike in [
        "Scenario-specific skill",
        "unclosed developer instruction",
        "user-supplied tags",
        "tool-supplied tags",
        "Independent instruction",
    ] {
        assert!(shape.contains(lookalike));
    }
}

#[test]
fn detailed_skills_use_the_same_truncation_and_dynamic_normalization_as_other_text() {
    let catalog = "## Skills\nRead the skill before using it.\n### Skill roots\n- `r0` = `/home/test/skills`\n### Available skills\n- notes:summarize: Find owners. (file: r0/summarize/SKILL.md)";
    let render = |body: &str| detailed(&[tagged_message("developer", "skills_instructions", body)]);
    let rendered = render(catalog);
    assert_eq!(
        rendered,
        "00:message/developer:\n    <skills_instructions>## Skills\n    Read the skill before using it.\n    ### Skill roots\n    - `r0` = `<SKILLS_ROOT>`\n    ### Available skills\n    - notes:summarize: Find owners. (file: r0/summarize/SKILL.md)</skills_instructions>"
    );
    assert_eq!(rendered, render(&catalog.replace("/home/test", "/other")));
    assert_eq!(
        rewritten(&[tagged_message("developer", "skills_instructions", catalog)]),
        "00:message/developer:\n    <SKILLS_INSTRUCTIONS>"
    );
    assert_eq!(
        rewritten(&[tagged_message("user", "skills_instructions", catalog)]),
        rendered.replace("message/developer", "message/user")
    );

    let large = (0..30)
        .map(|index| format!("- skill-{index}: Description"))
        .collect::<Vec<_>>()
        .join("\n");
    let large_text =
        format!("<skills_instructions>### Available skills\n{large}</skills_instructions>");
    let large = render(&format!("### Available skills\n{large}"));
    assert_eq!(
        large,
        detailed(&[message("user", &large_text)]).replace("message/user", "message/developer")
    );
    assert!(large.contains("skill-0"));
    assert!(large.contains("skill-29"));
    assert!(large.contains("<OMITTED 15 LINES;"));
    assert!(!large.contains("skill-15"));
}

#[test]
fn rewritten_segments_share_one_tag_format_and_keep_compaction_data() {
    let developer = |tag, body| tagged_message("developer", tag, body);
    let items = [
        developer("apps_instructions", "Apps guidance"),
        developer("plugins_instructions", "Plugins guidance"),
        message(
            "developer",
            "You are judging one planned coding-agent action.\nRoutine guidance.",
        ),
        message(
            "user",
            "# AGENTS.md instructions for project\n\n<INSTRUCTIONS>\nProject rules\n</INSTRUCTIONS>",
        ),
        message(
            "user",
            "You are performing a CONTEXT CHECKPOINT COMPACTION. Routine guidance.",
        ),
        message(
            "user",
            "Another language model started to solve this problem.\nGenerated summary",
        ),
    ];
    let original = rewritten(&items);
    let lines = original
        .lines()
        .filter_map(|line| line.strip_prefix("    "))
        .collect::<Vec<_>>();
    assert_eq!(
        lines,
        [
            "<APPS_INSTRUCTIONS>",
            "<PLUGINS_INSTRUCTIONS>",
            "<GUARDIAN_INSTRUCTIONS>",
            "<AGENTS_MD>",
            "<SUMMARIZATION_PROMPT>",
            "<COMPACTION_SUMMARY>",
            "Generated summary",
        ]
    );
    let full = detailed(&items);
    assert!(full.contains("Routine guidance."));
    assert!(full.contains("Project rules"));
    assert!(full.contains("Another language model started to solve this problem."));
    assert!(full.contains("Generated summary"));
}

#[test]
fn detailed_permissions_normalize_paths_and_keep_policy_changes_visible() {
    let render = |cwd: &str, external: &str, separator: &str, network: &str, denied: &str| {
        let permissions = format!(
            "<permissions instructions>\nFilesystem sandboxing defines which files can be read or written. `sandbox_mode` is `workspace-write`: {} Network access is {network}.\nApproval policy is currently never.\n The writable roots are `{cwd}`, `{external}`.\n- path `{cwd}{separator}{denied}`\n- glob `{external}{separator}*.key`\n</permissions instructions>",
            "Some additional sandbox guidance. ".repeat(5)
        );
        let environment = format!(
            "<environment_context>\n<cwd>{cwd}</cwd>\n<root>{external}</root>\n</environment_context>"
        );
        let body = json!({"input": [
            {"type": "message", "role": "developer", "content": [{"type": "input_text", "text": permissions}]},
            {"type": "message", "role": "user", "content": [{"type": "input_text", "text": environment}]}
        ]});
        captured(&body)
    };
    let unix = render("/tmp/random", "/private/random", "/", "enabled", "secret");
    assert_eq!(
        unix,
        render(r"C:\Temp\random", r"D:\Random", r"\", "enabled", "secret")
    );
    assert!(unix.contains("`workspace-write`"));
    assert!(unix.contains("Network access is enabled."));
    assert!(unix.contains("Approval policy is currently never."));
    let changed = |network, denied| render("/tmp/random", "/private/random", "/", network, denied);
    assert_ne!(unix, changed("restricted", "secret"));
    assert_ne!(unix, changed("enabled", "different"));

    let external = |root: &str, denied: &str| {
        let text = format!(
            "<permissions instructions>Policy\nApproval\n The writable root is `{root}`.\n- path `{denied}`\n</permissions instructions>"
        );
        detailed(&[message("developer", &text)])
    };
    assert_ne!(
        external("/policy/secret", "/policy/secret"),
        external("/policy/public", "/policy/secret")
    );
    assert_ne!(
        external("/policy/secret", "/policy/secret"),
        external("/policy/secret", "/policy/public")
    );
    let temp_path = |name: &str| {
        std::env::temp_dir()
            .join(name)
            .join("secret")
            .to_string_lossy()
            .into_owned()
    };
    assert_eq!(
        external("/policy/stable", &temp_path(".tmpAbc123")),
        external("/policy/stable", &temp_path(".tmpDef456"))
    );
    assert_ne!(
        external("/policy/stable", &temp_path("named-one")),
        external("/policy/stable", &temp_path("named-two"))
    );
}

#[test]
fn normalization_labels_distinct_working_directories_and_retains_permissions() {
    let mut normalizer = Normalizer::default();
    let render = |normalizer: &mut Normalizer, cwd: &str| {
        normalizer.environment(
            &format!("<environment_context>\n<cwd>{cwd}</cwd>\n<shell>zsh</shell>\n<filesystem><root>{cwd}/src</root></filesystem>\n</environment_context>"),
        )
    };
    assert!(render(&mut normalizer, "/tmp/one").contains("<cwd><CWD></cwd>"));
    let second = render(&mut normalizer, "/tmp/two");
    assert!(second.contains("<cwd><CWD 2></cwd>"));
    assert!(second.contains("<root><CWD 2>/src</root>"));
    let nested = render(&mut normalizer, "/tmp/one/PRETURN_CONTEXT_DIFF_CWD");
    assert!(nested.contains("<cwd><CWD>/PRETURN_CONTEXT_DIFF_CWD</cwd>"));
    let permissions = render_text(
        "<permissions instructions>\nAsk approval\n</permissions instructions>",
        TextSource::Message("developer"),
        &ContextSnapshotOptions::default(),
        &mut normalizer,
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
    let text = normalizer.environment(
            "<environment_context><cwd>/tmp/repo</cwd><root>/tmp/repo/src</root><root>/tmp/repo-other</root></environment_context>",
        );
    assert!(text.contains("<root><CWD>/src</root>"));
    assert!(text.contains("<root><WORKSPACE_ROOT 1></root>"));

    let mut windows = Normalizer::default();
    let first = windows.environment(
            r"<environment_context><cwd>C:\tmp\repo</cwd><root>C:\tmp\repo\src</root></environment_context>",
        );
    assert!(first.contains("<root><CWD>/src</root>"));
    let nested = windows.environment(
            r"<environment_context><cwd>C:\tmp\repo\PRETURN_CONTEXT_DIFF_CWD</cwd></environment_context>",
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
        message(
            "developer",
            &format!("You are judging one planned coding-agent action.\n{policy}"),
        )
    };
    let before = detailed(&[guardian("Original policy")]);
    assert!(before.contains("Original policy"));
    assert_ne!(before, detailed(&[guardian("Changed policy")]));
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
    assert!(before.contains("<OMITTED 64 LINES; ~129 TOKENS; hash="));
    assert!(render(&"é".repeat(13), "visible before").contains("~129 TOKENS; hash="));
    assert_ne!(before, hidden_change);
    assert_eq!(
        before.lines().find(|line| line.contains("<OMITTED")),
        visible_change
            .lines()
            .find(|line| line.contains("<OMITTED"))
    );
    let near_threshold = (0..24)
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
