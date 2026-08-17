use super::OpenCodeFence;
use super::has_possible_closing_line;
use crate::render::highlight::MAX_HIGHLIGHT_LINE_BYTES;
use crate::render::highlight::syntax_theme_revision;

#[test]
fn detects_only_source_preserving_open_fences() {
    for source in [
        "```rust,no_run\nfn main() {}\n",
        "~~~~python title=demo\nvalue = 1\n",
        "```\u{b}rust\u{c}\nfn main() {}\n",
        "```unknown-language\ntext\n",
    ] {
        let fence =
            OpenCodeFence::detect(source, source.len(), syntax_theme_revision()).expect(source);
        assert!(fence.highlighter.is_none());
    }
    for source in [
        " ```rust\ncode\n",
        "```\ncode\n",
        "```rust\r\ncode\r\n",
        "```rust\n'\0'\n",
        "```r&#117;st\ncode\n",
        "```rust\\,no_run\ncode\n",
        "```rust\npartial",
    ] {
        assert!(
            OpenCodeFence::detect(source, source.len(), syntax_theme_revision()).is_none(),
            "{source:?}"
        );
    }
    let source = format!("```{}\n", "x".repeat(MAX_HIGHLIGHT_LINE_BYTES + 1));
    assert!(OpenCodeFence::detect(&source, source.len(), syntax_theme_revision()).is_none());
}

#[test]
fn every_possible_closer_returns_to_the_canonical_parser() {
    let opening = "~~~~rust\n";
    assert!(!has_possible_closing_line(
        "~~~\n```\n",
        /*marker*/ b'~',
        /*marker_len*/ 4
    ));
    for line in [
        "~~~~\n",
        "   ~~~~~~  \n",
        "~~~~\t\n",
        "~~~~\u{a0}\n",
        "\t~~~~\n",
        "    ~~~~\n",
        "~~~~ trailing text\n",
    ] {
        let source = format!("{opening}{line}");
        assert!(OpenCodeFence::detect(&source, source.len(), syntax_theme_revision()).is_none());
    }
}

#[test]
fn lazy_initialization_rejects_a_changed_theme() {
    let source = "```rust\nfn first() {}\n";
    let mut fence = OpenCodeFence::detect(source, source.len(), syntax_theme_revision()).unwrap();
    fence.theme_revision = fence.theme_revision.wrapping_sub(1);
    let appended = "fn second() {}\n";
    assert!(
        fence
            .append(&format!("{source}{appended}"), appended)
            .is_none()
    );
}
