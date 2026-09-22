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
fn unicode_math_schrodinger_equation_snapshot() {
    insta::assert_snapshot!(plain(
        r"\[
i\hbar \frac{\partial}{\partial t}\Psi(\mathbf r,t)
=
\hat H\Psi(\mathbf r,t)
=
\left[-\frac{\hbar^2}{2m}\nabla^2+V(\mathbf r,t)\right]\Psi(\mathbf r,t).
\]",
        /*width*/ 100
    ));
}

#[test]
fn unicode_math_optimization_model() {
    let source = r"Let \(X_{gpt}=x_{gpt}\), with \(X_{gp,-1}=I_{gp}\).

\[
\boxed{
\begin{aligned}
\underset{z,u,s,m\ge0}{\operatorname{minimize}}\quad
&\sum_{g,t}w_{\pi_{gt}}u_{gt}
+\lambda\sum_t\sum_{(g,p)\in\mathcal J_t}\kappa_{pt}m_{gpt}
+\mu\sum_{(g,t)\in\mathcal Q}s_{gt}
\\[2mm]
\text{subject to}\quad
&\widehat F_{gt}+u_{gt}-s_{gt}=D_{gt}
&&\forall g,t
\\
&\sum_gx_{gpt}\le S_{pt}-B_{pt}
&&\forall p,t
\\
&\sum_{g,p,k}z_{gpk\ell t}\le C_{\ell t}
&&\forall \ell,t
\\
&\sum_{\substack{g\in G_r\\p:\operatorname{region}(p)\in M_r}}x_{gpt}
\ge\sum_{g\in G_r,p}\alpha_{rg}x_{gpt}
&&\forall r,t
\\
&q_aL_{adt}\le L_{at}
&&\forall a,d,t
\\
&m_{gpt}\ge X_{gp,t-1}-X_{gpt}
&&\forall (g,p)\in\mathcal J_t
\end{aligned}}
\]

After the model: \(\alpha^2\).";
    let rendered = plain(source, /*width*/ 240);
    assert!(!rendered.contains('\\'), "{rendered}");
    insta::assert_snapshot!(rendered);
}

#[test]
fn unicode_math_display_limits() {
    insta::assert_snapshot!(plain(
        r"\[
\underset{x,u,m\ge0}{\operatorname{minimize}}\quad
\sum_{g,t}x_{gpt}+\lambda\sum^{N}_{i=1}y_i
\]

\[
\sum_{i=1}^N\frac{x_i}{y_i}\overset{\text{def}}{=}\underset{j\in J}{\min} z_j
\]

\[
\sum_{j=\sum_i^n i}^N x_j
\]

\[
\sum_{ij}^{100}x+\sum^{ij}_{100}y
\]

\[
\frac{\sum_i x_i}{\sum_j y_j}+\sqrt{\sum_i x_i}+x^{\sum_i a_i}+x_{\underset{i}{\min}}+\sum_k z_k
\]

Inline: \(\sum_{i=1}^N x_i\), \(\underset{x\ge0}{\min} x\).",
        /*width*/ 100
    ));
    assert_eq!(
        render(r"\sum_i^N x_i", /*display*/ true),
        render(r"\sum^N_i x_i", /*display*/ true)
    );
    for source in [r"\sum_i_j", r"\sum^i^j", r"\sum_", r"\sum^_i"] {
        assert_eq!(render(source, /*display*/ true), None, "{source}");
    }
    let source = r"\[\sum_{i,j,k=1}^{123456789}x_i\]";
    assert_eq!(
        plain(source, /*width*/ 12),
        plain(&format!("`{source}`"), /*width*/ 12)
    );
    assert_eq!(
        render(
            &format!(
                r"\begin{{aligned}}{}\end{{aligned}}",
                r"\sum_i x_i\\".repeat(/*n*/ 9)
            ),
            /*display*/ true
        ),
        None
    );
}

