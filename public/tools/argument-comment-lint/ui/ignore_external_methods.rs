#![warn(uncommented_anonymous_literal_argument)]

fn main() {
    let line = "{\"type\":\"response_item\"}";
    let _ = line.starts_with('{');
    let _ = line.find("type");
    let parts = ["type", "response_item"];
    let _ = parts.join("\n");
}
