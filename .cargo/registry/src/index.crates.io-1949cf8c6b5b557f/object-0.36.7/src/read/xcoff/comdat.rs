//! XCOFF doesn't support the COMDAT section.

use core::fmt::Debug;

use crate::read::{self, ComdatKind, ObjectComdat, ReadRef, Result, SectionIndex, SymbolIndex};
use crate::xcoff;

use super::{FileHeader, XcoffFile};

/// An iterator for the COMDAT section groups in a [`XcoffFile32`](super::XcoffFile32).
pub type XcoffComdatIterator32<'data, 'file, R = &'data [u8]> =
    XcoffComdatIterator<'data, 'file, xcoff::FileHeader32, R>;
/// An iterator for the COMDAT section groups in a [`XcoffFile64`](super::XcoffFile64).
pub type XcoffComdatIterator64<'data, 'file, R = &'data [u8]> =
    XcoffComdatIterator<'data, 'file, xcoff::FileHeader64, R>;

/// An iterator for the COMDAT section groups in a [`XcoffFile`].
///
/// This is a stub that doesn't implement any functionality.
#[derive(Debug)]
pub struct XcoffComdatIterator<'data, 'file, Xcoff, R = &'data [u8]>
where
    Xcoff: FileHeader,
    R: ReadRef<'data>,
{
    #[allow(unused)]
    pub(crate) file: &'file XcoffFile<'data, Xcoff, R>,
}

impl<'data, 'file, Xcoff, R> Iterator for XcoffComdatIterator<'data, 'file, Xcoff, R>
where
    Xcoff: FileHeader,
    R: ReadRef<'data>,
{
    type Item = XcoffComdat<'data, 'file, Xcoff, R>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        None
    }
}

/// A COMDAT section group in a [`XcoffFile32`](super::XcoffFile32).
pub type XcoffComdat32<'data, 'file, R = &'data [u8]> =
    XcoffComdat<'data, 'file, xcoff::FileHeader32, R>;

/// A COMDAT section group in a [`XcoffFile64`](super::XcoffFile64).
pub type XcoffComdat64<'data, 'file, R = &'data [u8]> =
    XcoffComdat<'data, 'file, xcoff::FileHeader64, R>;

/// A COMDAT section group in a [`XcoffFile`].
///
/// This is a stub that doesn't implement any functionality.
#[derive(Debug)]
pub struct XcoffComdat<'data, 'file, Xcoff, R = &'data [u8]>
where
    Xcoff: FileHeader,
    R: ReadRef<'data>,
{
    #[allow(unused)]
    file: &'file XcoffFile<'data, Xcoff, R>,
}

impl<'data, 'file, Xcoff, R> read::private::Sealed for XcoffComdat<'data, 'file, Xcoff, R>
where
    Xcoff: FileHeader,
    R: ReadRef<'data>,
{
}

impl<'data, 'file, Xcoff, R> ObjectComdat<'data> for XcoffComdat<'data, 'file, Xcoff, R>
where
    Xcoff: FileHeader,
    R: ReadRef<'data>,
{
    type SectionIterator = XcoffComdatSectionIterator<'data, 'file, Xcoff, R>;

    #[inline]
    fn kind(&self) -> ComdatKind {
        unreachable!();
    }

    #[inline]
    fn symbol(&self) -> SymbolIndex {
        unreachable!();
    }

    #[inline]
    fn name_bytes(&self) -> Result<&'data [u8]> {
        unreachable!();
    }

    #[inline]
    fn name(&self) -> Result<&'data str> {
        unreachable!();
    }

    #[inline]
    fn sections(&self) -> Self::SectionIterator {
        unreachable!();
    }
}

/// An iterator for the sections in a COMDAT section group in a [`XcoffFile32`](super::XcoffFile32).
pub type XcoffComdatSectionIterator32<'data, 'file, R = &'data [u8]> =
    XcoffComdatSectionIterator<'data, 'file, xcoff::FileHeader32, R>;
/// An iterator for the sections in a COMDAT section group in a [`XcoffFile64`](super::XcoffFile64).
pub type XcoffComdatSectionIterator64<'data, 'file, R = &'data [u8]> =
    XcoffComdatSectionIterator<'data, 'file, xcoff::FileHeader64, R>;

/// An iterator for the sections in a COMDAT section group in a [`XcoffFile`].
///
/// This is a stub that doesn't implement any functionality.
#[derive(Debug)]
pub struct XcoffComdatSectionIterator<'data, 'file, Xcoff, R = &'data [u8]>
where
    Xcoff: FileHeader,
    R: ReadRef<'data>,
{
    #[allow(unused)]
    file: &'file XcoffFile<'data, Xcoff, R>,
}

impl<'data, 'file, Xcoff, R> Iterator for XcoffComdatSectionIterator<'data, 'file, Xcoff, R>
where
    Xcoff: FileHeader,
    R: ReadRef<'data>,
{
    type Item = SectionIndex;

    fn next(&mut self) -> Option<Self::Item> {
        None
    }
}
