#[cfg(not(no_global_oom_handling))]
use alloc_crate::borrow::Cow;

use crate::stable::alloc::Allocator;

use super::Vec;

macro_rules! __impl_slice_eq1 {
    ([$($vars:tt)*] $lhs:ty, $rhs:ty $(where $ty:ty: $bound:ident)?) => {
        impl<T, U, $($vars)*> PartialEq<$rhs> for $lhs
        where
            T: PartialEq<U>,
            $($ty: $bound)?
        {
            #[inline(always)]
            fn eq(&self, other: &$rhs) -> bool { self[..] == other[..] }
            #[inline(always)]
            fn ne(&self, other: &$rhs) -> bool { self[..] != other[..] }
        }
    }
}

__impl_slice_eq1! { [A1: Allocator, A2: Allocator] Vec<T, A1>, Vec<U, A2> }
__impl_slice_eq1! { [A: Allocator] Vec<T, A>, &[U] }
__impl_slice_eq1! { [A: Allocator] Vec<T, A>, &mut [U] }
__impl_slice_eq1! { [A: Allocator] &[T], Vec<U, A> }
__impl_slice_eq1! { [A: Allocator] &mut [T], Vec<U, A> }
__impl_slice_eq1! { [A: Allocator] Vec<T, A>, [U]  }
__impl_slice_eq1! { [A: Allocator] [T], Vec<U, A>  }
#[cfg(not(no_global_oom_handling))]
__impl_slice_eq1! { [A: Allocator] Cow<'_, [T]>, Vec<U, A> where T: Clone }
__impl_slice_eq1! { [A: Allocator, const N: usize] Vec<T, A>, [U; N] }
__impl_slice_eq1! { [A: Allocator, const N: usize] Vec<T, A>, &[U; N] }

// NOTE: some less important impls are omitted to reduce code bloat
// FIXME(Centril): Reconsider this?
//__impl_slice_eq1! { [const N: usize] Vec<A>, &mut [B; N], }
//__impl_slice_eq1! { [const N: usize] [A; N], Vec<B>, }
//__impl_slice_eq1! { [const N: usize] &[A; N], Vec<B>, }
//__impl_slice_eq1! { [const N: usize] &mut [A; N], Vec<B>, }
//__impl_slice_eq1! { [const N: usize] Cow<'a, [A]>, [B; N], }
//__impl_slice_eq1! { [const N: usize] Cow<'a, [A]>, &[B; N], }
//__impl_slice_eq1! { [const N: usize] Cow<'a, [A]>, &mut [B; N], }
