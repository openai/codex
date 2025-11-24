/// Creates a `CompactString` using interpolation of runtime expressions.
///
/// The first argument `format_compact!` receives is a format string.
/// This must be a string literal.
/// The power of the formatting string is in the `{}`s contained.
///
/// Additional parameters passed to `format_compact!` replace the `{}`s within
/// the formatting string in the order given unless named or
/// positional parameters are used; see [`std::fmt`] for more information.
///
/// A common use for `format_compact!` is concatenation and interpolation
/// of strings.
/// The same convention is used with [`print!`] and [`write!`] macros,
/// depending on the intended destination of the string.
///
/// To convert a single value to a string, use the
/// `ToCompactString::to_compact_string` method, which uses
/// the [`std::fmt::Display`] formatting trait.
///
/// # Panics
///
/// `format_compact!` panics if a formatting trait implementation returns
/// an error.
///
/// This indicates an incorrect implementation since
/// `ToCompactString::to_compact_string` never returns an error itself.
#[macro_export]
macro_rules! format_compact {
    ($($arg:tt)*) => {
        $crate::ToCompactString::to_compact_string(&$crate::core::format_args!($($arg)*))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_macros() {
        assert_eq!(format_compact!("2"), "2");
        assert_eq!(format_compact!("{}", 2), "2");

        assert!(!format_compact!("2").is_heap_allocated());
        assert!(!format_compact!("{}", 2).is_heap_allocated());
    }
}
