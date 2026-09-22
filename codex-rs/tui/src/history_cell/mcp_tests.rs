use super::*;
use base64::Engine;
use codex_protocol::mcp::CallToolResult;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::sync::Arc;

const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==";

#[test]
fn mcp_inventory_connection_states() {
    use McpServerConnectionStatus as Status;

    let statuses = [
        ("unknown", None),
        ("starting", Some(Status::Starting)),
        ("failed", Some(Status::Failed)),
        ("disabled", Some(Status::Disabled)),
        ("deferred", Some(Status::NotStarted)),
        ("connected-empty", Some(Status::Connected)),
        ("cancelled", Some(Status::Cancelled)),
        ("auth", Some(Status::AuthenticationRequired)),
    ]
    .into_iter()
    .map(|(name, runtime_status)| McpServerStatus {
        server_capabilities: None,
        name: name.to_string(),
        runtime_status,
        plugin_id: None,
        http_origin: None,
        server_info: None,
        tools: HashMap::new(),
        tools_error: None,
        resources: Vec::new(),
        resource_templates: Vec::new(),
        auth_status: McpAuthStatus::Unknown,
    })
    .collect::<Vec<_>>();
    let cell =
        new_mcp_tools_output_from_statuses(&statuses, McpServerStatusDetail::ToolsAndAuthOnly);
    let rendered = cell
        .display_lines(/*width*/ 100)
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(rendered);
    for (name, expected) in [
        ("auth", status_style(StatusTone::Attention)),
        ("cancelled", Style::default().dim()),
        ("connected-empty", status_style(StatusTone::Success)),
        ("deferred", Style::default().dim()),
        ("disabled", Style::default().dim()),
        ("failed", status_style(StatusTone::Failure)),
        ("starting", accent_style()),
        ("unknown", Style::default().dim()),
    ] {
        let row = cell
            .lines
            .iter()
            .find(|line| line.spans.get(1).is_some_and(|span| span.content == name))
            .expect("connection-state row");
        assert_eq!(
            (row.spans[0].style, row.spans[3].style),
            (expected, expected)
        );
    }
}

#[test]
fn mcp_inventory_older_app_server_authentication() {
    let statuses = serde_json::from_value::<Vec<McpServerStatus>>(json!([
        {
            "name": "legacy-auth",
            "tools": {},
            "resources": [],
            "resourceTemplates": [],
            "authStatus": "notLoggedIn"
        },
        {
            "name": "legacy-healthy",
            "tools": {"lookup": {"name": "lookup", "inputSchema": {"type": "object"}}},
            "resources": [],
            "resourceTemplates": [],
            "authStatus": "oAuth"
        },
        {
            "name": "runtime-disabled",
            "runtimeStatus": "disabled",
            "tools": {},
            "resources": [],
            "resourceTemplates": [],
            "authStatus": "notLoggedIn"
        }
    ]))
    .expect("mixed-version MCP statuses");
    let cell =
        new_mcp_tools_output_from_statuses(&statuses, McpServerStatusDetail::ToolsAndAuthOnly);
    let rendered = cell
        .display_lines(/*width*/ 100)
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(rendered);
    let styles = cell
        .lines
        .iter()
        .filter(|line| {
            line.spans
                .get(1)
                .is_some_and(|span| statuses.iter().any(|status| span.content == status.name))
        })
        .map(|line| (line.spans[0].style, line.spans[3].style))
        .collect::<Vec<_>>();
    let attention = status_style(StatusTone::Attention);
    let neutral = Style::default().dim();
    assert_eq!(
        styles,
        vec![
            (attention, attention),
            (neutral, neutral),
            (neutral, neutral)
        ]
    );
}

fn result(content: Vec<Value>) -> CallToolResult {
    CallToolResult {
        content,
        structured_content: None,
        is_error: None,
        meta: None,
    }
}

#[test]
fn mcp_preview_shares_one_limit_across_blocks_and_preserves_transcript() {
    let mut cell = new_active_mcp_tool_call(
        "call-preview".into(),
        McpInvocation {
            server: "search".into(),
            tool: "lookup".into(),
            arguments: Some(json!({"title": "Search query"})),
        },
        /*animations_enabled*/ false,
    );
    cell.complete(
        Duration::ZERO,
        Ok(result(vec![
            json!({"type": "text", "text": "first\nsecond"}),
            json!({"type": "text", "text": "third\nfourth\nfifth"}),
        ])),
    );
    let display = cell.display_lines(/*width*/ 80);
    let transcript = cell.transcript_lines(/*width*/ 80);
    insta::assert_snapshot!(format!(
        "compact:\n{}\n\nhistory:\n{}\n\ntranscript:\n{}",
        visible_lines(cell.compact_hyperlink_lines(/*width*/ 80))
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n"),
        display
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n"),
        transcript
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n"),
    ));
    assert_eq!(
        cell.raw_lines()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        [
            "Called search.lookup({\"title\":\"Search query\"})",
            "first",
            "second",
            "third",
            "fourth",
            "fifth"
        ],
    );
}

