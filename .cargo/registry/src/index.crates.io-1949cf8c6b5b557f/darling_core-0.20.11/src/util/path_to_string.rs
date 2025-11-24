/// Transform Rust paths to a readable and comparable string.
///
/// # Limitations
/// * Leading colons are ignored.
/// * Angle brackets and `as` elements are ignored.
///
/// # Example
/// ```rust
/// # use darling_core::util::path_to_string;
/// # use syn::parse_quote;
/// assert_eq!(path_to_string(&parse_quote!(a::b)), "a::b");
/// ```
pub fn path_to_string(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|s| s.ident.to_string())
        .collect::<Vec<String>>()
        .join("::")
}

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::path_to_string;

    #[test]
    fn simple_ident() {
        assert_eq!(path_to_string(&parse_quote!(a)), "a");
    }

    #[test]
    fn simple_path() {
        assert_eq!(path_to_string(&parse_quote!(a::b)), "a::b");
    }
}
