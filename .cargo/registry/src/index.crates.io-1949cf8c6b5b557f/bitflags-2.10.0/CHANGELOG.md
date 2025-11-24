# 2.10.0

## What's Changed
* Implement iterator for all named flags by @ssrlive in https://github.com/bitflags/bitflags/pull/465
* Depend on serde_core instead of serde by @KodrAus in https://github.com/bitflags/bitflags/pull/467

## New Contributors
* @ssrlive made their first contribution in https://github.com/bitflags/bitflags/pull/465

**Full Changelog**: https://github.com/bitflags/bitflags/compare/2.9.4...2.10.0

# 2.9.4

## What's Changed
* Add Cargo features to readme by @KodrAus in https://github.com/bitflags/bitflags/pull/460

**Full Changelog**: https://github.com/bitflags/bitflags/compare/2.9.3...2.9.4

# 2.9.3

## What's Changed
* Streamline generated code by @nnethercote in https://github.com/bitflags/bitflags/pull/458

## New Contributors
* @nnethercote made their first contribution in https://github.com/bitflags/bitflags/pull/458

**Full Changelog**: https://github.com/bitflags/bitflags/compare/2.9.2...2.9.3

# 2.9.2

## What's Changed
* Fix difference in the spec by @KodrAus in https://github.com/bitflags/bitflags/pull/446
* Fix up inaccurate docs on bitflags_match by @KodrAus in https://github.com/bitflags/bitflags/pull/453
* Remove rustc internal crate feature by @KodrAus in https://github.com/bitflags/bitflags/pull/454


**Full Changelog**: https://github.com/bitflags/bitflags/compare/2.9.1...2.9.2

# 2.9.1

## What's Changed
* Document Cargo features by @KodrAus in https://github.com/bitflags/bitflags/pull/444


**Full Changelog**: https://github.com/bitflags/bitflags/compare/2.9.0...2.9.1

# 2.9.0

## What's Changed
* `Flags` trait: add `clear(&mut self)` method by @wysiwys in https://github.com/bitflags/bitflags/pull/437
* Fix up UI tests by @KodrAus in https://github.com/bitflags/bitflags/pull/438


**Full Changelog**: https://github.com/bitflags/bitflags/compare/2.8.0...2.9.0

# 2.8.0

## What's Changed
* feat(core): Add bitflags_match macro for bitflag matching by @YuniqueUnic in https://github.com/bitflags/bitflags/pull/423
* Finalize bitflags_match by @KodrAus in https://github.com/bitflags/bitflags/pull/431

## New Contributors
* @YuniqueUnic made their first contribution in https://github.com/bitflags/bitflags/pull/423

**Full Changelog**: https://github.com/bitflags/bitflags/compare/2.7.0...2.8.0

# 2.7.0

## What's Changed
* Fix `clippy::doc_lazy_continuation` lints by @waywardmonkeys in https://github.com/bitflags/bitflags/pull/414
* Run clippy on extra features in CI. by @waywardmonkeys in https://github.com/bitflags/bitflags/pull/415
* Fix CI: trybuild refresh, allow some clippy restrictions. by @waywardmonkeys in https://github.com/bitflags/bitflags/pull/417
* Update zerocopy version in example by @KodrAus in https://github.com/bitflags/bitflags/pull/422
* Add method to check if unknown bits are set by @wysiwys in https://github.com/bitflags/bitflags/pull/426
* Update error messages by @KodrAus in https://github.com/bitflags/bitflags/pull/427
* Add `truncate(&mut self)` method to unset unknown bits by @wysiwys in https://github.com/bitflags/bitflags/pull/428
* Update error messages by @KodrAus in https://github.com/bitflags/bitflags/pull/429

## New Contributors
* @wysiwys made their first contribution in https://github.com/bitflags/bitflags/pull/426

**Full Changelog**: https://github.com/bitflags/bitflags/compare/2.6.0...2.7.0

# 2.6.0

