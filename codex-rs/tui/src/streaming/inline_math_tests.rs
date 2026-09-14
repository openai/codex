use super::StreamingRender;
use crate::history_cell::HistoryRenderMode;
use crate::markdown::render_streaming_markdown_agent_with_links_and_cwd;
use pretty_assertions::assert_eq;

#[test]
fn unicode_math_inline_stream_preserves_display_exclusion_boundaries() {
    let cwd = std::env::temp_dir();
    for delimiters in [("$$", "$$"), ("\\[", "\\]")] {
        let mut render = StreamingRender::new();
        let mut source = String::new();
        for chunk in [
            delimiters.0,
            "\n\nx^2\n\n",
            delimiters.1,
            "\n\n",
            "After $\\alpha$.\n",
        ] {
            source.push_str(chunk);
            render.append(
                &source,
                chunk,
                Some(40),
                &cwd,
                HistoryRenderMode::Rich,
                /*inline_visualization_context*/ None,
            );
            assert_eq!(
                render.lines,
                render_streaming_markdown_agent_with_links_and_cwd(&source, Some(40), Some(&cwd))
                    .lines
            );
            for width in [16, 40] {
                let mut resized = StreamingRender::new();
                resized.recompute(
                    &source,
                    Some(width),
                    &cwd,
                    HistoryRenderMode::Rich,
                    /*inline_visualization_context*/ None,
                );
                assert_eq!(
                    resized.lines,
                    render_streaming_markdown_agent_with_links_and_cwd(
                        &source,
                        Some(width),
                        Some(&cwd)
                    )
                    .lines
                );
            }
        }
    }
}
