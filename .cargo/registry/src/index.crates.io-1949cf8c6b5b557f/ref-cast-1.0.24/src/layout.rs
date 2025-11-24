use core::mem;

#[doc(hidden)]
pub struct Layout<T: ?Sized>(T);

#[doc(hidden)]
pub trait LayoutUnsized<T: ?Sized> {
    const SIZE: usize = usize::MAX;
    const ALIGN: usize = usize::MAX;
}

impl<T: ?Sized> LayoutUnsized<T> for Layout<T> {}

impl<T> Layout<T> {
    pub const SIZE: usize = mem::size_of::<T>();
    pub const ALIGN: usize = mem::align_of::<T>();
}

#[doc(hidden)]
#[inline]
pub fn assert_layout<Outer: ?Sized, Inner: ?Sized>(
    name: &'static str,
    outer_size: usize,
    inner_size: usize,
    outer_align: usize,
    inner_align: usize,
) {
    if outer_size != inner_size {
        #[cfg(no_intrinsic_type_name)]
        panic!(
            "unexpected size in cast to {}: {} != {}",
            name, outer_size, inner_size,
        );
        #[cfg(not(no_intrinsic_type_name))]
        panic!(
            "unexpected size in cast from {} to {}: {} != {}",
            core::any::type_name::<Inner>(),
            core::any::type_name::<Outer>(),
            inner_size,
            outer_size,
        );
    }
    if outer_align != inner_align {
        #[cfg(no_intrinsic_type_name)]
        panic!(
            "unexpected alignment in cast to {}: {} != {}",
            name, outer_align, inner_align,
        );
        #[cfg(not(no_intrinsic_type_name))]
        panic!(
            "unexpected alignment in cast from {} to {}: {} != {}",
            core::any::type_name::<Inner>(),
            core::any::type_name::<Outer>(),
            inner_align,
            outer_align,
        );
    }
    #[cfg(not(no_intrinsic_type_name))]
    let _ = name;
}