#[test]
fn code_mode_output_shares_a_row_budget_across_blocks() {
    let mut cell = new_active_mcp_tool_call(
        "browser-call".to_string(),
        McpInvocation {
            server: "node_repl".to_string(),
            tool: "js".to_string(),
            arguments: Some(json!({"title": "Inspect page", "code": "await tab.snapshot()"})),
        },
        /*animations_enabled*/ false,
    );
    cell.complete(
        Duration::ZERO,
        Ok(result(vec![
            json!({"type": "text", "text": "Script completed\nOutput:\n"}),
            json!({"type": "text", "text": "Page title\nNavigation\nMain content"}),
            json!({"type": "text", "text": "Button\nLink\nFooter"}),
        ])),
    );

    let display = cell
        .display_lines(/*width*/ 40)
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(display, @r"
    • Inspect page
      └ Page title
        Navigation
        Main content
        +3 lines (ctrl+t to view transcript)
    ");
    let transcript = cell
        .transcript_lines(/*width*/ 100)
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(transcript.contains("await tab.snapshot()"));
    assert!(transcript.ends_with("    Button\n    Link\n    Footer"));
}

#[test]
fn code_mode_output_preserves_trailing_failure_diagnostics_in_transcript() {
    let mut cell = new_active_mcp_tool_call(
        "browser-error".to_string(),
        McpInvocation {
            server: "node_repl".to_string(),
            tool: "js".to_string(),
            arguments: Some(json!({"title": "Inspect page"})),
        },
        /*animations_enabled*/ false,
    );
    cell.complete(
        Duration::ZERO,
        Ok(CallToolResult {
            is_error: Some(true),
            ..result(vec![
                json!({"type": "text", "text": "Script failed"}),
                json!({"type": "text", "text": "Page title\nNavigation\nMain content\nButton\nLink\nFooter"}),
                json!({"type": "text", "text": "Script error:\npermission denied"}),
            ])
        }),
    );
    let display = cell
        .display_lines(/*width*/ 40)
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(display, @r"
    • Inspect page
      └ Script failed
        Page title
        Navigation
        +6 lines (ctrl+t to view transcript)
    ");
    let transcript = cell
        .transcript_lines(/*width*/ 80)
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(transcript.ends_with("    Script error:\n    permission denied"));
}

#[test]
fn code_mode_output_row_budget_applies_after_wrapping_and_to_errors() {
    let output = format!("{}\ntranscript tail", "Browser 页面 👩‍💻\n".repeat(40));
    for server in ["node_repl", "cua_repl"] {
        for completion in [
            Ok(result(vec![json!({"type": "text", "text": output})])),
            Ok(result(vec![json!({
                "type": "text",
                "text": format!("https://example.com/{}\ntranscript tail", "页面".repeat(100)),
            })])),
            Ok(CallToolResult {
                is_error: Some(true),
                ..result(vec![json!({"type": "text", "text": output})])
            }),
            Err(output.clone()),
        ] {
            let mut cell = new_active_mcp_tool_call(
                "browser-call".to_string(),
                McpInvocation {
                    server: server.to_string(),
                    tool: "js".to_string(),
                    arguments: Some(json!({"title": "Inspect"})),
                },
                /*animations_enabled*/ false,
            );
            cell.complete(Duration::ZERO, completion);
            for width in [20, 40, 80] {
                let compact = cell.compact_hyperlink_lines(width);
                assert!(compact.len() <= 4);
                assert!(
                    compact
                        .last()
                        .unwrap()
                        .line
                        .to_string()
                        .ends_with("transcript tail")
                );
                let display = cell.display_lines(width);
                assert_eq!(display.len(), 5); // Header, three output rows, and omission hint.
                assert!(
                    display
                        .iter()
                        .all(|line| line.width() <= usize::from(width))
                );
                assert!(display.last().unwrap().to_string().starts_with("    +"));
                let transcript = cell
                    .transcript_lines(width)
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n");
                assert!(
                    transcript
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ")
                        .contains("transcript tail")
                );
            }
        }
    }
}

#[test]
fn projected_content_preserves_full_rendering() {
    let text = "{\"result\": [1, 2, 3], \"text\": \"long output 🦀\"}";
    let malformed = json!({"type": "image", "data": PNG});
    let invalid_metadata =
        json!({"type": "text", "text": "not a valid block", "annotations": {"priority": "high"}});
    let unknown = json!({"type": "future_block", "text": "unknown output 🦀"});
    let projected = McpToolResult::new(
        result(vec![
            json!({"type": "text", "text": text}),
            json!({"type": "image", "mimeType": "image/png", "data": PNG}),
            json!({"type": "audio", "mimeType": "audio/wav", "data": "audio data"}),
            json!({"type": "resource", "resource": {"uri": "file:///text.txt", "text": "resource body"}}),
            json!({"type": "resource", "resource": {"uri": "file:///blob.bin", "blob": "binary data"}}),
            json!({"type": "resource_link", "uri": "file:///linked.txt", "name": "linked"}),
            malformed.clone(),
            invalid_metadata.clone(),
            unknown.clone(),
        ]),
        McpResultKind::Standard,
    );

    let format_text = |text: &str| format_json_compact(text).unwrap_or_else(|| text.to_owned());
    assert_eq!(
        projected
            .content
            .iter()
            .map(result::McpContentBlock::render)
            .collect::<Vec<_>>(),
        vec![
            format_text(text),
            "Returned image".to_string(),
            "<audio content>".to_string(),
            "embedded resource: file:///text.txt".to_string(),
            "embedded resource: file:///blob.bin".to_string(),
            "link: file:///linked.txt".to_string(),
            format_text(&malformed.to_string()),
            format_text(&invalid_metadata.to_string()),
            format_text(&unknown.to_string()),
        ],
    );
}

#[test]
fn projected_image_marker_still_requires_a_complete_image() {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(PNG)
        .expect("decode PNG fixture");
    let truncated = base64::engine::general_purpose::STANDARD.encode(&bytes[..33]);
    let invalid = json!({"type": "image", "mimeType": "image/png", "data": truncated});
    let valid = json!({"type": "image", "mimeType": "image/png", "data": format!("data:image/png;base64,{PNG}")});

    let projected = McpToolResult::new(result(vec![invalid.clone()]), McpResultKind::Standard);
    assert!(!projected.has_image);
    assert_eq!(projected.content[0].render(), "Returned image");

    let projected = McpToolResult::new(result(vec![invalid, valid]), McpResultKind::Standard);
    assert!(projected.has_image);
}

#[test]
fn code_mode_preserves_text_fields_on_nontext_and_unknown_blocks() {
    let mut cell = new_active_mcp_tool_call(
        "call-code-mode".to_string(),
        McpInvocation {
            server: "node_repl".to_string(),
            tool: "js".to_string(),
            arguments: Some(json!({"title": "Inspect results"})),
        },
        /*animations_enabled*/ false,
    );
    let unknown =
        json!({"type": "future_block", "text": "Script completed\nOutput:\nunknown-side output"});
    let tool_result = result(vec![
        json!({"type": "image", "mimeType": "image/png", "data": PNG, "text": "Script completed\nOutput:\nimage-side output"}),
        unknown,
    ]);
    cell.complete(Duration::ZERO, Ok(tool_result.clone()));

    let narrow = cell.display_lines(/*width*/ 16);
    assert!(narrow.iter().all(|line| line.width() <= 16));
    assert_eq!(
        narrow
            .iter()
            .skip(1)
            .take(2)
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        vec!["  └ Returned", "    image"],
    );

    let compact = cell.compact_hyperlink_lines(/*width*/ 80);
    let compact = visible_lines(compact)
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    let narrow_compact = cell.compact_hyperlink_lines(/*width*/ 16);
    assert!(narrow_compact.iter().all(|line| line.width() <= 16));

    let display = cell
        .display_lines(/*width*/ 200)
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    let transcript = cell
        .transcript_lines(/*width*/ 200)
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(format!("compact:\n{compact}\n\nhistory:\n{display}\n\ntranscript:\n{transcript}"), @r#"
    compact:
    • Called Inspect results
      └ Returned image
        image-side output
        unknown-side output

    history:
    • Inspect results
      └ Returned image
        image-side output
        unknown-side output

    transcript:
    • Called node_repl.js({"title":"Inspect results"})
      └ Returned image
        Script completed
        Output:
        image-side output
        Script completed
        Output:
        unknown-side output
    "#);
    assert_eq!(
        cell.raw_lines(),
        vec![
            Line::from("Called node_repl.js({\"title\":\"Inspect results\"})"),
            Line::from("Script completed"),
            Line::from("Output:"),
            Line::from("image-side output"),
            Line::from("Script completed"),
            Line::from("Output:"),
            Line::from("unknown-side output"),
        ],
    );

    let mut cua_cell = new_active_mcp_tool_call(
        "call-cua-repl".to_string(),
        McpInvocation {
            server: "cua_repl".to_string(),
            tool: "js".to_string(),
            arguments: None,
        },
        /*animations_enabled*/ false,
    );
    cua_cell.complete(Duration::ZERO, Ok(tool_result));
    let display = cua_cell
        .display_lines(/*width*/ 200)
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    let transcript = cua_cell
        .transcript_lines(/*width*/ 200)
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(format!("history:\n{display}\n\ntranscript:\n{transcript}"), @r"
    history:
    • Called cua_repl.js
      └ Returned image
        image-side output
        unknown-side output

    transcript:
    • Called cua_repl.js()
      └ Returned image
        Script completed
        Output:
        image-side output
        Script completed
        Output:
        unknown-side output
    ");
}

#[test]
fn titled_image_call_keeps_error_and_full_title_when_narrow() {
    let title = "Inspect a very long screenshot title 🦀";
    let mut cell = new_active_mcp_tool_call(
        "call".into(),
        McpInvocation {
            server: "node_repl".into(),
            tool: "js".into(),
            arguments: Some(json!({"title": title})),
        },
        /*animations_enabled*/ false,
    );
    assert_eq!(
        cell.display_lines(/*width*/ 80)[0].to_string(),
        format!("• {title}")
    );
    cell.complete(
        Duration::ZERO,
        Ok(CallToolResult {
            is_error: Some(true),
            ..result(vec![
                json!({"type": "text", "text": "Screenshot partially failed"}),
                json!({"type": "image", "mimeType": "image/png", "data": PNG}),
            ])
        }),
    );
    let lines = cell.display_lines(/*width*/ 32);
    assert!(lines[0].width() <= 32);
    assert_eq!(lines[0].spans[0].style, "•".red().bold().style);
    insta::assert_snapshot!(
        lines
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert!(cell.raw_lines()[0].to_string().contains(title));
    assert!(
        cell.transcript_lines(/*width*/ 200)[0]
            .to_string()
            .contains(title)
    );
}

/// Reconstruct only from explicit provenance, checking every displayed source slice on the way.
fn source_lines(lines: &[HyperlinkLine]) -> Vec<String> {
    let mut sources: Vec<Arc<str>> = Vec::new();
    for line in lines {
        let source = line.source.as_ref().expect("MCP row source");
        let visible = line.line.to_string();
        assert_eq!(
            &visible[source.prefix_bytes..source.prefix_bytes + source.range.len()],
            &source.text[source.range.clone()],
        );
        if !sources
            .last()
            .is_some_and(|previous| Arc::ptr_eq(previous, &source.text))
        {
            sources.push(Arc::clone(&source.text));
        }
    }
    sources
        .into_iter()
        .map(|source| source.to_string())
        .collect()
}

#[test]
fn code_mode_transcript_preserves_long_code_and_indentation_across_wrapping() {
    let code = "const values = await tools.fetch_values({ project: 'selected-project', include_details: true });\n  const total = values.reduce((sum, item) => sum + item.value, 0);\n\ntext({ total, label: 'exact code output' });";
    let mut cell = new_active_mcp_tool_call(
        "call-code".to_owned(),
        McpInvocation {
            server: "node_repl".to_owned(),
            tool: "js".to_owned(),
            arguments: None,
        },
        /*animations_enabled*/ false,
    );
    cell.complete(
        Duration::ZERO,
        Ok(result(vec![json!({"type": "text", "text": code})])),
    );
    let expected = code.split('\n').map(str::to_owned).collect::<Vec<_>>();
    for width in [16, 24, 40, 80] {
        let lines = cell.transcript_hyperlink_lines(width);
        let source = source_lines(&lines);
        assert!(source.ends_with(&expected), "{source:?}");
        assert!(
            lines.len() > expected.len(),
            "long code should wrap at width {width}"
        );
    }
}

#[test]
fn mcp_result_preview_preserves_source_text_and_excludes_tree_gutters() {
    let text = "one long result line with repeated spaces  and enough content to wrap across several narrow rows";
    let mut cell = new_active_mcp_tool_call(
        "call-mcp".to_owned(),
        McpInvocation {
            server: "docs".to_owned(),
            tool: "read".to_owned(),
            arguments: None,
        },
        /*animations_enabled*/ false,
    );
    cell.complete(
        Duration::ZERO,
        Ok(result(vec![json!({"type": "text", "text": text})])),
    );
    for width in [16, 24, 40, 80] {
        let lines = cell.display_hyperlink_lines(width);
        let backed = lines
            .into_iter()
            .filter(|line| line.source.is_some())
            .collect::<Vec<_>>();
        let source = source_lines(&backed);
        let output = source
            .iter()
            .find(|line| line.starts_with("one long result"))
            .expect("result source");
        assert!(
            text.starts_with(output),
            "preview must retain the original whitespace"
        );
        if width >= 40 {
            assert_eq!(output, text);
        }
        assert!(
            source
                .iter()
                .all(|line| !line.starts_with('•') && !line.starts_with("  └ "))
        );
    }
}
