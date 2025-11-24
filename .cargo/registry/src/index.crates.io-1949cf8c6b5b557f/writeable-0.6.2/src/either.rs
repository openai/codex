// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::*;
use ::either::Either;

/// A [`Writeable`] impl that delegates to one type or another type.
impl<W0, W1> Writeable for Either<W0, W1>
where
    W0: Writeable,
    W1: Writeable,
{
    fn write_to<W: fmt::Write + ?Sized>(&self, sink: &mut W) -> fmt::Result {
        match self {
            Either::Left(w) => w.write_to(sink),
            Either::Right(w) => w.write_to(sink),
        }
    }

    fn write_to_parts<S: PartsWrite + ?Sized>(&self, sink: &mut S) -> fmt::Result {
        match self {
            Either::Left(w) => w.write_to_parts(sink),
            Either::Right(w) => w.write_to_parts(sink),
        }
    }

    fn writeable_length_hint(&self) -> LengthHint {
        match self {
            Either::Left(w) => w.writeable_length_hint(),
            Either::Right(w) => w.writeable_length_hint(),
        }
    }

    fn writeable_borrow(&self) -> Option<&str> {
        match self {
            Either::Left(w) => w.writeable_borrow(),
            Either::Right(w) => w.writeable_borrow(),
        }
    }

    #[cfg(feature = "alloc")]
    fn write_to_string(&self) -> Cow<'_, str> {
        match self {
            Either::Left(w) => w.write_to_string(),
            Either::Right(w) => w.write_to_string(),
        }
    }
}
