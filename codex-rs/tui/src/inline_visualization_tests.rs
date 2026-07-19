use super::*;
use crate::history_cell::AgentMarkdownCell;
use crate::history_cell::HistoryCell;
use crate::history_cell::HistoryRenderMode;
use crate::keymap::RuntimeKeymap;
use crate::pager_overlay::TranscriptOverlay;
use crate::streaming::controller::StreamController;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::style::Stylize;
use std::sync::Arc;
use tempfile::TempDir;

fn context_with_fragment(fragment: &str) -> (TempDir, InlineVisualizationContext) {
    let codex_home = tempfile::tempdir().expect("temp codex home");
    let thread_id = ThreadId::new();
    let context = InlineVisualizationContext::new(codex_home.path(), thread_id)
        .expect("UUIDv7 thread id should provide a timestamp");
    fs::create_dir_all(&context.thread_dir).expect("create visualization directory");
    fs::write(context.thread_dir.join("chart.html"), fragment).expect("write fragment");
    (codex_home, context)
}

#[test]
fn granted_visualization_root_overrides_thread_id_derived_root() {
    let codex_home = tempfile::tempdir().expect("temp codex home");
    let granted_context = InlineVisualizationContext::new(codex_home.path(), ThreadId::new())
        .expect("granted context");
    fs::create_dir_all(&granted_context.thread_dir).expect("create granted directory");
    fs::write(
        granted_context.thread_dir.join("chart.html"),
        "<div>chart</div>",
    )
    .expect("write fragment");

    let context = InlineVisualizationContext::new_with_writable_roots(
        codex_home.path(),
        ThreadId::new(),
        [granted_context.thread_dir.as_path()],
    )
    .expect("context");

    assert!(context.link_for("chart.html").is_some());
}

fn line_text(line: &ratatui::text::Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}

