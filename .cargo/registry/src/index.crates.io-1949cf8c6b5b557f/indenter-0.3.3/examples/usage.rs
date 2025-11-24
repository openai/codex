use indenter::indented;
use std::fmt::Write;

fn main() {
    let input = "verify\nthis";
    let mut output = String::new();

    indented(&mut output).ind(12).write_str(input).unwrap();

    println!("Before:\n{}\n", input);
    println!("After:\n{}", output);
}
