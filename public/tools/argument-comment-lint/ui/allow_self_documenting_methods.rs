#![warn(argument_comment_mismatch)]
#![warn(uncommented_anonymous_literal_argument)]

struct Builder;

impl Builder {
    fn enabled(self, enabled: bool) -> Self {
        let _ = enabled;
        self
    }

    fn retry_count(self, retry_count: usize) -> Self {
        let _ = retry_count;
        self
    }

    fn base_url(self, base_url: Option<String>) -> Self {
        let _ = base_url;
        self
    }
}

fn main() {
    let _ = Builder.enabled(false).retry_count(3).base_url(None);
}
