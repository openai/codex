pub mod normalized_name;
pub mod normalized_tags;
pub mod normalized_unit;

pub use normalized_name::normalize_name;
pub use normalized_tags::normalize_tags;
pub use normalized_unit::normalize_unit;

pub fn truncate(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        None => s,
        Some((i, _)) => &s[..i],
    }
}

#[cfg(test)]
mod test {

    #[test]
    fn test_truncate_ascii_chars() {
        assert_eq!("abc", super::truncate("abcde", 3));
    }

    #[test]
    fn test_truncate_unicode_chars() {
        assert_eq!("😀😀😀", super::truncate("😀😀😀😀😀", 3));
    }
}
