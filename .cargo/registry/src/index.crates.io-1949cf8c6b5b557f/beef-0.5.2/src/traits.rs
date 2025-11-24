pub(crate) use internal::Beef;
pub(crate) use internal::Capacity;

pub(crate) mod internal {
    use alloc::borrow::ToOwned;
    use alloc::string::String;
    use alloc::vec::Vec;
    use core::mem::ManuallyDrop;
    use core::ptr::{slice_from_raw_parts, NonNull};

    pub trait Capacity {
        type Field: Copy;
        type NonZero: Copy;

        fn len(fat: usize) -> usize;

        fn empty(len: usize) -> (usize, Self::Field);

        fn store(len: usize, capacity: usize) -> (usize, Self::Field);

        fn unpack(fat: usize, capacity: Self::NonZero) -> (usize, usize);

        fn maybe(fat: usize, capacity: Self::Field) -> Option<Self::NonZero>;
    }

    /// Helper trait required by `Cow<T>` to extract capacity of owned
    /// variant of `T`, and manage conversions.
    ///
    /// This can be only implemented on types that match requirements:
    ///
    /// + `T::Owned` has a `capacity`, which is an extra word that is absent in `T`.
    /// + `T::Owned` with `capacity` of `0` does not allocate memory.
    /// + `T::Owned` can be reconstructed from `*mut T` borrowed out of it, plus capacity.
    pub unsafe trait Beef: ToOwned {
        type PointerT;

        fn ref_into_parts<U>(&self) -> (NonNull<Self::PointerT>, usize, U::Field)
        where
            U: Capacity;

        unsafe fn ref_from_parts<U>(ptr: NonNull<Self::PointerT>, len: usize) -> *const Self
        where
            U: Capacity;

        /// Convert `T::Owned` to `NonNull<T>` and capacity.
        /// Return `None` for `0` capacity.
        fn owned_into_parts<U>(owned: Self::Owned) -> (NonNull<Self::PointerT>, usize, U::Field)
        where
            U: Capacity;

        /// Rebuild `T::Owned` from `NonNull<T>` and `capacity`. This can be done by the likes
        /// of [`Vec::from_raw_parts`](https://doc.rust-lang.org/std/vec/struct.Vec.html#method.from_raw_parts).
        unsafe fn owned_from_parts<U>(
            ptr: NonNull<Self::PointerT>,
            fat: usize,
            capacity: U::NonZero,
        ) -> Self::Owned
        where
            U: Capacity;
    }

    unsafe impl Beef for str {
        type PointerT = u8;

        #[inline]
        fn ref_into_parts<U>(&self) -> (NonNull<u8>, usize, U::Field)
        where
            U: Capacity,
        {
            let (fat, cap) = U::empty(self.len());

            // A note on soundness:
            //
            // We are casting *const T to *mut T, however for all borrowed values
            // this raw pointer is only ever dereferenced back to &T.
            (
                unsafe { NonNull::new_unchecked(self.as_ptr() as *mut u8) },
                fat,
                cap,
            )
        }

        #[inline]
        unsafe fn ref_from_parts<U>(ptr: NonNull<u8>, fat: usize) -> *const str
        where
            U: Capacity,
        {
            slice_from_raw_parts(ptr.as_ptr(), U::len(fat)) as *const str
        }

        #[inline]
        fn owned_into_parts<U>(owned: String) -> (NonNull<u8>, usize, U::Field)
        where
            U: Capacity,
        {
            // Convert to `String::into_raw_parts` once stabilized
            // We need to go through Vec here to get provenance for the entire allocation
            // instead of just the initialized parts.
            let mut owned = ManuallyDrop::new(owned.into_bytes());
            let (fat, cap) = U::store(owned.len(), owned.capacity());

            (
                unsafe { NonNull::new_unchecked(owned.as_mut_ptr()) },
                fat,
                cap,
            )
        }

        #[inline]
        unsafe fn owned_from_parts<U>(ptr: NonNull<u8>, fat: usize, capacity: U::NonZero) -> String
        where
            U: Capacity,
        {
            let (len, cap) = U::unpack(fat, capacity);

            String::from_utf8_unchecked(Vec::from_raw_parts(ptr.as_ptr(), len, cap))
        }
    }

    unsafe impl<T: Clone> Beef for [T] {
        type PointerT = T;

        #[inline]
        fn ref_into_parts<U>(&self) -> (NonNull<T>, usize, U::Field)
        where
            U: Capacity,
        {
            let (fat, cap) = U::empty(self.len());

            // A note on soundness:
            //
            // We are casting *const T to *mut T, however for all borrowed values
            // this raw pointer is only ever dereferenced back to &T.
            (
                unsafe { NonNull::new_unchecked(self.as_ptr() as *mut T) },
                fat,
                cap,
            )
        }

        #[inline]
        unsafe fn ref_from_parts<U>(ptr: NonNull<T>, fat: usize) -> *const [T]
        where
            U: Capacity,
        {
            slice_from_raw_parts(ptr.as_ptr(), U::len(fat))
        }

        #[inline]
        fn owned_into_parts<U>(owned: Vec<T>) -> (NonNull<T>, usize, U::Field)
        where
            U: Capacity,
        {
            // Convert to `Vec::into_raw_parts` once stabilized
            let mut owned = ManuallyDrop::new(owned);
            let (fat, cap) = U::store(owned.len(), owned.capacity());

            (
                unsafe { NonNull::new_unchecked(owned.as_mut_ptr()) },
                fat,
                cap,
            )
        }

        #[inline]
        unsafe fn owned_from_parts<U>(ptr: NonNull<T>, fat: usize, capacity: U::NonZero) -> Vec<T>
        where
            U: Capacity,
        {
            let (len, cap) = U::unpack(fat, capacity);

            Vec::from_raw_parts(ptr.as_ptr(), len, cap)
        }
    }
}
