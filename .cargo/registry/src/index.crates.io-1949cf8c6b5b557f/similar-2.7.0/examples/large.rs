use similar::TextDiff;

fn main() {
    let x = "abc".repeat(2000);
    let y = "abd".repeat(2000);
    let diff = TextDiff::from_chars(&x, &y);
    println!("{}", diff.unified_diff());
}
