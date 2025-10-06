use tracing::warn;

pub(crate) fn parse_tool_arguments(
    arguments: &str,
) -> Result<Option<serde_json::Value>, serde_json::Error> {
    if arguments.trim().is_empty() {
        return Ok(None);
    }

    match serde_json::from_str(arguments) {
        Ok(value) => Ok(Some(value)),
        Err(original_error) => {
            if let Some(fixed) = fix_unclosed_json_delimiters(arguments) {
                match serde_json::from_str(&fixed) {
                    Ok(value) => {
                        warn!("synthesized missing closing delimiters for tool arguments");
                        Ok(Some(value))
                    }
                    Err(_) => Err(original_error),
                }
            } else {
                Err(original_error)
            }
        }
    }
}

pub(crate) fn repair_tool_arguments(arguments: &str) -> Option<String> {
    if arguments.trim().is_empty() {
        return None;
    }

    if serde_json::from_str::<serde_json::Value>(arguments).is_ok() {
        return None;
    }

    let fixed = fix_unclosed_json_delimiters(arguments)?;
    serde_json::from_str::<serde_json::Value>(&fixed).ok()?;
    Some(fixed)
}

fn fix_unclosed_json_delimiters(arguments: &str) -> Option<String> {
    let trimmed = arguments.trim_end();
    if trimmed.is_empty() {
        return None;
    }

    let trailing_whitespace = &arguments[trimmed.len()..];
    let mut result = trimmed.to_string();
    let mut stack: Vec<char> = Vec::new();
    let mut in_string = false;
    let mut escaped = false;

    for ch in trimmed.chars() {
        if in_string {
            if escaped {
                escaped = false;
                continue;
            }

            match ch {
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
        } else {
            match ch {
                '"' => in_string = true,
                '{' => stack.push('}'),
                '[' => stack.push(']'),
                '(' => stack.push(')'),
                '}' | ']' | ')' => {
                    if let Some(expected) = stack.pop() {
                        if expected != ch {
                            return None;
                        }
                    } else {
                        return None;
                    }
                }
                _ => {}
            }
        }
    }

    if stack.is_empty() {
        return None;
    }

    while let Some(ch) = stack.pop() {
        result.push(ch);
    }

    result.push_str(trailing_whitespace);

    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn parse_tool_arguments_handles_missing_closing_brace() {
        let args = "{\"foo\": \"bar\"";
        let parsed = parse_tool_arguments(args).expect("should parse");
        let value = parsed.expect("expected some value");
        assert_eq!(value["foo"], "bar");
    }

    #[test]
    fn fix_unclosed_json_delimiters_adds_multiple_missing_braces() {
        let args = "{\"outer\": {\"inner\": 1";
        let fixed = fix_unclosed_json_delimiters(args).expect("should fix");
        assert_eq!(fixed, "{\"outer\": {\"inner\": 1}}");
    }

    #[test]
    fn fix_unclosed_json_delimiters_adds_missing_array_bracket() {
        let args = "{\"items\": [1, 2";
        let fixed = fix_unclosed_json_delimiters(args).expect("should fix");
        assert_eq!(fixed, "{\"items\": [1, 2]}");
    }

    #[test]
    fn fix_unclosed_json_delimiters_ignores_braces_in_strings() {
        let args = "{\"text\": \"use {curly}\"";
        let fixed = fix_unclosed_json_delimiters(args).expect("should fix");
        assert_eq!(fixed, "{\"text\": \"use {curly}\"}");
    }

    #[test]
    fn fix_unclosed_json_delimiters_preserves_trailing_whitespace() {
        let args = "{\"foo\": 1\n";
        let fixed = fix_unclosed_json_delimiters(args).expect("should fix");
        assert_eq!(fixed, "{\"foo\": 1}\n");
    }

    #[test]
    fn fix_unclosed_json_delimiters_returns_none_when_not_needed() {
        let args = "{\"foo\": true}";
        assert!(fix_unclosed_json_delimiters(args).is_none());
    }

    #[test]
    fn repair_tool_arguments_returns_fixed_string_when_needed() {
        let args = "{\"foo\": 1";
        let repaired = repair_tool_arguments(args).expect("should repair");
        assert_eq!(repaired, "{\"foo\": 1}");
    }

    #[test]
    fn repair_tool_arguments_returns_none_when_valid() {
        let args = "{\"foo\": 1}";
        assert!(repair_tool_arguments(args).is_none());
    }

    #[test]
    fn repair_tool_arguments_returns_none_for_empty() {
        assert!(repair_tool_arguments("").is_none());
    }
}
