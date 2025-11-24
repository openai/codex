//! The fallback implementation of the [SharedLibrary
//! trait](../trait.SharedLibrary.html) that does nothing.

use crate::Segment as SegmentTrait;
use crate::SharedLibrary as SharedLibraryTrait;
use crate::{Bias, IterationControl, SharedLibraryId, Svma};

use std::ffi::OsStr;
use std::marker::PhantomData;
use std::usize;

/// An unsupported segment
#[derive(Debug)]
pub struct Segment<'a> {
    phantom: PhantomData<&'a SharedLibrary<'a>>,
}

impl<'a> SegmentTrait for Segment<'a> {
    type SharedLibrary = SharedLibrary<'a>;

    #[inline]
    fn name(&self) -> &str {
        unreachable!()
    }

    #[inline]
    fn stated_virtual_memory_address(&self) -> Svma {
        unreachable!()
    }

    #[inline]
    fn len(&self) -> usize {
        unreachable!()
    }
}

/// An iterator over Mach-O segments.
#[derive(Debug)]
pub struct SegmentIter<'a> {
    phantom: PhantomData<&'a SharedLibrary<'a>>,
}

impl<'a> Iterator for SegmentIter<'a> {
    type Item = Segment<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        None
    }
}

/// The fallback implementation of the [SharedLibrary
/// trait](../trait.SharedLibrary.html).
#[derive(Debug)]
pub struct SharedLibrary<'a> {
    phantom: PhantomData<&'a SharedLibrary<'a>>,
}

impl<'a> SharedLibraryTrait for SharedLibrary<'a> {
    type Segment = Segment<'a>;
    type SegmentIter = SegmentIter<'a>;

    #[inline]
    fn name(&self) -> &OsStr {
        unreachable!()
    }

    fn id(&self) -> Option<SharedLibraryId> {
        unreachable!()
    }

    fn segments(&self) -> Self::SegmentIter {
        SegmentIter {
            phantom: PhantomData,
        }
    }

    #[inline]
    fn virtual_memory_bias(&self) -> Bias {
        unreachable!()
    }

    fn each<F, C>(_f: F)
    where
        F: FnMut(&Self) -> C,
        C: Into<IterationControl>,
    {
    }
}