#[test]
fn unicode_math_grouped_scripts_and_annotations() {
    insta::assert_snapshot!(plain(
        r"Indices: \(x_{gpt} + x_{i,j} + w_{\pi_{gt}} + x^{q+1} + x_{g}^2\).
Native scripts: \(x_i^2 + y_{10}\). Sets: \(\mathcal Q_t\).
Annotations: \(\underset{i\in I}{\min} x_i\), \(\overset{\text{def}}{=}\).",
        /*width*/ 100
    ));
}

#[test]
fn unicode_math_aligned_fraction_and_narrow_fallback() {
    let formula = r"\[
\begin{aligned}
\frac{x}{z}&=\frac{1}{2} &&\text{first}\\
y_{gpt}&=3 &&\text{second}\\
\end{aligned}
\]";
    insta::assert_snapshot!(format!(
        "Wide:\n{}\n\nNarrow:\n{}",
        plain(formula, /*width*/ 80),
        plain(formula, /*width*/ 12)
    ));
}

#[test]
fn unicode_math_aligned_preserves_fraction_geometry() {
    let source = r"\[
\begin{aligned}
\frac{x}{y}+\frac{1}{123456789}&=0\\
\frac{123456789}{1}+\frac{y}{x}&=1
\end{aligned}
\]

\[
\begin{aligned}\frac{a}{b}\end{aligned}=c
\]

\[
\begin{aligned}x&=\begin{aligned}\frac{a}{b}\end{aligned}\\y&=2\end{aligned}
\]";
    let rendered = plain(source, /*width*/ 80);
    insta::assert_snapshot!(rendered);
    let spaced = source
        .replace('&', " \n &")
        .replace(r"\\", " \n \\\\")
        .replace(r"\end{aligned}", " \n \\end{aligned}");
    assert_eq!(plain(&spaced, /*width*/ 80), rendered);
}

#[test]
fn unicode_math_structured_bounds_and_invalid_input() {
    let source = format!(
        r"\begin{{aligned}}\frac{{a}}{{b}}{}&=0\end{{aligned}}",
        "x".repeat(/*n*/ 252)
    );
    let rendered = render(&source, /*display*/ true).expect("fits the layout width limit");
    assert_eq!(
        render(&source.replace('&', " \n &"), /*display*/ true),
        Some(rendered)
    );
    assert_eq!(
        render(
            &format!(
                r"\begin{{aligned}}{}\end{{aligned}}",
                "x&=1\\\\".repeat(/*n*/ 16)
            ),
            /*display*/ true
        ),
        Some(vec!["x =1"; 16].join("\n"))
    );
    for source in [
        r"\begin{aligned}x&=1",
        r"\begin{aligned}x&=1\end{matrix}",
        r"\begin{aligned}[b]x&=1\end{aligned}",
        r"\begin{aligned}x&={a&b}\end{aligned}",
        r"\begin{aligned}x&=1\\[bad]y&=2\end{aligned}",
        r"\begin{aligned}x&=1\\[NaNmm]y&=2\end{aligned}",
        r"\underset{x}",
        r"\substack{x&y}",
    ] {
        assert_eq!(render(source, /*display*/ true), None, "{source}");
    }
    for source in [
        format!(
            r"\begin{{aligned}}{}\end{{aligned}}",
            "x&=1\\\\".repeat(/*n*/ 17)
        ),
        format!(
            r"\begin{{aligned}}{}x\end{{aligned}}",
            "x&".repeat(/*n*/ 17)
        ),
        format!(r"\boxed{{{}}}", "x".repeat(/*n*/ 254)),
        format!("{}x{}", r"\boxed{".repeat(/*n*/ 40), "}".repeat(/*n*/ 40)),
        format!(r"\substack{{{}}}", "x\\\\".repeat(/*n*/ 17)),
    ] {
        assert_eq!(render(&source, /*display*/ true), None, "{source}");
    }
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
