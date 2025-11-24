# Custom attributes

The `Copy` and `Clone` traits support the following attributes:

* **Container attributes**
    * [`<Copy or Clone>(bound="<where-clause or empty>")`](#custom-bound)
    * [`Clone(clone_from="true")`](#clone-from)
* **Field attributes**
    * [`<Copy or Clone>(bound="<where-clause or empty>")`](#custom-bound)

# `clone_from`

The [`Clone`] trait has a default implementation for [`clone_from`] and
`derive(Clone)` never implements that method. *derivative* can implement it if
asked explicitly.

Note that while the generated implementation is good for structures, it might
not be very efficient for enumerations. What it does is check if both `self`
and the clone-from value have the same variant, if they have, use `clone_from`
on the members, otherwise fallback to `*self = other.clone();`. Ask yourself if
you really need this.

# Custom bound
As most other traits, `Copy` and `Debug` support a custom bound on container
and fields. See [`Debug`'s documentation](Debug.md#custom-bound) for more
information.

# Limitations

*rustc* can optimize `derive(Clone, Copy)` to generate faster, smaller code.
So does *derivative*. But *rustc* does not know about `derivative(Copy)` and
would not optimize `#[derivative(Copy)] #[derive(Clone)]`.
To avoid that issue, you should avoid deriving `Clone` using *rustc*'s default
`derive` and `Copy` using `derivative`. *derivative* will error if it detects
that, but can't always do it.

[`Clone`]: https://doc.rust-lang.org/std/clone/trait.Clone.html
[`clone_from`]: https://doc.rust-lang.org/std/clone/trait.Clone.html#method.clone_from
