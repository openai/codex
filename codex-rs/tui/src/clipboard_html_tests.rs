use super::render_markdown;
use pretty_assertions::assert_eq;

#[test]
fn markdown_clipboard_preserves_rich_formatting() {
    let markdown = "# Findings\n\nRead [the source](https://example.com/source?q=1&v=2).\n\n\
        - **Bold**, *italic*, ~~removed~~, and `inline <code>`\n\n\
        > A quoted finding.\n\n\
        ```rust\nfn main() { println!(\"<hello>\"); }\n```\n\n\
        | Item | Result |\n| --- | --- |\n| Copy | Formatted |\n";
    insta::assert_snapshot!(render_markdown(markdown));
}

#[test]
fn markdown_clipboard_preserves_local_link_destinations() {
    let markdown = r#"[parser](/repo/src/parser.rs:42)

[**Windows parser**](C:\repo\src\parser.rs:42:7)

[`relative parser`](../src/parser.rs#L42-L48)

[file URL](file:///repo/src/parser.rs#L42)

[escaped path](/repo/a&amp;b/&lt;parser&gt;.rs:42)

[Unicode path](</repo/my project/解析.rs:42>)

[![diagram](https://example.com/diagram.png)](/repo/diagram.rs:7)

[web link](https://example.com/source) and [another file](src/lib.rs:9)
"#;
    insta::assert_snapshot!(render_markdown(markdown));
}

#[test]
fn markdown_clipboard_images_are_inert() {
    assert_eq!(
        render_markdown(
            "![tracking pixel](https://example.com/pixel)\n\n\
             [![**Preview** & details](https://example.com/image.png)](https://example.com/page)"
        ),
        "<p>tracking pixel</p>\n\
         <p><a href=\"https://example.com/page\"><strong>Preview</strong> &amp; details</a></p>\n"
    );
}

#[test]
fn markdown_clipboard_escapes_raw_html_and_rejects_unsafe_destinations() {
    assert_eq!(
        render_markdown("<script>alert(1)</script>\n\nHello <img src=x onerror=alert(1)>!"),
        "&lt;script&gt;alert(1)&lt;/script&gt;\n<p>Hello &lt;img src=x onerror=alert(1)&gt;!</p>\n"
    );
    for (destination, visible_destination) in [
        ("javascript:alert(1)", "javascript:alert(1)"),
        ("JaVaScRiPt:alert(1)", "JaVaScRiPt:alert(1)"),
        ("jav&#x61;script:alert(1)", "javascript:alert(1)"),
        ("data:text/html,unsafe", "data:text/html,unsafe"),
        (
            "javascript:alert(&quot;&lt;&gt;&amp;&quot;)",
            "javascript:alert(\"&lt;&gt;&amp;\")",
        ),
    ] {
        assert_eq!(
            render_markdown(&format!("[link]({destination}) ![image]({destination})")),
            format!("<p>link ({visible_destination}) image</p>\n")
        );
    }
}

#[test]
fn markdown_clipboard_unwraps_table_fences_independent_of_tui_preferences() {
    let table = "| Item | Result |\n| --- | --- |\n| Copy | Formatted |\n";
    for tables in [true, false] {
        crate::markdown_render::preferences::init(codex_config::types::TuiRendering {
            tables,
            ..Default::default()
        });
        assert_eq!(
            render_markdown(&format!("```markdown\n{table}```")),
            render_markdown(table)
        );
    }
}
