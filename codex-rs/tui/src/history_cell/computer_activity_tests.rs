use super::*;
use codex_protocol::mcp::CallToolResult;
use pretty_assertions::assert_eq;
use serde_json::json;

fn call(id: &str, title: &str) -> McpToolCallCell {
    McpToolCallCell::new(
        id.to_string(),
        McpInvocation {
            server: "cua_repl".to_string(),
            tool: "js".to_string(),
            arguments: Some(json!({"title": title, "code": "await cua.getState()"})),
        },
        /*animations_enabled*/ false,
    )
}

fn result(text: &str) -> Result<CallToolResult, String> {
    Ok(CallToolResult {
        content: vec![json!({"type": "text", "text": text})],
        structured_content: None,
        is_error: Some(false),
        meta: None,
    })
}

fn render(cell: &ComputerActivityCell, width: u16) -> String {
    cell.display_lines(width)
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn computer_activity_active_and_interrupted() {
    let mut cell = ComputerActivityCell::default();
    cell.complete(call("1", "Opened Chrome"), Duration::ZERO, result("ready"));
    cell.start(call("2", "Load the terminal"));
    cell.start(call("2", "Duplicate start"));
    insta::assert_snapshot!("computer_activity_active", render(&cell, /*width*/ 80));
    cell.mark_failed();
    insta::assert_snapshot!("computer_activity_interrupted", render(&cell, /*width*/ 80));
    assert_eq!(
        cell.group
            .calls
            .iter()
            .map(McpToolCallCell::success)
            .collect::<Vec<_>>(),
        vec![Some(true), Some(false)]
    );
}

#[test]
fn computer_activity_prioritizes_errors_and_images_without_reordering() {
    let mut cell = ComputerActivityCell::default();
    cell.complete(
        call("1", "Open browser"),
        Duration::ZERO,
        Err("No browser is available\n\n## Computer Use\nFull tool documentation".to_string()),
    );
    cell.complete(
        call("2", "Connect to Chrome"),
        Duration::ZERO,
        result("connected"),
    );
    cell.complete(
        call("3", "Reproduce the clear-command bug"),
        Duration::ZERO,
        result("reproduced"),
    );
    let mut image = result("screenshot metadata").unwrap();
    image.content.push(json!({"type": "image", "mimeType": "image/png", "data": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg=="}));
    cell.complete(
        call("4", "Capture the reproduced bug"),
        Duration::ZERO,
        Ok(image),
    );
    cell.complete(
        call("5", "Check the final page"),
        Duration::ZERO,
        result("checked"),
    );
    insta::assert_snapshot!("computer_activity_mixed", render(&cell, /*width*/ 90));
    insta::assert_snapshot!("computer_activity_narrow", render(&cell, /*width*/ 24));
    let transcript = cell
        .transcript_lines(/*width*/ 100)
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!("computer_activity_expanded", transcript);
    assert!(transcript.contains("Full tool documentation"));
    assert_eq!(
        cell.raw_lines(),
        plain_lines(cell.transcript_lines(u16::MAX))
    );
    assert!(
        cell.display_lines(/*width*/ 24)
            .iter()
            .all(|line| line.width() <= 24)
    );
}

#[test]
fn computer_activity_completed_and_transport_errors_keep_full_details() {
    let mut cell = ComputerActivityCell::default();
    cell.complete(
        call("1", "Inspect page"),
        Duration::ZERO,
        result("A successful result with details"),
    );
    cell.complete(call("2", "Reload page"), Duration::ZERO, result("Loaded"));
    insta::assert_snapshot!("computer_activity_success", render(&cell, /*width*/ 80));
    let mut failure =
        result("Browser state changed\nRe-query the browser before continuing").unwrap();
    failure.is_error = Some(true);
    cell.complete(
        call("3", "Inspect refreshed page"),
        Duration::ZERO,
        Ok(failure),
    );
    insta::assert_snapshot!(
        "computer_activity_failed_result",
        render(&cell, /*width*/ 90)
    );
    cell.mark_failed();
    assert_eq!(
        cell.group
            .calls
            .iter()
            .map(McpToolCallCell::success)
            .collect::<Vec<_>>(),
        vec![Some(true), Some(true), Some(false)]
    );
}

#[test]
fn computer_activity_preview_handles_unicode_and_long_diagnostics() {
    assert_eq!(preview(" 你好\nworld ", /*width*/ 6), "你好 …");
    let mut cell = ComputerActivityCell::default();
    cell.complete(
        call("1", "检查浏览器"),
        Duration::ZERO,
        Err("diagnostic ".repeat(200)),
    );
    insta::assert_snapshot!("computer_activity_unicode", render(&cell, /*width*/ 24));
    assert!(
        cell.display_lines(/*width*/ 24)
            .iter()
            .all(|line| line.width() <= 24)
    );
    assert!(cell.transcript_lines(/*width*/ 100).len() > 20);
}
