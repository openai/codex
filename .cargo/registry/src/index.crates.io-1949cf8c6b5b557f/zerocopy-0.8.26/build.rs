// Copyright 2024 The Fuchsia Authors
//
// Licensed under the 2-Clause BSD License <LICENSE-BSD or
// https://opensource.org/license/bsd-2-clause>, Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to
// those terms.

// Sometimes we want to use lints which were added after our MSRV.
// `unknown_lints` is `warn` by default and we deny warnings in CI, so without
// this attribute, any unknown lint would cause a CI failure when testing with
// our MSRV.
#![allow(unknown_lints)]
#![deny(renamed_and_removed_lints)]
#![deny(
    anonymous_parameters,
    deprecated_in_future,
    late_bound_lifetime_arguments,
    missing_copy_implementations,
    missing_debug_implementations,
    path_statements,
    patterns_in_fns_without_body,
    rust_2018_idioms,
    trivial_numeric_casts,
    unreachable_pub,
    unsafe_op_in_unsafe_fn,
    unused_extern_crates,
    variant_size_differences
)]
#![deny(
    clippy::all,
    clippy::alloc_instead_of_core,
    clippy::arithmetic_side_effects,
    clippy::as_underscore,
    clippy::assertions_on_result_states,
    clippy::as_conversions,
    clippy::correctness,
    clippy::dbg_macro,
    clippy::decimal_literal_representation,
    clippy::get_unwrap,
    clippy::indexing_slicing,
    clippy::missing_inline_in_public_items,
    clippy::missing_safety_doc,
    clippy::obfuscated_if_else,
    clippy::perf,
    clippy::print_stdout,
    clippy::style,
    clippy::suspicious,
    clippy::todo,
    clippy::undocumented_unsafe_blocks,
    clippy::unimplemented,
    clippy::unnested_or_patterns,
    clippy::unwrap_used,
    clippy::use_debug
)]

use std::{env, fs, process::Command, str};

fn main() {
    // Avoid unnecessary re-building.
    println!("cargo:rerun-if-changed=build.rs");
    // This is necessary because changes to the list of detected Rust toolchain
    // versions will affect what `--cfg`s this script emits. Without this,
    // changes to that list have no effect on the build without running `cargo
    // clean` or similar.
    println!("cargo:rerun-if-changed=Cargo.toml");

    let version_cfgs = parse_version_cfgs_from_cargo_toml();
    let rustc_version = rustc_version();

    if rustc_version >= (Version { major: 1, minor: 77, patch: 0 }) {
        for version_cfg in &version_cfgs {
            // This tells the `unexpected_cfgs` lint to expect to see all of
            // these `cfg`s. The `cargo::` syntax was only added in 1.77, so we
            // don't emit these on earlier toolchain versions.
            println!("cargo:rustc-check-cfg=cfg({})", version_cfg.cfg_name);

            // This tells the `unexpected_cfgs` lint to expect to see `cfg`s of
            // the form `rust = "1.2.3"`. These aren't real `cfg`s, but we use
            // them in `cfg_attr(doc_cfg, doc(cfg(rust = "1.2.3")))` on items
            // that are version-gated so that the rendered Rustdoc shows which
            // Rust toolchain versions those items are available on.
            let Version { major, minor, patch } = version_cfg.version;
            println!("cargo:rustc-check-cfg=cfg(rust, values(\"{}.{}.{}\"))", major, minor, patch);
        }
        // FIXME(https://github.com/rust-lang/rust/issues/124816): Remove these
        // once they're no longer needed.
        println!("cargo:rustc-check-cfg=cfg(doc_cfg)");
        println!("cargo:rustc-check-cfg=cfg(kani)");
        println!(
            "cargo:rustc-check-cfg=cfg(__ZEROCOPY_INTERNAL_USE_ONLY_NIGHTLY_FEATURES_IN_TESTS)"
        );
        println!("cargo:rustc-check-cfg=cfg(coverage_nightly)");
    }

    for version_cfg in version_cfgs {
        if rustc_version >= version_cfg.version {
            println!("cargo:rustc-cfg={}", version_cfg.cfg_name);
        }
    }
}

#[derive(Debug, Ord, PartialEq, PartialOrd, Eq)]
struct Version {
    major: usize,
    minor: usize,
    patch: usize,
}

#[derive(Debug)]
struct VersionCfg {
    version: Version,
    cfg_name: String,
}

const ITER_FIRST_NEXT_EXPECT_MSG: &str = "unreachable: a string split cannot produce 0 items";