## What's Changed
* Sync CHANGELOG.md with github release notes by @dextero in https://github.com/bitflags/bitflags/pull/402
* Update error messages and zerocopy by @KodrAus in https://github.com/bitflags/bitflags/pull/403
* Bump minimum declared versions of dependencies by @dextero in https://github.com/bitflags/bitflags/pull/404
* chore(deps): bump serde_derive and bytemuck versions by @joshka in https://github.com/bitflags/bitflags/pull/405
* add OSFF Scorecard workflow by @KodrAus in https://github.com/bitflags/bitflags/pull/396
* Update stderr messages by @KodrAus in https://github.com/bitflags/bitflags/pull/408
* Fix typo by @waywardmonkeys in https://github.com/bitflags/bitflags/pull/410
* Allow specifying outer attributes in impl mode by @KodrAus in https://github.com/bitflags/bitflags/pull/411

## New Contributors
* @dextero made their first contribution in https://github.com/bitflags/bitflags/pull/402
* @joshka made their first contribution in https://github.com/bitflags/bitflags/pull/405
* @waywardmonkeys made their first contribution in https://github.com/bitflags/bitflags/pull/410

**Full Changelog**: https://github.com/bitflags/bitflags/compare/2.5.0...2.6.0

# 2.5.0

## What's Changed
* Derive `Debug` for `Flag<B>` by @tgross35 in https://github.com/bitflags/bitflags/pull/398
* Support truncating or strict-named variants of parsing and formatting by @KodrAus in https://github.com/bitflags/bitflags/pull/400

## New Contributors
* @tgross35 made their first contribution in https://github.com/bitflags/bitflags/pull/398

**Full Changelog**: https://github.com/bitflags/bitflags/compare/2.4.2...2.5.0

# 2.4.2

## What's Changed
* Cargo.toml: Anchor excludes to root of the package by @jamessan in https://github.com/bitflags/bitflags/pull/387
* Update error messages by @KodrAus in https://github.com/bitflags/bitflags/pull/390
* Add support for impl mode structs to be repr(packed) by @GnomedDev in https://github.com/bitflags/bitflags/pull/388
* Remove old `unused_tuple_struct_fields` lint by @dtolnay in https://github.com/bitflags/bitflags/pull/393
* Delete use of `local_inner_macros` by @dtolnay in https://github.com/bitflags/bitflags/pull/392

## New Contributors
* @jamessan made their first contribution in https://github.com/bitflags/bitflags/pull/387
* @GnomedDev made their first contribution in https://github.com/bitflags/bitflags/pull/388

**Full Changelog**: https://github.com/bitflags/bitflags/compare/2.4.1...2.4.2

# 2.4.1

## What's Changed
* Allow some new pedantic clippy lints by @KodrAus in https://github.com/bitflags/bitflags/pull/380

**Full Changelog**: https://github.com/bitflags/bitflags/compare/2.4.0...2.4.1

# 2.4.0

## What's Changed
* Remove html_root_url by @eldruin in https://github.com/bitflags/bitflags/pull/368
* Support unnamed flags by @KodrAus in https://github.com/bitflags/bitflags/pull/371
* Update smoke test to verify all Clippy and rustc lints by @MitMaro in https://github.com/bitflags/bitflags/pull/374
* Specify the behavior of bitflags by @KodrAus in https://github.com/bitflags/bitflags/pull/369

## New Contributors
* @eldruin made their first contribution in https://github.com/bitflags/bitflags/pull/368
* @MitMaro made their first contribution in https://github.com/bitflags/bitflags/pull/374

**Full Changelog**: https://github.com/bitflags/bitflags/compare/2.3.3...2.4.0

# 2.3.3

## Changes to `-=`

The `-=` operator was incorrectly changed to truncate bits that didn't correspond to valid flags in `2.3.0`. This has
been fixed up so it once again behaves the same as `-` and `difference`.

## Changes to `!`

The `!` operator previously called `Self::from_bits_truncate`, which would truncate any bits that only partially
overlapped with a valid flag. It will now use `bits & Self::all().bits()`, so any bits that overlap any bits
specified by any flag will be respected. This is unlikely to have any practical implications, but enables defining
a flag like `const ALL = !0` as a way to signal that any bit pattern is a known set of flags.

## Changes to formatting

Zero-valued flags will never be printed. You'll either get `0x0` for empty flags using debug formatting, or the
set of flags with zero-valued flags omitted for others.

Composite flags will no longer be redundantly printed if there are extra bits to print at the end that don't correspond
to a valid flag.

## What's Changed
* Fix up incorrect sub assign behavior and other cleanups by @KodrAus in https://github.com/bitflags/bitflags/pull/366

