# Starlark

## 0.13 (Dec 13, 2024)

- Some API changes (but alas, too many for us to enumerate).
- Move to more Python-like type annotations, e.g. `dict[str, int]` now.
- Support for the `set` data type.
- Improve documentation, both the docs and the doc generator.
- Performance improvements.
- More trait implementation (`Trace`, `Allocative`, `AllocValue` etc).
- Bump dependencies.

## 0.12 (Feb 9, 2024)

- Implemented `reverse()` for `SmallMap`.
- Added a few more traits and functions to `OrderedSet`.
- Fixed "go to label" in bzl LSP.
- Added `OwnedFrozenRef`.
- Added `call_stack_frame()` function.
- Categorized lexer errors.
- Used correct error printing in `assert`.
- Cleaned up some code around imports, code consistency, and formatting.
- Fixed multi-line doc generation.
- Updated `dashmap` from 5.4.0 to 5.5.3.
- Removed autocompletion on reserved words and some trigger chars for Starlark
  LSP.
- Updated `hashbrown` from 0.13.2 to 0.14.3.
- Updated `allocative` from 0.3.1 to 0.3.2.

## 0.11 (Jan 19, 2024)

- `UnpackValue` is no longer implemented for `Vec`. Instead, `UnpackList`,
  `UnpackTuple` or `UnpackListOrTuple` can be used.
- Added `call_stack()` function.
- Removed `experimental_regex()` function.
- Switched from `anyhow::Error` and `Diagnostic` to `starlark::Error` in many
  APIs.
