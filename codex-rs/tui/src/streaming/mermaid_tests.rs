use super::controller::StreamController;
use super::render::StreamingRender;
use crate::history_cell::HistoryRenderMode;
use insta::assert_snapshot;
use pretty_assertions::assert_eq;

#[test]
fn mermaid_stream_closes_resizes_and_preserves_raw_source() {
    let cwd = std::env::temp_dir();
    let mut render = StreamingRender::new();
    let mut source = String::new();
    let mut stages = Vec::new();
    for chunk in [
        "Before.\n\n```mermaid\n",
        "flowchart LR\nA[Request] --> B[Reply]\n",
        "```\n",
        "\nAfter.\n",
    ] {
        source.push_str(chunk);
        render.append(
            &source,
            chunk,
            Some(80),
            &cwd,
            HistoryRenderMode::Rich,
            /*inline_visualization_context*/ None,
        );
        assert_eq!(
            render.lines,
            super::render::render_source(
                &source,
                Some(80),
                &cwd,
                HistoryRenderMode::Rich,
                /*inline_visualization_context*/ None
            )
        );
        stages.push(
            render
                .lines
                .iter()
                .map(|line| line.line.to_string())
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    for (width, mode) in [
        (8, HistoryRenderMode::Rich),
        (80, HistoryRenderMode::Raw),
        (80, HistoryRenderMode::Rich),
    ] {
        render.recompute(
            &source,
            Some(width),
            &cwd,
            mode,
            /*inline_visualization_context*/ None,
        );
        assert_eq!(
            render.lines,
            super::render::render_source(
                &source,
                Some(width),
                &cwd,
                mode,
                /*inline_visualization_context*/ None
            )
        );
        stages.push(
            render
                .lines
                .iter()
                .map(|line| line.line.to_string())
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    assert_snapshot!(stages.join("\n\n---\n\n"));
}

#[test]
fn mermaid_holdback_tracks_nested_blocks_after_normalized_markdown() {
    let cwd = std::env::temp_dir();
    let prefix = "```md\n| Name | Value |\n| --- | --- |\n| A | B |\n```\n\n";
    for block in [
        "> ```mermaid\n> flowchart LR\n> A --> B\n> ```\n",
        "- Diagram:\n\n  ```mermaid\n  flowchart LR\n  A --> B\n  ```\n",
    ] {
        let mut render = StreamingRender::new();
        let mut source = prefix.to_owned();
        render.append(
            &source,
            prefix,
            Some(80),
            &cwd,
            HistoryRenderMode::Rich,
            /*inline_visualization_context*/ None,
        );
        for chunk in block.split_inclusive('\n') {
            source.push_str(chunk);
            render.append(
                &source,
                chunk,
                Some(80),
                &cwd,
                HistoryRenderMode::Rich,
                /*inline_visualization_context*/ None,
            );
        }
        assert_eq!(render.mermaid_start, Some(prefix.len()));
        render.recompute(
            &source,
            Some(80),
            &cwd,
            HistoryRenderMode::Rich,
            /*inline_visualization_context*/ None,
        );
        assert_eq!(render.mermaid_start, Some(prefix.len()));
        let continuation = "\nAfter.\n\n";
        source.push_str(continuation);
        render.append(
            &source,
            continuation,
            Some(80),
            &cwd,
            HistoryRenderMode::Rich,
            /*inline_visualization_context*/ None,
        );
        assert_eq!(render.mermaid_start, None);
        render.recompute(
            &source,
            Some(80),
            &cwd,
            HistoryRenderMode::Rich,
            /*inline_visualization_context*/ None,
        );
        assert_eq!(render.mermaid_start, None);
    }
}

#[test]
fn mermaid_controller_keeps_diagram_mutable_and_returns_original_source() {
    let mut controller =
        StreamController::new(Some(80), &std::env::temp_dir(), HistoryRenderMode::Rich);
    let source = "```mermaid\nflowchart LR\nA[Request] --> B[Reply]\n```\n";
    for chunk in source.split_inclusive('\n') {
        controller.push(chunk);
        assert!(
            controller.on_commit_tick().0.is_none(),
            "Mermaid escaped into scrollback"
        );
    }
    let diagram = controller.current_tail_lines();
    assert!(
        diagram
            .iter()
            .any(|line| line.line.to_string().contains('┌'))
    );
    controller.set_width(Some(8));
    assert!(
        controller
            .current_tail_lines()
            .iter()
            .any(|line| line.line.to_string().contains("A[Request]"))
    );
    controller.set_width(Some(80));
    assert_eq!(controller.current_tail_lines(), diagram);
    assert_eq!(
        crate::markdown::extract_copy_targets(source),
        vec![crate::markdown::CopyTarget::Code {
            language: Some("mermaid".to_owned()),
            content: std::sync::Arc::from("flowchart LR\nA[Request] --> B[Reply]\n"),
        }],
    );
    let mut full_source = source.to_owned();
    for i in 0..256 {
        let continuation = format!("\nStep {i}\n");
        full_source.push_str(&continuation);
        controller.push(&continuation);
        assert!(!controller.has_live_tail());
        assert!(controller.current_tail_lines().is_empty());
        assert!(controller.on_commit_tick_batch(usize::MAX).0.is_some());
    }
    controller.set_width(Some(8));
    controller.set_width(Some(80));
    full_source.push('\n');
    controller.push("\n");
    controller.on_commit_tick_batch(usize::MAX);
    for chunk in source.split_inclusive('\n') {
        full_source.push_str(chunk);
        controller.push(chunk);
        assert!(controller.on_commit_tick().0.is_none());
    }
    assert!(controller.has_live_tail());
    let second_diagram = controller.current_tail_lines();
    assert!(second_diagram.len() <= diagram.len() + 1);
    controller.set_width(Some(8));
    assert!(
        controller
            .current_tail_lines()
            .iter()
            .any(|line| line.line.to_string().contains("A[Request]"))
    );
    controller.set_width(Some(80));
    assert_eq!(controller.current_tail_lines(), second_diagram);
    controller.set_render_mode(HistoryRenderMode::Raw);
    assert_eq!(controller.queued_lines(), source.lines().count());
    controller.set_render_mode(HistoryRenderMode::Rich);
    assert_eq!(controller.current_tail_lines(), second_diagram);
    controller.set_render_mode(HistoryRenderMode::Raw);
    assert!(controller.on_commit_tick_batch(/*max_lines*/ 1).0.is_some());
    controller.set_render_mode(HistoryRenderMode::Rich);
    assert_eq!(controller.current_tail_lines(), second_diagram[1..]);
    assert_eq!(
        controller.finalize().1.as_deref(),
        Some(full_source.as_str())
    );
}
