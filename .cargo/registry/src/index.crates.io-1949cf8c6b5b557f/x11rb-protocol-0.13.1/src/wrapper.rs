//! Helpers for the generated code

use super::x11_utils::TryParse;
use core::marker::PhantomData;

/// Iterator implementation used by [GetPropertyReply].
///
/// This is the actual type returned by [GetPropertyReply::value8], [GetPropertyReply::value16],
/// and [GetPropertyReply::value32]. This type needs to be public due to Rust's visibility rules.
///
/// [GetPropertyReply]: crate::protocol::xproto::GetPropertyReply
/// [GetPropertyReply::value8]: crate::protocol::xproto::GetPropertyReply::value8
/// [GetPropertyReply::value16]: crate::protocol::xproto::GetPropertyReply::value16
/// [GetPropertyReply::value32]: crate::protocol::xproto::GetPropertyReply::value32
#[derive(Debug, Clone)]
pub struct PropertyIterator<'a, T>(&'a [u8], PhantomData<T>);

impl<'a, T> PropertyIterator<'a, T> {
    pub(crate) fn new(value: &'a [u8]) -> Self {
        PropertyIterator(value, PhantomData)
    }
}

impl<T> Iterator for PropertyIterator<'_, T>
where
    T: TryParse,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        match T::try_parse(self.0) {
            Ok((value, remaining)) => {
                self.0 = remaining;
                Some(value)
            }
            Err(_) => {
                self.0 = &[];
                None
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let size = self.0.len() / core::mem::size_of::<T>();
        (size, Some(size))
    }
}

impl<T: TryParse> core::iter::FusedIterator for PropertyIterator<'_, T> {}

#[cfg(test)]
mod tests {
    use super::PropertyIterator;
    use alloc::vec::Vec;

    #[test]
    fn test_parse_u8() {
        let input = [0u8, 1, 2, 3, 4, 5];
        let output = PropertyIterator::new(&input).collect::<Vec<u8>>();
        assert_eq!(&input[..], output);
    }

    #[test]
    fn test_parse_u32() {
        let expected = [0u32, 1, 2, 3, 4, 5];
        let input = {
            let mut input = Vec::new();
            for value in &expected {
                input.extend_from_slice(&value.to_ne_bytes());
            }
            input
        };

        let output = PropertyIterator::new(&input).collect::<Vec<u32>>();
        assert_eq!(&expected[..], output);
    }

    #[test]
    fn test_size_hint() {
        let hint = PropertyIterator::<u32>::new(&[0; 0]).size_hint();
        assert_eq!(hint, (0, Some(0)));

        let hint = PropertyIterator::<u32>::new(&[0; 8]).size_hint();
        assert_eq!(hint, (2, Some(2)));

        // In this case, the data is not an exact multiple of the element size
        let hint = PropertyIterator::<u32>::new(&[0; 30]).size_hint();
        assert_eq!(hint, (7, Some(7)));
    }
}