**Full Changelog**: https://github.com/bitflags/bitflags/compare/2.3.2...2.3.3

# 2.3.2

## What's Changed
* [doc] [src/lib.rs]  delete redundant path prefix by @OccupyMars2025 in https://github.com/bitflags/bitflags/pull/361

## New Contributors
* @OccupyMars2025 made their first contribution in https://github.com/bitflags/bitflags/pull/361

**Full Changelog**: https://github.com/bitflags/bitflags/compare/2.3.1...2.3.2

# 2.3.1

## What's Changed
* Fix Self in flags value expressions by @KodrAus in https://github.com/bitflags/bitflags/pull/355

**Full Changelog**: https://github.com/bitflags/bitflags/compare/2.3.0...2.3.1

# 2.3.0

## Major changes

### `BitFlags` trait deprecated in favor of `Flags` trait

This release introduces the `Flags` trait and deprecates the `BitFlags` trait. These two traits are semver compatible so if you have public API code depending on `BitFlags` you can move to `Flags` without breaking end-users. This is possible because the `BitFlags` trait was never publicly implementable, so it now carries `Flags` as a supertrait. All implementations of `Flags` additionally implement `BitFlags`.

The `Flags` trait is a publicly implementable version of the old `BitFlags` trait. The original `BitFlags` trait carried some macro baggage that made it difficult to implement, so a new `Flags` trait has been introduced as the _One True Trait_ for interacting with flags types generically. See the the `macro_free` and `custom_derive` examples for more details.

### `Bits` trait publicly exposed

The `Bits` trait for the underlying storage of flags values is also now publicly implementable. This lets you define your own exotic backing storage for flags. See the `custom_bits_type` example for more details.

## What's Changed
* Use explicit hashes for actions steps by @KodrAus in https://github.com/bitflags/bitflags/pull/350
* Support ejecting flags types from the bitflags macro by @KodrAus in https://github.com/bitflags/bitflags/pull/351

**Full Changelog**: https://github.com/bitflags/bitflags/compare/2.2.1...2.3.0

# 2.2.1

## What's Changed
* Refactor attribute filtering to apply per-flag by @KodrAus in https://github.com/bitflags/bitflags/pull/345

**Full Changelog**: https://github.com/bitflags/bitflags/compare/2.2.0...2.2.1

# 2.2.0

## What's Changed
* Create SECURITY.md by @KodrAus in https://github.com/bitflags/bitflags/pull/338
* add docs to describe the behavior of multi-bit flags by @nicholasbishop in https://github.com/bitflags/bitflags/pull/340
* Add support for bytemuck by @KodrAus in https://github.com/bitflags/bitflags/pull/336
* Add a top-level macro for filtering attributes by @KodrAus in https://github.com/bitflags/bitflags/pull/341

## New Contributors
* @nicholasbishop made their first contribution in https://github.com/bitflags/bitflags/pull/340

**Full Changelog**: https://github.com/bitflags/bitflags/compare/2.1.0...2.2.0

# 2.1.0

## What's Changed
* Add docs for the internal Field0 and examples of formatting/parsing by @KodrAus in https://github.com/bitflags/bitflags/pull/328
* Add support for arbitrary by @KodrAus in https://github.com/bitflags/bitflags/pull/324
* Fix up missing docs for consts within consts by @KodrAus in https://github.com/bitflags/bitflags/pull/330
* Ignore clippy lint in generated code by @Jake-Shadle in https://github.com/bitflags/bitflags/pull/331

## New Contributors
* @Jake-Shadle made their first contribution in https://github.com/bitflags/bitflags/pull/331

**Full Changelog**: https://github.com/bitflags/bitflags/compare/2.0.2...2.1.0

# 2.0.2

## What's Changed
* Fix up missing isize and usize Bits impls by @KodrAus in https://github.com/bitflags/bitflags/pull/321

**Full Changelog**: https://github.com/bitflags/bitflags/compare/2.0.1...2.0.2

# 2.0.1

## What's Changed
* Fix up some docs issues by @KodrAus in https://github.com/bitflags/bitflags/pull/309
* Make empty_flag() const. by @tormeh in https://github.com/bitflags/bitflags/pull/313
* Fix formatting of multi-bit flags with partial overlap by @KodrAus in https://github.com/bitflags/bitflags/pull/316

