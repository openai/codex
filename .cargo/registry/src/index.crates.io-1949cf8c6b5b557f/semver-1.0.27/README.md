semver
======

[<img alt="github" src="https://img.shields.io/badge/github-dtolnay/semver-8da0cb?style=for-the-badge&labelColor=555555&logo=github" height="20">](https://github.com/dtolnay/semver)
[<img alt="crates.io" src="https://img.shields.io/crates/v/semver.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20">](https://crates.io/crates/semver)
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-semver-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs" height="20">](https://docs.rs/semver)
[<img alt="build status" src="https://img.shields.io/github/actions/workflow/status/dtolnay/semver/ci.yml?branch=master&style=for-the-badge" height="20">](https://github.com/dtolnay/semver/actions?query=branch%3Amaster)

A parser and evaluator for Cargo's flavor of Semantic Versioning.

Semantic Versioning (see <https://semver.org>) is a guideline for how version
numbers are assigned and incremented. It is widely followed within the
Cargo/crates.io ecosystem for Rust.

```toml
[dependencies]
semver = "1.0"
```

*Compiler support: requires rustc 1.61+*

<br>

## Example

```rust
use semver::{BuildMetadata, Prerelease, Version, VersionReq};

fn main() {
    let req = VersionReq::parse(">=1.2.3, <1.8.0").unwrap();

    // Check whether this requirement matches version 1.2.3-alpha.1 (no)
    let version = Version {
        major: 1,
        minor: 2,
        patch: 3,
        pre: Prerelease::new("alpha.1").unwrap(),
        build: BuildMetadata::EMPTY,
    };
    assert!(!req.matches(&version));

    // Check whether it matches 1.3.0 (yes it does)
    let version = Version::parse("1.3.0").unwrap();
    assert!(req.matches(&version));
}
```

<br>

## Scope of this crate

Besides Cargo, several other package ecosystems and package managers for other
languages also use SemVer:&ensp;RubyGems/Bundler for Ruby, npm for JavaScript,
Composer for PHP, CocoaPods for Objective-C...

The `semver` crate is specifically intended to implement Cargo's interpretation
of Semantic Versioning.

Where the various tools differ in their interpretation or implementation of the
spec, this crate follows the implementation choices made by Cargo. If you are
operating on version numbers from some other package ecosystem, you will want to
use a different semver library which is appropriate to that ecosystem.

The extent of Cargo's SemVer support is documented in the *[Specifying
Dependencies]* chapter of the Cargo reference.

[Specifying Dependencies]: https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html

<br>

#### License

<sup>
Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.
</sup>

<br>

<sub>
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
</sub>
