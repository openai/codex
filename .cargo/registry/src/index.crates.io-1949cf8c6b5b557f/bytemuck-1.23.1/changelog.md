# `bytemuck` changelog

## 1.23.1

* Added a windows-only `ZeroableInOption` impl for "stdcall" functions.

## 1.23

* `impl_core_error` crate feature adds `core::error::Error` impl.
* More `ZeroableInOption` impls.

## 1.22

* Add the `pod_saturating` feature, which adds `Pod` impls for `Saturating<T>`
  when `T` is already `Pod`.
* A bump in the minimum `bytemuck_derive` dependency from 1.4.0 to 1.4.1 to
  avoid a bug if you have a truly ancient `cargo.lock` file sitting around.
* Adds `Send` and `Sync` impls to `BoxBytes`.

## 1.21

* Implement `Pod` and `Zeroable` for `core::arch::{x86, x86_64}::__m512`, `__m512d` and `__m512i` without nightly.
  Requires Rust 1.72, and is gated through the `avx512_simd` cargo feature.
* Allow the use of `must_cast_mut` and `must_cast_slice_mut` in const contexts.
  Requires Rust 1.83, and is gated through the `must_cast_extra` cargo feature.
* internal: introduced the `maybe_const_fn` macro that allows defining some function
  to be const depending upon some `cfg` predicate.

## 1.20

* New functions to allocate zeroed `Arc` and `Rc`. Requires Rust 1.82
* `TransparentWrapper` impls for `core::cmp::Reverse` and `core::num::Saturating`.
* internal: Simplified the library's `fill_zeroes` calls to `write_bytes`

## 1.19

* Adds the `#[track_caller]` attribute to functions which may panic.

## 1.18

* Adds the `latest_stable_rust` cargo feature, which is a blanket feature that turns all other features on that are both sound and compatible with Stable rust.

## 1.17.1

* Adds `#[repr(C)]` to the `union Transmute<A, B>` type that's used internally
  for most of the transmutations.

## 1.17.0

