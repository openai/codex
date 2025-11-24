// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! Types to enable polymorphic carts.

use crate::CloneableCart;

use core::ops::Deref;
use stable_deref_trait::StableDeref;

/// A cart that can be one type or the other. Enables ergonomic polymorphic carts.
///
/// `EitherCart` enables yokes originating from different data sources and therefore
/// having different cart types to be merged into the same yoke type, but still being
/// able to recover the original cart type if necessary.
///
/// All relevant Cart traits are implemented for `EitherCart`, and carts can be
/// safely wrapped in an `EitherCart`.
///
/// Also see [`Yoke::erase_box_cart()`](crate::Yoke::erase_box_cart).
///
/// # Examples
///
/// ```
/// use std::rc::Rc;
/// use yoke::either::EitherCart;
/// use yoke::Yoke;
///
/// let y1: Yoke<&'static str, Rc<str>> =
///     Yoke::attach_to_zero_copy_cart("reference counted hello world".into());
///
/// let y2: Yoke<&'static str, &str> = Yoke::attach_to_zero_copy_cart("borrowed hello world");
///
/// type CombinedYoke<'a> = Yoke<&'static str, EitherCart<Rc<str>, &'a str>>;
///
/// // Both yokes can be combined into a single yoke type despite different carts
/// let y3: CombinedYoke = y1.wrap_cart_in_either_a();
/// let y4: CombinedYoke = y2.wrap_cart_in_either_b();
///
/// assert_eq!(*y3.get(), "reference counted hello world");
/// assert_eq!(*y4.get(), "borrowed hello world");
///
/// // The resulting yoke is cloneable if both cart types implement CloneableCart
/// let y5 = y4.clone();
/// assert_eq!(*y5.get(), "borrowed hello world");
/// ```
#[derive(Clone, PartialEq, Eq, Debug)]
#[allow(clippy::exhaustive_enums)] // stable
pub enum EitherCart<C0, C1> {
    A(C0),
    B(C1),
}

impl<C0, C1, T> Deref for EitherCart<C0, C1>
where
    C0: Deref<Target = T>,
    C1: Deref<Target = T>,
    T: ?Sized,
{
    type Target = T;
    fn deref(&self) -> &T {
        use EitherCart::*;
        match self {
            A(a) => a.deref(),
            B(b) => b.deref(),
        }
    }
}

// Safety: Safe because both sub-types implement the trait.
unsafe impl<C0, C1, T> StableDeref for EitherCart<C0, C1>
where
    C0: StableDeref,
    C1: StableDeref,
    C0: Deref<Target = T>,
    C1: Deref<Target = T>,
    T: ?Sized,
{
}

// Safety: Safe because both sub-types implement the trait.
unsafe impl<C0, C1> CloneableCart for EitherCart<C0, C1>
where
    C0: CloneableCart,
    C1: CloneableCart,
{
}
