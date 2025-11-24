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

use std::env;
use std::fmt::Write;
use std::fs;

use anyhow::Context;

const REGENERATE_VAR_NAME: &str = "STARLARK_RUST_REGENERATE_GOLDEN_TESTS";

#[allow(clippy::write_literal)] // We mark generated files as generated, but not this file.
fn make_golden(output: &str) -> String {
    let mut golden = String::new();
    writeln!(golden, "# {at}generated", at = "@").unwrap();
    writeln!(golden, "# To regenerate, run:").unwrap();
    writeln!(golden, "# ```").unwrap();
    writeln!(
        golden,
        // TODO(nga): fix instruction for `starlark_syntax` crate.
        "# {REGENERATE_VAR_NAME}=1 cargo test -p starlark --lib"
    )
    .unwrap();
    writeln!(golden, "# ```").unwrap();
    writeln!(golden).unwrap();
    writeln!(golden, "{}", output.trim_end()).unwrap();
    golden
}

/// Common code for golden tests.
pub fn golden_test_template(golden_rel_path: &str, output: &str) {
    assert!(golden_rel_path.starts_with("src/"));
    assert!(golden_rel_path.contains(".golden"));

    let manifest_dir =
        env::var("CARGO_MANIFEST_DIR").expect("`CARGO_MANIFEST_DIR` variable must be set");

    let golden_file_path = format!("{manifest_dir}/{golden_rel_path}");

    let output_with_prefix = make_golden(output);

    if env::var(REGENERATE_VAR_NAME).is_ok() {
        fs::write(&golden_file_path, &output_with_prefix)
            .with_context(|| format!("Writing `{golden_file_path}`"))
            .unwrap();
    } else {
        let expected = fs::read_to_string(&golden_file_path)
            .with_context(|| format!("Reading `{golden_file_path}`"))
            .unwrap();

        let expected = if cfg!(windows) {
            // Git may check out files on Windows with \r\n as line separator.
            // We could configure git, but it's more reliable to handle it in the test.
            expected.replace("\r\n", "\n")
        } else {
            expected
        };
        assert_eq!(expected, output_with_prefix);
    }
}
