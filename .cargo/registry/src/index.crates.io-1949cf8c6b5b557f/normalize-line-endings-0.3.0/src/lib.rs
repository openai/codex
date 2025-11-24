#![warn(missing_docs)]
//!
//! Normalize line endings
//!
//! This crate provides a `normalize` method that takes a char iterator and returns
//! a new one with `\n` for all line endings

/// This struct wraps a `std::io::Chars` to normalize line endings.
///
/// Implements `Iterator<Item=char>` so can be used in place
struct Normalized<I> {
    iter: I,
    prev_was_cr: bool,
}

/// Take a Chars and return similar struct with normalized line endings
///
/// # Example
/// ```
/// use std::iter::FromIterator;
/// use normalize_line_endings::normalized;
///
/// let input = "This is a string \n with \r some \n\r\n random newlines\r\r\n\n";
/// assert_eq!(
///     &String::from_iter(normalized(input.chars())),
///     "This is a string \n with \n some \n\n random newlines\n\n\n"
/// );
/// ```
#[inline]
pub fn normalized(iter: impl Iterator<Item = char>) -> impl Iterator<Item = char> {
    Normalized {
        iter,
        prev_was_cr: false,
    }
}

impl<I> Iterator for Normalized<I>
where
    I: Iterator<Item = char>,
{
    type Item = char;
    fn next(&mut self) -> Option<char> {
        match self.iter.next() {
            Some('\n') if self.prev_was_cr => {
                self.prev_was_cr = false;
                match self.iter.next() {
                    Some('\r') => {
                        self.prev_was_cr = true;
                        Some('\n')
                    }
                    any => {
                        self.prev_was_cr = false;
                        any
                    }
                }
            }
            Some('\r') => {
                self.prev_was_cr = true;
                Some('\n')
            }
            any => {
                self.prev_was_cr = false;
                any
            }
        }
    }
}

// tests
#[cfg(test)]
mod tests {
    use std::iter::FromIterator;

    #[test]
    fn normalized() {
        let input = "This is a string \n with \r some \n\r\n random newlines\r\r\n\n";
        assert_eq!(
            &String::from_iter(super::normalized(input.chars())),
            "This is a string \n with \n some \n\n random newlines\n\n\n"
        );
    }
}
