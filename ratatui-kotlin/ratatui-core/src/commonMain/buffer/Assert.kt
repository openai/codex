/// Assert that two buffers are equal by comparing their areas and content.
///
/// # Panics
/// When the buffers differ this method panics and displays the differences similar to
/// `assert_eq!()`.
#[deprecated = "use `assert_eq!(&actual, &expected)`"]
#[macro_export]
macro_rules! assert_buffer_eq {
    ($actual_expr:expr, $expected_expr:expr) => {
        match (&$actual_expr, &$expected_expr) {
            (actual, expected) => {
                assert!(
                    actual.area == expected.area,
                    "buffer areas not equal\nexpected: {expected:?}\nactual:   {actual:?}",
                );
                let nice_diff = expected
                    .diff(actual)
                    .into_iter()
                    .enumerate()
                    .map(|(i, (x, y, cell))| {
                        let expected_cell = &expected[(x, y)];
                        ::alloc::format!("{i}: at ({x}, {y})\n  expected: {expected_cell:?}\n  actual:   {cell:?}")
                    })
                    .collect::<::alloc::vec::Vec<::alloc::string::String>>()
                    .join("\n");
                assert!(
                    nice_diff.is_empty(),
                    "buffer contents not equal\nexpected: {expected:?}\nactual:   {actual:?}\ndiff:\n{nice_diff}",
                );
                // shouldn't get here, but this guards against future behavior
                // that changes equality but not area or content
                assert_eq!(
                    actual, expected,
                    "buffers are not equal in an unexpected way. Please open an issue about this."
                );
            }
        }
    };
}

#[expect(deprecated)]
#[cfg(test)]
mod tests {
    use crate::buffer::Buffer;
    use crate::layout::Rect;
    use crate::style::{Color, Style};

    #[test]
    fn assert_buffer_eq_does_not_panic_on_equal_buffers() {
        let buffer = Buffer::empty(Rect::new(0, 0, 5, 1));
        let other_buffer = Buffer::empty(Rect::new(0, 0, 5, 1));
        assert_buffer_eq!(buffer, other_buffer);
    }

    #[should_panic = "buffer areas not equal"]
    #[test]
    fn assert_buffer_eq_panics_on_unequal_area() {
        let buffer = Buffer::empty(Rect::new(0, 0, 5, 1));
        let other_buffer = Buffer::empty(Rect::new(0, 0, 6, 1));
        assert_buffer_eq!(buffer, other_buffer);
    }

    #[should_panic = "buffer contents not equal"]
    #[test]
    fn assert_buffer_eq_panics_on_unequal_style() {
        let buffer = Buffer::empty(Rect::new(0, 0, 5, 1));
        let mut other_buffer = Buffer::empty(Rect::new(0, 0, 5, 1));
        other_buffer.set_string(0, 0, " ", Style::default().fg(Color::Red));
        assert_buffer_eq!(buffer, other_buffer);
    }
}
