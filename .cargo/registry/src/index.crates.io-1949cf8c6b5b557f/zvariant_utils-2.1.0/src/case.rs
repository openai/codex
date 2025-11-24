//! Contains utilities used to convert strings between different cases.

/// Convert to pascal or camel case, assuming snake or kebab case.
///
/// If `s` is already in pascal or camel case, should yield the same result.
pub fn pascal_or_camel_case(s: &str, is_pascal_case: bool) -> String {
    let mut result = String::new();
    let mut capitalize = is_pascal_case;
    let mut first = true;
    for ch in s.chars() {
        if ch == '_' || ch == '-' {
            capitalize = true;
        } else if capitalize {
            result.push(ch.to_ascii_uppercase());
            capitalize = false;
        } else if first && !is_pascal_case {
            result.push(ch.to_ascii_lowercase());
        } else {
            result.push(ch);
        }

        if first {
            first = false;
        }
    }
    result
}

/// Convert to snake or kebab case, assuming camel or Pascal case.
///
/// If `s` is already in snake or kebab case, should yield the same result.
pub fn snake_or_kebab_case(s: &str, is_snake_case: bool) -> String {
    let mut result = String::new();
    for ch in s.chars() {
        if ch.is_ascii_uppercase() && !result.is_empty() {
            if is_snake_case {
                result.push('_');
            } else {
                result.push('-');
            }
        };

        if ch == '_' || ch == '-' {
            if is_snake_case {
                result.push('_');
            } else {
                result.push('-');
            }
        } else {
            result.push(ch.to_ascii_lowercase());
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pascal_case() {
        assert_eq!("FooBar", pascal_or_camel_case("foo_bar", true));
        assert_eq!("FooBar", pascal_or_camel_case("fooBar", true));
        assert_eq!("FooBar", pascal_or_camel_case("foo-bar", true));
        assert_eq!("FooBar", pascal_or_camel_case("FooBar", true));
    }

    #[test]
    fn test_camel_case() {
        assert_eq!("fooBar", pascal_or_camel_case("foo_bar", false));
        assert_eq!("fooBar", pascal_or_camel_case("fooBar", false));
        assert_eq!("fooBar", pascal_or_camel_case("foo-bar", false));
        assert_eq!("fooBar", pascal_or_camel_case("FooBar", false));
    }

    #[test]
    fn test_snake_case() {
        assert_eq!("foo_bar", snake_or_kebab_case("foo_bar", true));
        assert_eq!("foo_bar", snake_or_kebab_case("fooBar", true));
        assert_eq!("foo_bar", snake_or_kebab_case("foo-bar", true));
        assert_eq!("foo_bar", snake_or_kebab_case("FooBar", true));
    }

    #[test]
    fn test_kebab_case() {
        assert_eq!("foo-bar", snake_or_kebab_case("foo_bar", false));
        assert_eq!("foo-bar", snake_or_kebab_case("fooBar", false));
        assert_eq!("foo-bar", snake_or_kebab_case("foo-bar", false));
        assert_eq!("foo-bar", snake_or_kebab_case("FooBar", false));
    }
}
