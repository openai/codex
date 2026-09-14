use super::render::render;
use crate::markdown_render::render_markdown_text_with_width;
use itertools::Itertools;
use pretty_assertions::assert_eq;

fn plain(source: &str, width: usize) -> String {
    render_markdown_text_with_width(source, Some(width))
        .lines
        .iter()
        .map(ToString::to_string)
        .join("\n")
}

#[test]
fn unicode_math_inline_snapshot() {
    insta::assert_snapshot!(plain(
        r"Inline $\alpha^2 + \beta_{10}$ on $\mathbb{R}^n$.
Root $\sqrt{x^2+y^2}$ and fraction $a/\frac{b}{c}$.
Unsupported: $\unknown{x_y}$ and $\frac{a}{b}^2$.
Angles: <$x$>, <\(\alpha\)>, and <$\unknown{x}$>.
Code: `$\alpha$`; money: $5 and $10; shell: $HOME.",
        /*width*/ 80
    ));
}

#[test]
fn unicode_math_inline_narrow_snapshot() {
    insta::assert_snapshot!(plain(
        r"Words $\alpha^2 + \beta_1$ more words and $\sqrt{x}$.",
        /*width*/ 16
    ));
}

#[test]
fn unicode_math_preserves_markdown_contexts() {
    for (source, expected) in [
        (
            r"**$\alpha_1$** &amp; \$5.00 and $\beta^2$",
            "α₁ & $5.00 and β²",
        ),
        (
            r"`$\alpha$` $HOME ${HOME} $(echo x) $5 and $10",
            r"$\alpha$ $HOME ${HOME} $(echo x) $5 and $10",
        ),
        (
            r"[x](https://example.com/$HOME) and $\alpha$",
            "x (https://example.com/$HOME) and α",
        ),
        (r"$\left. x \right|$ $\text{x_y^z}$", " x | x_y^z"),
        (r"\(\alpha^2\)", "α²"),
        (r"Value: <$x$> and <\(\alpha\)>.", "Value: <x> and <α>."),
        (r"$USD$+$\alpha$", "$USD$+α"),
        (r"Costs $5; $\alpha$ and $x^2$.", "Costs $5; α and x²."),
        (r"$HOME then $\alpha$", "$HOME then α"),
        (
            r"Run echo $$ to print PID. Then $\alpha$",
            "Run echo $$ to print PID. Then α",
        ),
    ] {
        assert_eq!(plain(source, /*width*/ 80), expected);
    }
    // This stage leaves display equations and TeX fences to ordinary Markdown rendering.
    for source in [r"$$x^2$$", "$$\nx^2\n$$", r"\[x^2\]", "```latex\nx^2\n```"] {
        assert!(!plain(source, /*width*/ 80).contains('²'));
    }
}

#[test]
fn unicode_math_bounds_and_unsupported_input() {
    for source in [
        r"\unknown{x}",
        r"\frac{a}",
        "{x",
        "x}",
        "x^{q}",
        "^2",
        "x^2^3",
        "{a+b}^2",
        "{x^2}^3",
        r"\frac{a}{b}^2",
        r"\sqrt[3]{x}",
        r"\sqrt [3]{x}",
        r"\left x",
        r"\text{\alpha}",
        r"\begin{matrix}a&b\end{matrix}",
    ] {
        assert_eq!(render(source), None, "{source}");
        assert_eq!(
            plain(&format!("\\({source}\\)"), /*width*/ 80),
            format!("\\({source}\\)")
        );
    }
    for source in [
        format!("{}x{}", "{".repeat(/*n*/ 40), "}".repeat(/*n*/ 40)),
        "x".repeat(/*n*/ 300),
        format!("{}x", "\\sqrt".repeat(/*n*/ 100)),
    ] {
        assert_eq!(render(&source), None);
    }
}

#[test]
fn unicode_math_oversized_display_does_not_hide_following_inline_math() {
    for (open, close) in [("$$", "$$"), (r"\[", r"\]")] {
        let source = format!(
            "{open}\n{}\n{close}\n\nAfter $\\alpha$.\n",
            "x".repeat(/*n*/ 5000)
        );
        assert!(plain(&source, /*width*/ 80).ends_with("After α."));
    }
}
