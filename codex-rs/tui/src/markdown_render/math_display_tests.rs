use super::MathMarkdown;
use super::render::render;
use crate::markdown_render::render_markdown_text_with_width;
use pretty_assertions::assert_eq;
use pulldown_cmark::Options;

fn plain(source: &str, width: usize) -> String {
    render_markdown_text_with_width(source, Some(width))
        .lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn unicode_math_snapshot() {
    let source = r"The spectrum is $\lambda_1 \leq \lambda_2$ on $\mathbb{R}^n$.

$$
x=\frac{-b\pm\sqrt{b^2-4ac}}{2a}
$$

$$\frac{1}{2}\frac{3}{4}$$

\[
\int_0^1 x^2\,dx=\frac{1}{3}
\]

$$
x
$$ trailing
$$

Inline \(\alpha^2 + \beta_{10}\), root $\sqrt{x^2+y^2}$,
and fraction $\frac{a}{b}$.

Unsupported: $\begin{matrix}a&b\end{matrix}$.

Code: `$\alpha$`; money: $5.00 and $10.00; shell: $HOME.

```latex
\frac{a}{b}
```

Shell examples: $HOME and echo $$.

After rejected equations: $\alpha$.

$$\beta$$";
    insta::assert_snapshot!(plain(source, /*width*/ 80));
}

#[test]
fn unicode_math_narrow_layout_stays_meaningful() {
    insta::assert_snapshot!(plain(
        "$$\\frac{a+b}{c+d}$$\n\nWords $\\alpha^2$ more words.",
        /*width*/ 12
    ));
    for width in [80, 14] {
        for prefix in [
            "- a\n  - b\n    - c\n\n      ",
            "- a\n  - b\n    - c\n",
            "> > > Quote\n",
        ] {
            let formula = r"$$\frac{1234567890}{x}$$";
            assert_eq!(
                plain(&format!("{prefix}{formula}"), width),
                plain(&format!("{prefix}`{formula}`"), width)
            );
        }
    }
    // A fraction too wide to retain its geometry stays source, using ordinary text wrapping.
    assert_eq!(
        plain("$$\\frac{abcdefghij}{k}$$", /*width*/ 12),
        plain("`$$\\frac{abcdefghij}{k}$$`", /*width*/ 12)
    );
}

#[test]
fn unicode_math_pending_display_tracks_original_offset() {
    assert_eq!(
        MathMarkdown::new("Prose\n\n$$\nx^2\n\n", Options::empty(), Some(80)).pending_start,
        Some(7)
    );
    assert_eq!(
        MathMarkdown::new("```\n$$\n", Options::empty(), Some(80)).pending_start,
        None
    );
    assert_eq!(
        MathMarkdown::new("echo $$\n", Options::empty(), Some(80)).pending_start,
        None
    );
}

#[test]
fn unicode_math_nested_fractions_and_oversized_pending_stay_source() {
    for source in [r"\frac{\frac{a}{b}}{c}", r"\frac{a}{\frac{b}{c}}"] {
        assert_eq!(render(source, /*display*/ true), None);
    }
    let source = format!("$$\n{}", "x".repeat(/*n*/ 5000));
    assert_eq!(
        MathMarkdown::new(&source, Options::empty(), Some(80)).pending_start,
        None
    );
}

#[test]
fn unicode_math_oversized_display_stays_literal() {
    for (open, close) in [("$$", "$$"), ("\\[", "\\]")] {
        for ending in ["", close] {
            let source = format!("{open}\n{}{ending}", "# x\n- y\n".repeat(/*n*/ 600));
            assert_eq!(plain(&source, /*width*/ 80), source.trim_end());
        }
    }
}

#[test]
fn unicode_math_pending_source_and_pid_boundaries() {
    for (open, close) in [("$$", "$$"), ("\\[", "\\]")] {
        assert_eq!(
            plain(&format!("{open}\nx^2\n\n+y^2\n{close}"), /*width*/ 80),
            "x² +y²"
        );
        let source = format!("{open}\nx\n{close} trailing\n{close}\n\nAfter $\\alpha$.");
        assert_eq!(
            plain(&source, /*width*/ 80),
            format!("{open}\nx\n{close} trailing\n{close}\n\nAfter α.")
        );
    }
    for source in ["$$\n# x\n", "$$\n- x\n", "\\[\n# x\n", "\\[\n- x\n"] {
        assert_eq!(plain(source, /*width*/ 80), source.trim_end());
    }
    let source = format!("{}After $\\alpha$.", "prose \\[\n".repeat(/*n*/ 1000));
    let math = MathMarkdown::new(&source, Options::empty(), Some(80));
    assert_eq!(math.display_ranges.len(), 1);
    assert!(math.replacements.is_empty());
    assert_eq!(
        plain(
            "echo $$\n\n$$\nx^2\n$$\n\nAfter $\\alpha$.",
            /*width*/ 80
        ),
        "echo $$\n\nx²\n\nAfter α."
    );
    assert_eq!(
        plain(
            "$$\nprice=\\$$$\n\nAfter $\\alpha$.\n\n$$\\beta$$",
            /*width*/ 80
        ),
        "$$\nprice=\\$$$\n\nAfter α.\n\nβ"
    );
}
