// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::Yoke;
use crate::Yokeable;

use core::ops::Deref;
use stable_deref_trait::StableDeref;

use crate::ZeroFrom;

impl<Y, C> Yoke<Y, C>
where
    Y: for<'a> Yokeable<'a>,
    for<'a> <Y as Yokeable<'a>>::Output: ZeroFrom<'a, <C as Deref>::Target>,
    C: StableDeref + Deref,
    <C as Deref>::Target: 'static,
{
    /// Construct a [`Yoke`]`<Y, C>` from a cart implementing `StableDeref` by zero-copy cloning
    /// the cart to `Y` and then yokeing that object to the cart.
    ///
    /// The type `Y` must implement [`ZeroFrom`]`<C::Target>`. This trait is auto-implemented
    /// on many common types and can be custom implemented or derived in order to make it easier
    /// to construct a `Yoke`.
    ///
    /// # Example
    ///
    /// Attach to a cart:
    ///
    /// ```
    /// use std::borrow::Cow;
    /// use yoke::Yoke;
    ///
    /// let yoke = Yoke::<Cow<'static, str>, String>::attach_to_zero_copy_cart(
    ///     "demo".to_owned(),
    /// );
    ///
    /// assert_eq!("demo", yoke.get());
    /// ```
    pub fn attach_to_zero_copy_cart(cart: C) -> Self {
        Yoke::<Y, C>::attach_to_cart(cart, |c| <Y as Yokeable>::Output::zero_from(c))
    }
}
