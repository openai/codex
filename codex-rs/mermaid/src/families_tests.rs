//! End-to-end grammar, topology, and rendered output checks for each supported family.

use super::RenderError;
use super::render;
use pretty_assertions::assert_eq;
use unicode_width::UnicodeWidthStr;

const SEQUENCE: &str = "sequenceDiagram
    actor U as Buyer
    participant A as API
    participant S as 库存
    participant P as Payments
    U->>A: Place order
    A->>S: Reserve items
    S-->>A: Reservation
    opt Items reserved
        loop Up to 3 attempts
            A->>P: Charge card
            P->>P: Check fraud
            P-->>A: Payment status
            alt Approved
                Note over A,P: Payment recorded
                A-->>U: Order confirmed
            else Declined
                A->>S: Release items
                A-->>U: Payment failed
            end
        end
    end";

const STATE: &str = "stateDiagram-v2
    state \"Payment pending\" as Charging
    [*] --> Draft
    Draft --> Validating: submit
    Validating --> Charging: valid
    Validating --> Rejected: invalid
    Charging --> Packing: paid
    Charging --> Rejected: declined
    Packing --> Shipped: dispatch
    Shipped --> Delivered: received
    Delivered --> [*]
    Rejected --> Draft: revise
    Charging: Retry up to 3 times";

const CLASS: &str = "classDiagram
    class Order {
        +String id
        +Status status
        +submit()
        +cancel()
    }
    class LineItem {
        +int quantity
        +Decimal price
        +subtotal()
    }
    class Payment {
        +Decimal amount
        +authorize()
    }
    class CardPayment {
        +String lastFour
        +authorize()
    }
    Order \"1\" *-- \"1..*\" LineItem : contains
    Order \"1\" --> \"1\" Payment : pays with
    Payment <|-- CardPayment
    CardPayment ..> Order : updates";