## New Contributors
* @tormeh made their first contribution in https://github.com/bitflags/bitflags/pull/313

**Full Changelog**: https://github.com/bitflags/bitflags/compare/2.0.0...2.0.1

# 2.0.0

## Major changes

This release includes some major changes over `1.x`. If you use `bitflags!` types in your public API then upgrading this library may cause breakage in your downstream users.

### ⚠️ Serialization

You'll need to add the `serde` Cargo feature in order to `#[derive(Serialize, Deserialize)]` on your generated flags types:

```rust
bitflags! {
    #[derive(Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct Flags: T {
        ..
    }
}
```

where `T` is the underlying bits type you're using, such as `u32`.

The default serialization format with `serde` **has changed** if you `#[derive(Serialize, Deserialize)]` on your generated flags types. It will now use a formatted string for human-readable formats and the underlying bits type for compact formats.

To keep the old format, see the https://github.com/KodrAus/bitflags-serde-legacy library.

### ⚠️ Traits

Generated flags types now derive fewer traits. If you need to maintain backwards compatibility, you can derive the following yourself:

```rust
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Copy)]
```

### ⚠️ Methods

The unsafe `from_bits_unchecked` method is now a safe `from_bits_retain` method.

You can add the following method to your generated types to keep them compatible:

```rust
#[deprecated = "use the safe `from_bits_retain` method instead"]
pub unsafe fn from_bits_unchecked(bits: T) -> Self {
    Self::from_bits_retain(bits)
}
```

where `T` is the underlying bits type you're using, such as `u32`.

### ⚠️ `.bits` field

You can now use the `.bits()` method instead of the old `.bits`.

The representation of generated flags types has changed from a struct with the single field `bits` to a newtype.

## What's Changed
* Fix a typo and call out MSRV bump by @KodrAus in https://github.com/bitflags/bitflags/pull/259
* BitFlags trait by @arturoc in https://github.com/bitflags/bitflags/pull/220
* Add a hidden trait to discourage manual impls of BitFlags by @KodrAus in https://github.com/bitflags/bitflags/pull/261
* Sanitize `Ok` by @konsumlamm in https://github.com/bitflags/bitflags/pull/266
* Fix bug in `Debug` implementation by @konsumlamm in https://github.com/bitflags/bitflags/pull/268
* Fix a typo in the generated documentation by @wackbyte in https://github.com/bitflags/bitflags/pull/271
* Use SPDX license format by @atouchet in https://github.com/bitflags/bitflags/pull/272
* serde tests fail in CI by @arturoc in https://github.com/bitflags/bitflags/pull/277
* Fix beta test output by @KodrAus in https://github.com/bitflags/bitflags/pull/279
* Add example to the README.md file by @tiaanl in https://github.com/bitflags/bitflags/pull/270
* Iterator over all the enabled options by @arturoc in https://github.com/bitflags/bitflags/pull/278
* from_bits_(truncate) fail with composite flags by @arturoc in https://github.com/bitflags/bitflags/pull/276
* Add more platform coverage to CI by @KodrAus in https://github.com/bitflags/bitflags/pull/280
* rework the way cfgs are handled by @KodrAus in https://github.com/bitflags/bitflags/pull/281
* Split generated code into two types by @KodrAus in https://github.com/bitflags/bitflags/pull/282
* expose bitflags iters using nameable types by @KodrAus in https://github.com/bitflags/bitflags/pull/286
* Support creating flags from their names by @KodrAus in https://github.com/bitflags/bitflags/pull/287
* Update README.md by @KodrAus in https://github.com/bitflags/bitflags/pull/288
* Prepare for 2.0.0-rc.1 release by @KodrAus in https://github.com/bitflags/bitflags/pull/289
* Add missing "if" to contains doc-comment in traits.rs by @rusty-snake in https://github.com/bitflags/bitflags/pull/291
* Forbid unsafe_code by @fintelia in https://github.com/bitflags/bitflags/pull/294
* serde: enable no-std support by @nim65s in https://github.com/bitflags/bitflags/pull/296
* Add a parser for flags formatted as bar-separated-values by @KodrAus in https://github.com/bitflags/bitflags/pull/297
* Prepare for 2.0.0-rc.2 release by @KodrAus in https://github.com/bitflags/bitflags/pull/299
* Use strip_prefix instead of starts_with + slice by @QuinnPainter in https://github.com/bitflags/bitflags/pull/301
* Fix up some clippy lints by @KodrAus in https://github.com/bitflags/bitflags/pull/302
* Prepare for 2.0.0-rc.3 release by @KodrAus in https://github.com/bitflags/bitflags/pull/303
* feat: Add minimum permissions to rust.yml workflow by @gabibguti in https://github.com/bitflags/bitflags/pull/305

