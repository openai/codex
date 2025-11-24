# 1.3.2

- Allow `non_snake_case` in generated flags types ([#256])

[#252]: https://github.com/bitflags/bitflags/pull/256

# 1.3.1

- Revert unconditional `#[repr(transparent)]` ([#252])

[#252]: https://github.com/bitflags/bitflags/pull/252

# 1.3.0 (yanked)

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
