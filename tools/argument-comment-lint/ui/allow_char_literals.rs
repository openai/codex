#![warn(uncommented_anonymous_literal_argument)]

fn split_top_level(body: &str, delimiter: char) {
    let _ = (body, delimiter);
}

fn main() {
    split_top_level("a|b|c", '|');
}
