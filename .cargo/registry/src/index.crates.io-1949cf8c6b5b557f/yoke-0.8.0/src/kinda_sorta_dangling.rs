// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use core::mem::{ManuallyDrop, MaybeUninit};
use core::ops::{Deref, DerefMut};

/// This type is intended to be similar to the type `MaybeDangling<T>`
/// proposed in [RFC 3336].
///
/// The effect of this is that in Rust's safety model, types inside here are not
/// expected to have any memory dependent validity properties (`dereferenceable`, `noalias`).
///
/// See [#3696] for a testcase where `Yoke` fails under miri's field-retagging mode if not using
/// KindaSortaDangling.
///
/// This has `T: 'static` since we don't need anything
/// else and we don't want to have to think (more) about variance over lifetimes or dropck.
///
/// After [RFC 3336] lands we can use `MaybeDangling` instead.
///
/// Note that a version of this type also exists publicly as the [`maybe_dangling`]
/// crate; which also exports a patched `ManuallyDrop` with similar semantics and
/// does not require `T: 'static`. Consider using this if you need something more general
/// and are okay with adding dependencies.
///
/// [RFC 3336]: https://github.com/rust-lang/rfcs/pull/3336
/// [#3696]: https://github.com/unicode-org/icu4x/issues/3696
/// [`maybe_dangling`](https://docs.rs/maybe-dangling/0.1.0/maybe_dangling/struct.MaybeDangling.html)
#[repr(transparent)]
pub(crate) struct KindaSortaDangling<T: 'static> {
    /// Safety invariant: This is always an initialized T, never uninit or other
    /// invalid bit patterns. Its drop glue will execute during Drop::drop rather than
    /// during the drop glue for KindaSortaDangling, which means that we have to be careful about
    /// not touching the values as initialized during `drop` after that, but that's a short period of time.
    dangle: MaybeUninit<T>,
}

impl<T: 'static> KindaSortaDangling<T> {
    #[inline]
    pub(crate) const fn new(dangle: T) -> Self {
        KindaSortaDangling {
            dangle: MaybeUninit::new(dangle),
        }
    }
    #[inline]
    pub(crate) fn into_inner(self) -> T {
        // Self has a destructor, we want to avoid having it be called
        let manual = ManuallyDrop::new(self);
        // Safety:
        // We can call assume_init_read() due to the library invariant on this type,
        // however since it is a read() we must be careful about data duplication.
        // The only code using `self` after this is the drop glue, which we have disabled via
        // the ManuallyDrop.
        unsafe { manual.dangle.assume_init_read() }
    }
}

impl<T: 'static> Deref for KindaSortaDangling<T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &T {
        // Safety: Due to the safety invariant on `dangle`, it is guaranteed to be always
        // initialized as deref is never called during drop.
        unsafe { self.dangle.assume_init_ref() }
    }
}

impl<T: 'static> DerefMut for KindaSortaDangling<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        // Safety: Due to the safety invariant on `dangle`, it is guaranteed to be always
        // initialized as deref_mut is never called during drop.
        unsafe { self.dangle.assume_init_mut() }
    }
}

impl<T: 'static> Drop for KindaSortaDangling<T> {
    #[inline]
    fn drop(&mut self) {
        // Safety: We are reading and dropping a valid initialized T.
        //
        // As `drop_in_place()` is a `read()`-like duplication operation we must be careful that the original value isn't
        // used afterwards. It won't be because this is drop and the only
        // code that will run after this is `self`'s drop glue, and that drop glue is empty
        // because MaybeUninit has no drop.
        //
        // We use `drop_in_place()` instead of `let _ = ... .assume_init_read()` to avoid creating a move
        // of the inner `T` (without `KindaSortaDangling` protection!) type into a local -- we don't want to
        // assert any of `T`'s memory-related validity properties here.
        unsafe {
            self.dangle.as_mut_ptr().drop_in_place();
        }
    }
}
