//! Render a diagram from standard input for manual prototype evaluation.

use std::io::Read;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut source = String::new();
    std::io::stdin()
        .take(16 * 1024 + 1)
        .read_to_string(&mut source)?;
    let max_width = std::env::args()
        .nth(1)
        .map(|value| value.parse())
        .transpose()?
        .unwrap_or(120);
    println!("{}", codex_mermaid::render(&source, max_width)?);
    Ok(())
}
