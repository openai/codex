pub(crate) use self::inner::{do_alloc, Allocator, Global};

#[cfg(feature = "nightly")]
mod inner {
    use crate::alloc::alloc::Layout;
    pub use crate::alloc::alloc::{Allocator, Global};
    use core::ptr::NonNull;

    #[allow(clippy::map_err_ignore)]
    pub fn do_alloc<A: Allocator>(alloc: &A, layout: Layout) -> Result<NonNull<u8>, ()> {
        match alloc.allocate(layout) {
            Ok(ptr) => Ok(ptr.as_non_null_ptr()),
            Err(_) => Err(()),
        }
    }

    #[cfg(feature = "bumpalo")]
    unsafe impl Allocator for crate::BumpWrapper<'_> {
        #[inline]
        fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, core::alloc::AllocError> {
            match self.0.try_alloc_layout(layout) {
                Ok(ptr) => Ok(NonNull::slice_from_raw_parts(ptr, layout.size())),
                Err(_) => Err(core::alloc::AllocError),
            }
        }
        #[inline]
        unsafe fn deallocate(&self, _ptr: NonNull<u8>, _layout: Layout) {}
    }
}

#[cfg(not(feature = "nightly"))]
mod inner {
    use crate::alloc::alloc::{alloc, dealloc, Layout};
    use core::ptr::NonNull;

    #[allow(clippy::missing_safety_doc)] // not exposed outside of this crate
    pub unsafe trait Allocator {
        fn allocate(&self, layout: Layout) -> Result<NonNull<u8>, ()>;
        unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout);
    }

    #[derive(Copy, Clone)]
    pub struct Global;
    unsafe impl Allocator for Global {
        #[inline]
        fn allocate(&self, layout: Layout) -> Result<NonNull<u8>, ()> {
            unsafe { NonNull::new(alloc(layout)).ok_or(()) }
        }
        #[inline]
        unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
            dealloc(ptr.as_ptr(), layout);
        }
    }
    impl Default for Global {
        #[inline]
        fn default() -> Self {
            Global
        }
    }

    pub fn do_alloc<A: Allocator>(alloc: &A, layout: Layout) -> Result<NonNull<u8>, ()> {
        alloc.allocate(layout)
    }

    #[cfg(feature = "bumpalo")]
    unsafe impl Allocator for crate::BumpWrapper<'_> {
        #[allow(clippy::map_err_ignore)]
        fn allocate(&self, layout: Layout) -> Result<NonNull<u8>, ()> {
            self.0.try_alloc_layout(layout).map_err(|_| ())
        }
        unsafe fn deallocate(&self, _ptr: NonNull<u8>, _layout: Layout) {}
    }
}
