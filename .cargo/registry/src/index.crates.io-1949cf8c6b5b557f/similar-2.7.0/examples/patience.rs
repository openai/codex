use similar::{Algorithm, TextDiff};

const OLD: &str = r#"
[
    (
        Major,
        2,
    ),
    (
        Minor,
        20,
    ),
    (
        Value,
        0,
    ),
]
"#;
const NEW: &str = r#"
[
    (
        Major,
        2,
    ),
    (
        Minor,
        0,
    ),
    (
        Value,
        0,
    ),
    (
        Value,
        1,
    ),
]
"#;

fn main() {
    println!(
        "{}",
        TextDiff::configure()
            .algorithm(Algorithm::Patience)
            .diff_lines(OLD, NEW)
            .unified_diff()
    );
}