* Makes the `must_cast` versions of the by-value and by-ref casts be `const`.
  The mut ref cast is unaffected for now (mut references aren't yet stable in `const fn`).
  This increases the MSRV of using that particular feature from 1.57 to 1.64.

## 1.16.3

* Fully described in https://github.com/Lokathor/bytemuck/pull/256, This makes
  casting slices to/from ZST elements more consistent between the crate's core
  module and other modules.

## 1.16.2

* Fixes potential UB where `BoxBytes` could attempt to free a dangling pointer
  if the `Layout` is zero sized. This type was introduced in 1.14.1, so that
  version and the others up to and including 1.16.1 are now yanked for safety.

## 1.16.1

* **NOT SEMVER SUPPORTED:** Adds the  `nightly_float` Cargo feature. This
  activates the `f16` and `f128` nightly features, and then provides `Zeroable`
  and `Pod` impls for those types.

## 1.16.0

* Adds a `const_zeroed` feature (MSRV 1.75) which puts a `zeroed` fn at the crate root.
  This is just like the `Zeroable::zeroed` method, but as a `const fn`.

## 1.15.0

This primarily relaxes the bounds on a `From` impl.

Previously:

> `impl<T: NoUninit> From<Box<T>> for BoxBytes`

Now:

> `impl<T: ?Sized + sealed::BoxBytesOf> From<Box<T>> for BoxBytes`

All related functions and methods are similarly updated.

We believe this to be backwards compatible with all previous uses,
and now `BoxBytes` can be converted to/from more types than before.

## 1.14.3

* The new std simd nightly features are apparently arch-specific.
  This adjusts the feature activation to be x86/ x86_64 only.

## 1.14.2

* Changes the name of the Nightly feature activated by the crate's
  `nightly_stdsimd` feature. This is needed as of (approximately) Nightly
  2024-02-06 and later, because the Nightly feature was changed.

## 1.14.1

* docs clarifications.

## 1.14

* `write_zeroes` and `fill_zeroes` functions: Writes (to one) or fills (a slice)
  zero bytes to all bytes covered by the provided reference. If your type has
  padding, this will even zero out the padding bytes.
* `align_offset` feature: causes pointer alignment checks to use the
  `align_offset` pointer method rather than as-casting the pointer to `usize`.
  This *may* improve codegen, if the compiler would have otherwise thought that
  the pointer address escaped. No formal benchmarks have been done either way.
* `must_cast` feature: Adds `must_*` family of functions. These functions will
  fail to compile if the cast requested can't be statically known to succeed.
  The error messages can be kinda bad when this happens, but eliminating the
  possibility of a runtime error might be worth it to you.

## 1.13.1

* Remove the requirement for the *source* data type to be `AnyBitPattern` on
  `pod_collect_to_vec`, allowing you to pod collect vecs of `char` into vecs of
  `u32`, or whatever.

## 1.13

* Now depends on `bytemuck_derive-1.4.0`
* Various small enhancements that would have been patch version updates, but
  which have been rolled into this minor version update.

## 1.12.4

* This has additional impls for existing traits and cleans up some internal code,
  but there's no new functions so I guess it counts as just a patch release.

## 1.12.3

* This bugfix makes the crate do stuff with `Arc` or not based on the
  `target_has_atomic` config. Previously, some targets that have allocation but
  not atomics were getting errors. This raises the MSRV of the
  `extern_crate_alloc` feature to 1.60, but opt-in features are *not* considered
  to be hard locked to 1.34 like the basic build of the crate is.

## 1.12.2

* Fixes `try_pod_read_unaligned` bug that made it always fail unless the target
  type was exactly pointer sized in which case UB *could* happen. The
  `CheckedBitPattern::is_valid_bit_pattern` was being asked to check that a
  *reference* to the `pod` value was a valid bit pattern, rather than the actual
  bit pattern itself, and so the check could in some cases be illegally
  bypassed.

## 1.12.1

* Patch bumped the required `bytemuck_derive` version because of a regression in
  how it handled `align(N)` attributes.

## 1.12

* This minor version bump is caused by a version bump in our `bytemuck_derive`
  dependency, which is in turn caused by a mixup in the minimum version of `syn`
  that `bytemuck_derive` uses. See [Issue
  122](https://github.com/Lokathor/bytemuck/issues/122). There's not any
  specific "new" API as you might normally expect from a minor version bump.
* [pali](https://github.com/pali6) fixed a problem with SPIR-V builds being
  broken. The error handling functions were trying to be generic over `Display`,
  which the error types normally support, except on SPIR-V targets (which run on
  the GPU and don't have text formatting).

## 1.11

* [WaffleLapkin](https://github.com/WaffleLapkin) added `wrap_box` and `peel_box`
  to the `TransparentWrapperAlloc` trait. Default impls of these functions are
  provided, and (as usual with the transparent trait stuff) you should not override
  the default versions.

## 1.10

* [TheEdward162](https://github.com/TheEdward162) added the `ZeroableInOption`
  and `PodInOption` traits. These are for types that are `Zeroable` or `Pod`
  *when in an option*, but not on their own. We provide impls for the various
  "NonZeroINTEGER" types in `core`, and if you need to newtype a NonZero value
  then you can impl these traits when you use `repr(transparent)`.

## 1.9.1

* Bumped the minimum `bytemuck_derive` dependency version from `1.0` to `1.1`.
  The fact that `bytemuck` and `bytemuck_derive` are separate crates at all is
  an unfortunate technical limit of current Rust, woe and calamity.

## 1.9.0

* [fu5ha](https://github.com/fu5ha) added the `NoUninit`, `AnyBitPattern`, and
  `CheckedBitPattern` traits. This allows for a more fine-grained level of
  detail in what casting operations are allowed for a type. Types that already
  implement `Zeroable` and `Pod` will have a blanket impl for these new traits.
  This is a "preview" of the direction that the crate will probably go in the
  eventual 2.0 version. We're still waiting on [Project Safe
  Transmute](https://github.com/rust-lang/project-safe-transmute) for an actual
  2.0 version of the crate, but until then please enjoy this preview.
* Also Fusha added better support for `union` types in the derive macros. I
  still don't know how any of the proc-macro stuff works at all, so please
  direct questions to her.

## 1.8.0

* `try_pod_read_unaligned` and `pod_read_unaligned` let you go from `&[u8]` to
  `T:Pod` without worrying about alignment.

## 1.7.3

* Experimental support for the `portable_simd` language extension under the
  `nightly_portable_simd` cargo feature. As the name implies, this is an
  experimental crate feature and it's **not** part of the semver contract. All
  it does is add the appropriate `Zeroable` and `Pod` impls.

## 1.7.2

* Why does this repo keep being hit with publishing problems? What did I do to
  deserve this curse, Ferris? This doesn't ever happen with tinyvec or fermium,
  only bytemuck.

## 1.7.1

* **Soundness Fix:** The wrap/peel methods for owned value conversion, added to
  `TransparentWrapper` in 1.6, can cause a double-drop if used with types that
  impl `Drop`. The fix was simply to add a `ManuallyDrop` layer around the value
  before doing the `transmute_copy` that is used to wrap/peel. While this fix
  could technically be backported to the 1.6 series, since 1.7 is semver
  compatible anyway the 1.6 series has simply been yanked.

## 1.7

* In response to [Unsafe Code Guidelines Issue
  #286](https://github.com/rust-lang/unsafe-code-guidelines/issues/286), this
  version of Bytemuck has a ***Soundness-Required Breaking Change***. This is
  "allowed" under Rust's backwards-compatibility guidelines, but it's still
  annoying of course so we're trying to keep the damage minimal.
  * **The Reason:** It turns out that pointer values should not have been `Pod`. More
    specifically, `ptr as usize` is *not* the same operation as calling
    `transmute::<_, usize>(ptr)`.
  * LLVM has yet to fully sort out their story, but until they do, transmuting
    pointers can cause miscompilations. They may fix things up in the future,
    but we're not gonna just wait and have broken code in the mean time.
  * **The Fix:** The breaking change is that the `Pod` impls for `*const T`,
    `*mut T`, and `Option<NonNull<T>` are now gated behind the
    `unsound_ptr_pod_impl` feature, which is off by default.
  * You are *strongly discouraged* from using this feature, but if a dependency
    of yours doesn't work when you upgrade to 1.7 because it relied on pointer
    casting, then you might wish to temporarily enable the feature just to get
    that dependency to build. Enabled features are global across all users of a
    given semver compatible version, so if you enable the feature in your own
    crate, your dependency will also end up getting the feature too, and then
    it'll be able to compile.
  * Please move away from using this feature as soon as you can. Consider it to
    *already* be deprecated.
  * [PR 65](https://github.com/Lokathor/bytemuck/pull/65)

## 1.6.3

* Small goof with an errant `;`, so [PR 69](https://github.com/Lokathor/bytemuck/pull/69)
  *actually* got things working on SPIR-V.

## 1.6.2

cargo upload goof! ignore this one.

## 1.6.1

* [DJMcNab](https://github.com/DJMcNab) did a fix so that the crate can build for SPIR-V
  [PR 67](https://github.com/Lokathor/bytemuck/pull/67)

## 1.6

* The `TransparentWrapper` trait now has more methods. More ways to wrap, and
  now you can "peel" too! Note that we don't call it "unwrap" because that name
  is too strongly associated with the Option/Result methods.
  Thanks to [LU15W1R7H](https://github.com/LU15W1R7H) for doing
  [PR 58](https://github.com/Lokathor/bytemuck/pull/58)
* Min Const Generics! Now there's Pod and Zeroable for arrays of any size when
  you turn on the `min_const_generics` crate feature.
  [zakarumych](https://github.com/zakarumych) got the work started in
  [PR 59](https://github.com/Lokathor/bytemuck/pull/59),
  and [chorman0773](https://github.com/chorman0773) finished off the task in
  [PR 63](https://github.com/Lokathor/bytemuck/pull/63)

## 1.5.1

* Fix `bytes_of` failing on zero sized types.
  [PR 53](https://github.com/Lokathor/bytemuck/pull/53)

## 1.5

* Added `pod_collect_to_vec`, which will gather a slice into a vec,
allowing you to change the pod type while also safely ignoring alignment.
[PR 50](https://github.com/Lokathor/bytemuck/pull/50)

## 1.4.2

* [Kimundi](https://github.com/Kimundi) fixed an issue that could make `try_zeroed_box`
stack overflow for large values at low optimization levels.
[PR 43](https://github.com/Lokathor/bytemuck/pull/43)

## 1.4.1

* [thomcc](https://github.com/thomcc) fixed up the CI and patched over a soundness hole in `offset_of!`.
[PR 38](https://github.com/Lokathor/bytemuck/pull/38)

## 1.4

* [icewind1991](https://github.com/icewind1991) has contributed the proc-macros
  for deriving impls of `Pod`, `TransparentWrapper`, `Zeroable`!! Everyone has
  been waiting for this one folks! It's a big deal. Just enable the `derive`
  cargo feature and then you'll be able to derive the traits on your types. It
  generates all the appropriate tests for you.
* The `zeroable_maybe_uninit` feature now adds a `Zeroable` impl to the
  `MaybeUninit` type. This is only behind a feature flag because `MaybeUninit`
  didn't exist back in `1.34.0` (the minimum rust version of `bytemuck`).

## 1.3.1

* The entire crate is now available under the `Apache-2.0 OR MIT` license as
  well as the previous `Zlib` license
  [#24](https://github.com/Lokathor/bytemuck/pull/24).
* [HeroicKatora](https://github.com/HeroicKatora) added the
  `try_zeroed_slice_box` function
  [#10](https://github.com/Lokathor/bytemuck/pull/17). `zeroed_slice_box` is
  also available.
* The `offset_of!` macro now supports a 2-arg version. For types that impl
  Default, it'll just make an instance using `default` and then call over to the
  3-arg version.
* The `PodCastError` type now supports `Hash` and `Display`. Also if you enable
  the `extern_crate_std` feature then it will support `std::error::Error`.
* We now provide a `TransparentWrapper<T>` impl for `core::num::Wrapper<T>`.
* The error type of `try_from_bytes` and `try_from_bytes_mut` when the input
  isn't aligned has been corrected from being `AlignmentMismatch` (intended for
  allocation casting only) to `TargetAlignmentGreaterAndInputNotAligned`.

## 1.3.0

* Had a bug because the CI was messed up! It wasn't soundness related, because
  it prevented the crate from building entirely if the `extern_crate_alloc`
  feature was used. Still, this is yanked, sorry.

## 1.2.0

* [thomcc](https://github.com/thomcc) added many things:
  * A fully sound `offset_of!` macro
    [#10](https://github.com/Lokathor/bytemuck/pull/10)
  * A `Contiguous` trait for when you've got enums with declared values
    all in a row [#12](https://github.com/Lokathor/bytemuck/pull/12)
  * A `TransparentWrapper` marker trait for when you want to more clearly
    enable adding and removing a wrapper struct to its inner value
    [#15](https://github.com/Lokathor/bytemuck/pull/15)
  * Now MIRI is run on CI in every single push!
    [#16](https://github.com/Lokathor/bytemuck/pull/16)

## 1.1.0

* [SimonSapin](https://github.com/SimonSapin) added `from_bytes`,
  `from_bytes_mut`, `try_from_bytes`, and `try_from_bytes_mut` ([PR
  Link](https://github.com/Lokathor/bytemuck/pull/8))

## 1.0.1

* Changed to the [zlib](https://opensource.org/licenses/Zlib) license.
* Added much more proper documentation.
* Reduced the minimum Rust version to 1.34
