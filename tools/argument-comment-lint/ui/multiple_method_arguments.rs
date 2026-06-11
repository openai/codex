#![warn(uncommented_anonymous_literal_argument)]

struct Options;

impl Options {
    fn enabled(self, enabled: bool, retry_count: usize) -> Self {
        let _ = (enabled, retry_count);
        self
    }
}

fn main() {
    let _ = Options.enabled(false, /*retry_count*/ 3);
}
