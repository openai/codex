pub fn parse_argument_comment(text: &str) -> Option<&str> {
    let trimmed = text.trim_end();
    let comment_start = trimmed.rfind("/*")?;
    let comment = &trimmed[comment_start..];
    let name = comment.strip_prefix("/*")?.strip_suffix("*/")?;
    is_identifier(name).then_some(name)
}

pub fn parse_argument_comment_prefix(text: &str) -> Option<&str> {
    let trimmed = text.trim_start();
    let comment = trimmed.strip_prefix("/*")?;
    let (name, _) = comment.split_once("*/")?;
    is_identifier(name).then_some(name)
}

fn is_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first == '_' || first.is_ascii_alphabetic()) {
        return false;
    }
    chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::parse_argument_comment;
    use super::parse_argument_comment_prefix;

    #[test]
    fn parses_trailing_comment() {
        assert_eq!(parse_argument_comment("(/*base_url*/ "), Some("base_url"));
        assert_eq!(
            parse_argument_comment(", /*timeout_ms*/ "),
            Some("timeout_ms")
        );
        assert_eq!(
            parse_argument_comment(".method::<u8>(/*base_url*/ "),
            Some("base_url")
        );
    }

    #[test]
    fn rejects_non_matching_shapes() {
        assert_eq!(parse_argument_comment("(\n"), None);
        assert_eq!(parse_argument_comment("(/* base_url*/ "), None);
        assert_eq!(parse_argument_comment("(/*base_url */ "), None);
        assert_eq!(parse_argument_comment("(/*base_url=*/ "), None);
        assert_eq!(parse_argument_comment("(/*1base_url*/ "), None);
        assert_eq!(parse_argument_comment_prefix("/*env=*/ None"), None);
    }

    #[test]
    fn parses_prefix_comment() {
        assert_eq!(parse_argument_comment_prefix("/*env*/ None"), Some("env"));
        assert_eq!(
            parse_argument_comment_prefix("\n    /*retry_count*/ 3"),
            Some("retry_count")
        );
    }
}