const ER: &str = "erDiagram
    CUSTOMER ||--o{ ORDER : places
    ORDER ||--|{ LINE_ITEM : contains
    PRODUCT ||..o{ LINE_ITEM : appears_in
    CUSTOMER {
        int id PK
        string email UK
        string name
    }
    ORDER {
        int id PK
        int customer_id FK
        string status
    }
    LINE_ITEM {
        int order_id PK, FK \"order key\"
        int product_id PK, FK
        int quantity
    }
    PRODUCT {
        int id PK
        string name
        decimal price
    }";

#[test]
fn complex_families() {
    for (name, source) in [
        ("sequence", SEQUENCE),
        ("state", STATE),
        ("class", CLASS),
        ("er", ER),
    ] {
        let output = render(source, /*max_width*/ 180).unwrap();
        let width = output.lines().map(UnicodeWidthStr::width).max().unwrap();
        assert_eq!(render(source, width), Ok(output.clone()));
        assert_eq!(render(source, width - 1), Err(RenderError::TooWide));
        insta::assert_snapshot!(name, output);
    }
}

#[test]
fn unicode_labels_and_later_declarations() {
    for direction in ["TD", "BT", "LR", "RL"] {
        let source = format!(
            "%% heading\ngraph {direction}; A -->|准备| B; A[请求]; B{{Réponse?}}; B -->|retry| A; B --> C[Ship 🚀]"
        );
        let output = render(&source, /*max_width*/ 160).unwrap();
        for label in ["请求", "Réponse?", "Ship 🚀", "准备", "retry"] {
            assert!(output.contains(label), "{direction}: {label}");
        }
        let width = output.lines().map(UnicodeWidthStr::width).max().unwrap();
        assert_eq!(render(&source, width), Ok(output.clone()));
        assert_eq!(render(&source, width - 1), Err(RenderError::TooWide));
        if direction == "LR" {
            insta::assert_snapshot!("LR", output);
        }
    }
}

#[test]
fn class_relationship_endpoints() {
    for (operator, source_tip, target_tip, dashed) in [
        ("<|--", '◁', '─', false),
        ("*--", '◆', '─', false),
        ("o--", '◇', '─', false),
        ("-->", '─', '◄', false),
        ("--", '─', '─', false),
        ("..>", '─', '◄', true),
        ("..|>", '─', '◁', true),
        ("..", '─', '─', true),
        ("<--", '◄', '─', false),
        ("--*", '─', '◆', false),
        ("--o", '─', '◇', false),
        ("--|>", '─', '◁', false),
    ] {
        let output = render(
            &format!("classDiagram; A \"one\" {operator} \"many\" B : uses"),
            /*max_width*/ 100,
        )
        .unwrap();
        let ports = output
            .lines()
            .filter(|line| line.contains('├'))
            .collect::<Vec<_>>();
        assert!(
            ports[0].contains(&format!("├{source_tip}")),
            "{operator}: {output}"
        );
        assert!(
            ports[1].contains(&format!("├{target_tip}")),
            "{operator}: {output}"
        );
        assert!(ports[0].contains("(one) uses"));
        assert!(ports[1].contains("(many)"));
        assert_eq!(output.contains('┆'), dashed);
    }
}

#[test]
fn er_cardinalities() {
    for (left, source_card) in [
        ("||", "1"),
        ("|o", "0..1"),
        ("}|", "1..many"),
        ("}o", "0..many"),
    ] {
        for (right, target_card) in [
            ("||", "1"),
            ("o|", "0..1"),
            ("|{", "1..many"),
            ("o{", "0..many"),
        ] {
            let output = render(
                &format!("erDiagram; A {left}--{right} B : owns"),
                /*max_width*/ 100,
            )
            .unwrap();
            let ports = output
                .lines()
                .filter(|line| line.contains('├'))
                .collect::<Vec<_>>();
            assert!(ports[0].contains(&format!("({source_card}) owns")));
            assert!(ports[1].contains(&format!("({target_card})")));
        }
    }
}

#[test]
fn rejects_incomplete_and_unsupported_families() {
    for source in [
        "sequenceDiagram; A->>B: hello; nonsense",
        "sequenceDiagram; A->>B: hello; activate B",
        "sequenceDiagram; participant A as x; participant A as y",
        "sequenceDiagram; alt ready; A->>B: hi",
        "sequenceDiagram; A->>B: hi; end",
        "sequenceDiagram; loop retry; else no; end",
        "sequenceDiagram; alt one; else two; else three; end",
        "sequenceDiagram; A->>B: <br/>",
        "sequenceDiagram; A->>B: \u{1b}[31m",
        "sequenceDiagram; participant A as e\u{301}",
        "stateDiagram-v2; state Processing {; A-->B; }",
        "stateDiagram-v2; A --> B: ok; garbage text",
        "stateDiagram-v2; state \"First\" as A; state \"Second\" as A",
        "stateDiagram-v2; accDescr: Order lifecycle; [*] --> Ready",
        "stateDiagram; ACCDESCR: Order lifecycle; [*] --> Ready",
        "stateDiagram-v2; A:::highlight; A --> B",
        "stateDiagram-v2; A --> B:::highlight",
        "classDiagram; accTitle: Account model; class Account",
        "classDiagram; class A {; +foo()",
        "classDiagram; class A; A --? B",
        "classDiagram; A --> B; click A",
        "classDiagram; class A {; +<html>; }",
        "erDiagram; A ||--o{ B",
        "erDiagram; A ||--?? B : owns",
        "erDiagram; A {; int x KEY; }",
        "erDiagram; A {; int x PK \"open comment; }",
        "erDiagram; A {; int x; }; A ||--|| B : uses; trailing junk",
        "erDiagram; accDescr {; A model; }; CUSTOMER",
        "erDiagram; ACCDESCR {; A model; }; CUSTOMER",
        "erDiagram; A ||--|| B:::highlight : owns",
        "%%{init: {}}%%\nclassDiagram; class A",
    ] {
        assert_eq!(
            render(source, /*max_width*/ 200),
            Err(RenderError::Unsupported),
            "{source}"
        );
    }
}

#[test]
fn metadata_names_and_style_text_remain_valid_in_content() {
    for source in [
        "classDiagram; class accTitle {; +id; }; class accDescr {; +id; }",
        "classDiagram; class A; A: ::: literal; A --> B: ::: literal",
        "erDiagram; accTitle {; int id; }",
        "erDiagram; A ||--|| B: ::: literal",
        "stateDiagram-v2; A: text ::: literal; A --> B: ::: literal",
    ] {
        assert!(render(source, /*max_width*/ 200).is_ok(), "{source}");
    }
}

#[test]
fn family_limits() {
    for source in [
        format!(
            "sequenceDiagram; {}",
            (0..9)
                .map(|i| format!("participant P{i};"))
                .collect::<String>()
        ),
        format!("sequenceDiagram; {}", "A->>B: msg;".repeat(65)),
        format!(
            "sequenceDiagram; {} A->>B: msg; {}",
            "loop retry;".repeat(5),
            "end;".repeat(5)
        ),
        format!("classDiagram; class A {{; {} }}", "+field;".repeat(17)),
        format!("erDiagram; A {{; {} }}", "int id;".repeat(17)),
        format!("stateDiagram-v2; {}", "A --> B;".repeat(25)),
    ] {
        assert_eq!(
            render(&source, /*max_width*/ usize::MAX),
            Err(RenderError::Limit),
            "{source}"
        );
    }
}

#[test]
fn truncated_sources_and_terminal_widths() {
    // Exercise incomplete streamed source at every UTF-8 boundary, including inside control blocks.
    for source in [SEQUENCE, STATE, CLASS, ER, "sequenceDiagram; A->>B: 请求"] {
        for end in source.char_indices().map(|(index, _)| index) {
            if let Ok(output) = render(&source[..end], /*max_width*/ 180) {
                assert!(output.lines().all(|line| line.width() <= 180));
                assert!(!output.chars().any(|ch| ch.is_control() && ch != '\n'));
            }
        }
    }
    for source in [
        "sequenceDiagram; A->>B: hi",
        "sequenceDiagram; A->>A: self",
        "sequenceDiagram; Note over A: memo",
    ] {
        let output = render(source, /*max_width*/ 100).unwrap();
        let width = output.lines().map(UnicodeWidthStr::width).max().unwrap();
        assert_eq!(render(source, width), Ok(output));
        assert_eq!(render(source, width - 1), Err(RenderError::TooWide));
    }
}

#[test]
fn sequence_arrows_preserve_sender_recipient_and_style() {
    for (operator, forward, reverse, dashed) in [
        ("->>", '▶', '◀', false),
        ("-->>", '▶', '◀', true),
        ("->", '─', '─', false),
        ("-->", '┄', '┄', true),
        ("-x", 'x', 'x', false),
        ("--x", 'x', 'x', true),
    ] {
        for (from, to, tip) in [
            ("A", "B", forward),
            ("B", "A", reverse),
            ("B", "B", reverse),
        ] {
            let output = render(
                &format!(
                    "sequenceDiagram; participant A; participant B; {from}{operator}{to}: msg"
                ),
                /*max_width*/ 100,
            )
            .unwrap();
            let rows = output
                .lines()
                .map(|line| line.chars().collect::<Vec<_>>())
                .collect::<Vec<_>>();
            let a = rows[1].iter().position(|ch| *ch == 'A').unwrap();
            let b = rows[1].iter().position(|ch| *ch == 'B').unwrap();
            let recipient = if to == "A" { a } else { b };
            let row = if from == to { 5 } else { 4 };
            assert_eq!(rows[row][recipient], tip, "{from}{operator}{to}");
            assert_eq!(rows[row].contains(&'┄'), dashed);
        }
    }
}

#[test]
fn canvas_limit_applies_even_with_unlimited_caller_width() {
    let graph = format!(
        "graph LR; {}",
        format!("A-->|{}|B;", "x".repeat(40)).repeat(24)
    );
    let sequence = format!(
        "sequenceDiagram; {} {}",
        (0..8)
            .map(|i| format!("participant P{i};"))
            .collect::<String>(),
        format!("P0->>P7: {};", "x".repeat(40)).repeat(64)
    );
    for source in [graph, sequence] {
        assert_eq!(
            render(&source, /*max_width*/ usize::MAX),
            Err(RenderError::Limit)
        );
    }
}
