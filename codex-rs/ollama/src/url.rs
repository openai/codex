/// Identify whether a base_url points at an OpenAI-compatible root (".../v1").
pub(crate) fn is_openai_compatible_base_url(base_url: &str) -> bool {
    base_url.trim_end_matches('/').ends_with("/v1")
}

/// Convert a provider base_url into the native Ollama host root.
/// For example, "http://localhost:11434/v1" -> "http://localhost:11434".
pub fn base_url_to_host_root(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    if trimmed.ends_with("/v1") {
        trimmed
            .trim_end_matches("/v1")
            .trim_end_matches('/')
            .to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_url_to_host_root() {
        assert_eq!(
            base_url_to_host_root("http://localhost:11434/v1"),
            "http://localhost:11434"
        );
        assert_eq!(
            base_url_to_host_root("http://localhost:11434"),
            "http://localhost:11434"
        );
        assert_eq!(
            base_url_to_host_root("http://localhost:11434/"),
            "http://localhost:11434"
        );
    }
}