fn parse_version_cfgs_from_cargo_toml() -> Vec<VersionCfg> {
    let cargo_toml = fs::read_to_string("Cargo.toml").expect("failed to read Cargo.toml");

    // Expect a Cargo.toml with the following format:
    //
    //   ...
    //
    //   [package.metadata.build-rs]
    //   # Comments...
    //   zerocopy-panic-in-const-fn = "1.57.0"
    //
    //   ...
    //
    //   [...]
    //
    // In other words, the following sections, in order:
    // - Arbitrary content
    // - The literal header `[package.metadata.build-rs]`
    // - Any number of:
    //   - Comments
    //   - Key/value pairs
    // - A TOML table, indicating the end of the section we care about

    const TABLE_HEADER: &str = "[package.metadata.build-rs]";

    if !cargo_toml.contains(TABLE_HEADER) {
        panic!("{}", format!("Cargo.toml does not contain `{}`", TABLE_HEADER));
    }

    // Now that we know there's at least one instance of `TABLE_HEADER`, we
    // consume the iterator until we find the text following that first
    // instance. This isn't terribly bullet-proof, but we also authored
    // `Cargo.toml`, and we'd have to mess up pretty badly to accidentally put
    // two copies of the same table header in that file.
    let mut iter = cargo_toml.split(TABLE_HEADER);
    let _prefix = iter.next().expect(ITER_FIRST_NEXT_EXPECT_MSG);
    let rest = iter.next().expect("unreachable: we already confirmed that there's a table header");

    // Scan until we find the next table section, which should start with a `[`
    // character at the beginning of a line.
    let mut iter = rest.split("\n[");
    let section = iter.next().expect("unreachable: a string split cannot produce 0 items");

    section
        .lines()
        .filter_map(|line| {
            // Parse lines of one of the following forms:
            //
            //   # Comment
            //
            //   name-of-key = "1.2.3" # Comment
            //
            // Comments on their own line are ignored, and comments after a
            // key/value pair will be stripped before further processing.

            // We don't need to handle the case where the `#` character isn't a
            // comment (which can happen if it's inside a string) since we authored
            // `Cargo.toml` and, in this section, we only put Rust version numbers
            // in strings.
            let before_comment = line.split('#').next().expect(ITER_FIRST_NEXT_EXPECT_MSG);
            let before_comment_without_whitespace = before_comment.trim_start();
            if before_comment_without_whitespace.is_empty() {
                return None;
            }

            // At this point, assuming Cargo.toml is correctly formatted according
            // to the format expected by this function, we know that
            // `before_comment_without_whitespace` is of the form:
            //
            //   name-of-key = "1.2.3" # Comment
            //
            // ...with no leading whitespace, and where the trailing comment is
            // optional.

            let mut iter = before_comment_without_whitespace.split_whitespace();
            let name = iter.next().expect(ITER_FIRST_NEXT_EXPECT_MSG);
            const EXPECT_MSG: &str =
                "expected lines of the format `name-of-key = \"1.2.3\" # Comment`";
            let equals_sign = iter.next().expect(EXPECT_MSG);
            let value = iter.next().expect(EXPECT_MSG);

            assert_eq!(equals_sign, "=", "{}", EXPECT_MSG);

            // Replace dashes with underscores.
            let name = name.replace('-', "_");

            // Strip the quotation marks.
            let value = value.trim_start_matches('"').trim_end_matches('"');

            let mut iter = value.split('.');
            let major = iter.next().expect(ITER_FIRST_NEXT_EXPECT_MSG);
            let minor = iter.next().expect(EXPECT_MSG);
            let patch = iter.next().expect(EXPECT_MSG);

            assert_eq!(iter.next(), None, "{}", EXPECT_MSG);

            let major: usize = major.parse().expect(EXPECT_MSG);
            let minor: usize = minor.parse().expect(EXPECT_MSG);
            let patch: usize = patch.parse().expect(EXPECT_MSG);

            Some(VersionCfg { version: Version { major, minor, patch }, cfg_name: name })
        })
        .collect()
}

fn rustc_version() -> Version {
    let rustc_cmd_name = env::var_os("RUSTC").expect("could not get rustc command name");
    let version =
        Command::new(rustc_cmd_name).arg("--version").output().expect("could not invoke rustc");
    if !version.status.success() {
        panic!(
            "rustc failed with status: {}\nrustc output: {}",
            version.status,
            String::from_utf8_lossy(version.stderr.as_slice())
        );
    }

    const RUSTC_EXPECT_MSG: &str = "could not parse rustc version output";
    let version = str::from_utf8(version.stdout.as_slice()).expect(RUSTC_EXPECT_MSG);
    let version = version.trim_start_matches("rustc ");
    // The version string is sometimes followed by other information such as the
    // string `-nightly` or other build information. We don't care about any of
    // that.
    let version = version
        .split(|c: char| c != '.' && !c.is_ascii_digit())
        .next()
        .expect(ITER_FIRST_NEXT_EXPECT_MSG);
    let mut iter = version.split('.');
    let major = iter.next().expect(ITER_FIRST_NEXT_EXPECT_MSG);
    let minor = iter.next().expect(RUSTC_EXPECT_MSG);
    let patch = iter.next().expect(RUSTC_EXPECT_MSG);

    let major: usize = major.parse().expect(RUSTC_EXPECT_MSG);
    let minor: usize = minor.parse().expect(RUSTC_EXPECT_MSG);
    let patch: usize = patch.parse().expect(RUSTC_EXPECT_MSG);

    Version { major, minor, patch }
}
