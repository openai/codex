# Changelog

All notable changes to this project will be documented in this file.

This project adheres to [Semantic Versioning](https://semver.org).

Releases may yanked if there is a security bug, a soundness bug, or a regression.

<!--
Note: In this file, do not use the hard wrap in the middle of a sentence for compatibility with GitHub comment style markdown rendering.
-->

## [Unreleased]

## [1.1.10] - 2025-03-03

- Suppress `clippy::elidable_lifetime_names` lint in generated code.

## [1.1.9] - 2025-02-03

- Suppress `clippy::missing_const_for_fn` lint in generated code.

## [1.1.8] - 2025-01-06

- Suppress `unnameable_types`, `clippy::absolute_paths`, `clippy::min_ident_chars`, `clippy::pub_with_shorthand`, `clippy::single_call_fn`, `clippy::single_char_lifetime_names` lints in generated code.

## [1.1.7] - 2024-10-24

- Work around an issue on negative_impls that allows unsound overlapping `Unpin` implementations. ([#357](https://github.com/taiki-e/pin-project/pull/357))

## [1.1.6] - 2024-10-05

- Suppress `clippy::needless_lifetimes` lint in generated code.

- Disable `derive` feature of `syn` dependency.

## [1.1.5] - 2024-03-05

- Suppress `unused_qualifications` lint in generated code.

## [1.1.4] - 2024-01-25

- Suppress `clippy::missing_docs_in_private_items` and `clippy::missing_inline_in_public_items` lints in generated code. ([#356](https://github.com/taiki-e/pin-project/pull/356), thanks @daxpedda)

## [1.1.3] - 2023-08-06

- Hide documentation of the `Unpin` implementation for `!Unpin` option to work around [rustdoc issue](https://github.com/rust-lang/rust/issues/80481). ([#355](https://github.com/taiki-e/pin-project/pull/355), thanks @matheus-consoli)

## [1.1.2] - 2023-07-02

- Inline project methods.

## [1.1.1] - 2023-06-29

- Fix build error from dependency when built with `-Z minimal-versions`.

## [1.1.0] - 2023-05-13

- Update `syn` dependency to 2. This increase the minimum supported Rust version from Rust 1.37 to Rust 1.56. ([#352](https://github.com/taiki-e/pin-project/pull/352), [#354](https://github.com/taiki-e/pin-project/pull/354), thanks @maurer and @daxpedda)

## [1.0.12] - 2022-08-15

- Suppress `unused_tuple_struct_fields` lint in generated code.

## [1.0.11] - 2022-07-02

- [Suppress `dead_code` lint in generated code.](https://github.com/taiki-e/pin-project/pull/346)

## [1.0.10] - 2021-12-31

- Revert the increase of the minimal version of `syn` that was done in 1.0.9.

## [1.0.9] - 2021-12-26

- [Prevent abuse of private module.](https://github.com/taiki-e/pin-project/pull/336)

- Update minimal version of `syn` to 1.0.84.

## [1.0.8] - 2021-07-21

- [Suppress `clippy::use_self` and `clippy::type_repetition_in_bounds` lints in generated code.](https://github.com/taiki-e/pin-project/pull/331)

## [1.0.7] - 2021-04-16

- [Fix compile error when using `self::` as prefix of path inside `#[pinned_drop]` impl.](https://github.com/taiki-e/pin-project/pull/326)

## [1.0.6] - 2021-03-25

- [Suppress `clippy::semicolon_if_nothing_returned` lint in generated code.](https://github.com/taiki-e/pin-project/pull/318)

## [1.0.5] - 2021-02-03

- [Suppress `deprecated` lint in generated code.](https://github.com/taiki-e/pin-project/pull/313)

## [1.0.4] - 2021-01-09

- [Suppress `clippy::ref_option_ref` lint in generated code.](https://github.com/taiki-e/pin-project/pull/308)

## [1.0.3] - 2021-01-05

- Exclude unneeded files from crates.io.

## [1.0.2] - 2020-11-18

- [Suppress `clippy::unknown_clippy_lints` lint in generated code.](https://github.com/taiki-e/pin-project/pull/303)

## [1.0.1] - 2020-10-15

**Note:** This release has been yanked because it [failed to compile with syn 1.0.84 and later](https://github.com/taiki-e/pin-project/commit/ddcd88079ba2d82857c365f2a3543ad146ade54c).

- [Fix warnings when `#[pin_project]` attribute used within `macro_rules!` macros.](https://github.com/taiki-e/pin-project/pull/298)

## [1.0.0] - 2020-10-13

**Note:** This release has been yanked because it [failed to compile with syn 1.0.84 and later](https://github.com/taiki-e/pin-project/commit/ddcd88079ba2d82857c365f2a3543ad146ade54c).

- [Remove deprecated `#[project]`, `#[project_ref]`, and `#[project_replace]` attributes.](https://github.com/taiki-e/pin-project/pull/265)

  Name the projected type by passing an argument with the same name as the method to the `#[pin_project]` attribute instead:

  ```diff
  - #[pin_project]
  + #[pin_project(project = EnumProj)]
    enum Enum<T> {
        Variant(#[pin] T),
    }

  - #[project]
    fn func<T>(x: Pin<&mut Enum<T>>) {
  -     #[project]
        match x.project() {
  -         Enum::Variant(_) => { /* ... */ }
  +         EnumProj::Variant(_) => { /* ... */ }
        }
    }
  ```

- [Remove deprecated `Replace` argument from `#[pin_project]` attribute.](https://github.com/taiki-e/pin-project/pull/266) Use `project_replace` argument instead.

- [Optimize code generation when used on enums.](https://github.com/taiki-e/pin-project/pull/270)

- [Raise the minimum supported Rust version of this crate from Rust 1.34 to Rust 1.37.](https://github.com/taiki-e/pin-project/pull/292)

- Suppress `explicit_outlives_requirements`, `box_pointers`, `clippy::large_enum_variant`, `clippy::pattern_type_mismatch`, `clippy::implicit_return`, and `clippy::redundant_pub_crate` lints in generated code. ([#276](https://github.com/taiki-e/pin-project/pull/276), [#277](https://github.com/taiki-e/pin-project/pull/277), [#284](https://github.com/taiki-e/pin-project/pull/284))

- Diagnostic improvements.

Changes since the 1.0.0-alpha.1 release:

- [Fix drop order of pinned fields in `project_replace`.](https://github.com/taiki-e/pin-project/pull/287)

- Update minimal version of `syn` to 1.0.44.

## [1.0.0-alpha.1] - 2020-09-22

- [Remove deprecated `#[project]`, `#[project_ref]`, and `#[project_replace]` attributes.](https://github.com/taiki-e/pin-project/pull/265)

  Name the projected type by passing an argument with the same name as the method to the `#[pin_project]` attribute instead:

  ```diff
  - #[pin_project]
  + #[pin_project(project = EnumProj)]
    enum Enum<T> {
        Variant(#[pin] T),
    }

  - #[project]
    fn func<T>(x: Pin<&mut Enum<T>>) {
  -     #[project]
        match x.project() {
  -         Enum::Variant(_) => { /* ... */ }
  +         EnumProj::Variant(_) => { /* ... */ }
        }
    }
  ```

- [Remove deprecated `Replace` argument from `#[pin_project]` attribute.](https://github.com/taiki-e/pin-project/pull/266) Use `project_replace` argument instead.

- [Optimize code generation when used on enums.](https://github.com/taiki-e/pin-project/pull/270)

- Suppress `explicit_outlives_requirements`, `box_pointers`, `clippy::large_enum_variant`, `clippy::pattern_type_mismatch`, and `clippy::implicit_return` lints in generated code. ([#276](https://github.com/taiki-e/pin-project/pull/276), [#277](https://github.com/taiki-e/pin-project/pull/277))

- Diagnostic improvements.

See also [tracking issue for 1.0 release](https://github.com/taiki-e/pin-project/issues/264).

## [0.4.30] - 2022-07-02

- [Suppress `dead_code` lint in generated code.](https://github.com/taiki-e/pin-project/pull/347)

## [0.4.29] - 2021-12-26

- [Fix compile error with `syn` 1.0.84 and later.](https://github.com/taiki-e/pin-project/pull/335)

## [0.4.28] - 2021-03-28

**Note:** This release has been yanked because it [failed to compile with syn 1.0.84 and later](https://github.com/taiki-e/pin-project/pull/335).

- [Fix `unused_must_use` warning on unused borrows, which will be added to rustc in the future.](https://github.com/taiki-e/pin-project/pull/322) See [#322](https://github.com/taiki-e/pin-project/pull/322) for more details.

  (Note: 1.0 does not have this problem.)

## [0.4.27] - 2020-10-11

**Note:** This release has been yanked because it [failed to compile with syn 1.0.84 and later](https://github.com/taiki-e/pin-project/pull/335).

- Update minimal version of `syn` to 1.0.44.

## [0.4.26] - 2020-10-04

**Note:** This release has been yanked because it [failed to compile with syn 1.0.84 and later](https://github.com/taiki-e/pin-project/pull/335).

- [Fix drop order of pinned fields in `project_replace`.](https://github.com/taiki-e/pin-project/pull/287)

## [0.4.25] - 2020-10-01

**Note:** This release has been yanked because it [failed to compile with syn 1.0.84 and later](https://github.com/taiki-e/pin-project/pull/335).

- [Suppress `drop_bounds` lint, which will be added to rustc in the future.](https://github.com/taiki-e/pin-project/pull/273) See [#272](https://github.com/taiki-e/pin-project/issues/272) for more details.

  (Note: 1.0.0-alpha.1 already contains this change.)

## [0.4.24] - 2020-09-26

**Note:** This release has been yanked because it [failed to compile with syn 1.0.84 and later](https://github.com/taiki-e/pin-project/pull/335).

- [Fix compatibility of generated code with `forbid(future_incompatible)`.](https://github.com/taiki-e/pin-project/pull/282)

  Note: This does not guarantee compatibility with `forbid(future_incompatible)` in the future.
  If rustc adds a new lint, we may not be able to keep this.

## [0.4.23] - 2020-07-27

**Note:** This release has been yanked because it [failed to compile with syn 1.0.84 and later](https://github.com/taiki-e/pin-project/pull/335).

- [Fix compile error with `?Sized` type parameters.](https://github.com/taiki-e/pin-project/pull/263)

## [0.4.22] - 2020-06-14

**Note:** This release has been yanked because it [failed to compile with syn 1.0.84 and later](https://github.com/taiki-e/pin-project/pull/335).

- Documentation improvements.

## [0.4.21] - 2020-06-13

**Note:** This release has been yanked because it [failed to compile with syn 1.0.84 and later](https://github.com/taiki-e/pin-project/pull/335).

- [Deprecated `#[project]`, `#[project_ref]`, and `#[project_replace]` attributes due to some unfixable limitations.](https://github.com/taiki-e/pin-project/pull/244)

  Consider naming the projected type by passing an argument with the same name as the method to the `#[pin_project]` attribute instead.

  ```rust
  #[pin_project(project = EnumProj)]
  enum Enum<T> {
      Variant(#[pin] T),
  }

  fn func<T>(x: Pin<&mut Enum<T>>) {
      match x.project() {
          EnumProj::Variant(y) => {
              let _: Pin<&mut T> = y;
          }
      }
  }
  ```

  See [#225](https://github.com/taiki-e/pin-project/pull/225) for more details.

- [Support `Self` in fields and generics in type definitions.](https://github.com/taiki-e/pin-project/pull/245)

- [Fix errors involving *"`self` value is a keyword only available in methods with `self` parameter"* in apparently correct code.](https://github.com/taiki-e/pin-project/pull/250)

- Diagnostic improvements.

## [0.4.20] - 2020-06-07

**Note:** This release has been yanked because it [failed to compile with syn 1.0.84 and later](https://github.com/taiki-e/pin-project/pull/335).

- [You can now use `project_replace` argument without Replace argument.](https://github.com/taiki-e/pin-project/pull/243)
  This used to require you to specify both.

  ```diff
  - #[pin_project(Replace, project_replace = EnumProjOwn)]
  + #[pin_project(project_replace = EnumProjOwn)]
    enum Enum<T> {
        Variant(#[pin] T)
    }
  ```

- [Make `project_replace` argument an alias for `Replace` argument so that it can be used without a value.](https://github.com/taiki-e/pin-project/pull/243)

  ```rust
  #[pin_project(project_replace)]
  enum Enum<T> {
      Variant(#[pin] T)
  }
  ```

  *The `Replace` argument will be deprecated in the future.*

- [Suppress `unreachable_pub` lint in generated code.](https://github.com/taiki-e/pin-project/pull/240)

## [0.4.19] - 2020-06-04

**Note:** This release has been yanked because it [failed to compile with syn 1.0.84 and later](https://github.com/taiki-e/pin-project/pull/335).

- [Suppress `unused_results` lint in generated code.](https://github.com/taiki-e/pin-project/pull/239)

## [0.4.18] - 2020-06-04

**Note:** This release has been yanked because it [failed to compile with syn 1.0.84 and later](https://github.com/taiki-e/pin-project/pull/335).

- [Support `Self` in more syntax positions inside `#[pinned_drop]` impl.](https://github.com/taiki-e/pin-project/pull/230)

- [Suppress `clippy::type_repetition_in_bounds` and `clippy::used_underscore_binding` lints in generated code.](https://github.com/taiki-e/pin-project/pull/233)

- Documentation improvements.

- Diagnostic improvements.

## [0.4.17] - 2020-05-18

**Note:** This release has been yanked because it [failed to compile with syn 1.0.84 and later](https://github.com/taiki-e/pin-project/pull/335).

- [Support naming the projection types.](https://github.com/taiki-e/pin-project/pull/202)

  By passing an argument with the same name as the method to the attribute, you can name the projection type returned from the method:

  ```rust
  #[pin_project(project = EnumProj)]
  enum Enum<T> {
      Variant(#[pin] T),
  }

  fn func<T>(x: Pin<&mut Enum<T>>) {
      match x.project() {
          EnumProj::Variant(y) => {
              let _: Pin<&mut T> = y;
          }
      }
  }
  ```

## [0.4.16] - 2020-05-11

**Note:** This release has been yanked because it [failed to compile with syn 1.0.84 and later](https://github.com/taiki-e/pin-project/pull/335).

- [Fix an issue that users can call internal function generated by `#[pinned_drop]`.](https://github.com/taiki-e/pin-project/pull/223)

## [0.4.15] - 2020-05-10

**Note:** This release has been yanked because it [failed to compile with syn 1.0.84 and later](https://github.com/taiki-e/pin-project/pull/335).

- [`#[project]` attribute can now handle all `project*` attributes in that scope with one wrapper attribute.](https://github.com/taiki-e/pin-project/pull/220)

## [0.4.14] - 2020-05-09

**Note:** This release has been yanked because it [failed to compile with syn 1.0.84 and later](https://github.com/taiki-e/pin-project/pull/335).

- [Add `!Unpin` option to `#[pin_project]` attribute for guarantee the type is `!Unpin`.](https://github.com/taiki-e/pin-project/pull/219)

  ```rust
  #[pin_project(!Unpin)]
  struct Struct<T, U> {
      field: T,
  }
  ```

  This is equivalent to use `#[pin]` attribute for `PhantomPinned` field.

  ```rust
  #[pin_project]
  struct Struct<T, U> {
      field: T,
      #[pin] // Note that using `PhantomPinned` without `#[pin]` attribute has no effect.
      _pin: PhantomPinned,
  }
  ```

  *[Note: This raises the minimum supported Rust version of this crate from Rust 1.33 to Rust 1.34.](https://github.com/taiki-e/pin-project/pull/219#pullrequestreview-408644187)*

- [Fix an issue where duplicate `#[project]` attributes were ignored.](https://github.com/taiki-e/pin-project/pull/218)

- [Suppress `single_use_lifetimes` lint in generated code.](https://github.com/taiki-e/pin-project/pull/217)

- [Support overlapping lifetime names in HRTB.](https://github.com/taiki-e/pin-project/pull/217)

- [Hide generated items from --document-private-items.](https://github.com/taiki-e/pin-project/pull/211) See [#211](https://github.com/taiki-e/pin-project/pull/211) for details.

- Documentation improvements.

## [0.4.13] - 2020-05-07

**Note:** This release has been yanked because it [failed to compile with syn 1.0.84 and later](https://github.com/taiki-e/pin-project/pull/335).

- [Fix a regression in 0.4.11.](https://github.com/taiki-e/pin-project/pull/207)

  Changes from [0.4.10](https://github.com/taiki-e/pin-project/releases/tag/v0.4.10) and [0.4.12](https://github.com/taiki-e/pin-project/releases/tag/v0.4.12):

  - [Fix an issue that `#[project]` on non-statement expression does not work without unstable features.](https://github.com/taiki-e/pin-project/pull/197)

  - [Support overwriting the name of core crate.](https://github.com/taiki-e/pin-project/pull/199)

  - [Suppress `clippy::needless_pass_by_value` lint in generated code of `#[pinned_drop]`.](https://github.com/taiki-e/pin-project/pull/200)

  - Documentation improvements.

  - Diagnostic improvements.

## [0.4.12] - 2020-05-07

**Note:** This release has been yanked because it [failed to compile with syn 1.0.84 and later](https://github.com/taiki-e/pin-project/pull/335).

- A release to avoid [a regression in 0.4.11](https://github.com/taiki-e/pin-project/issues/206). No code changes from [0.4.10](https://github.com/taiki-e/pin-project/releases/tag/v0.4.10).

## [0.4.11] - 2020-05-07

**Note:** This release has been yanked. See [#206](https://github.com/taiki-e/pin-project/issues/206) for details.

- [Fix an issue that `#[project]` on non-statement expression does not work without unstable features.](https://github.com/taiki-e/pin-project/pull/197)

- [Support overwriting the name of core crate.](https://github.com/taiki-e/pin-project/pull/199)

- [Suppress `clippy::needless_pass_by_value` lint in generated code of `#[pinned_drop]`.](https://github.com/taiki-e/pin-project/pull/200)

- Documentation improvements.

- Diagnostic improvements.

## [0.4.10] - 2020-05-04

**Note:** This release has been yanked because it [failed to compile with syn 1.0.84 and later](https://github.com/taiki-e/pin-project/pull/335).

- [Add `project_replace` method and `#[project_replace]` attribute.](https://github.com/taiki-e/pin-project/pull/194)
  `project_replace` method is optional and can be enabled by passing the `Replace` argument to `#[pin_project]` attribute.
  See [the documentation](https://docs.rs/pin-project/0.4/pin_project/attr.pin_project.html#project_replace) for more details.

- [Support `Self` and `self` in more syntax positions inside `#[pinned_drop]` impl.](https://github.com/taiki-e/pin-project/pull/190)

- [Hide all generated items except for projected types from calling code.](https://github.com/taiki-e/pin-project/pull/192) See [#192](https://github.com/taiki-e/pin-project/pull/192) for details.

## [0.4.9] - 2020-04-14

**Note:** This release has been yanked because it [failed to compile with syn 1.0.84 and later](https://github.com/taiki-e/pin-project/pull/335).

- [Fix lifetime inference error when associated types are used in fields.](https://github.com/taiki-e/pin-project/pull/188)

- [Fix compile error with tuple structs with `where` clauses.](https://github.com/taiki-e/pin-project/pull/186)

- [`#[project]` attribute can now be used for `if let` expressions.](https://github.com/taiki-e/pin-project/pull/181)

## [0.4.8] - 2020-01-27

**Note:** This release has been yanked because it [failed to compile with syn 1.0.84 and later](https://github.com/taiki-e/pin-project/pull/335).

- [Ensure that users cannot implement `PinnedDrop` without proper attribute argument.](https://github.com/taiki-e/pin-project/pull/180)

- [Fix use of `Self` in expression position inside `#[pinned_drop]` impl.](https://github.com/taiki-e/pin-project/pull/177)

## [0.4.7] - 2020-01-20

**Note:** This release has been yanked because it [failed to compile with syn 1.0.84 and later](https://github.com/taiki-e/pin-project/pull/335).

- [Fix support for lifetime bounds.](https://github.com/taiki-e/pin-project/pull/176)

## [0.4.6] - 2019-11-20

**Note:** This release has been yanked because it [failed to compile with syn 1.0.84 and later](https://github.com/taiki-e/pin-project/pull/335).

- [Fix compile error when there is `Self` in the where clause.](https://github.com/taiki-e/pin-project/pull/169)

## [0.4.5] - 2019-10-21

**Note:** This release has been yanked because it [failed to compile with syn 1.0.84 and later](https://github.com/taiki-e/pin-project/pull/335).

- [Fix compile error with `dyn` types.](https://github.com/taiki-e/pin-project/pull/158)

## [0.4.4] - 2019-10-17

**Note:** This release has been yanked because it [failed to compile with syn 1.0.84 and later](https://github.com/taiki-e/pin-project/pull/335).

- [Fix an issue where `PinnedDrop` implementations can call unsafe code without an unsafe block.](https://github.com/taiki-e/pin-project/pull/149)

## [0.4.3] - 2019-10-15

**Note:** This release has been yanked. See [#148](https://github.com/taiki-e/pin-project/pull/148) for details.

- [`#[pin_project]` can now interoperate with `#[cfg_attr()]`.](https://github.com/taiki-e/pin-project/pull/135)

- [`#[pin_project]` can now interoperate with `#[cfg()]` on tuple structs and tuple variants.](https://github.com/taiki-e/pin-project/pull/135)

- [Fix support for DSTs(Dynamically Sized Types) on `#[pin_project(UnsafeUnpin)]`](https://github.com/taiki-e/pin-project/pull/120)

- Diagnostic improvements.

## [0.4.2] - 2019-09-29

**Note:** This release has been yanked. See [#148](https://github.com/taiki-e/pin-project/pull/148) for details.

- [Fix support for DSTs(Dynamically Sized Types).](https://github.com/taiki-e/pin-project/pull/113)

## [0.4.1] - 2019-09-26

**Note:** This release has been yanked. See [#148](https://github.com/taiki-e/pin-project/pull/148) for details.

- [Fix an issue that caused an error when using `#[pin_project]` on a type that has `#[pin]` + `!Unpin` field with no generics or lifetime.](https://github.com/taiki-e/pin-project/pull/111)

## [0.4.0] - 2019-09-25

**Note:** This release has been yanked. See [#148](https://github.com/taiki-e/pin-project/pull/148) for details.

- [**Pin projection has become a safe operation.**](https://github.com/taiki-e/pin-project/pull/18) In the absence of other unsafe code that you write, it is impossible to cause undefined behavior.

- `#[unsafe_project]` attribute has been replaced with `#[pin_project]` attribute. ([#18](https://github.com/taiki-e/pin-project/pull/18), [#33](https://github.com/taiki-e/pin-project/pull/33))

- [The `Unpin` argument has been removed - an `Unpin` impl is now generated by default.](https://github.com/taiki-e/pin-project/pull/18)

- Drop impls must be specified with `#[pinned_drop]` instead of via a normal `Drop` impl. ([#18](https://github.com/taiki-e/pin-project/pull/18), [#33](https://github.com/taiki-e/pin-project/pull/33), [#86](https://github.com/taiki-e/pin-project/pull/86))

- [`Unpin` impls must be specified with an impl of `UnsafeUnpin`, instead of implementing the normal `Unpin` trait.](https://github.com/taiki-e/pin-project/pull/18)

- [`#[pin_project]` attribute now determines the visibility of the projection type/method is based on the original type.](https://github.com/taiki-e/pin-project/pull/96)

- [`#[pin_project]` can now be used for public type with private field types.](https://github.com/taiki-e/pin-project/pull/53)

- [`#[pin_project]` can now interoperate with `#[cfg()]`.](https://github.com/taiki-e/pin-project/pull/77)

- [Add `project_ref` method to `#[pin_project]` types.](https://github.com/taiki-e/pin-project/pull/93)

- [Add `#[project_ref]` attribute.](https://github.com/taiki-e/pin-project/pull/93)

- [Remove "project_attr" feature and always enable `#[project]` attribute.](https://github.com/taiki-e/pin-project/pull/94)

- [`#[project]` attribute can now be used for `impl` blocks.](https://github.com/taiki-e/pin-project/pull/46)

- [`#[project]` attribute can now be used for `use` statements.](https://github.com/taiki-e/pin-project/pull/85)

- [`#[project]` attribute now supports `match` expressions at the position of the initializer expression of `let` expressions.](https://github.com/taiki-e/pin-project/pull/51)

Changes since the 0.4.0-beta.1 release:

- [Fix an issue that caused an error when using `#[pin_project(UnsafeUnpin)]` and not providing a manual `UnsafeUnpin` implementation on a type with no generics or lifetime.](https://github.com/taiki-e/pin-project/pull/107)

## [0.4.0-beta.1] - 2019-09-21

- [Change the argument type of project method back to `self: Pin<&mut Self>`.](https://github.com/taiki-e/pin-project/pull/90)

- [Remove "project_attr" feature and always enable `#[project]` attribute.](https://github.com/taiki-e/pin-project/pull/94)

- [Remove "renamed" feature.](https://github.com/taiki-e/pin-project/pull/100)

- [`#[project]` attribute can now be used for `use` statements.](https://github.com/taiki-e/pin-project/pull/85)

- [Add `project_ref` method and `#[project_ref]` attribute.](https://github.com/taiki-e/pin-project/pull/93)

- [`#[pin_project]` attribute now determines the visibility of the projection type/method is based on the original type.](https://github.com/taiki-e/pin-project/pull/96)

## [0.4.0-alpha.11] - 2019-09-11

- [Change #[pinned_drop] to trait implementation.](https://github.com/taiki-e/pin-project/pull/86)

  ```rust
  #[pinned_drop]
  impl<T> PinnedDrop for Foo<'_, T> {
      fn drop(mut self: Pin<&mut Self>) {
          **self.project().was_dropped = true;
      }
  }
  ```

- Add some examples and generated code.

- Diagnostic improvements.

## [0.4.0-alpha.10] - 2019-09-07

- [`#[pin_project]` can now interoperate with `#[cfg()]`.](https://github.com/taiki-e/pin-project/pull/77)

- Documentation improvements.

## [0.4.0-alpha.9] - 2019-09-05

- [Add `project_into` method to `#[pin_project]` types](https://github.com/taiki-e/pin-project/pull/69). This can be useful when returning a pin projection from a method.

  ```rust
  fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut T> {
      self.project_into().pinned
  }
  ```

- [Prevent `UnpinStruct` from appearing in the document by default.](https://github.com/taiki-e/pin-project/pull/71) See [#71](https://github.com/taiki-e/pin-project/pull/71) for more details.

## [0.4.0-alpha.8] - 2019-09-03

- [Improve document of generated code.](https://github.com/taiki-e/pin-project/pull/62). Also added an option to control the document of generated code. See [#62](https://github.com/taiki-e/pin-project/pull/62) for more details.

- [Diagnostic improvements.](https://github.com/taiki-e/pin-project/pull/61)

## [0.4.0-alpha.7] - 2019-09-02

- [Suppress `dead_code` lint in generated types.](https://github.com/taiki-e/pin-project/pull/57)

## [0.4.0-alpha.6] - 2019-09-01

- [Allow using `#[pin_project]` type with private field types](https://github.com/taiki-e/pin-project/pull/53)

## [0.4.0-alpha.5] - 2019-08-24

- [`#[project]` attribute now supports `match` expressions at the position of the initializer expression of `let` expressions.](https://github.com/taiki-e/pin-project/pull/51)

## [0.4.0-alpha.4] - 2019-08-23

- Suppress `clippy::drop_bounds` lint in generated code.

## [0.4.0-alpha.3] - 2019-08-23

- [Change `project` method generated by `#[pin_project]` attribute to take an `&mut Pin<&mut Self>` argument.](https://github.com/taiki-e/pin-project/pull/47)

- [`#[project]` attribute can now be used for impl blocks.](https://github.com/taiki-e/pin-project/pull/46)

- [`#[pin_project]` attribute can now detect that the type used does not have its own drop implementation without actually implementing drop.](https://github.com/taiki-e/pin-project/pull/48) This removed some restrictions.

## [0.4.0-alpha.2] - 2019-08-13

- Update `proc-macro2`, `syn`, and `quote` to 1.0.

## [0.4.0-alpha.1] - 2019-08-11

- **Pin projection has become a safe operation.**

- `#[unsafe_project]` has been replaced with `#[pin_project]`.

- The `Unpin` argument has been removed - an `Unpin` impl is now generated by default.

- Drop impls must be specified with `#[pinned_drop]` instead of via a normal `Drop` impl.

- `Unpin` impls must be specified with an impl of `UnsafeUnpin`, instead of implementing the normal `Unpin` trait.

- Make `#[project]` attribute disabled by default.

See also [tracking issue for 0.4 release](https://github.com/taiki-e/pin-project/issues/21).

## [0.3.5] - 2019-08-14

- Update `proc-macro2`, `syn`, and `quote` to 1.0.

## [0.3.4] - 2019-07-21

- Diagnostic improvements.

## [0.3.3] - 2019-07-15

**Note:** This release has been yanked. See [#16](https://github.com/taiki-e/pin-project/issues/16) for details.

- Diagnostic improvements.

## [0.3.2] - 2019-03-30

- Avoid suffixes on tuple index.

## [0.3.1] - 2019-03-02

- Documentation improvements.

- Update minimum `syn` version to 0.15.22.

## [0.3.0] - 2019-02-20

- Remove `unsafe_fields` attribute.

- Remove `unsafe_variants` attribute.

## [0.2.2] - 2019-02-20

- Fix a bug that generates incorrect code for the some structures with trait bounds on type generics.

## [0.2.1] - 2019-02-20

- Fix a bug that generates incorrect code for the structures with where clause and associated type fields.

## [0.2.0] - 2019-02-11

- Make `unsafe_fields` optional.

- Documentation improvements.

## [0.1.8] - 2019-02-02

- Add the feature to create projected enums to `unsafe_project`.

- Add `project` attribute to support pattern matching.

## [0.1.7] - 2019-01-19

- Fix documentation.

## [0.1.6] - 2019-01-19

- `unsafe_fields` can now opt-out.

- Add `unsafe_variants` attribute. This attribute is available if pin-project is built with the "unsafe_variants" feature.

## [0.1.5] - 2019-01-17

- Add support for tuple struct to `unsafe_project`.

## [0.1.4] - 2019-01-12

- Add options for automatically implementing `Unpin` to both `unsafe_project` and `unsafe_fields`.

## [0.1.3] - 2019-01-11

- Fix dependencies.

- Add `unsafe_fields` attribute.

## [0.1.2] - 2019-01-09

- Documentation improvements.

## [0.1.1] - 2019-01-08

- Rename from `unsafe_pin_project` to `unsafe_project`.

## [0.1.0] - 2019-01-08

**Note:** This release has been yanked.

Initial release

[Unreleased]: https://github.com/taiki-e/pin-project/compare/v1.1.10...HEAD
[1.1.10]: https://github.com/taiki-e/pin-project/compare/v1.1.9...v1.1.10
[1.1.9]: https://github.com/taiki-e/pin-project/compare/v1.1.8...v1.1.9
[1.1.8]: https://github.com/taiki-e/pin-project/compare/v1.1.7...v1.1.8
[1.1.7]: https://github.com/taiki-e/pin-project/compare/v1.1.6...v1.1.7
[1.1.6]: https://github.com/taiki-e/pin-project/compare/v1.1.5...v1.1.6
[1.1.5]: https://github.com/taiki-e/pin-project/compare/v1.1.4...v1.1.5
[1.1.4]: https://github.com/taiki-e/pin-project/compare/v1.1.3...v1.1.4
[1.1.3]: https://github.com/taiki-e/pin-project/compare/v1.1.2...v1.1.3
[1.1.2]: https://github.com/taiki-e/pin-project/compare/v1.1.1...v1.1.2
[1.1.1]: https://github.com/taiki-e/pin-project/compare/v1.1.0...v1.1.1
[1.1.0]: https://github.com/taiki-e/pin-project/compare/v1.0.12...v1.1.0
[1.0.12]: https://github.com/taiki-e/pin-project/compare/v1.0.11...v1.0.12
[1.0.11]: https://github.com/taiki-e/pin-project/compare/v1.0.10...v1.0.11
[1.0.10]: https://github.com/taiki-e/pin-project/compare/v1.0.9...v1.0.10
[1.0.9]: https://github.com/taiki-e/pin-project/compare/v1.0.8...v1.0.9
[1.0.8]: https://github.com/taiki-e/pin-project/compare/v1.0.7...v1.0.8
[1.0.7]: https://github.com/taiki-e/pin-project/compare/v1.0.6...v1.0.7
[1.0.6]: https://github.com/taiki-e/pin-project/compare/v1.0.5...v1.0.6
[1.0.5]: https://github.com/taiki-e/pin-project/compare/v1.0.4...v1.0.5
[1.0.4]: https://github.com/taiki-e/pin-project/compare/v1.0.3...v1.0.4
[1.0.3]: https://github.com/taiki-e/pin-project/compare/v1.0.2...v1.0.3
[1.0.2]: https://github.com/taiki-e/pin-project/compare/v1.0.1...v1.0.2
[1.0.1]: https://github.com/taiki-e/pin-project/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/taiki-e/pin-project/compare/v1.0.0-alpha.1...v1.0.0
[1.0.0-alpha.1]: https://github.com/taiki-e/pin-project/compare/v0.4.23...v1.0.0-alpha.1
[0.4.30]: https://github.com/taiki-e/pin-project/compare/v0.4.29...v0.4.30
[0.4.29]: https://github.com/taiki-e/pin-project/compare/v0.4.28...v0.4.29
[0.4.28]: https://github.com/taiki-e/pin-project/compare/v0.4.27...v0.4.28
[0.4.27]: https://github.com/taiki-e/pin-project/compare/v0.4.26...v0.4.27
[0.4.26]: https://github.com/taiki-e/pin-project/compare/v0.4.25...v0.4.26
[0.4.25]: https://github.com/taiki-e/pin-project/compare/v0.4.24...v0.4.25
[0.4.24]: https://github.com/taiki-e/pin-project/compare/v0.4.23...v0.4.24
[0.4.23]: https://github.com/taiki-e/pin-project/compare/v0.4.22...v0.4.23
[0.4.22]: https://github.com/taiki-e/pin-project/compare/v0.4.21...v0.4.22
[0.4.21]: https://github.com/taiki-e/pin-project/compare/v0.4.20...v0.4.21
[0.4.20]: https://github.com/taiki-e/pin-project/compare/v0.4.19...v0.4.20
[0.4.19]: https://github.com/taiki-e/pin-project/compare/v0.4.18...v0.4.19
[0.4.18]: https://github.com/taiki-e/pin-project/compare/v0.4.17...v0.4.18
[0.4.17]: https://github.com/taiki-e/pin-project/compare/v0.4.16...v0.4.17
[0.4.16]: https://github.com/taiki-e/pin-project/compare/v0.4.15...v0.4.16
[0.4.15]: https://github.com/taiki-e/pin-project/compare/v0.4.14...v0.4.15
[0.4.14]: https://github.com/taiki-e/pin-project/compare/v0.4.13...v0.4.14
[0.4.13]: https://github.com/taiki-e/pin-project/compare/v0.4.11...v0.4.13
[0.4.12]: https://github.com/taiki-e/pin-project/compare/v0.4.10...v0.4.12
[0.4.11]: https://github.com/taiki-e/pin-project/compare/v0.4.10...v0.4.11
[0.4.10]: https://github.com/taiki-e/pin-project/compare/v0.4.9...v0.4.10
[0.4.9]: https://github.com/taiki-e/pin-project/compare/v0.4.8...v0.4.9
[0.4.8]: https://github.com/taiki-e/pin-project/compare/v0.4.7...v0.4.8
[0.4.7]: https://github.com/taiki-e/pin-project/compare/v0.4.6...v0.4.7
[0.4.6]: https://github.com/taiki-e/pin-project/compare/v0.4.5...v0.4.6
[0.4.5]: https://github.com/taiki-e/pin-project/compare/v0.4.4...v0.4.5
[0.4.4]: https://github.com/taiki-e/pin-project/compare/v0.4.3...v0.4.4
[0.4.3]: https://github.com/taiki-e/pin-project/compare/v0.4.2...v0.4.3
[0.4.2]: https://github.com/taiki-e/pin-project/compare/v0.4.1...v0.4.2
[0.4.1]: https://github.com/taiki-e/pin-project/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/taiki-e/pin-project/compare/v0.4.0-beta.1...v0.4.0
[0.4.0-beta.1]: https://github.com/taiki-e/pin-project/compare/v0.4.0-alpha.11...v0.4.0-beta.1
[0.4.0-alpha.11]: https://github.com/taiki-e/pin-project/compare/v0.4.0-alpha.10...v0.4.0-alpha.11
[0.4.0-alpha.10]: https://github.com/taiki-e/pin-project/compare/v0.4.0-alpha.9...v0.4.0-alpha.10
[0.4.0-alpha.9]: https://github.com/taiki-e/pin-project/compare/v0.4.0-alpha.8...v0.4.0-alpha.9
[0.4.0-alpha.8]: https://github.com/taiki-e/pin-project/compare/v0.4.0-alpha.7...v0.4.0-alpha.8
[0.4.0-alpha.7]: https://github.com/taiki-e/pin-project/compare/v0.4.0-alpha.6...v0.4.0-alpha.7
[0.4.0-alpha.6]: https://github.com/taiki-e/pin-project/compare/v0.4.0-alpha.5...v0.4.0-alpha.6
[0.4.0-alpha.5]: https://github.com/taiki-e/pin-project/compare/v0.4.0-alpha.4...v0.4.0-alpha.5
[0.4.0-alpha.4]: https://github.com/taiki-e/pin-project/compare/v0.4.0-alpha.3...v0.4.0-alpha.4
[0.4.0-alpha.3]: https://github.com/taiki-e/pin-project/compare/v0.4.0-alpha.2...v0.4.0-alpha.3
[0.4.0-alpha.2]: https://github.com/taiki-e/pin-project/compare/v0.4.0-alpha.1...v0.4.0-alpha.2
[0.4.0-alpha.1]: https://github.com/taiki-e/pin-project/compare/v0.3.5...v0.4.0-alpha.1
[0.3.5]: https://github.com/taiki-e/pin-project/compare/v0.3.4...v0.3.5
[0.3.4]: https://github.com/taiki-e/pin-project/compare/v0.3.3...v0.3.4
[0.3.3]: https://github.com/taiki-e/pin-project/compare/v0.3.2...v0.3.3
[0.3.2]: https://github.com/taiki-e/pin-project/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/taiki-e/pin-project/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/taiki-e/pin-project/compare/v0.2.2...v0.3.0
[0.2.2]: https://github.com/taiki-e/pin-project/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/taiki-e/pin-project/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/taiki-e/pin-project/compare/v0.1.8...v0.2.0
[0.1.8]: https://github.com/taiki-e/pin-project/compare/v0.1.7...v0.1.8
[0.1.7]: https://github.com/taiki-e/pin-project/compare/v0.1.6...v0.1.7
[0.1.6]: https://github.com/taiki-e/pin-project/compare/v0.1.5...v0.1.6
[0.1.5]: https://github.com/taiki-e/pin-project/compare/v0.1.4...v0.1.5
[0.1.4]: https://github.com/taiki-e/pin-project/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/taiki-e/pin-project/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/taiki-e/pin-project/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/taiki-e/pin-project/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/taiki-e/pin-project/releases/tag/v0.1.0
