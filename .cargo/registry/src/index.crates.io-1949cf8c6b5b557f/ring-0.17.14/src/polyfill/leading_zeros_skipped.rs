use core::iter::Peekable;

/// An iterator that skips all leading zeros.
///
/// When the wrapped iterator is all zeros, then the last item is retained.
pub struct LeadingZerosStripped<I>
where
    I: Iterator,
{
    inner: Peekable<I>,
}

impl<I> Clone for LeadingZerosStripped<I>
where
    I: Iterator,
    Peekable<I>: Clone,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<I> LeadingZerosStripped<I>
where
    I: ExactSizeIterator<Item = u8>,
{
    pub fn new(inner: I) -> Self {
        let mut len = inner.len();
        let mut inner = inner.peekable();
        // Strip all leading zeroes, but don't strip the last byte if all bytes
        // were zero.
        while len > 1 && inner.next_if_eq(&0).is_some() {
            len -= 1;
        }
        Self { inner }
    }
}

impl<I> Iterator for LeadingZerosStripped<I>
where
    I: Iterator,
{
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<I> ExactSizeIterator for LeadingZerosStripped<I> where I: ExactSizeIterator {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_leading_zeroes_stripped() {
        static TEST_CASES: &[(&[u8], &[u8])] = &[
            (&[], &[]),
            (&[0], &[0]),
            (&[0, 1], &[1]),
            (&[0, 0, 1], &[1]),
            (&[0, 0, 0, 1], &[1]),
            (&[1, 0], &[1, 0]),
            (&[0, 1, 0], &[1, 0]),
        ];
        TEST_CASES.iter().copied().for_each(|(input, expected)| {
            let stripped = LeadingZerosStripped::new(input.iter().copied());
            super::super::test::assert_iterator(stripped, expected);
        });
    }
}
