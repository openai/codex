use super::RenderError;
use super::render;
use insta::assert_snapshot;
use pretty_assertions::assert_eq;
use unicode_width::UnicodeWidthStr;

#[test]
fn stadium_declarations_and_references() {
    let graph =
        super::parse::parse("flowchart TD", &["A", "A([Ready?])", "A", "A([Ready?])"]).unwrap();
    assert_eq!(
        graph.nodes,
        vec![super::Node {
            id: "A".to_owned(),
            label: "Ready?".to_owned(),
            shape: super::Shape::Stadium,
            declared: true,
            members: Vec::new(),
        }]
    );
}

#[test]
fn stadiums_with_other_shapes_in_every_direction() {
    let mut cases = Vec::new();
    for direction in ["TD", "BT", "LR", "RL"] {
        let source =
            format!("flowchart {direction}; A([请求]) --> B[Work] --> C{{Done?}}; C --> A");
        let output = render(&source, /*max_width*/ 100).unwrap();
        let width = output.lines().map(UnicodeWidthStr::width).max().unwrap();
        assert_eq!(render(&source, width), Ok(output.clone()));
        assert_eq!(render(&source, width - 1), Err(RenderError::TooWide));
        cases.push(format!("{direction}\n{output}"));
    }
    assert_snapshot!(cases.join("\n\n"));
}

#[test]
fn quoted_flowchart_labels_match_unquoted_labels() {
    let quoted = r#"A["Review & confirm"] -->|"Yes & continue"| B{"Ready?"}
B --> C(["Checkout"])
A[Review & confirm]"#;
    let unquoted = quoted.replace('"', "");
    assert_eq!(
        super::parse::parse("flowchart TD", &quoted.lines().collect::<Vec<_>>()).unwrap(),
        super::parse::parse("flowchart TD", &unquoted.lines().collect::<Vec<_>>()).unwrap(),
    );
}

#[test]
fn entity_labels_keep_source_fallback() {
    for entity in ["&amp;", "&#38;", "&#x26;"] {
        for source in [
            format!("sequenceDiagram\nA->>B: {entity}"),
            format!("flowchart TD; A[\"{entity}\"]"),
            format!("flowchart TD; A -->|\"{entity}\"| B"),
        ] {
            assert_eq!(
                render(&source, /*max_width*/ 100),
                Err(RenderError::Unsupported),
                "{source:?}",
            );
        }
    }
}

#[test]
fn flowchart_ampersands_preserve_statement_separators() {
    let source = "flowchart TD; A[R&D] -->|R&D| B[Review & confirm]; B --> C";
    assert_eq!(
        render(&format!("%% &amp;\n{source}"), /*max_width*/ 100),
        render(&source.replace(';', "\n"), /*max_width*/ 100),
    );
    assert!(
        render(source, /*max_width*/ 100)
            .unwrap()
            .contains("Review & confirm")
    );
}

#[test]
fn quoted_flowchart_labels_reject_malformed_and_unsafe_text() {
    for label in [
        "\"unfinished",
        "unfinished\"",
        "\"embedded\"quote\"",
        "\"\"",
        "\"<b>HTML</b>\"",
        "\"\u{1b}\"",
    ] {
        for source in [
            format!("flowchart TD; A[{label}]"),
            format!("flowchart TD; A -->|{label}| B"),
        ] {
            assert_eq!(
                render(&source, /*max_width*/ 100),
                Err(RenderError::Unsupported),
                "{source:?}",
            );
        }
    }
}

#[test]
fn branches_merges_and_retry_loop() {
    let source = "flowchart TD\nA[Checkout] --> B{In stock?}\nB -->|yes| C[Reserve]\nB -->|no| D[Waitlist]\nC --> E{Paid?}\nE -->|yes| F[Ship]\nE -->|no| G[Retry payment]\nG --> E\nD --> H[Notify buyer]\nF --> H";
    assert_snapshot!(render(source, /*max_width*/ 100).unwrap());
}

#[test]
fn rejects_partial_or_unsupported_input() {
    for source in [
        "flowchart TD; subgraph X; A; end",
        "flowchart TD; A --> B; garbage syntax",
        "flowchart TD; A -.-> B",
        "flowchart TD; A & B --> C",
        "flowchart TD; A[one]; A[two]",
        "flowchart TD; A[<b>HTML</b>]",
        "flowchart TD; A[&#27;]",
        "flowchart TD; A[\u{1b}]",
        "flowchart TD; A[e\u{301}]",
        "flowchart TD; A[👍🏽]",
        "flowchart TD; A[👩\u{200d}💻]",
        "flowchart TD; A[✈\u{fe0f}]",
        "flowchart TD; A[zero\u{200b}width]",
        "flowchart TD; A[left\u{202e}right]",
        "flowchart TD; A[\u{2066}isolated\u{2069}]",
        "flowchart TD; A[unclosed",
        "flowchart TD; click A",
        "flowchart TD; A((circle))",
        "flowchart TD; A(rounded)",
        "flowchart TD; A([unclosed]",
        "flowchart TD; A([unclosed)",
        "flowchart TD; A([label]) trailing",
        "flowchart TD; A([])",
        "flowchart TD; A([nested[label]])",
        "flowchart TD; A([one]); A([two])",
        "flowchart TD; A([same]); A[same]",
        "flowchart TD; A{same}; A([same])",
        "flowchart TD; A -->|unclosed B",
        "flowchart TD; A[foo;bar]",
        "flowchart TD; A[لا]",
        "flowchart TD; A -->|yes┐| B",
    ] {
        assert_eq!(
            render(source, /*max_width*/ 100),
            Err(RenderError::Unsupported),
            "{source:?}"
        );
    }
}

