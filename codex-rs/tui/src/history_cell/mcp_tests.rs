use super::*;
use base64::Engine;
use codex_protocol::mcp::CallToolResult;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;

const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==";

fn result(content: Vec<Value>) -> CallToolResult {
    CallToolResult {
        content,
        structured_content: None,
        is_error: None,
        meta: None,
    }
}

#[test]
fn projected_content_preserves_width_dependent_rendering() {
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

    for width in [1, 8, 40, 120, RAW_TOOL_OUTPUT_WIDTH] {
        let format_text =
            |text: &str| format_and_truncate_tool_result(text, TOOL_CALL_MAX_LINES, width);
        assert_eq!(
            projected
                .content
                .iter()
                .map(|block| block.render(width))
                .collect::<Vec<_>>(),
            vec![
                format_text(text),
                "<image content>".to_string(),
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
    assert_eq!(projected.content[0].render(/*width*/ 80), "<image content>");

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
    cell.complete(
        Duration::ZERO,
        Ok(result(vec![
            json!({"type": "image", "mimeType": "image/png", "data": PNG, "text": "Script completed\nOutput:\nimage-side output"}),
            unknown.clone(),
        ])),
    );

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
    insta::assert_snapshot!(format!("history:\n{display}\n\ntranscript:\n{transcript}"), @r#"
    history:
    • Called Inspect results
      └ image-side output
        unknown-side output

    transcript:
    • Called node_repl.js({"title":"Inspect results"})
      └ Script completed
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
            Line::from("<image content>"),
            Line::from(format_and_truncate_tool_result(
                &unknown.to_string(),
                TOOL_CALL_MAX_LINES,
                RAW_TOOL_OUTPUT_WIDTH,
            )),
        ],
    );
}
