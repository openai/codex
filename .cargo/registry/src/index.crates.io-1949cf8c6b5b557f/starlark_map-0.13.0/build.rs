/*
 * Copyright 2018 The Starlark in Rust Authors.
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
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
