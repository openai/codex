//! Coverage of the Markdown grammar and visible transformations used by selection copy.

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn supported_elements_preserve_structure_at_narrow_and_wide_widths() {
    let samples = [
        "# One\n\n## Two\n\n### Three\n\n#### Four\n\n##### Five\n\n###### Six",
        "Setext heading\n===\n\nSubheading\n---",
        "**strong** _emphasis_ ~~deleted~~ and `inline_code` with ``a`b``.",
        "Escaped \\*stars\\*, &amp; entity, café 界, and 👩‍💻.",
        "- first\n- second\n  - nested\n\n3. third\n4. fourth",
        "- [ ] pending\n- [x] complete",
        "> quoted **text**\n>\n> - item\n>   - nested",
        "First paragraph.\n\nHard break:  \nnext line.\n\n---\n\nLast paragraph.",
        "Prose\n\n```\nlet x = \"*literal* <tag>\";\n```\n\nAfter",
        "Image ![**alt** text](https://example.com/image.png).",
        "Literal <b>HTML</b> and unsupported [^footnote].",
    ];
    for source in samples {
        for width in [18, 100] {
            let layout = markdown_layout(source, width);
            let copied = payload(&layout, 0..layout.text().len()).0;
            assert_eq!(
                crate::clipboard_html::render_markdown(&copied),
                crate::clipboard_html::render_markdown(source),
                "width {width}: {source}\n{copied}"
            );
        }
    }
}

#[test]
fn transformed_content_copies_visible_text_without_active_html_or_images() {
    for (source, expected) in [
        ("Soft break\nsame paragraph.", "Soft break\nsame paragraph."),
        ("[**local**](/repo/file.rs)", "**local** (repo/file.rs)"),
        (
            "[label][reference]\n\n[reference]: https://example.com",
            "[label (https://example.com)](https://example.com)",
        ),
        (
            "<https://example.com>",
            "[https://example.com (https://example.com)](https://example.com)",
        ),
        ("![image alt](https://example.com/image.png)", "image alt"),
    ] {
        let layout = markdown_layout(source, /*width*/ 40);
        let copied = payload(&layout, 0..layout.text().len()).0;
        assert_eq!(
            crate::clipboard_html::render_markdown(&copied),
            crate::clipboard_html::render_markdown(expected),
            "{source}\n{copied}"
        );
    }
}

#[test]
fn math_and_mermaid_copy_the_visible_rendering() {
    for source in [
        "Inline $\\alpha_1$ and \\(x^{2}\\).",
        "\\[\n\\frac{a}{b}\n\\]",
        "```mermaid\nflowchart LR\nA[Start] --> B[Finish]\n```",
    ] {
        let layout = markdown_layout(source, /*width*/ 80);
        let (copied, format) = payload(&layout, 0..layout.text().len());
        if format == CopyFormat::PlainText {
            assert_eq!(copied, layout.text());
        } else {
            let rendered = markdown_layout(&copied, /*width*/ 80);
            assert_eq!(rendered.text(), layout.text());
        }
    }
}
