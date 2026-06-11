#![warn(argument_comment_mismatch)]

fn create_openai_url(base_url: Option<String>) -> String {
    let _ = base_url;
    String::new()
}

struct Options;

impl Options {
    fn enabled(self, enabled: bool) -> Self {
        let _ = enabled;
        self
    }
}

fn main() {
    let _ = create_openai_url(/*api_base*/ None);
    let _ = Options.enabled(/*value*/ false);
}
