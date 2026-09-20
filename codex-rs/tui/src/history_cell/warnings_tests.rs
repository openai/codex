//! Warning identities survive replay and preserve complete transcript details.

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn warning_count_deduplicates_messages_mcp_summaries_and_composites() {
    let cells: Vec<Arc<dyn HistoryCell>> = vec![
        Arc::new(new_warning_event("Repeated warning".into())),
        Arc::new(StartupWarningsCell::new(vec!["Repeated warning".into()])),
        Arc::new(StartupWarningsCell::mcp(
            vec!["MCP alpha failed".into()],
            ["alpha".into()],
            /*failure_reason*/ None,
        )),
        Arc::new(StartupWarningsCell::mcp(
            vec!["MCP startup incomplete".into()],
            ["alpha".into()],
            /*failure_reason*/ None,
        )),
        Arc::new(CompositeHistoryCell::new(vec![Box::new(
            new_warning_event("Another warning".into()),
        )])),
    ];
    assert_eq!(warning_count(&cells), 3);
    assert_eq!(
        warning_entries(&cells),
        vec![
            WarningEntry {
                id: WarningId::Message("Repeated warning".into()),
                source: "Warning".into(),
                details: "Repeated warning".into()
            },
            WarningEntry {
                id: WarningId::McpServer("alpha".into()),
                source: "MCP · alpha".into(),
                details: "MCP alpha failed\n\nMCP startup incomplete".into()
            },
            WarningEntry {
                id: WarningId::Message("Another warning".into()),
                source: "Warning".into(),
                details: "Another warning".into()
            },
        ]
    );
    let replay = [cells.clone(), cells.clone()].concat();
    assert_eq!(warning_count(&replay), 3);
    assert_eq!(warning_entries(&replay), warning_entries(&cells));
    assert_eq!(warning_count(&[]), 0);
    assert!(
        cells
            .iter()
            .all(|cell| cell.compact_hyperlink_lines(/*width*/ 40).is_empty())
    );
    assert!(cells.iter().all(|cell| {
        cell.display_lines_for_mode(/*width*/ 40, HistoryRenderMode::Raw)
            .is_empty()
    }));
    assert!(cells.iter().all(|cell| !cell.raw_lines().is_empty()));
}
