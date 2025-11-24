//! Empty symbolication strategy used to compile for platforms that have no
//! support.

use super::{BytesOrWideString, ResolveWhat, SymbolName};
use core::ffi::c_void;
use core::marker;

pub unsafe fn resolve(_addr: ResolveWhat<'_>, _cb: &mut dyn FnMut(&super::Symbol)) {}

pub struct Symbol<'a> {
    _marker: marker::PhantomData<&'a i32>,
}

impl Symbol<'_> {
    pub fn name(&self) -> Option<SymbolName<'_>> {
        None
    }

    pub fn addr(&self) -> Option<*mut c_void> {
        None
    }

    pub fn filename_raw(&self) -> Option<BytesOrWideString<'_>> {
        None
    }

    #[cfg(feature = "std")]
    pub fn filename(&self) -> Option<&::std::path::Path> {
        None
    }

    pub fn lineno(&self) -> Option<u32> {
        None
    }

    pub fn colno(&self) -> Option<u32> {
        None
    }
}

pub unsafe fn clear_symbol_cache() {}
