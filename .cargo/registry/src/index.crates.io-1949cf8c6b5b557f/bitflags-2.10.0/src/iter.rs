/*!
Yield the bits of a source flags value in a set of contained flags values.
*/

use crate::{Flag, Flags};

/**
An iterator over flags values.

This iterator will yield flags values for contained, defined flags first, with any remaining bits yielded
as a final flags value.
*/
pub struct Iter<B: 'static> {
    inner: IterNames<B>,
    done: bool,
}

impl<B: Flags> Iter<B> {
    pub(crate) fn new(flags: &B) -> Self {
        Iter {
            inner: IterNames::new(flags),
            done: false,
        }
    }
}

impl<B: 'static> Iter<B> {
    // Used by the `bitflags` macro
    #[doc(hidden)]
    pub const fn __private_const_new(flags: &'static [Flag<B>], source: B, remaining: B) -> Self {
        Iter {
            inner: IterNames::__private_const_new(flags, source, remaining),
            done: false,
        }
    }
}

impl<B: Flags> Iterator for Iter<B> {
    type Item = B;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next() {
            Some((_, flag)) => Some(flag),
            None if !self.done => {
                self.done = true;

                // After iterating through valid names, if there are any bits left over
                // then return one final value that includes them. This makes `into_iter`
                // and `from_iter` roundtrip
                if !self.inner.remaining().is_empty() {
                    Some(B::from_bits_retain(self.inner.remaining.bits()))
                } else {
                    None
                }
            }
            None => None,
        }
    }
}

/**
An iterator over flags values.

This iterator only yields flags values for contained, defined, named flags. Any remaining bits
won't be yielded, but can be found with the [`IterNames::remaining`] method.
*/
pub struct IterNames<B: 'static> {
    flags: &'static [Flag<B>],
    idx: usize,
    source: B,
    remaining: B,
}

impl<B: Flags> IterNames<B> {
    pub(crate) fn new(flags: &B) -> Self {
        IterNames {
            flags: B::FLAGS,
            idx: 0,
            remaining: B::from_bits_retain(flags.bits()),
            source: B::from_bits_retain(flags.bits()),
        }
    }
}

impl<B: 'static> IterNames<B> {
    // Used by the bitflags macro
    #[doc(hidden)]
    pub const fn __private_const_new(flags: &'static [Flag<B>], source: B, remaining: B) -> Self {
        IterNames {
            flags,
            idx: 0,
            remaining,
            source,
        }
    }

    /// Get a flags value of any remaining bits that haven't been yielded yet.
    ///
    /// Once the iterator has finished, this method can be used to
    /// check whether or not there are any bits that didn't correspond
    /// to a contained, defined, named flag remaining.
    pub fn remaining(&self) -> &B {
        &self.remaining
    }
}

impl<B: Flags> Iterator for IterNames<B> {
    type Item = (&'static str, B);

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(flag) = self.flags.get(self.idx) {
            // Short-circuit if our state is empty
            if self.remaining.is_empty() {
                return None;
            }

            self.idx += 1;

            // Skip unnamed flags
            if flag.name().is_empty() {
                continue;
            }

            let bits = flag.value().bits();

            // If the flag is set in the original source _and_ it has bits that haven't
            // been covered by a previous flag yet then yield it. These conditions cover
            // two cases for multi-bit flags:
            //
            // 1. When flags partially overlap, such as `0b00000001` and `0b00000101`, we'll
            // yield both flags.
            // 2. When flags fully overlap, such as in convenience flags that are a shorthand for others,
            // we won't yield both flags.
            if self.source.contains(B::from_bits_retain(bits))
                && self.remaining.intersects(B::from_bits_retain(bits))
            {
                self.remaining.remove(B::from_bits_retain(bits));

                return Some((flag.name(), B::from_bits_retain(bits)));
            }
        }

        None
    }
}

/**
An iterator over all defined named flags.

This iterator will yield flags values for all defined named flags, regardless of
whether they are contained in a particular flags value.
*/
pub struct IterDefinedNames<B: 'static> {
    flags: &'static [Flag<B>],
    idx: usize,
}

impl<B: Flags> IterDefinedNames<B> {
    pub(crate) fn new() -> Self {
        IterDefinedNames {
            flags: B::FLAGS,
            idx: 0,
        }
    }
}

impl<B: Flags> Iterator for IterDefinedNames<B> {
    type Item = (&'static str, B);

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(flag) = self.flags.get(self.idx) {
            self.idx += 1;

            // Only yield named flags
            if flag.is_named() {
                return Some((flag.name(), B::from_bits_retain(flag.value().bits())));
            }
        }

        None
    }
}
