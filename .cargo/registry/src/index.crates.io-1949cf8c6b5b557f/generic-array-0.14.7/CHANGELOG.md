* **`0.14.7`**
    * Backport [#133](https://github.com/fizyk20/generic-array/pull/133) and [#134](https://github.com/fizyk20/generic-array/pull/134)

* **`0.14.6`**
    * Add an optional `Zeroize` impl for `GenericArray` ([#126](https://github.com/fizyk20/generic-array/pull/126) and [#112](https://github.com/fizyk20/generic-array/pull/112))
    * Cleanup some unsafe ([#125](https://github.com/fizyk20/generic-array/pull/125)) and typos ([#114](https://github.com/fizyk20/generic-array/pull/114))
    * Use `include` in `Cargo.toml` to reduce package size

* **`0.14.5`**
    * Fix unsoundness behavior in `GenericArrayIter::clone` ([#120](https://github.com/fizyk20/generic-array/pull/120))

* **`0.14.4`**
    * Update `typenum` to `1.12.0`
    * Make `Drop` a no-op when the inner type does not require `Drop` (using `core::mem::needs_drop`)

* **`0.14.3`**
    * Improve behavior of `GenericArray::from_exact_iter` to assume `ExactIterator`s can lie.
    * Fix alignment of zero-length `GenericArray`s
    * Implement `From<&[T; N]> for &GenericArray<T, N>` and its mutable variant

* **`0.14.2`**
    * Lower MSRV to `1.36.0` without `From<[T; N]>` implementations.

* **`0.14.1`**
    * Fix element conversions in `arr!` macro.

* **`0.14.0`**
    * Replace `Into` implementations with the more general `From`.
        * Requires minumum Rust version of 1.41.0
    * Fix unsoundness in `arr!` macro.
    * Fix meta variable misuse
    * Fix Undefined Behavior across the crate by switching to `MaybeUninit`
    * Improve some documentation and doctests
    * Add `AsRef<[T; N]>` and `AsMut<[T; N]>` impls to `GenericArray<T, N>`
    * Add `Split` impl for `&GenericArray` and `&mut GenericArray`

* **`0.13.2`**
    * Add feature `more_lengths`, which adds more `From`/`Into` implementations for arrays of various lengths.

* **`0.13.1`**
    * Mark `GenericArray` as `#[repr(transparent)]`
    * Implement `Into<[T; N]>` for `GenericArray<T, N>` up to N=32

* **`0.13.0`**
    * Allow `arr!` to be imported with use syntax.
        * Requires minumum Rust version of 1.30.1

* **`0.12.2`**
    * Implement `FusedIterator` for `GenericArrayIter`

* **`0.12.1`**
    * Use internal iteration where possible and provide more efficient internal iteration methods.

* **`0.12.0`**
    * Allow trailing commas in `arr!` macro.
    * **BREAKING**: Serialize `GenericArray` using `serde` tuples, instead of variable-length sequences. This may not be compatible with old serialized data.

* **`0.11.0`**
    * **BREAKING** Redesign `GenericSequence` with an emphasis on use in generic type parameters.
    * Add `MappedGenericSequence` and `FunctionalSequence`
        * Implements optimized `map`, `zip` and `fold` for `GenericArray`, `&GenericArray` and `&mut GenericArray`
    * **BREAKING** Remove `map_ref`, `zip_ref` and `map_slice`
        * `map_slice` is now equivalent to `GenericArray::from_iter(slice.iter().map(...))`
* **`0.10.0`**
    * Add `GenericSequence`, `Lengthen`, `Shorten`, `Split` and `Concat` traits.
    * Redefine `transmute` to avert errors.
* **`0.9.0`**
    * Rewrite construction methods to be well-defined in panic situations, correctly dropping elements.
    * `NoDrop` crate replaced by `ManuallyDrop` as it became stable in Rust core.
    * Add optimized `map`/`map_ref` and `zip`/`zip_ref` methods to `GenericArray`
* **`0.8.0`**
    * Implement `AsRef`, `AsMut`, `Borrow`, `BorrowMut`, `Hash` for `GenericArray`
    * Update `serde` to `1.0`
    * Update `typenum`
    * Make macro `arr!` non-cloning
    * Implement `From<[T; N]>` up to `N=32`
    * Fix #45
* **`0.7.0`**
    * Upgrade `serde` to `0.9`
    * Make `serde` with `no_std`
    * Implement `PartialOrd`/`Ord` for `GenericArray`
* **`0.6.0`**
    * Fixed #30
    * Implement `Default` for `GenericArray`
    * Implement `LowerHex` and `UpperHex` for `GenericArray<u8, N>`
    * Use `precision` formatting field in hex representation
    * Add `as_slice`, `as_mut_slice`
    * Remove `GenericArray::new` in favor of `Default` trait
    * Add `from_slice` and `from_mut_slice`
    * `no_std` and `core` for crate.
* **`0.5.0`**
    * Update `serde`
    * remove `no_std` feature, fixed #19
* **`0.4.0`**
    * Re-export `typenum`
* **`0.3.0`**
    * Implement `IntoIter` for `GenericArray`
    * Add `map` method
    * Add optional `serde` (de)serialization support feature.
* **`< 0.3.0`**
    * Initial implementation in late 2015