fn buffer_to_text(buffer: &Buffer, width: u16) -> String {
    buffer
        .content
        .chunks(usize::from(width))
        .map(|row| {
            row.iter()
                .map(|cell| {
                    let symbol = cell.symbol();
                    symbol
                        .strip_prefix("\x1b]8;;")
                        .and_then(|symbol| symbol.split_once('\x07'))
                        .and_then(|(_, symbol)| symbol.strip_suffix("\x1b]8;;\x07"))
                        .unwrap_or(symbol)
                })
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn rewrites_complete_directive_to_trusted_static_file_placeholder() {
    let (_codex_home, context) = context_with_fragment("<div>chart</div>");

    let rewritten = rewrite_inline_visualizations(
        "Before\n::codex-inline-vis{file=\"chart.html\"}\nAfter",
        Some(&context),
    );

    assert!(
        rewritten
            .markdown
            .starts_with("Before\nOpen chart visualization in the browser  \n[")
    );
    assert!(rewritten.markdown.ends_with(")\nAfter"));
    assert_eq!(rewritten.trusted_file_links.len(), 1);
    let destination = rewritten
        .trusted_file_links
        .values()
        .next()
        .expect("trusted destination");
    assert_eq!(destination.destination.scheme(), "file");
    assert!(
        destination
            .destination
            .to_file_path()
            .expect("file URL")
            .is_file()
    );
    assert_eq!(
        destination.display_label,
        "Open chart visualization in the browser"
    );
    assert!(rewritten.markdown.contains(&format!(
        "  \n[{}](",
        destination.markdown_destination_label
    )));
}

#[test]
fn hides_incomplete_streaming_directive() {
    let rewritten = rewrite_inline_visualizations(
        "Before\n::codex-inline-vis{file=\"chart",
        /*context*/ None,
    );

    assert_eq!(rewritten.markdown, "Before\n");
    assert!(rewritten.trusted_file_links.is_empty());
}

#[test]
fn unavailable_artifact_has_explicit_fallback() {
    let codex_home = tempfile::tempdir().expect("temp codex home");
    let context = InlineVisualizationContext::new(codex_home.path(), ThreadId::new())
        .expect("UUIDv7 thread id should provide a timestamp");

    assert_eq!(
        rewrite_inline_visualizations("::codex-inline-vis{file=\"missing.html\"}", Some(&context),)
            .markdown,
        "_Visualization unavailable on this device._"
    );
}

#[test]
fn rejects_parent_path_and_non_html_file() {
    let (_codex_home, context) = context_with_fragment("<div>chart</div>");

    for file in ["../chart.html", "chart.svg"] {
        assert_eq!(
            rewrite_inline_visualizations(
                &format!("::codex-inline-vis{{file=\"{file}\"}}"),
                Some(&context),
            )
            .markdown,
            "_Visualization unavailable on this device._"
        );
    }
}

#[test]
fn rejects_oversized_fragment() {
    let (_codex_home, context) = context_with_fragment("<div>chart</div>");
    let fragment = fs::OpenOptions::new()
        .write(true)
        .open(context.thread_dir.join("chart.html"))
        .expect("open fragment");
    fragment
        .set_len(MAX_FRAGMENT_BYTES + 1)
        .expect("enlarge fragment");

    assert_eq!(
        rewrite_inline_visualizations("::codex-inline-vis{file=\"chart.html\"}", Some(&context),)
            .markdown,
        "_Visualization unavailable on this device._"
    );
}

#[test]
fn viewer_materializes_sandboxed_static_document() {
    let (_codex_home, context) = context_with_fragment(
        "<div id=\"widget\"><div class=\"viz-controls\">controls</div><canvas id=\"chart\"></canvas></div><script>globalThis.chartRendered = true;</script>",
    );
    let url = context.link_for("chart.html").expect("visualization link");
    assert_eq!(url.scheme(), "file");
    let viewer_path = url.to_file_path().expect("viewer file path");
    assert_eq!(
        viewer_path.parent().and_then(Path::file_name),
        Some(std::ffi::OsStr::new(".codex-viewers"))
    );
    let document = fs::read_to_string(viewer_path).expect("read static viewer");

    assert!(document.contains("sandbox=\"allow-scripts\""));
    assert!(!document.contains("allow-same-origin"));
    assert!(document.contains("script-src 'unsafe-inline' 'unsafe-eval'"));
    assert!(document.contains(".viz-controls"));
    assert!(document.contains("https://unpkg.com/@floating-ui/dom@1.7.4"));
    assert!(document.contains("https://unpkg.com/lucide@1.17.0"));
    assert!(document.contains("&lt;canvas id=&quot;chart&quot;&gt;&lt;/canvas&gt;"));
    assert!(document.contains("globalThis.chartRendered = true"));
    assert!(document.contains("Content-Security-Policy"));

    let shell = document
        .split_once(" srcdoc=")
        .map(|(shell, _)| shell)
        .expect("viewer shell");
    let contract = format!(
        "{shell} srcdoc=\"[canonical visualization frame]\"></iframe></body></html>\n\nembedded frame:\n- canonical control styles\n- Floating UI tooltip runtime\n- Lucide icon runtime\n- visualization fragment"
    );
    insta::assert_snapshot!("viewer_document_contract", contract);
}

#[test]
fn viewer_reuses_path_and_refreshes_static_document() {
    let (_codex_home, context) = context_with_fragment("<div>first</div>");
    let first_url = context.link_for("chart.html").expect("first viewer link");
    let viewer_path = first_url.to_file_path().expect("viewer file path");
    assert!(
        fs::read_to_string(&viewer_path)
            .expect("read first viewer")
            .contains("first")
    );

    fs::write(context.thread_dir.join("chart.html"), "<div>second</div>").expect("update fragment");
    let second_url = context.link_for("chart.html").expect("second viewer link");

    assert_eq!(second_url, first_url);
    let refreshed = fs::read_to_string(viewer_path).expect("read refreshed viewer");
    assert!(refreshed.contains("second"));
    assert!(!refreshed.contains("first"));
}

#[test]
fn finalized_agent_cell_replays_visualization_link() {
    let (_codex_home, context) = context_with_fragment("<div>chart</div>");
    let cell = AgentMarkdownCell::new_with_inline_visualizations(
        "Before\n\n::codex-inline-vis{file=\"chart.html\"}\n\nAfter".to_string(),
        Path::new("/workspace"),
        Some(context),
    );

    let lines = cell.display_hyperlink_lines(/*width*/ 80);
    let text = lines
        .iter()
        .map(|line| line_text(&line.line))
        .collect::<Vec<_>>()
        .join("\n");
    let snapshot_text = text
        .lines()
        .map(|line| {
            line.find("file://").map_or_else(
                || line.to_string(),
                |start| format!("{}file://<viewer-path>", &line[..start]),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    insta::assert_snapshot!("finalized_agent_cell_visualization_link", snapshot_text);
    let title_span = lines
        .iter()
        .flat_map(|line| &line.line.spans)
        .find(|span| span.content == "Open chart visualization in the browser")
        .expect("visualization title span");
    assert_eq!(title_span.style, Style::new());
    let url_span = lines
        .iter()
        .flat_map(|line| &line.line.spans)
        .find(|span| span.content.starts_with("file://"))
        .expect("visualization URL span");
    assert_eq!(url_span.style, Style::new().cyan().underlined());
    let destinations = lines
        .iter()
        .flat_map(|line| &line.hyperlinks)
        .map(|link| link.destination.as_str())
        .collect::<Vec<_>>();
    assert_eq!(destinations.len(), 1);
    assert!(
        destinations
            .iter()
            .all(|destination| destination.starts_with("file://"))
    );
}

#[test]
fn transcript_overlay_remeasures_visualization_when_artifact_becomes_available() {
    let codex_home = tempfile::tempdir().expect("temp codex home");
    let context = InlineVisualizationContext::new(codex_home.path(), ThreadId::new())
        .expect("UUIDv7 thread id should provide a timestamp");
    fs::create_dir_all(&context.thread_dir).expect("create visualization directory");

    let cell = AgentMarkdownCell::new_with_inline_visualizations(
        "::codex-inline-vis{file=\"chart.html\"}".to_string(),
        Path::new("/workspace"),
        Some(context.clone()),
    );
    let mut overlay = TranscriptOverlay::new(vec![Arc::new(cell)], RuntimeKeymap::defaults().pager);
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 240, /*height*/ 12,
    );
    let mut buffer = Buffer::empty(area);

    overlay.render(area, &mut buffer);
    let unavailable = buffer_to_text(&buffer, area.width);
    assert!(unavailable.contains("Visualization unavailable on this device"));

    fs::write(context.thread_dir.join("chart.html"), "<div>chart</div>")
        .expect("write visualization fragment");
    overlay.insert_cell(Arc::new(AgentMarkdownCell::new(
        "next message".to_string(),
        Path::new("/workspace"),
    )));
    buffer = Buffer::empty(area);
    overlay.render(area, &mut buffer);

    let available = buffer_to_text(&buffer, area.width);
    assert!(available.contains("Open chart visualization in the browser"));
    assert!(
        available.contains("file://"),
        "viewer URL was clipped: {available:?}"
    );

    let available = available
        .lines()
        .map(|line| {
            line.find("file://").map_or_else(
                || line.to_string(),
                |start| format!("{}file://<viewer-path>", &line[..start]),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(
        "transcript_overlay_visualization_becomes_available",
        format!("before:\n{unavailable}\n\nafter:\n{available}")
    );
}

#[test]
fn agent_code_blocks_preserve_visualization_directive_literals() {
    let (_codex_home, context) = context_with_fragment("<div>chart</div>");
    let cell = AgentMarkdownCell::new_with_inline_visualizations(
        "Fenced:\n\n```text\n::codex-inline-vis{file=\"chart.html\"}\n```\n\nIndented:\n\n    ::codex-inline-vis{file=\"chart.html\"}"
            .to_string(),
        Path::new("/workspace"),
        Some(context),
    );

    let text = cell
        .display_hyperlink_lines(/*width*/ 80)
        .iter()
        .map(|line| line_text(&line.line))
        .collect::<Vec<_>>()
        .join("\n");

    insta::assert_snapshot!(text);
}

#[test]
fn streaming_hides_partial_directive_and_renders_completed_link() {
    let (_codex_home, context) = context_with_fragment("<div>chart</div>");
    let mut controller = StreamController::new_with_inline_visualizations(
        /*width*/ Some(80),
        Path::new("/workspace"),
        HistoryRenderMode::Rich,
        Some(context),
    );

    controller.push("Before\n::codex-inline-vis{file=\"chart");
    assert!(
        controller
            .current_tail_lines()
            .iter()
            .all(|line| !line_text(&line.line).contains(DIRECTIVE_PREFIX))
    );

    controller.push(".html\"}");
    let (cell, source) = controller.finalize();
    assert_eq!(
        source.as_deref(),
        Some("Before\n::codex-inline-vis{file=\"chart.html\"}\n")
    );
    let cell = cell.expect("final streamed cell");
    let lines = cell.display_hyperlink_lines(/*width*/ 80);
    assert!(
        lines.iter().any(|line| {
            line_text(&line.line).contains("Open chart visualization in the browser")
        })
    );
}

#[test]
fn visualization_link_uses_the_artifact_name() {
    let (_codex_home, context) = context_with_fragment("<div>chart</div>");
    fs::rename(
        context.thread_dir.join("chart.html"),
        context.thread_dir.join("compound-interest-explorer.html"),
    )
    .expect("rename fragment");

    let rewritten = rewrite_inline_visualizations(
        "::codex-inline-vis{file=\"compound-interest-explorer.html\"}",
        Some(&context),
    );

    assert!(
        rewritten
            .markdown
            .starts_with("Open compound\\-interest\\-explorer visualization in the browser  \n[")
    );
    assert_eq!(
        rewritten
            .trusted_file_links
            .values()
            .next()
            .expect("trusted destination")
            .display_label,
        "Open compound-interest-explorer visualization in the browser"
    );
}

#[test]
fn user_markdown_keeps_directive_literal() {
    let rendered =
        crate::markdown_render::render_markdown_text("::codex-inline-vis{file=\"chart.html\"}");
    let text = rendered
        .lines
        .iter()
        .flat_map(|line| line.spans.iter())
        .map(|span| span.content.as_ref())
        .collect::<String>();

    assert_eq!(text, "::codex-inline-vis{file=\"chart.html\"}");
}
