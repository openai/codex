const DEFAULT_ORIGINATOR: &str = "codex_cli_rs";

// Conservative header value sanitization: limit to a safe subset of ASCII commonly
// accepted in User-Agent strings. Anything else is replaced with underscore.
fn is_valid_header_value_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/' | ' ' | '(' | ')' | ';' | ':')
}

fn sanitize_header_value<S: AsRef<str>>(value: S) -> String {
    value
        .as_ref()
        .chars()
        .map(|c| {
            if is_valid_header_value_char(c) {
                c
            } else {
                '_'
            }
        })
        .collect()
}

pub fn get_codex_user_agent(originator: Option<&str>) -> String {
    let build_version = env!("CARGO_PKG_VERSION");
    let os_info = os_info::get();

    // Sanitize each dynamic component to avoid reqwest HeaderValue builder errors on
    // unusual locales or terminals that include non-ASCII characters.
    let originator = sanitize_header_value(originator.unwrap_or(DEFAULT_ORIGINATOR));
    let os_type = sanitize_header_value(os_info.os_type().to_string());
    let os_version = sanitize_header_value(os_info.version().to_string());
    let arch = sanitize_header_value(os_info.architecture().unwrap_or("unknown"));
    let term = sanitize_header_value(crate::terminal::user_agent());

    // Build descriptive UA first.
    let ua = format!("{originator}/{build_version} ({os_type} {os_version}; {arch}) {term}");

    // Validate against reqwest's HeaderValue rules; fall back if rejected.
    if reqwest::header::HeaderValue::from_str(&ua).is_ok() {
        ua
    } else {
        format!("{DEFAULT_ORIGINATOR}/{build_version}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_codex_user_agent() {
        let user_agent = get_codex_user_agent(None);
        assert!(user_agent.starts_with("codex_cli_rs/"));
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_macos() {
        use regex_lite::Regex;
        let user_agent = get_codex_user_agent(None);
        let re = Regex::new(
            r"^codex_cli_rs/\d+\.\d+\.\d+ \(Mac OS \d+\.\d+\.\d+; (x86_64|arm64)\) (\S+)$",
        )
        .unwrap();
        assert!(re.is_match(&user_agent));
    }
}
