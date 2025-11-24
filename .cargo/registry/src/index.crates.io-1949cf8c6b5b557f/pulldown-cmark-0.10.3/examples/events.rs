use std::io::Read;

use pulldown_cmark::{Event, Parser};

/// Show all events from the text on stdin.
fn main() {
    let mut text = String::new();
    std::io::stdin().read_to_string(&mut text).unwrap();

    eprintln!("{text:?} -> [");
    let mut width = 0;
    for event in Parser::new(&text) {
        if let Event::End(_) = event {
            width -= 2;
        }
        eprintln!("  {:width$}{event:?}", "");
        if let Event::Start(_) = event {
            width += 2;
        }
    }
    eprintln!("]");
}
