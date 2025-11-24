// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::*;

/// A wrapper around a type implementing [`fmt::Write`] that implements [`PartsWrite`].
#[derive(Debug)]
#[allow(clippy::exhaustive_structs)] // newtype
pub struct CoreWriteAsPartsWrite<W: fmt::Write + ?Sized>(pub W);

impl<W: fmt::Write + ?Sized> fmt::Write for CoreWriteAsPartsWrite<W> {
    #[inline]
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.0.write_str(s)
    }

    #[inline]
    fn write_char(&mut self, c: char) -> fmt::Result {
        self.0.write_char(c)
    }
}

impl<W: fmt::Write + ?Sized> PartsWrite for CoreWriteAsPartsWrite<W> {
    type SubPartsWrite = CoreWriteAsPartsWrite<W>;

    #[inline]
    fn with_part(
        &mut self,
        _part: Part,
        mut f: impl FnMut(&mut Self::SubPartsWrite) -> fmt::Result,
    ) -> fmt::Result {
        f(self)
    }
}

/// A [`Writeable`] that writes out the given part.
///
/// # Examples
///
/// ```
/// use writeable::adapters::WithPart;
/// use writeable::assert_writeable_parts_eq;
/// use writeable::Part;
///
/// // Simple usage:
///
/// const PART: Part = Part {
///     category: "foo",
///     value: "bar",
/// };
///
/// assert_writeable_parts_eq!(
///     WithPart {
///         writeable: "Hello World",
///         part: PART
///     },
///     "Hello World",
///     [(0, 11, PART)],
/// );
///
/// // Can be nested:
///
/// const PART2: Part = Part {
///     category: "foo2",
///     value: "bar2",
/// };
///
/// assert_writeable_parts_eq!(
///     WithPart {
///         writeable: WithPart {
///             writeable: "Hello World",
///             part: PART
///         },
///         part: PART2
///     },
///     "Hello World",
///     [(0, 11, PART), (0, 11, PART2)],
/// );
/// ```
#[derive(Debug)]
#[allow(clippy::exhaustive_structs)] // public adapter
pub struct WithPart<T: ?Sized> {
    pub part: Part,
    pub writeable: T,
}

impl<T: Writeable + ?Sized> Writeable for WithPart<T> {
    #[inline]
    fn write_to<W: fmt::Write + ?Sized>(&self, sink: &mut W) -> fmt::Result {
        self.writeable.write_to(sink)
    }

    #[inline]
    fn write_to_parts<W: PartsWrite + ?Sized>(&self, sink: &mut W) -> fmt::Result {
        sink.with_part(self.part, |w| self.writeable.write_to_parts(w))
    }

    #[inline]
    fn writeable_length_hint(&self) -> LengthHint {
        self.writeable.writeable_length_hint()
    }

    fn writeable_borrow(&self) -> Option<&str> {
        self.writeable.writeable_borrow()
    }

    #[inline]
    #[cfg(feature = "alloc")]
    fn write_to_string(&self) -> Cow<'_, str> {
        self.writeable.write_to_string()
    }
}

impl<T: Writeable + ?Sized> fmt::Display for WithPart<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Writeable::write_to(&self, f)
    }
}