## New Contributors
* @wackbyte made their first contribution in https://github.com/bitflags/bitflags/pull/271
* @atouchet made their first contribution in https://github.com/bitflags/bitflags/pull/272
* @tiaanl made their first contribution in https://github.com/bitflags/bitflags/pull/270
* @rusty-snake made their first contribution in https://github.com/bitflags/bitflags/pull/291
* @fintelia made their first contribution in https://github.com/bitflags/bitflags/pull/294
* @nim65s made their first contribution in https://github.com/bitflags/bitflags/pull/296
* @QuinnPainter made their first contribution in https://github.com/bitflags/bitflags/pull/301
* @gabibguti made their first contribution in https://github.com/bitflags/bitflags/pull/305

**Full Changelog**: https://github.com/bitflags/bitflags/compare/1.3.2...2.0.0

# 2.0.0-rc.3

## What's Changed
* Use strip_prefix instead of starts_with + slice by @QuinnPainter in https://github.com/bitflags/bitflags/pull/301
* Fix up some clippy lints by @KodrAus in https://github.com/bitflags/bitflags/pull/302

## New Contributors
* @QuinnPainter made their first contribution in https://github.com/bitflags/bitflags/pull/301

**Full Changelog**: https://github.com/bitflags/bitflags/compare/2.0.0-rc.2...2.0.0-rc.3

# 2.0.0-rc.2

## Changes to `serde` serialization

**⚠️ NOTE ⚠️** This release changes the default serialization you'll get if you `#[derive(Serialize, Deserialize)]`
on your generated flags types. It will now use a formatted string for human-readable formats and the underlying bits
type for compact formats.

