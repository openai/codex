# assert_cmd

> **Assert `process::Command`** - Easy command initialization and assertions.

[![Documentation](https://img.shields.io/badge/docs-master-blue.svg)][Documentation]
![License](https://img.shields.io/crates/l/assert_cmd.svg)
[![Crates Status](https://img.shields.io/crates/v/assert_cmd.svg)][Crates.io]

`assert_cmd` aims to simplify the process for doing integration testing of CLIs, including:
- Finding your crate's binary to test
- Assert on the result of your program's run.

## Example

Here's a trivial example:

```rust,no_run
use assert_cmd::Command;

let mut cmd = Command::cargo_bin("bin_fixture").unwrap();
cmd.assert().success();
```

See the [docs](http://docs.rs/assert_cmd) for more.

## Relevant crates

Other crates that might be useful in testing command line programs.
* [escargot][escargot] for more control over configuring the crate's binary.
* [duct][duct] for orchestrating multiple processes.
  * or [commandspec] for easier writing of commands
* [rexpect][rexpect] for testing interactive programs.
* [`assert_fs`][assert_fs] for filesystem fixtures and assertions.
  * or [tempfile][tempfile] for scratchpad directories.
* [dir-diff][dir-diff] for testing file side-effects.
* [cross][cross] for cross-platform testing.

[escargot]: http://docs.rs/escargot
[rexpect]: https://crates.io/crates/rexpect
[dir-diff]: https://crates.io/crates/dir-diff
[tempfile]: https://crates.io/crates/tempfile
[duct]: https://crates.io/crates/duct
[assert_fs]: https://crates.io/crates/assert_fs
[commandspec]: https://crates.io/crates/commandspec
[cross]: https://github.com/cross-rs/cross

## License

Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

## Testimonials

fitzgen
> assert_cmd is just such a pleasure to use every single time, I fall in love all over again
>
> bravo bravo WG-cli

passcod
> Running commands and dealing with output can be complex in many many ways, so assert_cmd smoothing that is excellent, very much welcome, and improves ergonomics significantly.

volks73
>  I have used [assert_cmd] in other projects and I am extremely pleased with it

coreyja
> [assert_cmd] pretty much IS my testing strategy so far, though my app under test is pretty small.
>
> This library has made it really easy to add some test coverage to my project, even when I am just learning how to write Rust!

## Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.

[Crates.io]: https://crates.io/crates/assert_cmd
[Documentation]: https://docs.rs/assert_cmd
