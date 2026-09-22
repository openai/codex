//! Task markers share list indentation in streaming and completed Markdown.

use super::ListSpacing;
use super::render_markdown_text_with_width;
use super::render_streaming_markdown_lines_with_width_and_cwd;
use pretty_assertions::assert_eq;
use ratatui::text::Text;

#[test]
fn lists_preserve_layout() {
    let sources = [
        "- [ ] Unchecked\n- [x] Checked\n- [X] Uppercase\n- Ordinary\n",
        "- [ ] Parent task with enough words to wrap\n  - [x] Nested task\n  - Ordinary child\n- [x] Done\n",
        "9. [ ] Numbered task with enough words to wrap\n10. [x] Done\n11. Ordinary\n",
        "> - [ ] Quoted task with enough words to wrap\n> - [x] Done\n",
        "- [ ] First paragraph\n\n  Second paragraph\n\n- [x] Next task\n",
        "- [ ]\n- [x]\n",
        "- [x] **Bold**, *italic*, and `code`\n\n`- [x] inline`\n\n```text\n- [x] fenced\n```\n\n[ ] Plain text\n\n- \\[x] Escaped\n",
        "- [ ] \n  - child\n\n- [x] # title\n\n- [x] > quote\n\n- [x] ---\n",
        "-     [x] code, not a task\n",
        "- [x] <b>done</b>\n\n- Parent\n  - [x] \n\n  Continued paragraph\n",
        "- [x] <div>done</div>\n",
        "- [x] \n  <div>done</div>\n",
        "> - Parent\n>   - [x] \n>\n>   Continued paragraph\n",
    ];
    let mut stages = Vec::new();
    for source in sources {
        let rendered = render_markdown_text_with_width(source, Some(24));
        let streamed = render_streaming_markdown_lines_with_width_and_cwd(
            source,
            Some(24),
            /*cwd*/ None,
            &|_| false,
            ListSpacing::AfterMultiline,
        );
        assert_eq!(
            Text::from(crate::terminal_hyperlinks::visible_lines(streamed.lines)),
            rendered
        );
        stages.push(rendered.to_string());
    }
    insta::assert_snapshot!(stages.join("\n---\n"));
}