- Fix `repr()` for enums.
- Various improvements to
  [DAP](https://microsoft.github.io/debug-adapter-protocol/) support.
- Add a few more traits and functions to `SortedMap`.
- Disable the legacy type representation, types as strings such as `"bool"` are
  no longer supported. See the revised
  [typing documentation](https://github.com/facebookexperimental/starlark-rust/blob/main/docs/types.md).

## 0.10.0 (Oct 16, 2023)

There were over eight hundred commits since the last release, with the main
areas of change including:

- Make it work on 32 bit architectures, including WASM.
- Introduce `starlark_syntax`, `starlark_lsp`, `starlark_bin` crates.
- Expose the AST through `starlark_syntax`.
- No longer require nightly Rust compiler.
- Change how types are represented, e.g. `{str.type, ""}` becomes
  `dict[str, typing.Any]`.
- Add static type checking options.
- Support f-strings extension.

## 0.9.0 (June 5, 2023)

There were over one thousand commits since the last release, with the main areas
of change including:

- Add `Module.names` to get the symbols in a module.
- Change iterators on `Value` to support reduced stack space and enable future
  garbage collection, removing `iterate_collect` and `with_iterator`.
- Improve DAP (debugger) support.
- Improve the performance and fix a few bugs in `%` string formatting.
- Introduce `StarlarkTypeRepr` as a trait for things that have a type
  representation.
- Optimise and increase standards conformance of the Starlark standard library.
- Rework how `List`/`Dict` are specified, making their internals private and
  introducing `ListRef`/`DictRef` for pointers to them.
- Add `AllocStruct`, `AllocList` and `AllocDict` as helpers to allocate the
  various types more efficiently.
- Remove the `dedupe` Starlark function.
- Integrate with the `allocative` crate to support the `Allocative` trait.
- Add more `Trace` implementations.
- Support type annotations on assignments.
- Make `Module.loads` also return the `FileSpan`.
- Add `FrozenModule.get_option`.
- Add support for integers greater than 32 bits.
- Move to a separate `starlark_map` crate for `SmallMap` and optimise it.
- Require the `starlark_type!` macro.
- Change `Module.extra_v` to be `Value<'v>`.
- Expose `FrozenHeap` from `Freezer` and `GlobalsBuilder`.
- Remove `Value::new_int`.
- In `starlark_module` functions must be more explicit - return
  `anyhow::Result`, have an explicit `'v` parameter, explicit `heap`/`eval`
  arguments.
- Add `StringValueLike` to cover string types that are values.
- Change the `json` extension method to be `json.encode` and `json.decode`.
- Support stable Rust.
- Inline all the relevant pieces of the `gazebo` library.
- Change all `starlark_module` annotations to be `#[starlark(...)]`,
  `#[starlark(require = named)]` for name-only parameters.
- Many performance optimisations.
- Change the API for profiling and add new profiling features.
- Make the `starlark` binary spawn the REPL by default.
- Many improvements and API changes to the documentation generation code.
- Many improvements to the LSP code.
- Add a `Dialect` option to enable types.

## 0.8.0 (May 9, 2022)

- Redo `StringValue` and `FrozenStringValue` as aliases for
  `ValueTyped<StarlarkStr>` and `FrozenValueTyped<StarlarkStr>` respectively.
- Add more trait implementations for `ValueTyped`.
- Format call stack reports more like Python.
- Implement `FromStr` for `ProfileMode`.
- Impelement `Eq` for `BorrowHashed` .
- Minor optimisations throughout.
- Better error messages, e.g for `1 << -2`.
- Add `StarlarkValue::bit_not` for the bit-negation operator.
- In the REPL print non-`None` values.
- Add `get_index` to `SmallSet`.
- Add a `Regex` type - note this is likely to change API in the next release and
  is not considered stable.
- Upgrade dependencies.

## 0.7.1 (April 13, 2022)

- Add `MethodsBuilder::alloc`.

## 0.7.0 (March 22, 2022)

There have been many changes since the last release, focused on performance and
features. These changes caused a number of API changes and behavioural changes,
the most significant of which are listed below.

- Requires all Starlark types support `Serialize`, and provide
  `derive(NoSerialize)` to easily add a failing `Serialize`.
- Remove `collection_json`.
- Support the latest `gazebo` dependency.
- `SmallSet` now returns an `ExactSizeIterator`.
- Many optimisations, particularly around strings.
- Many optimisations to the bytecode compiler.
- Rename functions for working with constant strings.
- Add `StringValue` type, for `Value`s which are known to be strings.
- Remove `SimpleValue`.
- Remove the `Span` type.
- Rename `SmallHashResult` to `SmallHashValue`.
- Improve error messages on the wrong number of positional arguments.
- Remove some float-related functions from the API.
- Adjust the profiling API to make it more modular.
- Add support for validators when freezing.

## 0.6.0 (November 22, 2021)

There have been many changes since the last release, focused on performance,
documentation, type safety and profiling. These changes caused a number of API
changes and behavioural changes, the most significant of which are listed below.

- Support for newer versions of `anyhow`.
- Some error messages contain "did you mean" suggestions.
- Addition of a bytecode interpreter, with associated performance gains.
- Constant propagation and speculative execution during compilation.
- Removed mutability around the file loader and `set_loader`.
- Several new forms of profiling, making use of the new `extra_memory` function.
- Improved errors from derivations.
- Changes around function invocation, in particular `Arguments` is now opaque.
- Changes around `ConstFrozenString`, which is now `StarlarkStrN`.
- Add `OwnedFrozenValue::owner`.
- Add `derive` support for `Freeze`.
- Add more Starlark typed wrappers, such as `StringValue` and `ValueTyped`.
- Make tuples and lists opaque types, with new functions for allocating them
  (e.g. `alloc_tuple`).
- Make all Starlark types implement `Display` in preference to `collect_repr`.
- Support for documentation annotations on all types.

## 0.5.0 (August 26, 2021)

There have been many changes since the last release, primarily focused on
performance (up to 100x in some benchmarks). These changes caused a number of
API changes, the most significant of which are listed below.

- Rename the `starlark_module` crate to `starlark_derive`.
- Rename the `walk` methods to `trace` to align to standard GC literature.
- Add `derive` for `Trace`.
- Add `StarlarkAttrs` derivation and scheme.
- Initial start of documentation generation (still unstable).
- More complete `SmallMap` API.
- Three profiling modes, heap, flame and statement.
- Changes to `invoke` to take an `Arguments` structure.
- Changed to iteration APIs.
- Many semantic improvements to non-ASCII strings.
- Refinements to types and how they work.
- Mark a few additional APIs as `unsafe`.
- Use the `gazebo` `Coerce` trait extensively, in particular required for some
  of the `starlark_value` macros.
- Delete `dict.copy` and `list.copy`, since they aren't in the Starlark spec.
- `UnpackValue` no longer takes a `heap` argument.

## 0.4.0 (April 6, 2021)

- Change maintainer to Facebook.
- Move repo to https://github.com/facebookexperimental/starlark-rust.
- Switch to a garbage collector.
- Add `#[starlark_module]` proc-macro.
- Significant rewrite of most code, changing most APIs.

## 0.3.2

- Commits and tag exist in https://github.com/indygreg/starlark-rust.
- Changed dependency versions from `X.Y.Z` to `X.Y` to allow adopting newer
  point releases.
- lalrpop crate upgraded from 0.16 to 0.19 and code ported to enable building on
  Rust 1.56+.
- Fixed compiler warnings on Rust 1.56+ related to semicolons in macro
  expansions.

## 0.3.1 and before

- The code was developed at https://github.com/google/starlark-rust.