#[test]
fn source_graph_and_width_limits() {
    for source in [
        " ".repeat(16 * 1024 + 1),
        format!("graph TD; A[{}]", "x".repeat(41)),
        format!("graph TD; A([{}])", "x".repeat(41)),
        format!(
            "graph TD; {}",
            (0..17).map(|n| format!("N{n};")).collect::<String>()
        ),
        format!("graph TD; {}", "A-->B;".repeat(25)),
    ] {
        assert_eq!(render(&source, /*max_width*/ 200), Err(RenderError::Limit));
    }
    let output = render("graph TD; A --> B", /*max_width*/ 100).unwrap();
    let width = output.lines().map(UnicodeWidthStr::width).max().unwrap();
    assert_eq!(render("graph TD; A --> B", width), Ok(output));
    assert_eq!(
        render("graph TD; A --> B", width - 1),
        Err(RenderError::TooWide)
    );
    assert_eq!(
        render("graph TD; A", /*max_width*/ 0),
        Err(RenderError::TooWide)
    );
}

#[test]
fn reconstruct_every_edge_from_rendered_paths() {
    // All 512 directed graphs on three nodes, including self-loops, cycles, fan-in and fan-out.
    // Reconstruct connections from the emitted glyphs without consulting the renderer's layout.
    for (mask, direction) in
        (0u16..512).flat_map(|mask| ["TD", "BT", "LR", "RL"].map(|direction| (mask, direction)))
    {
        let mut source = format!("graph {direction}; A; B; C;");
        let mut expected = Vec::new();
        for from in 0..3 {
            for to in 0..3 {
                if mask & (1 << (from * 3 + to)) != 0 {
                    let a = char::from(b'A' + from);
                    let b = char::from(b'A' + to);
                    source.push_str(&format!("{a}-->{b};"));
                    expected.push((a, b));
                }
            }
        }
        let output = render(&source, /*max_width*/ 100).unwrap();
        let mut rows = output
            .lines()
            .map(|line| line.chars().collect::<Vec<_>>())
            .collect::<Vec<_>>();
        if matches!(direction, "LR" | "RL") {
            let width = rows.iter().map(Vec::len).max().unwrap();
            rows = (0..width)
                .map(|x| {
                    rows.iter()
                        .map(|row| match row.get(x).copied().unwrap_or(' ') {
                            '─' => '│',
                            '│' => '─',
                            '┐' => '└',
                            '└' => '┐',
                            '┬' => '├',
                            '▲' => '◄',
                            other => other,
                        })
                        .collect()
                })
                .collect();
        }
        let mut order = Vec::new();
        let mut owners = vec![' '; rows.len()];
        for (top, row) in rows.iter().enumerate() {
            if row.first() != Some(&'┌') {
                continue;
            }
            let bottom = (top + 1..rows.len())
                .find(|y| rows[*y].first() == Some(&'└'))
                .unwrap();
            let owner = rows[top..=bottom]
                .iter()
                .flatten()
                .find(|ch| matches!(ch, 'A' | 'B' | 'C'))
                .unwrap();
            owners[top..=bottom].fill(*owner);
            order.push(*owner);
        }
        assert_eq!(
            order,
            if matches!(direction, "BT" | "RL") {
                vec!['C', 'B', 'A']
            } else {
                vec!['A', 'B', 'C']
            }
        );
        let mut actual = Vec::new();
        for (y, row) in rows.iter().enumerate() {
            if let Some(port) = row.windows(2).position(|pair| pair == ['├', '─']) {
                let lane = (port + 1..row.len())
                    .find(|x| matches!(row[*x], '┐' | '┘'))
                    .unwrap();
                let mut target = y;
                loop {
                    target = if row[lane] == '┐' {
                        target + 1
                    } else {
                        target - 1
                    };
                    let ch = rows[target][lane];
                    if matches!(ch, '┘' | '┐') {
                        break;
                    }
                    assert!(matches!(ch, '│' | '╪'), "broken vertical path: {source}");
                }
                assert_eq!(&rows[target][port..port + 2], &['├', '◄']);
                assert!(
                    rows[target][port + 2..lane]
                        .iter()
                        .all(|ch| matches!(ch, '─' | '╪'))
                );
                actual.push((owners[y], owners[target]));
            }
        }
        actual.sort_unstable();
        assert_eq!(actual, expected, "{source}");
    }
}

#[test]
fn semantic_spans_distinguish_labels_from_matching_endpoint_glyphs() {
    use super::Role;

    let lines = super::render_spans("sequenceDiagram; A-xB: x", /*max_width*/ 100).unwrap();
    let roles = lines
        .iter()
        .flatten()
        .flat_map(|span| span.text.chars().filter(|ch| *ch == 'x').map(|_| span.role))
        .collect::<Vec<_>>();
    assert_eq!(roles, vec![Role::Text, Role::Edge]);
    assert_eq!(
        lines[0]
            .iter()
            .find(|span| span.text.contains('┌'))
            .unwrap()
            .role,
        Role::Node
    );
    for direction in ["TD", "BT", "LR", "RL"] {
        let lines = super::render_spans(
            &format!("flowchart {direction}; A --> B"),
            /*max_width*/ 100,
        )
        .unwrap();
        let ports = lines
            .iter()
            .flatten()
            .flat_map(|span| {
                span.text
                    .chars()
                    .filter(|ch| matches!(ch, '├' | '┬'))
                    .map(|_| span.role)
            })
            .collect::<Vec<_>>();
        assert_eq!(ports, vec![Role::Node, Role::Node]);
    }
}
