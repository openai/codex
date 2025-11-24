/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

fn main() {
    rust_nightly();
}

fn rust_nightly() {
    let rustc = std::env::var("RUSTC").unwrap();
    let version = std::process::Command::new(rustc)
        .arg("--version")
        .output()
        .unwrap();

    assert!(version.status.success());

    // Nightly output:
    // rustc 1.64.0-nightly (affe0d3a0 2022-08-05)
    // Stable output:
    // rustc 1.64.0 (a55dd71d5 2022-09-19)
    // Meta internal output:
    // rustc 1.64.0-dev

    let stdout = String::from_utf8(version.stdout).unwrap();
    assert!(stdout.contains("rustc"), "Sanity check");
    let nightly = stdout.contains("nightly") || stdout.contains("dev");
    if nightly {
        println!("cargo:rustc-cfg=rust_nightly");
    }
}
