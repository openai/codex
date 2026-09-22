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
fn unicode_math_accents_and_symbols_snapshot() {
    insta::assert_snapshot!(plain(
        r"Operators: $\hat H^2 + \hbar^2$, $\ell$, $A\dagger$, $B\ddagger$.
Accents: $\bar{x}_1$, $\tilde{x}^2$, $\vec{v}_i$, $\dot{x}$, $\ddot{x}$.
Greek accents: $\hat{\Psi}^2$, $\bar{\varrho}$, $\tilde{\varsigma}$.
Other symbols: $\varkappa+\varpi$, $\Re z+\Im z$, $\aleph_0$, $\hat{\imath}$.
Relations: $a\propto b\sim c\simeq d\ll e\gg f$.
Geometry: $a\perp b\parallel c$, $\angle ABC\cong\angle DEF$.
Bounds: $a\lesssim b\gtrsim c$; operators: $a\oplus b\otimes c\odot d$.
Sets: $A\supseteq B\supset C\ni x$, $A\setminus B=\varnothing$.
Logic: $\neg P\land Q\lor R\implies S\iff T\impliedby U$, $\nexists x$.
Arrows: $a\leftrightarrow b\mapsto c\Leftarrow d$, $\uparrow\downarrow\updownarrow$.
Integrals: $\iint f$, $\iiint g$, $\oint h$.
Collections: $\coprod A$, $\bigcup B$, $\bigcap C$.
Punctuation: $x\prime$, $a\circ b\bullet c$, $\vdots\ddots$.",
        /*width*/ 80
    ));
}

#[test]
fn unicode_math_named_delimiters_snapshot() {
    insta::assert_snapshot!(plain(
        r"Inner product: $\left\langle\hat H\right\rangle$.
Rounding: $\lfloor x\rfloor + \left\lceil y\right\rceil$.
Norms: $\left\lVert v\right\rVert$, $\lvert x\rvert$, $\left\|w\right\|$.
Sets: $\left\{x\right\}$, $\lbrace y\rbrace$, $\lbrack z\rbrack$.
Angles: $\left<x\right>$.",
        /*width*/ 80
    ));
}

#[test]
fn unicode_math_accents_reject_ambiguous_arguments() {
    for source in [
        r"\hat{xy}",
        r"\bar{x+y}",
        r"\tilde{x^2}",
        r"\vec{\frac{x}{y}}",
        r"\dot{}",
        r"\ddot{ }",
        r"\hat",
        "\\hat{\u{0302}}",
    ] {
        for display in [false, true] {
            assert_eq!(render(source, display), None, "{source}");
        }
        assert_eq!(
            plain(&format!("\\({source}\\)"), /*width*/ 80),
            format!("\\({source}\\)")
        );
    }
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
}

#[test]
fn unicode_math_bounds_and_unsupported_input() {
    for source in [
        r"\unknown{x}",
        r"\frac{a}",
        "{x",
        "x}",
        "^2",
        "x^2^3",
        "{a+b}^2",
        "{x^2}^3",
        r"\frac{a}{b}^2",
        r"\sqrt[3]{x}",
        r"\sqrt [3]{x}",
        r"\left x",
        r"\left\alpha x\right\rangle",
        r"\left\ ",
        r"\text{\alpha}",
        r"\begin{matrix}a&b\end{matrix}",
    ] {
        assert_eq!(render(source, /*display*/ false), None, "{source}");
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
        assert_eq!(render(&source, /*display*/ false), None);
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
