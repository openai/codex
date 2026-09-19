use crate::markdown_render::render_markdown_text_with_width;
use insta::assert_snapshot;
use pretty_assertions::assert_eq;

fn markdown_text(source: &str, width: usize) -> String {
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
fn mermaid_fences_use_native_renderer_for_every_family() {
    for source in [
        "%% heading\nflowchart TD; A --> B",
        "graph LR; A --> B",
        "sequenceDiagram; A->>B: request; B-->>A: response",
        "stateDiagram-v2; [*] --> Active; Active --> [*]",
        "stateDiagram; [*] --> Active; Active --> [*]",
        "classDiagram; Order \"1\" *-- \"many\" Item : contains",
        "erDiagram; CUSTOMER ||--o{ ORDER : places",
    ] {
        let markdown = format!("```mermaid title=example\n{source}\n```\n");
        assert_eq!(
            markdown_text(&markdown, /*width*/ 100),
            codex_mermaid::render(source, /*max_width*/ 100).unwrap()
        );
    }
}

#[test]
fn mermaid_nested_fences_and_unicode() {
    let source = "> ~~~~mermaid\n> flowchart TD\n>     A[请求] --> B[Réponse]\n> ~~~~~\n\n- Diagram:\n\n  ```mermaid\n  flowchart LR\n      A --> B\n  ```\n";
    assert_snapshot!(markdown_text(source, /*width*/ 60));
}

#[test]
fn mermaid_unclosed_invalid_unsupported_and_wide_blocks_keep_source() {
    for (source, width) in [
        ("```mermaid\nflowchart LR\nA --> B\n", 80),
        ("````mermaid\nflowchart LR\nA --> B\n```\n", 80),
        ("```mermaid\nflowchart LR\nA[unfinished\n```", 80),
        ("```mermaid\npie\n\"Cats\": 2\n```", 80),
        ("```mermaid\nflowchart LR\nA[Request] --> B[Reply]\n```", 8),
        ("> ```mermaid\n> flowchart LR\n> A --> B\n", 80),
    ] {
        assert_eq!(
            markdown_text(source, width),
            markdown_text(&source.replacen("mermaid", "unknown", /*count*/ 1), width),
            "source: {source:?}",
        );
    }
}

#[test]
fn mermaid_styles_follow_the_supplied_theme() {
    use two_face::theme::EmbeddedThemeName;

    let themes = two_face::theme::extra();
    let fallback = syntect::highlighting::Theme::default();
    let mut cases = Vec::new();
    for (name, theme) in [
        ("dark", themes.get(EmbeddedThemeName::Dracula)),
        ("light", themes.get(EmbeddedThemeName::SolarizedLight)),
        ("fallback", &fallback),
    ] {
        let colors = if name == "light" {
            crate::terminal_probe::DefaultColors {
                fg: (30, 30, 30),
                bg: (255, 255, 255),
            }
        } else {
            crate::terminal_probe::DefaultColors {
                fg: (220, 220, 220),
                bg: (20, 20, 20),
            }
        };
        let lines = crate::terminal_palette::with_test_default_colors(colors, || {
            super::render(
                "flowchart LR; A[请求] --> B[Reply]",
                /*width*/ Some(40),
                theme,
            )
            .unwrap()
        });
        let styled = lines
            .into_iter()
            .map(|line| {
                line.spans
                    .into_iter()
                    .map(|span| format!("{:?} {:?}", span.style, span.content))
                    .collect::<Vec<_>>()
                    .join(" | ")
            })
            .collect::<Vec<_>>()
            .join("\n");
        cases.push(format!("{name}\n{styled}"));
    }
    assert_snapshot!(cases.join("\n\n"));
}