To keep the old behavior, see the [`bitflags-serde-legacy`](https://github.com/KodrAus/bitflags-serde-legacy) library.

## What's Changed

* Add missing "if" to contains doc-comment in traits.rs by @rusty-snake in https://github.com/bitflags/bitflags/pull/291
* Forbid unsafe_code by @fintelia in https://github.com/bitflags/bitflags/pull/294
* serde: enable no-std support by @nim65s in https://github.com/bitflags/bitflags/pull/296
* Add a parser for flags formatted as bar-separated-values by @KodrAus in https://github.com/bitflags/bitflags/pull/297

## New Contributors
* @rusty-snake made their first contribution in https://github.com/bitflags/bitflags/pull/291
* @fintelia made their first contribution in https://github.com/bitflags/bitflags/pull/294
* @nim65s made their first contribution in https://github.com/bitflags/bitflags/pull/296

**Full Changelog**: https://github.com/bitflags/bitflags/compare/2.0.0-rc.1...2.0.0-rc.2

# 2.0.0-rc.1

This is a big release including a few years worth of work on a new `BitFlags` trait, iteration, and better macro organization for future extensibility.

## What's Changed
* Fix a typo and call out MSRV bump by @KodrAus in https://github.com/bitflags/bitflags/pull/259
* BitFlags trait by @arturoc in https://github.com/bitflags/bitflags/pull/220
* Add a hidden trait to discourage manual impls of BitFlags by @KodrAus in https://github.com/bitflags/bitflags/pull/261
* Sanitize `Ok` by @konsumlamm in https://github.com/bitflags/bitflags/pull/266
* Fix bug in `Debug` implementation by @konsumlamm in https://github.com/bitflags/bitflags/pull/268
* Fix a typo in the generated documentation by @wackbyte in https://github.com/bitflags/bitflags/pull/271
* Use SPDX license format by @atouchet in https://github.com/bitflags/bitflags/pull/272
* serde tests fail in CI by @arturoc in https://github.com/bitflags/bitflags/pull/277
* Fix beta test output by @KodrAus in https://github.com/bitflags/bitflags/pull/279
* Add example to the README.md file by @tiaanl in https://github.com/bitflags/bitflags/pull/270
* Iterator over all the enabled options by @arturoc in https://github.com/bitflags/bitflags/pull/278
* from_bits_(truncate) fail with composite flags by @arturoc in https://github.com/bitflags/bitflags/pull/276
* Add more platform coverage to CI by @KodrAus in https://github.com/bitflags/bitflags/pull/280
* rework the way cfgs are handled by @KodrAus in https://github.com/bitflags/bitflags/pull/281
* Split generated code into two types by @KodrAus in https://github.com/bitflags/bitflags/pull/282
* expose bitflags iters using nameable types by @KodrAus in https://github.com/bitflags/bitflags/pull/286
* Support creating flags from their names by @KodrAus in https://github.com/bitflags/bitflags/pull/287
* Update README.md by @KodrAus in https://github.com/bitflags/bitflags/pull/288

## New Contributors
* @wackbyte made their first contribution in https://github.com/bitflags/bitflags/pull/271
* @atouchet made their first contribution in https://github.com/bitflags/bitflags/pull/272
* @tiaanl made their first contribution in https://github.com/bitflags/bitflags/pull/270

**Full Changelog**: https://github.com/bitflags/bitflags/compare/1.3.2...2.0.0-rc.1

# 1.3.2

- Allow `non_snake_case` in generated flags types ([#256])

[#256]: https://github.com/bitflags/bitflags/pull/256

# 1.3.1

- Revert unconditional `#[repr(transparent)]` ([#252])

[#252]: https://github.com/bitflags/bitflags/pull/252

# 1.3.0 (yanked)

**This release bumps the Minimum Supported Rust Version to `1.46.0`**

- Add `#[repr(transparent)]` ([#187])

- End `empty` doc comment with full stop ([#202])

- Fix typo in crate root docs ([#206])

- Document from_bits_unchecked unsafety ([#207])

- Let `is_all` ignore extra bits ([#211])

- Allows empty flag definition ([#225])

- Making crate accessible from std ([#227])

- Make `from_bits` a const fn ([#229])

- Allow multiple bitflags structs in one macro invocation ([#235])

- Add named functions to perform set operations ([#244])

- Fix typos in method docs ([#245])

- Modernization of the `bitflags` macro to take advantage of newer features and 2018 idioms ([#246])

- Fix regression (in an unreleased feature) and simplify tests ([#247])

- Use `Self` and fix bug when overriding `stringify!` ([#249])

[#187]: https://github.com/bitflags/bitflags/pull/187
[#202]: https://github.com/bitflags/bitflags/pull/202
[#206]: https://github.com/bitflags/bitflags/pull/206
[#207]: https://github.com/bitflags/bitflags/pull/207
[#211]: https://github.com/bitflags/bitflags/pull/211
[#225]: https://github.com/bitflags/bitflags/pull/225
[#227]: https://github.com/bitflags/bitflags/pull/227
[#229]: https://github.com/bitflags/bitflags/pull/229
[#235]: https://github.com/bitflags/bitflags/pull/235
[#244]: https://github.com/bitflags/bitflags/pull/244
[#245]: https://github.com/bitflags/bitflags/pull/245
[#246]: https://github.com/bitflags/bitflags/pull/246
[#247]: https://github.com/bitflags/bitflags/pull/247
[#249]: https://github.com/bitflags/bitflags/pull/249

# 1.2.1

- Remove extraneous `#[inline]` attributes ([#194])

[#194]: https://github.com/bitflags/bitflags/pull/194

# 1.2.0

- Fix typo: {Lower, Upper}Exp - {Lower, Upper}Hex ([#183])

- Add support for "unknown" bits ([#188])

[#183]: https://github.com/rust-lang-nursery/bitflags/pull/183
[#188]: https://github.com/rust-lang-nursery/bitflags/pull/188

# 1.1.0

This is a re-release of `1.0.5`, which was yanked due to a bug in the RLS.

# 1.0.5

- Use compiletest_rs flags supported by stable toolchain ([#171])

- Put the user provided attributes first ([#173])

- Make bitflags methods `const` on newer compilers ([#175])

[#171]: https://github.com/rust-lang-nursery/bitflags/pull/171
[#173]: https://github.com/rust-lang-nursery/bitflags/pull/173
[#175]: https://github.com/rust-lang-nursery/bitflags/pull/175

# 1.0.4

- Support Rust 2018 style macro imports ([#165])

  ```rust
  use bitflags::bitflags;
  ```

[#165]: https://github.com/rust-lang-nursery/bitflags/pull/165

# 1.0.3

- Improve zero value flag handling and documentation ([#157])

[#157]: https://github.com/rust-lang-nursery/bitflags/pull/157

# 1.0.2

- 30% improvement in compile time of bitflags crate ([#156])

- Documentation improvements ([#153])

- Implementation cleanup ([#149])

[#156]: https://github.com/rust-lang-nursery/bitflags/pull/156
[#153]: https://github.com/rust-lang-nursery/bitflags/pull/153
[#149]: https://github.com/rust-lang-nursery/bitflags/pull/149

# 1.0.1
- Add support for `pub(restricted)` specifier on the bitflags struct ([#135])
- Optimize performance of `all()` when called from a separate crate ([#136])

[#135]: https://github.com/rust-lang-nursery/bitflags/pull/135
[#136]: https://github.com/rust-lang-nursery/bitflags/pull/136

# 1.0.0
- **[breaking change]** Macro now generates [associated constants](https://doc.rust-lang.org/reference/items.html#associated-constants) ([#24])

- **[breaking change]** Minimum supported version is Rust **1.20**, due to usage of associated constants

- After being broken in 0.9, the `#[deprecated]` attribute is now supported again ([#112])

- Other improvements to unit tests and documentation ([#106] and [#115])

[#24]: https://github.com/rust-lang-nursery/bitflags/pull/24
[#106]: https://github.com/rust-lang-nursery/bitflags/pull/106
[#112]: https://github.com/rust-lang-nursery/bitflags/pull/112
[#115]: https://github.com/rust-lang-nursery/bitflags/pull/115

## How to update your code to use associated constants
Assuming the following structure definition:
```rust
bitflags! {
  struct Something: u8 {
     const FOO = 0b01,
     const BAR = 0b10
  }
}
```
In 0.9 and older you could do:
```rust
let x = FOO.bits | BAR.bits;
```
Now you must use:
```rust
let x = Something::FOO.bits | Something::BAR.bits;
```

# 0.9.1
- Fix the implementation of `Formatting` traits when other formatting traits were present in scope ([#105])

[#105]: https://github.com/rust-lang-nursery/bitflags/pull/105

# 0.9.0
- **[breaking change]** Use struct keyword instead of flags to define bitflag types ([#84])

- **[breaking change]** Terminate const items with semicolons instead of commas ([#87])

- Implement the `Hex`, `Octal`, and `Binary` formatting traits ([#86])

- Printing an empty flag value with the `Debug` trait now prints "(empty)" instead of nothing ([#85])

- The `bitflags!` macro can now be used inside of a fn body, to define a type local to that function ([#74])

[#74]: https://github.com/rust-lang-nursery/bitflags/pull/74
[#84]: https://github.com/rust-lang-nursery/bitflags/pull/84
[#85]: https://github.com/rust-lang-nursery/bitflags/pull/85
[#86]: https://github.com/rust-lang-nursery/bitflags/pull/86
[#87]: https://github.com/rust-lang-nursery/bitflags/pull/87

# 0.8.2
- Update feature flag used when building bitflags as a dependency of the Rust toolchain

# 0.8.1
- Allow bitflags to be used as a dependency of the Rust toolchain

# 0.8.0
- Add support for the experimental `i128` and `u128` integer types ([#57])
- Add set method: `flags.set(SOME_FLAG, true)` or `flags.set(SOME_FLAG, false)` ([#55])
  This may break code that defines its own set method

[#55]: https://github.com/rust-lang-nursery/bitflags/pull/55
[#57]: https://github.com/rust-lang-nursery/bitflags/pull/57

# 0.7.1
*(yanked)*

# 0.7.0
- Implement the Extend trait ([#49])
- Allow definitions inside the `bitflags!` macro to refer to items imported from other modules ([#51])

[#49]: https://github.com/rust-lang-nursery/bitflags/pull/49
[#51]: https://github.com/rust-lang-nursery/bitflags/pull/51

# 0.6.0
- The `no_std` feature was removed as it is now the default
- The `assignment_operators` feature was remove as it is now enabled by default
- Some clippy suggestions have been applied
